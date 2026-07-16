use serde::Serialize;
#[derive(Clone, Default)]
pub struct Health {
    pub ready: std::sync::Arc<std::sync::atomic::AtomicBool>,
}
#[derive(Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
}
