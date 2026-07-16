use crate::frame::Frame;
use serde::Serialize;
use tokio::sync::broadcast;

#[derive(Clone, Debug, Serialize)]
pub struct Event {
    pub kind: String,
    pub source_id: Option<String>,
    pub message: String,
}

#[derive(Clone)]
pub struct EventBus {
    sender: broadcast::Sender<Event>,
}
impl EventBus {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(128);
        Self { sender }
    }
    pub fn publish(&self, event: Event) {
        let _ = self.sender.send(event);
    }
    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.sender.subscribe()
    }
}
pub trait VisionEngine: Send + Sync {
    fn process(&self, _frame: &Frame) {}
}
pub trait SceneUnderstanding: Send + Sync {}
pub trait EventPublisher: Send + Sync {
    fn publish(&self, _event: &str) {}
}
