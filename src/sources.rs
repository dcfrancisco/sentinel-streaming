use crate::{
    config::ReconnectConfig,
    errors::SourceError,
    events::{Event, EventBus},
    frame::Frame,
    recovery::RecoveryEngine,
};
use async_trait::async_trait;
use nokhwa::{
    pixel_format::RgbFormat,
    utils::{CameraIndex, RequestedFormat, RequestedFormatType},
    Camera,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fmt, fs,
    io::Read,
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{mpsc, Arc},
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::net::UdpSocket;
use tokio::sync::{mpsc as tokio_mpsc, watch, Mutex, RwLock};

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

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RtspCredentials {
    pub username_env: Option<String>,
    pub password_env: Option<String>,
    #[serde(skip_serializing)]
    pub username: Option<String>,
    #[serde(skip_serializing)]
    pub password: Option<String>,
}
#[allow(dead_code)]
pub struct OnvifCamera;

#[derive(Clone, Debug, Default, Deserialize)]
pub struct SourceOptions {
    pub name: Option<String>,
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
#[derive(Clone, Debug, Serialize)]
pub struct SourceInfo {
    pub id: String,
    pub name: String,
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

struct SourceEntry {
    info: SourceInfo,
    desired_running: bool,
    started_at: Option<std::time::Instant>,
    disconnected_at: Option<std::time::Instant>,
    options: SourceOptions,
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
}
impl fmt::Display for SourceManagerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => f.write_str("source not found"),
            Self::AlreadyExists => f.write_str("source already exists"),
            Self::Unsupported(kind) => write!(f, "source type '{kind}' is not implemented"),
            Self::Camera(error) => f.write_str(error),
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
        }
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
        };
        sources.insert(
            request.id,
            SourceEntry {
                info: info.clone(),
                desired_running: false,
                started_at: None,
                disconnected_at: None,
                options: request.options,
            },
        );
        Ok(info)
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
