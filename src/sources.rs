use crate::{
    config::ReconnectConfig,
    errors::SourceError,
    events::{Event, EventBus, EventRecord},
    frame::Frame,
    media::{
        MediaDeliveryHealth, MediaGateway, MediaGatewayStatus, MediaMtxAdapter,
        MediaSourceRegistration, PlaybackInfo,
    },
    onboarding::{
        check, OnboardingCompletion, OnboardingDraft, OnboardingFailure, OnboardingSessionView,
    },
    onvif::{
        CameraCapabilities, MediaProfile, OnvifClient, OnvifDevice, OnvifDiscoveryRequest,
        OnvifInspectRequest, PtzCapabilities, PtzMoveMode, PtzMoveRequest, PtzOperation,
        PtzOperationResult, PtzPreset, PtzResponse, PtzSession,
    },
    recovery::RecoveryEngine,
    rtsp::{
        RtspFailure, RtspFailureCode, RtspValidationRequest, RtspValidationResult, RtspValidator,
    },
};
use async_trait::async_trait;
use nokhwa::{
    pixel_format::RgbFormat,
    utils::{CameraIndex, RequestedFormat, RequestedFormatType},
    Camera,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashSet},
    fmt, fs,
    io::Read,
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc, Arc,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::net::UdpSocket;
use tokio::sync::{mpsc as tokio_mpsc, watch, Mutex, RwLock, Semaphore};
use tokio::task::JoinHandle;

#[allow(dead_code)]
#[async_trait(?Send)]
pub trait VideoSource {
    fn name(&self) -> &'static str;
    async fn next_frame(&mut self) -> Result<Frame, SourceError>;
}

pub struct BuiltInCamera {
    camera: Camera,
    sequence: u64,
}
impl BuiltInCamera {
    pub fn new(_fps: u32) -> Result<Self, SourceError> {
        let requested =
            RequestedFormat::new::<RgbFormat>(RequestedFormatType::AbsoluteHighestFrameRate);
        let camera_index: u32 = std::env::var("SENTINEL_CAMERA_INDEX")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(0);
        let mut camera = Camera::new(CameraIndex::Index(camera_index), requested)
            .map_err(|e| SourceError(e.to_string()))?;
        camera
            .open_stream()
            .map_err(|e| SourceError(e.to_string()))?;
        Ok(Self {
            camera,
            sequence: 0,
        })
    }
}
#[async_trait(?Send)]
impl VideoSource for BuiltInCamera {
    fn name(&self) -> &'static str {
        "built-in-camera"
    }
    async fn next_frame(&mut self) -> Result<Frame, SourceError> {
        self.next_frame_sync()
    }
}
impl BuiltInCamera {
    fn next_frame_sync(&mut self) -> Result<Frame, SourceError> {
        let frame = self
            .camera
            .frame()
            .map_err(|e| SourceError(e.to_string()))?;
        let decoded = frame
            .decode_image::<RgbFormat>()
            .map_err(|e| SourceError(e.to_string()))?;
        self.sequence += 1;
        Ok(Frame::from_rgb(
            self.sequence,
            decoded.width(),
            decoded.height(),
            decoded.into_raw(),
        ))
    }
}

#[allow(dead_code)]
pub struct UsbCamera;
#[allow(dead_code)]
pub struct RtspCamera;

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct RtspCredentials {
    pub username_env: Option<String>,
    pub password_env: Option<String>,
    #[serde(skip_serializing)]
    pub username: Option<String>,
    #[serde(skip_serializing)]
    pub password: Option<String>,
}

impl std::fmt::Debug for RtspCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RtspCredentials")
            .field("username_env", &self.username_env)
            .field("password_env", &self.password_env)
            .field("username", &self.username.as_ref().map(|_| "[REDACTED]"))
            .field("password", &self.password.as_ref().map(|_| "[REDACTED]"))
            .finish()
    }
}
#[allow(dead_code)]
pub struct OnvifCamera;

#[derive(Clone, Default, Deserialize)]
pub struct SourceOptions {
    pub name: Option<String>,
    pub location: Option<String>,
    pub vendor: Option<String>,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub path: Option<String>,
    pub uri: Option<String>,
    pub transport: Option<String>,
    pub credentials: Option<RtspCredentials>,
    #[serde(rename = "loop")]
    pub loop_playback: Option<bool>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub fps: Option<u32>,
}

impl std::fmt::Debug for SourceOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SourceOptions")
            .field("name", &self.name)
            .field("location", &self.location)
            .field("vendor", &self.vendor)
            .field("host", &self.host)
            .field("port", &self.port)
            .field("path", &self.path)
            .field(
                "uri",
                &self.uri.as_deref().map(crate::config::redact_uri_for_debug),
            )
            .field("transport", &self.transport)
            .field("credentials", &self.credentials)
            .finish()
    }
}

pub struct SyntheticSource {
    width: u32,
    height: u32,
    fps: u32,
    sequence: u64,
}
impl SyntheticSource {
    pub fn new(width: u32, height: u32, fps: u32) -> Self {
        Self {
            width: width.max(1),
            height: height.max(1),
            fps: fps.max(1),
            sequence: 0,
        }
    }
    pub fn fps(&self) -> u32 {
        self.fps
    }
    pub fn next_frame_sync(&mut self) -> Frame {
        self.sequence += 1;
        let mut data = vec![0u8; (self.width * self.height * 3) as usize];
        let x_offset = (self.sequence as u32 * 8) % self.width;
        for y in 0..self.height {
            for x in 0..self.width {
                let index = ((y * self.width + x) * 3) as usize;
                data[index] = ((x * 255) / self.width) as u8;
                data[index + 1] = ((y * 255) / self.height) as u8;
                data[index + 2] = 32;
                if x >= x_offset
                    && x < x_offset + self.width.min(80)
                    && y >= self.height / 3
                    && y < self.height * 2 / 3
                {
                    data[index] = 255;
                    data[index + 1] = 255;
                    data[index + 2] = 255;
                }
            }
        }
        Frame::from_rgb(self.sequence, self.width, self.height, data)
    }
}
#[async_trait(?Send)]
impl VideoSource for SyntheticSource {
    fn name(&self) -> &'static str {
        "synthetic"
    }
    async fn next_frame(&mut self) -> Result<Frame, SourceError> {
        Ok(self.next_frame_sync())
    }
}

pub struct ImageSequenceSource {
    frames: Vec<Frame>,
    index: usize,
    loop_playback: bool,
    fps: u32,
}
impl ImageSequenceSource {
    pub fn open(
        path: impl Into<PathBuf>,
        loop_playback: bool,
        fps: u32,
    ) -> Result<Self, SourceError> {
        let path = path.into();
        let mut paths = Vec::new();
        if path.is_dir() {
            let entries = fs::read_dir(&path).map_err(|e| SourceError(e.to_string()))?;
            for entry in entries.flatten() {
                let item = entry.path();
                if item.is_file() {
                    paths.push(item);
                }
            }
            paths.sort();
        } else if path.is_file() {
            paths.push(path.clone());
        }
        if paths.is_empty() {
            return Err(SourceError(format!(
                "image sequence source has no readable frames: {}",
                path.display()
            )));
        }
        let mut frames = Vec::new();
        for (index, item) in paths.iter().enumerate() {
            let image = image::open(item)
                .map_err(|error| SourceError(format!("decode {}: {error}", item.display())))?
                .to_rgb8();
            frames.push(Frame::from_rgb(
                index as u64 + 1,
                image.width(),
                image.height(),
                image.into_raw(),
            ));
        }
        Ok(Self {
            frames,
            index: 0,
            loop_playback,
            fps: fps.max(1),
        })
    }
    pub fn fps(&self) -> u32 {
        self.fps
    }
    pub fn next_frame_sync(&mut self) -> Result<Frame, SourceError> {
        if self.index >= self.frames.len() {
            if !self.loop_playback {
                return Err(SourceError("video file reached end of playback".into()));
            }
            self.index = 0;
            tracing::info!("video file loop completed");
        }
        let frame = self.frames[self.index].clone();
        self.index += 1;
        Ok(frame)
    }
}
#[async_trait(?Send)]
impl VideoSource for ImageSequenceSource {
    fn name(&self) -> &'static str {
        "image-sequence"
    }
    async fn next_frame(&mut self) -> Result<Frame, SourceError> {
        self.next_frame_sync()
    }
}

/// RTSP decoding boundary. FFmpeg owns RTSP session setup, TCP transport, and
/// H.264 decoding; Sentinel receives raw RGB frames through stdout.
pub struct RtspVideoSource {
    child: Child,
    stdout: std::process::ChildStdout,
    width: u32,
    height: u32,
    sequence: u64,
}

