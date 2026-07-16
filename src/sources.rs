use crate::{
    errors::SourceError,
    events::{Event, EventBus},
    frame::Frame,
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
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::sync::{Mutex, RwLock};

#[async_trait]
pub trait VideoSource: Send {
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
        let mut camera = Camera::new(CameraIndex::Index(0), requested)
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
#[async_trait]
impl VideoSource for BuiltInCamera {
    fn name(&self) -> &'static str {
        "built-in-camera"
    }
    async fn next_frame(&mut self) -> Result<Frame, SourceError> {
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

pub struct UsbCamera;
pub struct RtspCamera;
pub struct OnvifCamera;
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
    built_in: Arc<Mutex<Option<BuiltInCamera>>>,
    events: EventBus,
}
impl VideoSourceManager {
    pub fn new(fps: u32, events: EventBus) -> Self {
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
            frames_received: 0,
        };
        let mut sources = BTreeMap::new();
        sources.insert(
            "builtin".into(),
            SourceEntry {
                info,
                desired_running: false,
                started_at: None,
            },
        );
        Self {
            fps,
            sources: Arc::new(RwLock::new(sources)),
            built_in: Arc::new(Mutex::new(None)),
            events,
        }
    }
    pub async fn list(&self) -> Vec<SourceInfo> {
        self.sources
            .read()
            .await
            .values()
            .map(|entry| self.snapshot(&entry.info, entry.started_at))
            .collect()
    }
    pub async fn get(&self, id: &str) -> Result<SourceInfo, SourceManagerError> {
        let sources = self.sources.read().await;
        let entry = sources.get(id).ok_or(SourceManagerError::NotFound)?;
        Ok(self.snapshot(&entry.info, entry.started_at))
    }
    fn snapshot(&self, info: &SourceInfo, started_at: Option<std::time::Instant>) -> SourceInfo {
        let mut info = info.clone();
        info.uptime_seconds = started_at
            .map(|started| started.elapsed().as_secs())
            .unwrap_or(0);
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
            frames_received: 0,
        };
        sources.insert(
            request.id,
            SourceEntry {
                info: info.clone(),
                desired_running: false,
                started_at: None,
            },
        );
        Ok(info)
    }
    pub async fn start(&self, id: &str) -> Result<SourceInfo, SourceManagerError> {
        if id != "builtin" {
            return Err(SourceManagerError::Unsupported(id.into()));
        }
        {
            let mut sources = self.sources.write().await;
            let entry = sources.get_mut(id).ok_or(SourceManagerError::NotFound)?;
            entry.desired_running = true;
            entry.info.status = SourceState::Starting;
        }
        match BuiltInCamera::new(self.fps) {
            Ok(camera) => {
                *self.built_in.lock().await = Some(camera);
                let mut sources = self.sources.write().await;
                let entry = sources.get_mut(id).ok_or(SourceManagerError::NotFound)?;
                entry.info.status = SourceState::Running;
                entry.started_at = Some(std::time::Instant::now());
                self.events.publish(Event {
                    kind: "source_started".into(),
                    source_id: Some(id.into()),
                    message: "source started".into(),
                });
                Ok(self.snapshot(&entry.info, entry.started_at))
            }
            Err(error) => {
                let mut sources = self.sources.write().await;
                if let Some(entry) = sources.get_mut(id) {
                    entry.info.status = SourceState::Failed;
                }
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
            self.built_in.lock().await.take();
        }
        let mut sources = self.sources.write().await;
        let entry = sources.get_mut(id).ok_or(SourceManagerError::NotFound)?;
        entry.info.status = SourceState::Stopped;
        entry.started_at = None;
        self.events.publish(Event {
            kind: "source_stopped".into(),
            source_id: Some(id.into()),
            message: "source stopped".into(),
        });
        Ok(self.snapshot(&entry.info, entry.started_at))
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
        let mut output = format!("sentinel_registered_sources {}\nsentinel_active_sources {}\nsentinel_failed_sources {}\n", list.len(), active, failed);
        for source in list {
            output.push_str(&format!(
                "sentinel_source_frames_received{{source=\"{}\"}} {}\n",
                source.id, source.frames_received
            ));
        }
        output
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
            let result = {
                let mut camera = self.built_in.lock().await;
                match camera.as_mut() {
                    Some(camera) => camera.next_frame().await,
                    None => Err(SourceError("camera is not open".into())),
                }
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
                            entry.info.status = SourceState::Failed;
                            entry.info.reconnect_count += 1;
                            entry.desired_running
                        })
                        .unwrap_or(false);
                    drop(sources);
                    self.built_in.lock().await.take();
                    self.events.publish(Event {
                        kind: "source_failed".into(),
                        source_id: Some("builtin".into()),
                        message: error.to_string(),
                    });
                    if reconnect {
                        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                        if let Ok(camera) = BuiltInCamera::new(self.fps) {
                            *self.built_in.lock().await = Some(camera);
                            let mut sources = self.sources.write().await;
                            if let Some(entry) = sources.get_mut("builtin") {
                                entry.info.status = SourceState::Running;
                                entry.started_at = Some(std::time::Instant::now());
                            }
                            self.events.publish(Event {
                                kind: "source_restarted".into(),
                                source_id: Some("builtin".into()),
                                message: "source reconnected".into(),
                            });
                        }
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
