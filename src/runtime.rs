use serde::Serialize;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone, Debug, Default, Serialize)]
pub struct RuntimeSnapshot {
    pub camera_opened: bool,
    pub pipeline_initialized: bool,
    pub http_started: bool,
    pub shutting_down: bool,
}

#[derive(Clone, Default)]
pub struct RuntimeStatus(Arc<RwLock<RuntimeSnapshot>>);
impl RuntimeStatus {
    pub async fn mark_camera_opened(&self) {
        self.0.write().await.camera_opened = true;
    }
    pub async fn mark_pipeline_initialized(&self) {
        self.0.write().await.pipeline_initialized = true;
    }
    pub async fn mark_http_started(&self) {
        self.0.write().await.http_started = true;
    }
    pub async fn mark_shutting_down(&self) {
        self.0.write().await.shutting_down = true;
    }
    pub async fn snapshot(&self) -> RuntimeSnapshot {
        self.0.read().await.clone()
    }
}
