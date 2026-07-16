#[derive(Clone, Default)]
pub struct Health {
    pub ready: std::sync::Arc<std::sync::atomic::AtomicBool>,
}
