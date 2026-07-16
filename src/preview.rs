use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct Preview(Arc<RwLock<Option<Vec<u8>>>>);
impl Preview {
    pub fn new() -> Self {
        Self(Arc::new(RwLock::new(None)))
    }
    pub fn set(&self, bytes: Vec<u8>) {
        let slot = self.0.clone();
        tokio::spawn(async move {
            *slot.write().await = Some(bytes);
        });
    }
    pub async fn get(&self) -> Option<Vec<u8>> {
        self.0.read().await.clone()
    }
}
impl Default for Preview {
    fn default() -> Self {
        Self::new()
    }
}
