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
    fmt,
    sync::{mpsc, Arc},
    time::{SystemTime, UNIX_EPOCH},
};
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
#[allow(dead_code)]
pub struct OnvifCamera;
#[allow(dead_code)]
pub struct VideoFile;

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
}
#[derive(Clone, Debug, Deserialize)]
pub struct AddSource {
    pub id: String,
    pub kind: String,
}

struct SourceEntry {
    info: SourceInfo,
    desired_running: bool,
    started_at: Option<std::time::Instant>,
    disconnected_at: Option<std::time::Instant>,
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
        };
        let mut sources = BTreeMap::new();
        sources.insert(
            "builtin".into(),
            SourceEntry {
                info,
                desired_running: false,
                started_at: None,
                disconnected_at: None,
            },
        );
        Self {
            fps,
            sources: Arc::new(RwLock::new(sources)),
            built_in: Arc::new(RwLock::new(None)),
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
        if request.kind != "built-in-camera" {
            return Err(SourceManagerError::Unsupported(request.kind));
        }
        let mut sources = self.sources.write().await;
        if sources.contains_key(&request.id) {
            return Err(SourceManagerError::AlreadyExists);
        }
        let info = SourceInfo {
            id: request.id.clone(),
            name: request.id.clone(),
            kind: request.kind,
            status: SourceState::Stopped,
            resolution: None,
            fps: 0.0,
            uptime_seconds: 0,
            last_frame: None,
            reconnect_count: 0,
            downtime_seconds: 0,
            frames_received: 0,
        };
        sources.insert(
            request.id,
            SourceEntry {
                info: info.clone(),
                desired_running: false,
                started_at: None,
                disconnected_at: None,
            },
        );
        Ok(info)
    }
    pub async fn start(&self, id: &str) -> Result<SourceInfo, SourceManagerError> {
        if id != "builtin" {
            return Err(SourceManagerError::Unsupported(id.into()));
        }
        if self.built_in.read().await.is_some() {
            return self.get(id).await;
        }
        {
            let mut sources = self.sources.write().await;
            let entry = sources.get_mut(id).ok_or(SourceManagerError::NotFound)?;
            entry.desired_running = true;
            entry.info.status = SourceState::Starting;
        }
        match CameraWorker::start(self.fps) {
            Ok(worker) => {
                *self.built_in.write().await = Some(worker);
                let mut sources = self.sources.write().await;
                let entry = sources.get_mut(id).ok_or(SourceManagerError::NotFound)?;
                entry.info.status = SourceState::Running;
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
                        "camera running",
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
        if id == "builtin" {
            if let Some(worker) = self.built_in.write().await.take() {
                worker.stop();
            }
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
        if id == "builtin" {
            self.stop(id).await?;
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
                "sentinel_source_frames_received{{source=\"{}\"}} {}\nsentinel_source_reconnect_count{{source=\"{}\"}} {}\nsentinel_source_downtime_seconds{{source=\"{}\"}} {}\n",
                source.id, source.frames_received, source.id, source.reconnect_count, source.id, source.downtime_seconds
            ));
        }
        output
    }
}
impl VideoSourceManager {
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
            let running = self
                .get("builtin")
                .await
                .map(|info| matches!(info.status, SourceState::Running))
                .unwrap_or(false);
            if !running {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                continue;
            }
            let receiver = self
                .built_in
                .read()
                .await
                .as_ref()
                .map(|worker| worker.frames.clone());
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
                    if let Some(entry) = sources.get_mut("builtin") {
                        entry.info.last_frame = Some(now_ms());
                        entry.info.frames_received += 1;
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
                        .get_mut("builtin")
                        .map(|entry| {
                            entry.info.status = SourceState::Disconnected;
                            entry.info.reconnect_count += 1;
                            entry
                                .disconnected_at
                                .get_or_insert_with(std::time::Instant::now);
                            entry.desired_running
                        })
                        .unwrap_or(false);
                    drop(sources);
                    if let Some(worker) = self.built_in.write().await.take() {
                        worker.stop();
                    }
                    self.events.publish(Event {
                        kind: "source_disconnected".into(),
                        source_id: Some("builtin".into()),
                        message: error.to_string(),
                    });
                    if reconnect {
                        self.reconnect_until_running().await;
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