/// Decodes a local video file in real time through FFmpeg. This keeps MP4
/// ingestion on the same FrameProvider boundary as cameras and RTSP sources.
pub struct FfmpegVideoFileSource {
    child: Child,
    stdout: std::process::ChildStdout,
    width: u32,
    height: u32,
    sequence: u64,
}
impl FfmpegVideoFileSource {
    pub fn open(
        path: String,
        loop_playback: bool,
        width: u32,
        height: u32,
        fps: u32,
    ) -> Result<Self, SourceError> {
        let executable = std::env::var("SENTINEL_FFMPEG").unwrap_or_else(|_| "ffmpeg".into());
        let mut command = Command::new(executable);
        command.args(["-hide_banner", "-loglevel", "error"]);
        if loop_playback {
            command.args(["-stream_loop", "-1"]);
        }
        command
            .args(["-re", "-i", path.as_str(), "-an", "-sn", "-dn"])
            .args([
                "-vf",
                &format!("scale={width}:{height}"),
                "-r",
                &fps.max(1).to_string(),
            ])
            .args(["-pix_fmt", "rgb24", "-f", "rawvideo", "pipe:1"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let mut child = command
            .spawn()
            .map_err(|error| SourceError(format!("video decoder unavailable: {error}")))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| SourceError("video decoder did not provide output".into()))?;
        Ok(Self {
            child,
            stdout,
            width: width.max(1),
            height: height.max(1),
            sequence: 0,
        })
    }
    pub fn next_frame_sync(&mut self) -> Result<Frame, SourceError> {
        let mut data = vec![0u8; (self.width * self.height * 3) as usize];
        self.stdout
            .read_exact(&mut data)
            .map_err(|error| SourceError(format!("video decode failed: {error}")))?;
        self.sequence += 1;
        Ok(Frame::from_rgb(
            self.sequence,
            self.width,
            self.height,
            data,
        ))
    }
}
impl Drop for FfmpegVideoFileSource {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
impl RtspVideoSource {
    pub fn connect(
        uri: String,
        transport: String,
        width: u32,
        height: u32,
        fps: u32,
        credentials: Option<RtspCredentials>,
    ) -> Result<Self, SourceError> {
        let executable = std::env::var("SENTINEL_FFMPEG").unwrap_or_else(|_| "ffmpeg".into());
        let uri = add_rtsp_credentials(uri, credentials)?;
        let mut command = Command::new(executable);
        command
            .args(["-hide_banner", "-loglevel", "error"])
            .args(["-rtsp_transport", transport.as_str()])
            .args(["-i", uri.as_str(), "-an", "-sn", "-dn"])
            .args(["-vf", &format!("scale={width}:{height}")])
            .args(["-r", &fps.max(1).to_string()])
            .args(["-pix_fmt", "rgb24", "-f", "rawvideo", "pipe:1"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let mut child = command
            .spawn()
            .map_err(|error| SourceError(format!("RTSP decoder unavailable: {error}")))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| SourceError("RTSP decoder did not provide video output".into()))?;
        Ok(Self {
            stdout,
            child,
            width: width.max(1),
            height: height.max(1),
            sequence: 0,
        })
    }
    pub fn next_frame_sync(&mut self) -> Result<Frame, SourceError> {
        let mut data = vec![0u8; (self.width * self.height * 3) as usize];
        self.stdout
            .read_exact(&mut data)
            .map_err(|error| SourceError(format!("RTSP decode failed: {error}")))?;
        self.sequence += 1;
        Ok(Frame::from_rgb(
            self.sequence,
            self.width,
            self.height,
            data,
        ))
    }
}
impl Drop for RtspVideoSource {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
#[async_trait(?Send)]
impl VideoSource for RtspVideoSource {
    fn name(&self) -> &'static str {
        "rtsp"
    }
    async fn next_frame(&mut self) -> Result<Frame, SourceError> {
        self.next_frame_sync()
    }
}

fn add_rtsp_credentials(
    uri: String,
    credentials: Option<RtspCredentials>,
) -> Result<String, SourceError> {
    let Some(credentials) = credentials else {
        return Ok(uri);
    };
    let username = credentials.username.or_else(|| {
        credentials
            .username_env
            .and_then(|name| std::env::var(name).ok())
    });
    let password = credentials.password.or_else(|| {
        credentials
            .password_env
            .and_then(|name| std::env::var(name).ok())
    });
    if username.is_none() && password.is_none() {
        return Ok(uri);
    }
    let Some(scheme_end) = uri.find("://") else {
        return Err(SourceError("invalid RTSP URI".into()));
    };
    let host_start = scheme_end + 3;
    if uri[host_start..].contains('@') {
        return Ok(uri);
    }
    let authority = match (username, password) {
        (Some(user), Some(pass)) => format!("{}:{}@", percent_encode(&user), percent_encode(&pass)),
        (Some(user), None) => format!("{}@", percent_encode(&user)),
        (None, Some(pass)) => format!(":{}@", percent_encode(&pass)),
        (None, None) => String::new(),
    };
    Ok(format!(
        "{}://{}{}",
        &uri[..scheme_end],
        authority,
        &uri[host_start..]
    ))
}
fn percent_encode(value: &str) -> String {
    value
        .bytes()
        .map(|byte| match byte {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                (byte as char).to_string()
            }
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

fn profile_score(profile: &MediaProfile) -> (u8, u32, u32, u32) {
    let h264 = u8::from(
        profile
            .encoding
            .as_deref()
            .map(|encoding| encoding.to_ascii_lowercase().contains("264"))
            .unwrap_or(false),
    );
    let (width, height): (u32, u32) = profile
        .resolution
        .as_deref()
        .and_then(|value| value.split_once('x'))
        .and_then(|(width, height)| Some((width.parse().ok()?, height.parse().ok()?)))
        .unwrap_or((0, 0));
    (
        h264,
        width.saturating_mul(height),
        profile.frame_rate.unwrap_or(0.0) as u32,
        u32::from(profile.audio),
    )
}

fn rtsp_user_message(code: &RtspFailureCode) -> &'static str {
    match code {
        RtspFailureCode::AuthenticationFailed => "Camera rejected the supplied credentials.",
        RtspFailureCode::SourceUnreachable => "Camera is unreachable.",
        RtspFailureCode::StreamNotFound => "RTSP stream was not found.",
        RtspFailureCode::ConnectionTimeout => "Connection to the camera timed out.",
        RtspFailureCode::InvalidSource => "The camera source configuration is invalid.",
        RtspFailureCode::UnsupportedSource => "This camera source is not supported.",
        RtspFailureCode::ProtocolError | RtspFailureCode::Unknown => {
            "Camera returned an unexpected RTSP response."
        }
    }
}

#[async_trait]
pub trait FrameProvider: Send {
    async fn next_frame(&mut self) -> Result<Frame, SourceError>;
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceState {
    Stopped,
    Starting,
    Running,
    Stopping,
    Failed,
    Disconnected,
    Reconnecting,
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ValidationState {
    Unknown,
    Validating,
    Validated,
    Failed,
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum StreamHealth {
    Unknown,
    Healthy,
    Degraded,
    Unhealthy,
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RecoveryState {
    Idle,
    Recovering,
    Exhausted,
}
#[derive(Clone, Debug, Serialize)]
pub struct SourceInfo {
    pub id: String,
    pub name: String,
    pub location: Option<String>,
    #[serde(rename = "type")]
    pub kind: String,
    pub status: SourceState,
    pub resolution: Option<String>,
    pub fps: f64,
    pub uptime_seconds: u64,
    pub last_frame: Option<u128>,
    pub reconnect_count: u64,
    pub downtime_seconds: u64,
    pub frames_received: u64,
    pub frames_generated: u64,
    pub frames_dropped: u64,
    pub packets_received: u64,
    pub connections: u64,
    pub disconnects: u64,
    pub decode_failures: u64,
    pub last_failure_category: Option<String>,
    pub validation: ValidationState,
    pub health: StreamHealth,
    pub last_validation_attempt: Option<u128>,
    pub last_successful_validation: Option<u128>,
    pub last_validation_failure: Option<RtspFailure>,
    pub consecutive_validation_failures: u64,
    pub recovery: RecoveryState,
    pub recovery_attempts: u32,
    pub last_recovery_started: Option<u128>,
    pub last_recovery_succeeded: Option<u128>,
    pub last_recovery_exhausted: Option<u128>,
    pub next_recovery_at: Option<u128>,
    pub capabilities: Option<CameraCapabilities>,
    pub media_health: MediaDeliveryHealth,
}
#[derive(Clone, Debug, Deserialize)]
pub struct AddSource {
    pub id: String,
    pub kind: String,
    #[serde(flatten)]
    pub options: SourceOptions,
}

#[derive(Clone, Debug, Serialize)]
pub struct CameraProviderInfo {
    pub id: String,
    pub name: String,
    pub discovery: bool,
    pub test_connection: bool,
    pub implemented: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct DiscoveredCamera {
    pub id: String,
    pub name: String,
    pub vendor: Option<String>,
    #[serde(rename = "type")]
    pub kind: String,
    pub address: Option<String>,
    pub capabilities: Vec<String>,
    pub requires_manual_stream: bool,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ConnectionTestRequest {
    pub id: Option<String>,
    pub kind: String,
    #[serde(flatten)]
    pub options: SourceOptions,
}

#[derive(Clone, Debug, Serialize)]
pub struct ConnectionTestResult {
    pub success: bool,
    pub source_type: String,
    pub resolution: Option<String>,
    pub fps: f64,
    pub latency_ms: u64,
    pub message: String,
}

#[derive(Clone, Debug)]
pub struct PtzAuditContext {
    pub actor: String,
    pub correlation_id: String,
}

struct SourceEntry {
    info: SourceInfo,
    desired_running: bool,
    started_at: Option<std::time::Instant>,
    disconnected_at: Option<std::time::Instant>,
    options: SourceOptions,
    ptz_session: Option<PtzSession>,
    ptz_presets: BTreeMap<String, String>,
}

struct CameraWorker {
    frames: Arc<Mutex<tokio_mpsc::Receiver<Frame>>>,
    stop: Option<mpsc::Sender<()>>,
}
impl CameraWorker {
    fn start(fps: u32) -> Result<Self, SourceManagerError> {
        let (ready_tx, ready_rx) = mpsc::channel();
        let (stop_tx, stop_rx) = mpsc::channel();
        let (frame_tx, frame_rx) = tokio_mpsc::channel(2);
        std::thread::Builder::new().name("sentinel-camera".into()).spawn(move || {
            let mut camera = match BuiltInCamera::new(fps) { Ok(camera) => { let _ = ready_tx.send(Ok(())); camera }, Err(error) => { let _ = ready_tx.send(Err(error.to_string())); return; } };
            loop { if stop_rx.try_recv().is_ok() { break; } match camera.next_frame_sync() { Ok(frame) => { if frame_tx.blocking_send(frame).is_err() { break; } }, Err(error) => { tracing::warn!(error=%error, "camera worker stopped after capture error"); break; } } }
        }).map_err(|error| SourceManagerError::Camera(error.to_string()))?;
        match ready_rx.recv_timeout(std::time::Duration::from_secs(5)) {
            Ok(Ok(())) => Ok(Self {
                frames: Arc::new(Mutex::new(frame_rx)),
                stop: Some(stop_tx),
            }),
            Ok(Err(error)) => Err(SourceManagerError::Camera(error)),
            Err(error) => Err(SourceManagerError::Camera(format!(
                "camera startup timed out: {error}"
            ))),
        }
    }
    fn stop(mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
    }
    fn start_synthetic(width: u32, height: u32, fps: u32) -> Result<Self, SourceManagerError> {
        let (ready_tx, ready_rx) = mpsc::channel();
        let (stop_tx, stop_rx) = mpsc::channel();
        let (frame_tx, frame_rx) = tokio_mpsc::channel(2);
        std::thread::Builder::new()
            .name("sentinel-synthetic".into())
            .spawn(move || {
                let mut source = SyntheticSource::new(width, height, fps);
                let _ = ready_tx.send(Ok(()));
                let interval = std::time::Duration::from_secs_f64(1.0 / source.fps() as f64);
                loop {
                    if stop_rx.try_recv().is_ok() {
                        break;
                    }
                    if frame_tx.blocking_send(source.next_frame_sync()).is_err() {
                        break;
                    }
                    std::thread::sleep(interval);
                }
            })
            .map_err(|error| SourceManagerError::Camera(error.to_string()))?;
        match ready_rx.recv_timeout(std::time::Duration::from_secs(5)) {
            Ok(Ok(())) => Ok(Self {
                frames: Arc::new(Mutex::new(frame_rx)),
                stop: Some(stop_tx),
            }),
            Ok(Err(error)) => Err(SourceManagerError::Camera(error)),
            Err(error) => Err(SourceManagerError::Camera(format!(
                "synthetic startup timed out: {error}"
            ))),
        }
    }
    fn start_video_file(
        path: String,
        loop_playback: bool,
        width: u32,
        height: u32,
        fps: u32,
    ) -> Result<Self, SourceManagerError> {
        let (ready_tx, ready_rx) = mpsc::channel();
        let (stop_tx, stop_rx) = mpsc::channel();
        let (frame_tx, frame_rx) = tokio_mpsc::channel(2);
        std::thread::Builder::new()
            .name("sentinel-video-file".into())
            .spawn(move || {
                let mut source =
                    match FfmpegVideoFileSource::open(path, loop_playback, width, height, fps) {
                        Ok(source) => {
                            let _ = ready_tx.send(Ok(()));
                            source
                        }
                        Err(error) => {
                            let _ = ready_tx.send(Err(error.to_string()));
                            return;
                        }
                    };
                loop {
                    if stop_rx.try_recv().is_ok() {
                        break;
                    }
                    match source.next_frame_sync() {
                        Ok(frame) => {
                            if frame_tx.blocking_send(frame).is_err() {
                                break;
                            }
                        }
                        Err(error) => {
                            tracing::info!(error=%error, "video file playback stopped");
                            break;
                        }
                    }
                }
            })
            .map_err(|error| SourceManagerError::Camera(error.to_string()))?;
        match ready_rx.recv_timeout(std::time::Duration::from_secs(5)) {
            Ok(Ok(())) => Ok(Self {
                frames: Arc::new(Mutex::new(frame_rx)),
                stop: Some(stop_tx),
            }),
            Ok(Err(error)) => Err(SourceManagerError::Camera(error)),
            Err(error) => Err(SourceManagerError::Camera(format!(
                "video file startup timed out: {error}"
            ))),
        }
    }
    fn start_mjpeg(options: SourceOptions, default_fps: u32) -> Result<Self, SourceManagerError> {
        let uri = options
            .uri
            .or_else(|| options.path.clone())
            .ok_or_else(|| SourceManagerError::Camera("MJPEG source requires uri".into()))?;
        Self::start_video_file(
            uri,
            false,
            options.width.unwrap_or(640),
            options.height.unwrap_or(360),
            options.fps.unwrap_or(default_fps),
        )
    }

    fn start_rtsp(options: SourceOptions, default_fps: u32) -> Result<Self, SourceManagerError> {
        let uri = options
            .uri
            .ok_or_else(|| SourceManagerError::Camera("RTSP source requires uri".into()))?;
        let transport = options.transport.unwrap_or_else(|| "tcp".into());
        if transport != "tcp" {
            return Err(SourceManagerError::Unsupported(format!(
                "RTSP transport '{transport}'"
            )));
        }
        let width = options.width.unwrap_or(640);
        let height = options.height.unwrap_or(360);
        let fps = options.fps.unwrap_or(default_fps);
        let (ready_tx, ready_rx) = mpsc::channel();
        let (stop_tx, stop_rx) = mpsc::channel();
        let (frame_tx, frame_rx) = tokio_mpsc::channel(2);
        std::thread::Builder::new()
            .name("sentinel-rtsp".into())
            .spawn(move || {
                let mut source = match RtspVideoSource::connect(
                    uri,
                    transport,
                    width,
                    height,
                    fps,
                    options.credentials,
                ) {
                    Ok(source) => {
                        let _ = ready_tx.send(Ok(()));
                        source
                    }
                    Err(error) => {
                        let _ = ready_tx.send(Err(error.to_string()));
                        return;
                    }
                };
                loop {
                    if stop_rx.try_recv().is_ok() {
                        break;
                    }
                    match source.next_frame_sync() {
                        Ok(frame) => {
                            if frame_tx.blocking_send(frame).is_err() {
                                break;
                            }
                        }
                        Err(error) => {
                            tracing::warn!(error=%error, "RTSP decoder stopped");
                            break;
                        }
                    }
                }
            })
            .map_err(|error| SourceManagerError::Camera(error.to_string()))?;
        match ready_rx.recv_timeout(std::time::Duration::from_secs(5)) {
            Ok(Ok(())) => Ok(Self {
                frames: Arc::new(Mutex::new(frame_rx)),
                stop: Some(stop_tx),
            }),
            Ok(Err(error)) => Err(SourceManagerError::Camera(error)),
            Err(error) => Err(SourceManagerError::Camera(format!(
                "RTSP startup timed out: {error}"
            ))),
        }
    }
}
#[derive(Debug)]
pub enum SourceManagerError {
    NotFound,
    AlreadyExists,
    Unsupported(String),
    Camera(String),
    Onvif(String),
    MediaGateway(String),
    PtzNotSupported,
    PtzOperationUnsupported(String),
    InvalidRequest(String),
}
impl fmt::Display for SourceManagerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => f.write_str("source not found"),
            Self::AlreadyExists => f.write_str("source already exists"),
            Self::Unsupported(kind) => write!(f, "source type '{kind}' is not implemented"),
            Self::Camera(error) => f.write_str(error),
            Self::Onvif(error) => f.write_str(error),
            Self::MediaGateway(error) => f.write_str(error),
            Self::PtzNotSupported => f.write_str("PTZ is not supported by this source"),
            Self::PtzOperationUnsupported(operation) => write!(
                f,
                "PTZ operation '{operation}' is not supported by this source"
            ),
            Self::InvalidRequest(error) => f.write_str(error),
        }
    }
}

#[derive(Clone)]
pub struct VideoSourceManager {
    fps: u32,
    sources: Arc<RwLock<BTreeMap<String, SourceEntry>>>,
    built_in: Arc<RwLock<Option<CameraWorker>>>,
    synthetic: Arc<RwLock<Option<CameraWorker>>>,
    video_file: Arc<RwLock<Option<CameraWorker>>>,
    rtsp: Arc<RwLock<Option<CameraWorker>>>,
    mjpeg: Arc<RwLock<Option<CameraWorker>>>,
    active_source: Arc<RwLock<Option<String>>>,
    events: EventBus,
    reconnect: ReconnectConfig,
    shutdown: watch::Receiver<bool>,
    recovery: RecoveryEngine,
    validation_timeout: Duration,
    validator: RtspValidator,
    onvif: OnvifClient,
    media_gateway: Arc<dyn MediaGateway>,
    health_config: crate::config::HealthConfig,
    health_semaphore: Arc<Semaphore>,
    health_inflight: Arc<Mutex<HashSet<String>>>,
    onboarding: Arc<RwLock<BTreeMap<String, OnboardingDraft>>>,
    next_onboarding_id: Arc<AtomicU64>,
}
impl VideoSourceManager {
    pub fn new(
        fps: u32,
        events: EventBus,
        reconnect: ReconnectConfig,
        shutdown: watch::Receiver<bool>,
        recovery: RecoveryEngine,
    ) -> Self {
        let info = SourceInfo {
            id: "builtin".into(),
            name: "Built-in camera".into(),
            location: None,
            kind: "built-in-camera".into(),
            status: SourceState::Stopped,
            resolution: None,
            fps: 0.0,
            uptime_seconds: 0,
            last_frame: None,
            reconnect_count: 0,
            downtime_seconds: 0,
            frames_received: 0,
            frames_generated: 0,
            frames_dropped: 0,
            packets_received: 0,
            connections: 0,
            disconnects: 0,
            decode_failures: 0,
            last_failure_category: None,
            validation: ValidationState::Unknown,
            health: StreamHealth::Unknown,
            last_validation_attempt: None,
            last_successful_validation: None,
            last_validation_failure: None,
            consecutive_validation_failures: 0,
            recovery: RecoveryState::Idle,
            recovery_attempts: 0,
            last_recovery_started: None,
            last_recovery_succeeded: None,
            last_recovery_exhausted: None,
            next_recovery_at: None,
            capabilities: None,
            media_health: MediaDeliveryHealth::Unknown,
        };
        let mut sources = BTreeMap::new();
        sources.insert(
            "builtin".into(),
            SourceEntry {
                info,
                desired_running: false,
                started_at: None,
                disconnected_at: None,
                options: SourceOptions::default(),
                ptz_session: None,
                ptz_presets: BTreeMap::new(),
            },
        );
        Self {
            fps,
            sources: Arc::new(RwLock::new(sources)),
            built_in: Arc::new(RwLock::new(None)),
            synthetic: Arc::new(RwLock::new(None)),
            video_file: Arc::new(RwLock::new(None)),
            rtsp: Arc::new(RwLock::new(None)),
            mjpeg: Arc::new(RwLock::new(None)),
            active_source: Arc::new(RwLock::new(None)),
            events,
            reconnect,
            shutdown,
            recovery,
            validation_timeout: Duration::from_secs(5),
            validator: RtspValidator::default(),
            onvif: OnvifClient::default(),
            media_gateway: Arc::new(MediaMtxAdapter::new(
                false,
                None,
                None,
                None,
                None,
                Duration::from_secs(3),
            )),
            health_config: crate::config::HealthConfig::default(),
            health_semaphore: Arc::new(Semaphore::new(4)),
            health_inflight: Arc::new(Mutex::new(HashSet::new())),
            onboarding: Arc::new(RwLock::new(BTreeMap::new())),
            next_onboarding_id: Arc::new(AtomicU64::new(1)),
        }
    }
    pub fn with_validation_timeout(mut self, timeout: Duration) -> Self {
        self.validation_timeout = timeout.max(Duration::from_millis(1));
        self.validator = RtspValidator::new(self.validation_timeout);
        self
    }
    pub fn with_validator(mut self, validator: RtspValidator) -> Self {
        self.validator = validator;
        self
    }
    pub fn with_onvif_client(mut self, onvif: OnvifClient) -> Self {
        self.onvif = onvif;
        self
    }
    pub fn with_media_gateway(mut self, media_gateway: Arc<dyn MediaGateway>) -> Self {
        self.media_gateway = media_gateway;
        self
    }
    pub fn with_health_config(mut self, config: crate::config::HealthConfig) -> Self {
        self.health_semaphore = Arc::new(Semaphore::new(config.max_concurrent_checks.max(1)));
        self.health_config = config;
        self
    }
    pub async fn list(&self) -> Vec<SourceInfo> {
        self.sources
            .read()
            .await
            .values()
            .map(|entry| self.snapshot(&entry.info, entry.started_at, entry.disconnected_at))
            .collect()
    }
    pub async fn get(&self, id: &str) -> Result<SourceInfo, SourceManagerError> {
        let sources = self.sources.read().await;
        let entry = sources.get(id).ok_or(SourceManagerError::NotFound)?;
        Ok(self.snapshot(&entry.info, entry.started_at, entry.disconnected_at))
    }
    fn snapshot(
        &self,
        info: &SourceInfo,
        started_at: Option<std::time::Instant>,
        disconnected_at: Option<std::time::Instant>,
    ) -> SourceInfo {
        let mut info = info.clone();
        info.uptime_seconds = started_at
            .map(|started| started.elapsed().as_secs())
            .unwrap_or(0);
        info.downtime_seconds = disconnected_at
            .map(|disconnected| disconnected.elapsed().as_secs())
            .unwrap_or(info.downtime_seconds);
        info
    }
    pub async fn add(&self, request: AddSource) -> Result<SourceInfo, SourceManagerError> {
        if !matches!(
            request.kind.as_str(),
            "built-in-camera" | "synthetic" | "image-sequence" | "video-file" | "rtsp" | "mjpeg"
        ) {
            return Err(SourceManagerError::Unsupported(request.kind));
        }
        let mut sources = self.sources.write().await;
        if sources.contains_key(&request.id) {
            return Err(SourceManagerError::AlreadyExists);
        }
        let info = SourceInfo {
            id: request.id.clone(),
            name: request
                .options
                .name
                .clone()
                .unwrap_or_else(|| request.id.clone()),
            location: request.options.location.clone(),
            kind: request.kind,
            status: SourceState::Stopped,
            resolution: None,
            fps: 0.0,
            uptime_seconds: 0,
            last_frame: None,
            reconnect_count: 0,
            downtime_seconds: 0,
            frames_received: 0,
            frames_generated: 0,
            frames_dropped: 0,
            packets_received: 0,
            connections: 0,
            disconnects: 0,
            decode_failures: 0,
            last_failure_category: None,
            validation: ValidationState::Unknown,
            health: StreamHealth::Unknown,
            last_validation_attempt: None,
            last_successful_validation: None,
            last_validation_failure: None,
            consecutive_validation_failures: 0,
            recovery: RecoveryState::Idle,
            recovery_attempts: 0,
            last_recovery_started: None,
            last_recovery_succeeded: None,
            last_recovery_exhausted: None,
            next_recovery_at: None,
            capabilities: None,
            media_health: MediaDeliveryHealth::Unknown,
        };
        sources.insert(
            request.id,
            SourceEntry {
                info: info.clone(),
                desired_running: false,
                started_at: None,
                disconnected_at: None,
                options: request.options,
                ptz_session: None,
                ptz_presets: BTreeMap::new(),
            },
        );
        Ok(info)
    }

    pub async fn onvif_discover(
        &self,
        request: OnvifDiscoveryRequest,
    ) -> Result<Vec<OnvifDevice>, SourceManagerError> {
        self.onvif
            .discover(&request)
            .await
            .map_err(|error| SourceManagerError::Onvif(error.to_string()))
    }

    pub async fn onboarding_discover(
        &self,
        request: OnvifDiscoveryRequest,
    ) -> Result<OnboardingSessionView, SourceManagerError> {
        let devices = self
            .onvif
            .discover(&request)
            .await
            .map_err(|error| SourceManagerError::Onvif(error.to_string()))?;
        let session_id = format!(
            "onboarding-{}",
            self.next_onboarding_id.fetch_add(1, Ordering::Relaxed)
        );
        let draft = OnboardingDraft::discovered(session_id.clone(), devices);
        let view = draft.view();
        self.onboarding.write().await.insert(session_id, draft);
        Ok(view)
    }

    pub async fn onboarding_session(
        &self,
        session_id: &str,
    ) -> Result<OnboardingSessionView, SourceManagerError> {
        self.onboarding
            .read()
            .await
            .get(session_id)
            .map(OnboardingDraft::view)
            .ok_or_else(|| {
                SourceManagerError::InvalidRequest("onboarding session not found".into())
            })
    }

    pub async fn onboarding_inspect(
        &self,
        session_id: &str,
        request: crate::onboarding::OnboardingInspectRequest,
    ) -> Result<OnboardingSessionView, SourceManagerError> {
        let inspection = self
            .onvif
            .inspect(&OnvifInspectRequest {
                endpoint: request.endpoint.clone(),
                username: request.username.clone(),
                password: request.password.clone(),
                timeout_ms: request.timeout_ms,
            })
            .await
            .map_err(|error| SourceManagerError::Onvif(error.to_string()))?;
        let selected_profile = inspection
            .device
            .profiles
            .iter()
            .filter(|profile| profile.rtsp_uri.is_some())
            .max_by_key(|profile| profile_score(profile))
            .cloned();
        let mut onboarding = self.onboarding.write().await;
        let draft = onboarding.get_mut(session_id).ok_or_else(|| {
            SourceManagerError::InvalidRequest("onboarding session not found".into())
        })?;
        draft.endpoint = Some(request.endpoint);
        draft.username = request.username;
        draft.password = request.password;
        draft.selected_profile = selected_profile;
        draft.inspection = Some(inspection);
        Ok(draft.view())
    }

    pub async fn onboarding_complete(
        &self,
        session_id: &str,
        request: crate::onboarding::OnboardingCompleteRequest,
    ) -> Result<OnboardingCompletion, SourceManagerError> {
        if request.source_id.trim().is_empty() || request.name.trim().is_empty() {
            return Err(SourceManagerError::InvalidRequest(
                "camera name and source ID are required".into(),
            ));
        }
        let draft = self
            .onboarding
            .read()
            .await
            .get(session_id)
            .cloned()
            .ok_or_else(|| {
                SourceManagerError::InvalidRequest("onboarding session not found".into())
            })?;
        let Some(inspection) = draft.inspection.clone() else {
            return Err(SourceManagerError::InvalidRequest(
                "inspect the camera before completing setup".into(),
            ));
        };
        let Some(profile) = draft.selected_profile.clone() else {
            return Ok(OnboardingCompletion {
                success: false,
                state: crate::onboarding::OnboardingState::Failed,
                source_id: None,
                checks: vec![check(
                    "video_profile",
                    "fail",
                    "Camera is reachable, but no usable video profile was found.",
                )],
                failure: Some(OnboardingFailure {
                    stage: "profile_selection".into(),
                    code: "NO_USABLE_VIDEO_PROFILE".into(),
                    message: "Camera is reachable, but no usable video profile was found.".into(),
                    technical_detail: None,
                }),
            });
        };
        let uri = profile.rtsp_uri.clone().ok_or_else(|| {
            SourceManagerError::InvalidRequest("selected video profile has no RTSP URI".into())
        })?;
        self.add(AddSource {
            id: request.source_id.clone(),
            kind: "rtsp".into(),
            options: SourceOptions {
                name: Some(request.name),
                location: request.location,
                uri: Some(uri),
                transport: Some("tcp".into()),
                credentials: Some(RtspCredentials {
                    username: draft.username,
                    password: draft.password,
                    ..Default::default()
                }),
                ..Default::default()
            },
        })
        .await?;
        let validation = self.validate(&request.source_id).await?;
        let mut checks = vec![
            check("onvif", "pass", "Camera capabilities discovered."),
            check(
                "video_profile",
                "pass",
                "A usable video profile was selected automatically.",
            ),
        ];
        if !validation.valid {
            let failure = validation.failure.clone().unwrap_or_else(|| RtspFailure {
                code: RtspFailureCode::Unknown,
                message: "RTSP validation failed.".into(),
                technical_detail: None,
            });
            checks.push(check("rtsp", "fail", rtsp_user_message(&failure.code)));
            let _ = self.remove(&request.source_id).await;
            return Ok(OnboardingCompletion {
                success: false,
                state: crate::onboarding::OnboardingState::Failed,
                source_id: None,
                checks,
                failure: Some(OnboardingFailure {
                    stage: "rtsp_validation".into(),
                    code: serde_json::to_string(&failure.code).unwrap_or_else(|_| "UNKNOWN".into()),
                    message: rtsp_user_message(&failure.code).into(),
                    technical_detail: failure.technical_detail,
                }),
            });
        }
        checks.push(check("rtsp", "pass", "Camera video stream validated."));
        let playback = match self.register_playback(&request.source_id).await {
            Ok(playback) => playback,
            Err(error) => {
                let _ = self.remove(&request.source_id).await;
                return Ok(OnboardingCompletion {
                    success: false,
                    state: crate::onboarding::OnboardingState::Failed,
                    source_id: None,
                    checks,
                    failure: Some(OnboardingFailure {
                        stage: "browser_playback".into(),
                        code: "BROWSER_PLAYBACK_UNAVAILABLE".into(),
                        message: "Video works, but browser playback is unavailable.".into(),
                        technical_detail: Some(error.to_string()),
                    }),
                });
            }
        };
        checks.push(check("media_gateway", "pass", "Media delivery registered."));
        checks.push(check(
            "browser_preview",
            "ready",
            "Browser live preview is ready to open.",
        ));
        checks.push(check(
            "ptz",
            if inspection.device.capabilities.ptz.supported {
                "supported"
            } else {
                "unsupported"
            },
            if inspection.device.capabilities.ptz.supported {
                "PTZ is supported and ready for testing."
            } else {
                "PTZ is not supported by this camera."
            },
        ));
        checks.push(check(
            "health",
            "ready",
            "Camera health monitoring is active.",
        ));
        self.onboarding.write().await.remove(session_id);
        let _ = playback;
        Ok(OnboardingCompletion {
            success: true,
            state: crate::onboarding::OnboardingState::Ready,
            source_id: Some(request.source_id),
            checks,
            failure: None,
        })
    }

    pub async fn inspect_onvif(
        &self,
        id: &str,
        request: OnvifInspectRequest,
    ) -> Result<serde_json::Value, SourceManagerError> {
        {
            let sources = self.sources.read().await;
            let entry = sources.get(id).ok_or(SourceManagerError::NotFound)?;
            if entry.info.kind != "rtsp" {
                return Err(SourceManagerError::Unsupported(
                    "ONVIF inspection requires an RTSP source".into(),
                ));
            }
        }
        let inspection = self
            .onvif
            .inspect(&request)
            .await
            .map_err(|error| SourceManagerError::Onvif(error.to_string()))?;
        {
            let mut sources = self.sources.write().await;
            let entry = sources.get_mut(id).ok_or(SourceManagerError::NotFound)?;
            entry.info.capabilities = Some(inspection.device.capabilities.clone());
            entry.ptz_session = inspection.ptz_session.clone();
            entry.ptz_presets.clear();
            if let Some(uri) = inspection.selected_rtsp_uri.clone() {
                entry.options.uri = Some(uri);
            }
            if request.username.is_some() || request.password.is_some() {
                entry.options.credentials = Some(RtspCredentials {
                    username: request.username.clone(),
                    password: request.password.clone(),
                    ..Default::default()
                });
            }
        }
        let validation = self
            .validate(id)
            .await
            .map_err(|error| SourceManagerError::Camera(error.to_string()))?;
        self.events.publish(Event {
            kind: "onvif_inspection_completed".into(),
            source_id: Some(id.into()),
            message: "ONVIF capabilities normalized and RTSP handoff validated".into(),
        });
        Ok(serde_json::json!({
            "device": inspection.device,
            "rtspValidation": validation
        }))
    }

    pub async fn capabilities(
        &self,
        id: &str,
    ) -> Result<Option<CameraCapabilities>, SourceManagerError> {
        let sources = self.sources.read().await;
        let entry = sources.get(id).ok_or(SourceManagerError::NotFound)?;
        Ok(entry.info.capabilities.clone())
    }

    pub async fn media_gateway_health(&self) -> MediaGatewayStatus {
        self.media_gateway.health().await
    }

    pub async fn register_playback(&self, id: &str) -> Result<PlaybackInfo, SourceManagerError> {
        let (uri, username, password) = {
            let sources = self.sources.read().await;
            let entry = sources.get(id).ok_or(SourceManagerError::NotFound)?;
            if entry.info.kind != "rtsp" {
                return Err(SourceManagerError::Unsupported(
                    "media playback registration requires an RTSP source".into(),
                ));
            }
            if !matches!(entry.info.validation, ValidationState::Validated) {
                return Err(SourceManagerError::InvalidRequest(
                    "validate the RTSP source before registering browser playback".into(),
                ));
            }
            let (username, password) = resolve_credentials(entry.options.credentials.as_ref());
            (
                entry.options.uri.clone().ok_or_else(|| {
                    SourceManagerError::InvalidRequest("RTSP source URI is missing".into())
                })?,
                username,
                password,
            )
        };
        let registration = MediaSourceRegistration {
            source_id: id.into(),
            rtsp_uri: uri,
            username,
            password,
        };
        match self.media_gateway.register_source(registration).await {
            Ok(()) => {
                self.set_media_health(id, MediaDeliveryHealth::Healthy)
                    .await;
                self.events.publish(Event {
                    kind: "media_source_registered".into(),
                    source_id: Some(id.into()),
                    message: "source registered with media gateway".into(),
                });
                self.playback(id).await
            }
            Err(error) => {
                self.set_media_health(id, MediaDeliveryHealth::Unavailable)
                    .await;
                self.events.publish(Event {
                    kind: "media_source_registration_failed".into(),
                    source_id: Some(id.into()),
                    message: error.message.clone(),
                });
                Err(SourceManagerError::MediaGateway(error.to_string()))
            }
        }
    }

    pub async fn remove_playback(&self, id: &str) -> Result<(), SourceManagerError> {
        if !self.sources.read().await.contains_key(id) {
            return Err(SourceManagerError::NotFound);
        }
        self.media_gateway
            .remove_source(id)
            .await
            .map_err(|error| SourceManagerError::MediaGateway(error.to_string()))?;
        self.set_media_health(id, MediaDeliveryHealth::Unknown)
            .await;
        self.events.publish(Event {
            kind: "media_source_removed".into(),
            source_id: Some(id.into()),
            message: "source removed from media gateway".into(),
        });
        Ok(())
    }

    pub async fn playback(&self, id: &str) -> Result<PlaybackInfo, SourceManagerError> {
        if !self.sources.read().await.contains_key(id) {
            return Err(SourceManagerError::NotFound);
        }
        let result = self.media_gateway.playback(id).await;
        match result {
            Ok(playback) => {
                self.set_media_health(id, playback.media_health.clone())
                    .await;
                Ok(playback)
            }
            Err(error) => {
                self.set_media_health(id, MediaDeliveryHealth::Unavailable)
                    .await;
                Err(SourceManagerError::MediaGateway(error.to_string()))
            }
        }
    }

    pub async fn shutdown_media_gateway(&self) {
        self.media_gateway.shutdown().await;
    }

    async fn set_media_health(&self, id: &str, health: MediaDeliveryHealth) {
        if let Some(entry) = self.sources.write().await.get_mut(id) {
            entry.info.media_health = health;
        }
    }

    pub async fn ptz_capabilities(&self, id: &str) -> Result<PtzCapabilities, SourceManagerError> {
        Ok(self
            .capabilities(id)
            .await?
            .map(|capabilities| capabilities.ptz)
            .unwrap_or_default())
    }

    pub async fn ptz_move(
        &self,
        id: &str,
        request: PtzMoveRequest,
        audit: PtzAuditContext,
    ) -> Result<PtzOperationResult, SourceManagerError> {
        let operation_name = format!("{:?}_move", request.mode).to_lowercase();
        let values = match validate_move(&request) {
            Ok(values) => values,
            Err(error) => {
                self.audit_ptz(
                    id,
                    &operation_name,
                    &audit,
                    &request,
                    false,
                    Some(error.to_string()),
                );
                return Err(error);
            }
        };
        let (session, capabilities) = match self.ptz_context(id).await {
            Ok(context) => context,
            Err(error) => {
                self.audit_ptz(
                    id,
                    &operation_name,
                    &audit,
                    &request,
                    false,
                    Some(error.to_string()),
                );
                return Err(error);
            }
        };
        let supported = match request.mode {
            PtzMoveMode::Continuous => capabilities.continuous_move,
            PtzMoveMode::Relative => capabilities.relative_move,
            PtzMoveMode::Absolute => capabilities.absolute_move,
        };
        if !supported {
            let error = SourceManagerError::PtzOperationUnsupported(operation_name.clone());
            self.audit_ptz(
                id,
                &operation_name,
                &audit,
                &request,
                false,
                Some(error.to_string()),
            );
            return Err(error);
        }
        if values.0 != 0.0 && !capabilities.pan
            || values.1 != 0.0 && !capabilities.tilt
            || values.2 != 0.0 && !capabilities.zoom
        {
            let error = SourceManagerError::PtzOperationUnsupported("requested_axis".into());
            self.audit_ptz(
                id,
                &operation_name,
                &audit,
                &request,
                false,
                Some(error.to_string()),
            );
            return Err(error);
        }
        let operation = match request.mode {
            PtzMoveMode::Continuous => PtzOperation::Continuous {
                pan: values.0,
                tilt: values.1,
                zoom: values.2,
            },
            PtzMoveMode::Relative => PtzOperation::Relative {
                pan: values.0,
                tilt: values.1,
                zoom: values.2,
            },
            PtzMoveMode::Absolute => PtzOperation::Absolute {
                pan: values.0,
                tilt: values.1,
                zoom: values.2,
            },
        };
        match self
            .onvif
            .ptz(&session, operation, self.validation_timeout)
            .await
        {
            Ok(PtzResponse::Ack) => {
                let result = PtzOperationResult {
                    operation: operation_name.clone(),
                    success: true,
                };
                self.audit_ptz(id, &operation_name, &audit, &request, true, None);
                Ok(result)
            }
            Ok(PtzResponse::Presets(_)) => {
                Err(SourceManagerError::Onvif("unexpected PTZ response".into()))
            }
            Err(error) => {
                self.audit_ptz(
                    id,
                    &operation_name,
                    &audit,
                    &request,
                    false,
                    Some(error.to_string()),
                );
                Err(SourceManagerError::Onvif(error.to_string()))
            }
        }
    }

    pub async fn ptz_stop(
        &self,
        id: &str,
        audit: PtzAuditContext,
    ) -> Result<PtzOperationResult, SourceManagerError> {
        let operation = "stop";
        let (session, _) = match self.ptz_context(id).await {
            Ok(context) => context,
            Err(error) => {
                self.audit_ptz(
                    id,
                    operation,
                    &audit,
                    &serde_json::json!({}),
                    false,
                    Some(error.to_string()),
                );
                return Err(error);
            }
        };
        match self
            .onvif
            .ptz(&session, PtzOperation::Stop, self.validation_timeout)
            .await
        {
            Ok(PtzResponse::Ack) => {
                let result = PtzOperationResult {
                    operation: operation.into(),
                    success: true,
                };
                self.audit_ptz(id, operation, &audit, &serde_json::json!({}), true, None);
                Ok(result)
            }
            Ok(PtzResponse::Presets(_)) => {
                Err(SourceManagerError::Onvif("unexpected PTZ response".into()))
            }
            Err(error) => {
                self.audit_ptz(
                    id,
                    operation,
                    &audit,
                    &serde_json::json!({}),
                    false,
                    Some(error.to_string()),
                );
                Err(SourceManagerError::Onvif(error.to_string()))
            }
        }
    }

    pub async fn ptz_presets(
        &self,
        id: &str,
        audit: PtzAuditContext,
    ) -> Result<Vec<PtzPreset>, SourceManagerError> {
        let (session, capabilities) = match self.ptz_context(id).await {
            Ok(context) => context,
            Err(error) => {
                self.audit_ptz(
                    id,
                    "get_presets",
                    &audit,
                    &serde_json::json!({}),
                    false,
                    Some(error.to_string()),
                );
                return Err(error);
            }
        };
        if !capabilities.presets {
            let error = SourceManagerError::PtzOperationUnsupported("presets".into());
            self.audit_ptz(
                id,
                "get_presets",
                &audit,
                &serde_json::json!({}),
                false,
                Some(error.to_string()),
            );
            return Err(error);
        }
        match self
            .onvif
            .ptz(&session, PtzOperation::GetPresets, self.validation_timeout)
            .await
        {
            Ok(PtzResponse::Presets(presets)) => {
                let mut sources = self.sources.write().await;
                let entry = sources.get_mut(id).ok_or(SourceManagerError::NotFound)?;
                entry.ptz_presets = presets
                    .iter()
                    .filter_map(|preset| {
                        preset.token.clone().map(|token| (preset.id.clone(), token))
                    })
                    .collect();
                self.audit_ptz(
                    id,
                    "get_presets",
                    &audit,
                    &serde_json::json!({"count": presets.len()}),
                    true,
                    None,
                );
                Ok(presets)
            }
            Ok(PtzResponse::Ack) => {
                Err(SourceManagerError::Onvif("unexpected PTZ response".into()))
            }
            Err(error) => {
                self.audit_ptz(
                    id,
                    "get_presets",
                    &audit,
                    &serde_json::json!({}),
                    false,
                    Some(error.to_string()),
                );
                Err(SourceManagerError::Onvif(error.to_string()))
            }
        }
    }

    pub async fn ptz_goto_preset(
        &self,
        id: &str,
        preset_id: &str,
        audit: PtzAuditContext,
    ) -> Result<PtzOperationResult, SourceManagerError> {
        let (session, capabilities, token) = {
            let sources = self.sources.read().await;
            let entry = sources.get(id).ok_or(SourceManagerError::NotFound)?;
            let capabilities = entry
                .info
                .capabilities
                .as_ref()
                .map(|value| value.ptz.clone())
                .unwrap_or_default();
            let session = entry
                .ptz_session
                .clone()
                .ok_or(SourceManagerError::PtzNotSupported)?;
            (
                session,
                capabilities,
                entry.ptz_presets.get(preset_id).cloned(),
            )
        };
        if !capabilities.supported {
            return Err(SourceManagerError::PtzNotSupported);
        }
        if !capabilities.presets {
            return Err(SourceManagerError::PtzOperationUnsupported(
                "presets".into(),
            ));
        }
        let token = token.ok_or_else(|| {
            SourceManagerError::InvalidRequest(
                "preset must be listed before it can be selected".into(),
            )
        })?;
        let operation = "goto_preset";
        match self
            .onvif
            .ptz(
                &session,
                PtzOperation::GotoPreset { token },
                self.validation_timeout,
            )
            .await
        {
            Ok(PtzResponse::Ack) => {
                let result = PtzOperationResult {
                    operation: operation.into(),
                    success: true,
                };
                self.audit_ptz(
                    id,
                    operation,
                    &audit,
                    &serde_json::json!({"presetId": preset_id}),
                    true,
                    None,
                );
                Ok(result)
            }
            Ok(PtzResponse::Presets(_)) => {
                Err(SourceManagerError::Onvif("unexpected PTZ response".into()))
            }
            Err(error) => {
                self.audit_ptz(
                    id,
                    operation,
                    &audit,
                    &serde_json::json!({"presetId": preset_id}),
                    false,
                    Some(error.to_string()),
                );
                Err(SourceManagerError::Onvif(error.to_string()))
            }
        }
    }

    async fn ptz_context(
        &self,
        id: &str,
    ) -> Result<(PtzSession, PtzCapabilities), SourceManagerError> {
        let sources = self.sources.read().await;
        let entry = sources.get(id).ok_or(SourceManagerError::NotFound)?;
        let capabilities = entry
            .info
            .capabilities
            .as_ref()
            .map(|value| value.ptz.clone())
            .unwrap_or_default();
        if !capabilities.supported {
            return Err(SourceManagerError::PtzNotSupported);
        }
        Ok((
            entry
                .ptz_session
                .clone()
                .ok_or(SourceManagerError::PtzNotSupported)?,
            capabilities,
        ))
    }

    fn audit_ptz<T: serde::Serialize>(
        &self,
        id: &str,
        operation: &str,
        audit: &PtzAuditContext,
        request: &T,
        success: bool,
        failure: Option<String>,
    ) {
        let mut record = EventRecord::simple(
            format!("ptz.{operation}"),
            Some(id.into()),
            if success {
                "PTZ operation succeeded"
            } else {
                "PTZ operation failed"
            },
        );
        record.metadata = serde_json::json!({"actor": audit.actor, "cameraId": id, "operation": operation, "request": request, "correlationId": audit.correlation_id, "outcome": if success { "success" } else { "failure" }, "failure": failure});
        self.events.publish_record(record);
    }

    pub fn spawn_health_monitor(&self) -> Option<JoinHandle<()>> {
        if !self.health_config.enabled {
            return None;
        }
        let manager = self.clone();
        let mut shutdown = self.shutdown.clone();
        let interval = Duration::from_secs(self.health_config.interval_seconds.max(1));
        Some(tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            loop {
                tokio::select! {
                    _ = ticker.tick() => manager.monitor_once().await,
                    changed = shutdown.changed() => {
                        if changed.is_err() || *shutdown.borrow() { break; }
                    }
                }
            }
        }))
    }

    pub async fn monitor_once(&self) {
        let ids = self
            .sources
            .read()
            .await
            .values()
            .filter(|entry| entry.info.kind == "rtsp")
            .map(|entry| entry.info.id.clone())
            .collect::<Vec<_>>();
        let mut tasks = Vec::new();
        for id in ids {
            let inserted = {
                let mut inflight = self.health_inflight.lock().await;
                inflight.insert(id.clone())
            };
            if !inserted {
                continue;
            }
            let manager = self.clone();
            let semaphore = self.health_semaphore.clone();
            tasks.push(tokio::spawn(async move {
                let Ok(permit) = semaphore.acquire_owned().await else {
                    manager.health_inflight.lock().await.remove(&id);
                    return;
                };
                manager.run_health_cycle(&id).await;
                drop(permit);
                manager.health_inflight.lock().await.remove(&id);
            }));
        }
        for task in tasks {
            let _ = task.await;
        }
    }

    async fn run_health_cycle(&self, id: &str) {
        let component = format!("source:{id}");
        let initial = self.validate(id).await;
        let Ok(result) = initial else {
            return;
        };
        if result.valid {
            self.mark_recovered(id, &component).await;
            return;
        }
        let Some(failure) = result.failure.as_ref() else {
            return;
        };
        if !is_retryable(&failure.code) || self.health_config.max_attempts == 0 {
            self.mark_exhausted(id, &component, failure.message.clone())
                .await;
            return;
        }
        let mut delay = Duration::from_millis(self.health_config.initial_backoff_ms.max(1));
        let max_delay = Duration::from_secs(self.health_config.max_backoff_seconds.max(1));
        for attempt in 1..=self.health_config.max_attempts {
            self.mark_recovery_started(id, attempt, delay).await;
            let mut shutdown = self.shutdown.clone();
            tokio::select! {
                _ = tokio::time::sleep(delay) => {},
                changed = shutdown.changed() => {
                    if changed.is_err() || *shutdown.borrow() { return; }
                }
            }
            let Ok(result) = self.validate(id).await else {
                return;
            };
            if result.valid {
                self.mark_recovered(id, &component).await;
                return;
            }
            let Some(failure) = result.failure.as_ref() else {
                return;
            };
            if !is_retryable(&failure.code) {
                self.mark_exhausted(id, &component, failure.message.clone())
                    .await;
                return;
            }
            if attempt == self.health_config.max_attempts {
                self.mark_exhausted(id, &component, failure.message.clone())
                    .await;
                return;
            }
            delay = (delay * 2).min(max_delay);
        }
    }

    async fn mark_recovery_started(&self, id: &str, attempt: u32, delay: Duration) {
        let now = now_ms();
        if let Some(entry) = self.sources.write().await.get_mut(id) {
            entry.info.recovery = RecoveryState::Recovering;
            entry.info.recovery_attempts = attempt;
            entry.info.last_recovery_started = Some(now);
            entry.info.next_recovery_at = Some(now + delay.as_millis());
        }
        self.recovery
            .monitor
            .set(
                format!("source:{id}").as_str(),
                crate::recovery::ComponentState::Recovering,
                "RTSP health recovery in progress",
                attempt as u64,
            )
            .await;
        self.events.publish(Event {
            kind: "source_recovery_started".into(),
            source_id: Some(id.into()),
            message: format!("RTSP recovery attempt {attempt}"),
        });
    }

    async fn mark_recovered(&self, id: &str, component: &str) {
        let now = now_ms();
        let was_recovering = if let Some(entry) = self.sources.write().await.get_mut(id) {
            let was = entry.info.recovery != RecoveryState::Idle;
            entry.info.recovery = RecoveryState::Idle;
            entry.info.recovery_attempts = 0;
            entry.info.last_recovery_succeeded = Some(now);
            entry.info.next_recovery_at = None;
            was
        } else {
            false
        };
        self.recovery
            .monitor
            .set(
                component,
                crate::recovery::ComponentState::Healthy,
                "RTSP source healthy",
                0,
            )
            .await;
        if was_recovering {
            self.events.publish(Event {
                kind: "source_recovered".into(),
                source_id: Some(id.into()),
                message: "RTSP source recovered".into(),
            });
        }
    }

    async fn mark_exhausted(&self, id: &str, component: &str, message: String) {
        let now = now_ms();
        if let Some(entry) = self.sources.write().await.get_mut(id) {
            entry.info.recovery = RecoveryState::Exhausted;
            entry.info.last_recovery_exhausted = Some(now);
            entry.info.next_recovery_at = None;
        }
        self.recovery
            .monitor
            .set(
                component,
                crate::recovery::ComponentState::Failed,
                message.clone(),
                self.health_config.max_attempts as u64,
            )
            .await;
        self.events.publish(Event {
            kind: "source_recovery_exhausted".into(),
            source_id: Some(id.into()),
            message,
        });
    }

    pub async fn validate(&self, id: &str) -> Result<RtspValidationResult, SourceManagerError> {
        let options = {
            let mut sources = self.sources.write().await;
            let entry = sources.get_mut(id).ok_or(SourceManagerError::NotFound)?;
            if entry.info.kind != "rtsp" {
                return Err(SourceManagerError::Unsupported(
                    "RTSP validation requires an RTSP source".into(),
                ));
            }
            entry.info.validation = ValidationState::Validating;
            entry.options.clone()
        };
        let uri = options.uri.clone().unwrap_or_default();
        let (username, password) = resolve_credentials(options.credentials.as_ref());
        let result = self
            .validator
            .validate(RtspValidationRequest {
                uri,
                username,
                password,
            })
            .await;
        let mut sources = self.sources.write().await;
        let entry = sources.get_mut(id).ok_or(SourceManagerError::NotFound)?;
        entry.info.last_validation_attempt = Some(result.checked_at);
        if result.valid {
            entry.info.validation = ValidationState::Validated;
            entry.info.health = StreamHealth::Healthy;
            entry.info.last_successful_validation = Some(result.checked_at);
            entry.info.last_validation_failure = None;
            entry.info.consecutive_validation_failures = 0;
        } else {
            entry.info.validation = ValidationState::Failed;
            entry.info.health = StreamHealth::Unhealthy;
            entry.info.consecutive_validation_failures =
                entry.info.consecutive_validation_failures.saturating_add(1);
            entry.info.last_validation_failure = result.failure.clone();
        }
        drop(sources);
        self.events.publish(Event {
            kind: if result.valid {
                "source_validation_succeeded"
            } else {
                "source_validation_failed"
            }
            .into(),
            source_id: Some(id.into()),
            message: result
                .failure
                .as_ref()
                .map(|failure| failure.message.clone())
                .unwrap_or_else(|| "RTSP source validation succeeded".into()),
        });
        Ok(result)
    }

    pub fn providers() -> Vec<CameraProviderInfo> {
        vec![
            CameraProviderInfo {
                id: "synthetic".into(),
                name: "Synthetic test camera".into(),
                discovery: true,
                test_connection: true,
                implemented: true,
            },
            CameraProviderInfo {
                id: "built-in-camera".into(),
                name: "Built-in / USB camera".into(),
                discovery: true,
                test_connection: true,
                implemented: true,
            },
            CameraProviderInfo {
                id: "rtsp".into(),
                name: "RTSP camera".into(),
                discovery: false,
                test_connection: true,
                implemented: true,
            },
            CameraProviderInfo {
                id: "mjpeg".into(),
                name: "HTTP/MJPEG camera".into(),
                discovery: false,
                test_connection: true,
                implemented: true,
            },
            CameraProviderInfo {
                id: "onvif".into(),
                name: "ONVIF discovery".into(),
                discovery: true,
                test_connection: false,
                implemented: true,
            },
            CameraProviderInfo {
                id: "video-file".into(),
                name: "Video file".into(),
                discovery: false,
                test_connection: true,
                implemented: true,
            },
        ]
    }

    pub async fn discover(&self) -> Vec<DiscoveredCamera> {
        let mut discovered = self
            .list()
            .await
            .into_iter()
            .map(|source| DiscoveredCamera {
                id: source.id,
                name: source.name,
                vendor: None,
                kind: source.kind,
                address: None,
                capabilities: vec!["live-preview".into(), "health-monitoring".into()],
                requires_manual_stream: false,
            })
            .collect::<Vec<_>>();
        if !discovered.iter().any(|camera| camera.kind == "synthetic") {
            discovered.push(DiscoveredCamera {
                id: "synthetic-discovery".into(),
                name: "Synthetic Test Camera".into(),
                vendor: Some("Sentinel".into()),
                kind: "synthetic".into(),
                address: None,
                capabilities: vec![
                    "live-preview".into(),
                    "health-monitoring".into(),
                    "hardware-free".into(),
                ],
                requires_manual_stream: false,
            });
        }
        const PROBE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<e:Envelope xmlns:e="http://www.w3.org/2003/05/soap-envelope" xmlns:w="http://schemas.xmlsoap.org/ws/2004/08/addressing" xmlns:d="http://schemas.xmlsoap.org/ws/2005/04/discovery" xmlns:dn="http://www.onvif.org/ver10/network/wsdl">
  <e:Header><w:MessageID>uuid:sentinel-discovery</w:MessageID><w:To>urn:schemas-xmlsoap-org:ws:2005:04:discovery</w:To><w:Action>http://schemas.xmlsoap.org/ws/2005/04/discovery/Probe</w:Action></e:Header>
  <e:Body><d:Probe><d:Type>dn:NetworkVideoTransmitter</d:Type></d:Probe></e:Body>
</e:Envelope>"#;
        if let Ok(socket) = UdpSocket::bind("0.0.0.0:0").await {
            let _ = socket
                .send_to(PROBE.as_bytes(), "239.255.255.250:3702")
                .await;
            let mut buffer = [0u8; 8192];
            loop {
                let received = tokio::time::timeout(
                    std::time::Duration::from_millis(250),
                    socket.recv_from(&mut buffer),
                )
                .await;
                let Ok(Ok((length, address))) = received else {
                    break;
                };
                let response = String::from_utf8_lossy(&buffer[..length]);
                let xaddr = xml_value(&response, "XAddrs");
                let scopes = xml_value(&response, "Scopes");
                let identity = xaddr.clone().unwrap_or_else(|| address.to_string());
                let id = format!("onvif-{:x}", stable_hash(&identity));
                if discovered.iter().any(|camera| camera.id == id) {
                    continue;
                }
                discovered.push(DiscoveredCamera {
                    id,
                    name: scopes
                        .as_deref()
                        .and_then(scope_name)
                        .unwrap_or("ONVIF camera")
                        .into(),
                    vendor: scopes.as_deref().and_then(scope_vendor),
                    kind: "onvif".into(),
                    address: xaddr,
                    capabilities: vec![
                        "onvif-discovery".into(),
                        "stream-profile-discovery".into(),
                        "health-monitoring".into(),
                    ],
                    requires_manual_stream: true,
                });
            }
        }
        discovered
    }

    pub async fn test_connection(&self, request: ConnectionTestRequest) -> ConnectionTestResult {
        let started = std::time::Instant::now();
        let kind = request.kind.clone();
        let result = match kind.as_str() {
            "built-in-camera" => CameraWorker::start(self.fps),
            "synthetic" => CameraWorker::start_synthetic(
                request.options.width.unwrap_or(640),
                request.options.height.unwrap_or(360),
                request.options.fps.unwrap_or(self.fps),
            ),
            "image-sequence" | "video-file" => request
                .options
                .path
                .clone()
                .ok_or_else(|| SourceManagerError::Camera("video file source requires path".into()))
                .and_then(|path| {
                    CameraWorker::start_video_file(
                        path,
                        request.options.loop_playback.unwrap_or(true),
                        request.options.width.unwrap_or(640),
                        request.options.height.unwrap_or(360),
                        request.options.fps.unwrap_or(self.fps),
                    )
                }),
            "rtsp" => CameraWorker::start_rtsp(request.options.clone(), self.fps),
            "mjpeg" => CameraWorker::start_mjpeg(request.options.clone(), self.fps),
            "onvif" => Err(SourceManagerError::Unsupported(
                "ONVIF requires a discovered stream profile".into(),
            )),
            other => Err(SourceManagerError::Unsupported(other.into())),
        };
        match result {
            Ok(worker) => {
                let frame = tokio::time::timeout(std::time::Duration::from_secs(5), async {
                    worker.frames.lock().await.recv().await
                })
                .await;
                let worker = worker;
                worker.stop();
                match frame {
                    Ok(Some(frame)) => ConnectionTestResult {
                        success: true,
                        source_type: kind,
                        resolution: Some(format!("{}x{}", frame.width, frame.height)),
                        fps: request.options.fps.unwrap_or(self.fps) as f64,
                        latency_ms: started.elapsed().as_millis() as u64,
                        message: "Connection succeeded and a frame was captured".into(),
                    },
                    Ok(None) => ConnectionTestResult {
                        success: false,
                        source_type: kind,
                        resolution: None,
                        fps: 0.0,
                        latency_ms: started.elapsed().as_millis() as u64,
                        message: "Source stopped before a frame was captured".into(),
                    },
                    Err(_) => ConnectionTestResult {
                        success: false,
                        source_type: kind,
                        resolution: None,
                        fps: 0.0,
                        latency_ms: started.elapsed().as_millis() as u64,
                        message: "Timed out waiting for the first frame".into(),
                    },
                }
            }
            Err(error) => ConnectionTestResult {
                success: false,
                source_type: kind,
                resolution: None,
                fps: 0.0,
                latency_ms: started.elapsed().as_millis() as u64,
                message: error.to_string(),
            },
        }
    }

    pub async fn start(&self, id: &str) -> Result<SourceInfo, SourceManagerError> {
        let (kind, options) = {
            let mut sources = self.sources.write().await;
            let entry = sources.get_mut(id).ok_or(SourceManagerError::NotFound)?;
            entry.desired_running = true;
            entry.info.status = SourceState::Starting;
            (entry.info.kind.clone(), entry.options.clone())
        };
        if self.worker(id).await.is_some() {
            return self.get(id).await;
        }
        // The current pipeline has one FrameProvider stream. Switching sources
        // therefore stops the previous active worker before selecting the new one.
        if let Some(active) = self.active_source.read().await.clone() {
            if active != id {
                let _ = self.stop(&active).await;
            }
        }
        let worker = match kind.as_str() {
            "built-in-camera" => CameraWorker::start(self.fps),
            "synthetic" => CameraWorker::start_synthetic(
                options.width.unwrap_or(640),
                options.height.unwrap_or(360),
                options.fps.unwrap_or(self.fps),
            ),
            "image-sequence" | "video-file" => CameraWorker::start_video_file(
                options.path.ok_or_else(|| {
                    SourceManagerError::Camera("video file source requires path".into())
                })?,
                options.loop_playback.unwrap_or(true),
                options.width.unwrap_or(640),
                options.height.unwrap_or(360),
                options.fps.unwrap_or(self.fps),
            ),
            "rtsp" => CameraWorker::start_rtsp(options, self.fps),
            "mjpeg" => CameraWorker::start_mjpeg(options, self.fps),
            other => return Err(SourceManagerError::Unsupported(other.into())),
        };
        match worker {
            Ok(worker) => {
                self.set_worker(id, worker).await;
                *self.active_source.write().await = Some(id.to_string());
                let mut sources = self.sources.write().await;
                let entry = sources.get_mut(id).ok_or(SourceManagerError::NotFound)?;
                entry.info.status = SourceState::Running;
                entry.info.connections += 1;
                entry.started_at = Some(std::time::Instant::now());
                entry.disconnected_at = None;
                entry.info.downtime_seconds = 0;
                let snapshot = self.snapshot(&entry.info, entry.started_at, entry.disconnected_at);
                drop(sources);
                self.recovery
                    .monitor
                    .set(
                        "camera",
                        crate::recovery::ComponentState::Healthy,
                        "source running",
                        0,
                    )
                    .await;
                self.events.publish(Event {
                    kind: "source_started".into(),
                    source_id: Some(id.into()),
                    message: "source started".into(),
                });
                Ok(snapshot)
            }
            Err(error) => {
                let mut sources = self.sources.write().await;
                if let Some(entry) = sources.get_mut(id) {
                    entry.info.status = SourceState::Failed;
                }
                drop(sources);
                self.recovery.failed("camera", error.to_string(), 1).await;
                self.events.publish(Event {
                    kind: "source_failed".into(),
                    source_id: Some(id.into()),
                    message: error.to_string(),
                });
                Err(SourceManagerError::Camera(error.to_string()))
            }
        }
    }
    pub async fn stop(&self, id: &str) -> Result<SourceInfo, SourceManagerError> {
        let mut sources = self.sources.write().await;
        let entry = sources.get_mut(id).ok_or(SourceManagerError::NotFound)?;
        entry.info.status = SourceState::Stopping;
        entry.desired_running = false;
        drop(sources);
        if let Some(worker) = self.take_worker(id).await {
            worker.stop();
        }
        if self.active_source.read().await.as_deref() == Some(id) {
            *self.active_source.write().await = None;
        }
        let mut sources = self.sources.write().await;
        let entry = sources.get_mut(id).ok_or(SourceManagerError::NotFound)?;
        entry.info.status = SourceState::Stopped;
        entry.started_at = None;
        entry.disconnected_at = None;
        entry.info.downtime_seconds = 0;
        self.events.publish(Event {
            kind: "source_stopped".into(),
            source_id: Some(id.into()),
            message: "source stopped".into(),
        });
        Ok(self.snapshot(&entry.info, entry.started_at, entry.disconnected_at))
    }
    pub async fn restart(&self, id: &str) -> Result<SourceInfo, SourceManagerError> {
        self.stop(id).await.ok();
        let result = self.start(id).await?;
        self.events.publish(Event {
            kind: "source_restarted".into(),
            source_id: Some(id.into()),
            message: "source restarted".into(),
        });
        Ok(result)
    }
    pub async fn remove(&self, id: &str) -> Result<(), SourceManagerError> {
        self.stop(id).await?;
        let is_rtsp = self
            .sources
            .read()
            .await
            .get(id)
            .map(|entry| entry.info.kind == "rtsp")
            .unwrap_or(false);
        if is_rtsp {
            let _ = self.media_gateway.remove_source(id).await;
        }
        if self.sources.write().await.remove(id).is_some() {
            Ok(())
        } else {
            Err(SourceManagerError::NotFound)
        }
    }
    pub async fn prometheus(&self) -> String {
        let list = self.list().await;
        let active = list
            .iter()
            .filter(|source| matches!(source.status, SourceState::Running))
            .count();
        let failed = list
            .iter()
            .filter(|source| matches!(source.status, SourceState::Failed))
            .count();
        let reconnects: u64 = list.iter().map(|source| source.reconnect_count).sum();
        let downtime: u64 = list.iter().map(|source| source.downtime_seconds).sum();
        let mut output = format!("sentinel_registered_sources {}\nsentinel_active_sources {}\nsentinel_failed_sources {}\nsentinel_source_reconnect_attempts {}\nsentinel_source_downtime_seconds {}\n", list.len(), active, failed, reconnects, downtime);
        for source in list {
            output.push_str(&format!(
                "sentinel_source_frames_received{{source=\"{}\"}} {}\nsentinel_source_frames_generated{{source=\"{}\"}} {}\nsentinel_source_frames_dropped{{source=\"{}\"}} {}\nsentinel_source_packets_received{{source=\"{}\"}} {}\nsentinel_source_connections{{source=\"{}\"}} {}\nsentinel_source_disconnects{{source=\"{}\"}} {}\nsentinel_source_decode_failures{{source=\"{}\"}} {}\nsentinel_source_reconnect_count{{source=\"{}\"}} {}\nsentinel_source_downtime_seconds{{source=\"{}\"}} {}\n",
                source.id, source.frames_received, source.id, source.frames_generated, source.id, source.frames_dropped, source.id, source.packets_received, source.id, source.connections, source.id, source.disconnects, source.id, source.decode_failures, source.id, source.reconnect_count, source.id, source.downtime_seconds
            ));
        }
        output
    }

    async fn worker(&self, id: &str) -> Option<Arc<Mutex<tokio_mpsc::Receiver<Frame>>>> {
        match id {
            "builtin" => self
                .built_in
                .read()
                .await
                .as_ref()
                .map(|w| w.frames.clone()),
            _ => {
                if self.synthetic.read().await.is_some()
                    && self.active_source.read().await.as_deref() == Some(id)
                {
                    self.synthetic
                        .read()
                        .await
                        .as_ref()
                        .map(|w| w.frames.clone())
                } else if self.video_file.read().await.is_some()
                    && self.active_source.read().await.as_deref() == Some(id)
                {
                    self.video_file
                        .read()
                        .await
                        .as_ref()
                        .map(|w| w.frames.clone())
                } else if self.rtsp.read().await.is_some()
                    && self.active_source.read().await.as_deref() == Some(id)
                {
                    self.rtsp.read().await.as_ref().map(|w| w.frames.clone())
                } else if self.mjpeg.read().await.is_some()
                    && self.active_source.read().await.as_deref() == Some(id)
                {
                    self.mjpeg.read().await.as_ref().map(|w| w.frames.clone())
                } else {
                    None
                }
            }
        }
    }
    async fn set_worker(&self, id: &str, worker: CameraWorker) {
        match id {
            "builtin" => *self.built_in.write().await = Some(worker),
            _ => {
                let kind = self
                    .sources
                    .read()
                    .await
                    .get(id)
                    .map(|e| e.info.kind.clone());
                if kind.as_deref() == Some("synthetic") {
                    *self.synthetic.write().await = Some(worker);
                } else if kind.as_deref() == Some("rtsp") {
                    *self.rtsp.write().await = Some(worker);
                } else if kind.as_deref() == Some("mjpeg") {
                    *self.mjpeg.write().await = Some(worker);
                } else {
                    *self.video_file.write().await = Some(worker);
                }
            }
        }
    }
    async fn take_worker(&self, id: &str) -> Option<CameraWorker> {
        match id {
            "builtin" => self.built_in.write().await.take(),
            _ => {
                let kind = self
                    .sources
                    .read()
                    .await
                    .get(id)
                    .map(|e| e.info.kind.clone());
                if kind.as_deref() == Some("synthetic") {
                    self.synthetic.write().await.take()
                } else if kind.as_deref() == Some("rtsp") {
                    self.rtsp.write().await.take()
                } else if kind.as_deref() == Some("mjpeg") {
                    self.mjpeg.write().await.take()
                } else {
                    self.video_file.write().await.take()
                }
            }
        }
    }
}

fn resolve_credentials(credentials: Option<&RtspCredentials>) -> (Option<String>, Option<String>) {
    let Some(credentials) = credentials else {
        return (None, None);
    };
    let username = credentials.username.clone().or_else(|| {
        credentials
            .username_env
            .as_ref()
            .and_then(|name| std::env::var(name).ok())
    });
    let password = credentials.password.clone().or_else(|| {
        credentials
            .password_env
            .as_ref()
            .and_then(|name| std::env::var(name).ok())
    });
    (username, password)
}
impl VideoSourceManager {
    async fn reconnect_virtual_until_running(&self, id: &str) -> bool {
        if !self.reconnect.enabled {
            return false;
        }
        let started = self
            .recovery
            .begin("camera", "source reconnect required")
            .await;
        let mut delay = std::time::Duration::from_millis(self.reconnect.initial_delay_ms.max(1));
        let maximum = std::time::Duration::from_secs(self.reconnect.max_delay_seconds.max(1));
        let mut shutdown = self.shutdown.clone();
        loop {
            let desired = self
                .sources
                .read()
                .await
                .get(id)
                .map(|entry| entry.desired_running)
                .unwrap_or(false);
            if !desired || *shutdown.borrow() {
                return false;
            }
            if let Some(entry) = self.sources.write().await.get_mut(id) {
                entry.info.status = SourceState::Reconnecting;
            }
            let wait = if self.reconnect.jitter {
                delay
                    + std::time::Duration::from_millis(
                        (now_ms() % (delay.as_millis().max(1) / 4 + 1)) as u64,
                    )
            } else {
                delay
            };
            self.events.publish(Event {
                kind: "source_reconnecting".into(),
                source_id: Some(id.into()),
                message: "source reconnecting".into(),
            });
            tokio::select! {
                _ = tokio::time::sleep(wait) => {}
                changed = shutdown.changed() => { if changed.is_err() || *shutdown.borrow() { return false; } }
            }
            match self.start(id).await {
                Ok(_) => {
                    self.recovery
                        .recovered("camera", started, "source recovered")
                        .await;
                    self.events.publish(Event {
                        kind: "source_recovered".into(),
                        source_id: Some(id.into()),
                        message: "source recovered".into(),
                    });
                    return true;
                }
                Err(error) => {
                    self.recovery
                        .attempt_failed("camera", "source reconnect failed", 1)
                        .await;
                    tracing::warn!(source = %id, error = %error, "source reconnect attempt failed");
                }
            }
            if !self.reconnect.retry_forever {
                return false;
            }
            delay = std::cmp::min(delay.saturating_mul(2), maximum);
        }
    }
    async fn reconnect_until_running(&self) -> bool {
        if !self.reconnect.enabled {
            return false;
        }
        let recovery_started = self
            .recovery
            .begin("camera", "camera reconnect required")
            .await;
        let mut delay = std::time::Duration::from_millis(self.reconnect.initial_delay_ms.max(1));
        let maximum = std::time::Duration::from_secs(self.reconnect.max_delay_seconds.max(1));
        let mut shutdown = self.shutdown.clone();
        loop {
            let desired = self
                .sources
                .read()
                .await
                .get("builtin")
                .map(|entry| entry.desired_running)
                .unwrap_or(false);
            if !desired || *shutdown.borrow() {
                return false;
            }
            {
                let mut sources = self.sources.write().await;
                if let Some(entry) = sources.get_mut("builtin") {
                    entry.info.status = SourceState::Reconnecting;
                    entry
                        .disconnected_at
                        .get_or_insert_with(std::time::Instant::now);
                }
            }
            let wait = if self.reconnect.jitter {
                let jitter = (now_ms() % (delay.as_millis().max(1) / 4 + 1)) as u64;
                delay + std::time::Duration::from_millis(jitter)
            } else {
                delay
            };
            tracing::warn!(
                delay_ms = wait.as_millis() as u64,
                "retrying camera connection"
            );
            tokio::select! {
                _ = tokio::time::sleep(wait) => {}
                changed = shutdown.changed() => { if changed.is_err() || *shutdown.borrow() { return false; } }
            }
            let fps = self.fps;
            let result = tokio::task::spawn_blocking(move || CameraWorker::start(fps)).await;
            match result {
                Ok(Ok(worker)) => {
                    *self.built_in.write().await = Some(worker);
                    let mut sources = self.sources.write().await;
                    if let Some(entry) = sources.get_mut("builtin") {
                        entry.info.status = SourceState::Running;
                        entry.started_at = Some(std::time::Instant::now());
                        entry.info.downtime_seconds = entry
                            .disconnected_at
                            .map(|at| at.elapsed().as_secs())
                            .unwrap_or(0);
                        entry.disconnected_at = None;
                    }
                    drop(sources);
                    self.recovery
                        .recovered("camera", recovery_started, "camera reconnected")
                        .await;
                    self.events.publish(Event {
                        kind: "source_restarted".into(),
                        source_id: Some("builtin".into()),
                        message: "source reconnected".into(),
                    });
                    tracing::info!("camera reconnected");
                    return true;
                }
                Ok(Err(error)) => {
                    let attempts = self
                        .sources
                        .write()
                        .await
                        .get_mut("builtin")
                        .map(|entry| {
                            entry.info.reconnect_count += 1;
                            entry.info.reconnect_count
                        })
                        .unwrap_or(0);
                    self.recovery
                        .attempt_failed("camera", error.to_string(), attempts)
                        .await;
                    tracing::warn!(error=%error, "camera reconnect attempt failed");
                }
                Err(error) => {
                    let attempts = self
                        .sources
                        .write()
                        .await
                        .get_mut("builtin")
                        .map(|entry| {
                            entry.info.reconnect_count += 1;
                            entry.info.reconnect_count
                        })
                        .unwrap_or(0);
                    self.recovery
                        .attempt_failed("camera", error.to_string(), attempts)
                        .await;
                    tracing::warn!(error=%error, "camera reconnect task failed");
                }
            }
            if !self.reconnect.retry_forever {
                return false;
            }
            delay = std::cmp::min(delay.saturating_mul(2), maximum);
        }
    }
}
#[async_trait]
impl FrameProvider for VideoSourceManager {
    async fn next_frame(&mut self) -> Result<Frame, SourceError> {
        loop {
            let active = self.active_source.read().await.clone();
            let Some(active) = active else {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                continue;
            };
            let receiver = self.worker(&active).await;
            let result = match receiver {
                Some(receiver) => receiver
                    .lock()
                    .await
                    .recv()
                    .await
                    .ok_or_else(|| SourceError("camera worker stopped".into())),
                None => Err(SourceError("camera is not open".into())),
            };
            match result {
                Ok(frame) => {
                    let mut sources = self.sources.write().await;
                    if let Some(entry) = sources.get_mut(&active) {
                        entry.info.last_frame = Some(now_ms());
                        entry.info.frames_received += 1;
                        entry.info.frames_generated += 1;
                        entry.info.packets_received += 1;
                        entry.info.resolution = Some(format!("{}x{}", frame.width, frame.height));
                        entry.info.fps = entry
                            .started_at
                            .map(|started| {
                                entry.info.frames_received as f64
                                    / started.elapsed().as_secs_f64().max(1.0)
                            })
                            .unwrap_or(0.0);
                    }
                    return Ok(frame);
                }
                Err(error) => {
                    let mut sources = self.sources.write().await;
                    let reconnect = sources
                        .get_mut(&active)
                        .map(|entry| {
                            entry.info.status = SourceState::Disconnected;
                            entry.info.reconnect_count += 1;
                            entry.info.disconnects += 1;
                            entry.info.decode_failures += 1;
                            entry.info.last_failure_category = Some("decode-or-connection".into());
                            entry
                                .disconnected_at
                                .get_or_insert_with(std::time::Instant::now);
                            entry.desired_running
                        })
                        .unwrap_or(false);
                    drop(sources);
                    if let Some(worker) = self.take_worker(&active).await {
                        worker.stop();
                    }
                    self.events.publish(Event {
                        kind: "source_disconnected".into(),
                        source_id: Some(active.clone()),
                        message: error.to_string(),
                    });
                    if reconnect && active == "builtin" {
                        self.reconnect_until_running().await;
                    } else if reconnect {
                        let _ = self.reconnect_virtual_until_running(&active).await;
                    }
                }
            }
        }
    }
}
fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn is_retryable(code: &crate::rtsp::RtspFailureCode) -> bool {
    matches!(
        code,
        crate::rtsp::RtspFailureCode::SourceUnreachable
            | crate::rtsp::RtspFailureCode::ConnectionTimeout
            | crate::rtsp::RtspFailureCode::Unknown
    )
}

fn validate_move(request: &PtzMoveRequest) -> Result<(f32, f32, f32), SourceManagerError> {
    let values = (
        request.pan.unwrap_or(0.0),
        request.tilt.unwrap_or(0.0),
        request.zoom.unwrap_or(0.0),
    );
    if [values.0, values.1, values.2]
        .iter()
        .any(|value| !value.is_finite() || !(-1.0..=1.0).contains(value))
    {
        return Err(SourceManagerError::InvalidRequest(
            "PTZ movement values must be finite and between -1 and 1".into(),
        ));
    }
    if values == (0.0, 0.0, 0.0) {
        return Err(SourceManagerError::InvalidRequest(
            "PTZ movement must request at least one non-zero axis".into(),
        ));
    }
    Ok(values)
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod onvif_handoff_tests {
    use super::*;
    use crate::onvif::{
        emulator::OnvifEmulator, CameraCapabilities, MediaProfile, OnvifDevice, OnvifInspection,
        PtzCapabilities,
    };
    use async_trait::async_trait;

    struct SuccessfulRtsp;
    #[async_trait]
    impl crate::rtsp::RtspValidationBackend for SuccessfulRtsp {
        async fn validate(
            &self,
            _request: &crate::rtsp::RtspValidationRequest,
            _timeout: Duration,
        ) -> Result<(u16, u16), RtspFailure> {
            Ok((200, 200))
        }
    }

    #[tokio::test]
    async fn inspection_hands_selected_rtsp_uri_to_existing_validator() {
        let endpoint = "http://emulator/onvif";
        let inspection = OnvifInspection {
            device: OnvifDevice {
                id: "onvif-emulator".into(),
                address: endpoint.into(),
                manufacturer: Some("Sentinel".into()),
                model: Some("Test PTZ".into()),
                capabilities: CameraCapabilities {
                    video: true,
                    audio: false,
                    snapshot: false,
                    events: true,
                    ptz: PtzCapabilities {
                        supported: true,
                        pan: true,
                        tilt: true,
                        zoom: true,
                        continuous_move: true,
                        relative_move: true,
                        absolute_move: true,
                        presets: true,
                    },
                },
                profiles: vec![MediaProfile {
                    name: "Main".into(),
                    encoding: Some("H264".into()),
                    resolution: Some("1280x720".into()),
                    frame_rate: Some(25.0),
                    audio: false,
                    rtsp_uri: Some("rtsp://camera/main".into()),
                    token: Some("profile-token".into()),
                }],
            },
            selected_rtsp_uri: Some("rtsp://camera/main".into()),
            ptz_session: Some(PtzSession {
                service_endpoint: endpoint.into(),
                profile_token: "profile-token".into(),
                username: Some("admin".into()),
                password: Some("secret".into()),
            }),
        };
        let emulator = Arc::new(OnvifEmulator::new(vec![inspection]));
        emulator.presets.lock().unwrap().push(PtzPreset {
            id: "preset-1".into(),
            name: "Entrance".into(),
            token: Some("preset-token".into()),
        });
        let events = EventBus::new(16);
        let (_, shutdown) = watch::channel(false);
        let manager = VideoSourceManager::new(
            30,
            events.clone(),
            crate::config::Config::default().recovery.camera,
            shutdown,
            RecoveryEngine::new(events),
        )
        .with_onvif_client(OnvifClient::default().with_backend(emulator.clone()))
        .with_validator(RtspValidator::default().with_backend(Arc::new(SuccessfulRtsp)));
        manager
            .add(AddSource {
                id: "camera".into(),
                kind: "rtsp".into(),
                options: SourceOptions {
                    uri: Some("rtsp://placeholder".into()),
                    ..Default::default()
                },
            })
            .await
            .unwrap();
        let result = manager
            .inspect_onvif(
                "camera",
                OnvifInspectRequest {
                    endpoint: endpoint.into(),
                    username: Some("admin".into()),
                    password: Some("secret".into()),
                    timeout_ms: Some(100),
                },
            )
            .await
            .unwrap();
        assert_eq!(result["rtspValidation"]["valid"], true);
        let source = manager.get("camera").await.unwrap();
        assert_eq!(source.validation, ValidationState::Validated);
        assert!(source.capabilities.unwrap().ptz.supported);
        let audit = PtzAuditContext {
            actor: "operator".into(),
            correlation_id: "corr-123".into(),
        };
        manager
            .ptz_move(
                "camera",
                PtzMoveRequest {
                    mode: PtzMoveMode::Continuous,
                    pan: Some(0.5),
                    tilt: Some(0.0),
                    zoom: Some(0.0),
                },
                audit.clone(),
            )
            .await
            .unwrap();
        manager.ptz_stop("camera", audit.clone()).await.unwrap();
        manager
            .ptz_move(
                "camera",
                PtzMoveRequest {
                    mode: PtzMoveMode::Relative,
                    pan: Some(0.1),
                    tilt: Some(0.0),
                    zoom: Some(0.0),
                },
                audit.clone(),
            )
            .await
            .unwrap();
        manager
            .ptz_move(
                "camera",
                PtzMoveRequest {
                    mode: PtzMoveMode::Absolute,
                    pan: Some(0.0),
                    tilt: Some(0.1),
                    zoom: Some(0.0),
                },
                audit.clone(),
            )
            .await
            .unwrap();
        let presets = manager.ptz_presets("camera", audit.clone()).await.unwrap();
        manager
            .ptz_goto_preset("camera", &presets[0].id, audit)
            .await
            .unwrap();
        let operations = emulator.soap_operations.lock().unwrap().clone();
        assert!(operations
            .iter()
            .any(|soap| soap.contains("ContinuousMove")));
        assert!(operations.iter().any(|soap| soap.contains("GotoPreset")));
        assert!(operations.iter().all(|soap| !soap.contains("preset-token")
            && !soap.contains("profile-token")
            && !soap.contains("secret")));
        tokio::time::sleep(Duration::from_millis(1)).await;
        let event = manager
            .events
            .store()
            .recent(20)
            .await
            .into_iter()
            .find(|event| event.event_type == "ptz.continuous_move")
            .unwrap();
        assert_eq!(event.metadata["correlationId"], "corr-123");
    }

    #[tokio::test]
    async fn onboarding_selects_best_profile_without_exposing_credentials() {
        let endpoint = "http://emulator/onvif";
        let inspection = OnvifInspection {
            device: OnvifDevice {
                id: "onvif-emulator".into(),
                address: endpoint.into(),
                manufacturer: Some("Sentinel".into()),
                model: Some("Onboarding Camera".into()),
                capabilities: CameraCapabilities {
                    video: true,
                    audio: true,
                    snapshot: true,
                    events: true,
                    ptz: PtzCapabilities::default(),
                },
                profiles: vec![
                    MediaProfile {
                        name: "Low".into(),
                        encoding: Some("H265".into()),
                        resolution: Some("640x360".into()),
                        frame_rate: Some(15.0),
                        audio: false,
                        rtsp_uri: Some("rtsp://camera/low".into()),
                        token: Some("low-token".into()),
                    },
                    MediaProfile {
                        name: "Main".into(),
                        encoding: Some("H264".into()),
                        resolution: Some("1920x1080".into()),
                        frame_rate: Some(25.0),
                        audio: true,
                        rtsp_uri: Some("rtsp://camera/main".into()),
                        token: Some("main-token".into()),
                    },
                ],
            },
            selected_rtsp_uri: Some("rtsp://camera/low".into()),
            ptz_session: None,
        };
        let emulator = Arc::new(OnvifEmulator::new(vec![inspection]));
        let events = EventBus::new(16);
        let (_, shutdown) = watch::channel(false);
        let manager = VideoSourceManager::new(
            30,
            events.clone(),
            crate::config::Config::default().recovery.camera,
            shutdown,
            RecoveryEngine::new(events),
        )
        .with_onvif_client(OnvifClient::default().with_backend(emulator));
        let session = manager
            .onboarding_discover(OnvifDiscoveryRequest {
                address: None,
                username: None,
                password: None,
                timeout_ms: Some(100),
            })
            .await
            .unwrap();
        let inspected = manager
            .onboarding_inspect(
                &session.session_id,
                crate::onboarding::OnboardingInspectRequest {
                    endpoint: endpoint.into(),
                    username: Some("admin".into()),
                    password: Some("secret".into()),
                    timeout_ms: Some(100),
                },
            )
            .await
            .unwrap();
        assert_eq!(inspected.selected_profile.as_ref().unwrap().name, "Main");
        let serialized = serde_json::to_string(&inspected).unwrap();
        assert!(!serialized.contains("secret"));
        assert!(!serialized.contains("main-token"));
        assert!(serialized.contains("A usable video profile was selected automatically"));
    }
}

fn xml_value(document: &str, element: &str) -> Option<String> {
    let namespaced_suffix = format!(":{element}");
    document.split('<').skip(1).find_map(|part| {
        let tag_end = part.find('>')?;
        let tag = &part[..tag_end];
        let tag_name = tag.split_whitespace().next().unwrap_or(tag);
        if tag_name != element && !tag_name.ends_with(&namespaced_suffix) {
            return None;
        }
        let tail = &part[tag_end + 1..];
        let close = format!("</{tag_name}>");
        let end = tail.find(&close)?;
        Some(tail[..end].trim().to_string())
    })
}

fn scope_name(scopes: &str) -> Option<&str> {
    scopes
        .split_whitespace()
        .find_map(|scope| scope.strip_prefix("onvif://www.onvif.org/name/"))
}

fn scope_vendor(scopes: &str) -> Option<String> {
    scopes
        .split_whitespace()
        .find_map(|scope| scope.strip_prefix("onvif://www.onvif.org/hardware/"))
        .map(str::to_string)
}

fn stable_hash(value: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}
