use crate::{errors::SourceError, frame::Frame};
use async_trait::async_trait;
use nokhwa::{
    pixel_format::RgbFormat,
    utils::{CameraIndex, RequestedFormat, RequestedFormatType},
    Camera,
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::Arc, time::Duration};
use tokio::sync::RwLock;

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

// Extension points for future milestones. They intentionally do not implement capture.
pub struct UsbCamera;
pub struct RtspCamera;
pub struct OnvifCamera;
pub struct VideoFile;

#[derive(Clone, Debug, Serialize)]
pub struct SourceInfo {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub running: bool,
}
#[derive(Clone, Debug, Deserialize)]
pub struct AddSource {
    pub id: String,
    pub kind: String,
}

#[derive(Clone)]
pub struct SourceRegistry {
    sources: Arc<RwLock<BTreeMap<String, SourceInfo>>>,
}
impl SourceRegistry {
    pub fn new() -> Self {
        Self {
            sources: Arc::new(RwLock::new(BTreeMap::from([(
                "built-in".into(),
                SourceInfo {
                    id: "built-in".into(),
                    name: "Built-in camera".into(),
                    kind: "built-in-camera".into(),
                    running: true,
                },
            )]))),
        }
    }
    pub async fn list(&self) -> Vec<SourceInfo> {
        self.sources.read().await.values().cloned().collect()
    }
    pub async fn add(&self, request: AddSource) -> Result<SourceInfo, String> {
        let mut sources = self.sources.write().await;
        if sources.contains_key(&request.id) {
            return Err("source already exists".into());
        }
        let info = SourceInfo {
            id: request.id.clone(),
            name: request.id,
            kind: request.kind,
            running: false,
        };
        sources.insert(info.id.clone(), info.clone());
        Ok(info)
    }
    pub async fn remove(&self, id: &str) -> bool {
        self.sources.write().await.remove(id).is_some()
    }
    pub async fn set_running(&self, id: &str, running: bool) -> Option<SourceInfo> {
        let mut sources = self.sources.write().await;
        let source = sources.get_mut(id)?;
        source.running = running;
        Some(source.clone())
    }
}
