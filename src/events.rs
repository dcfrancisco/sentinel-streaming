use crate::frame::Frame;
use serde::Serialize;
use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::sync::{broadcast, RwLock};

#[derive(Clone, Debug, Serialize)]
pub struct Event {
    pub kind: String,
    pub source_id: Option<String>,
    pub message: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct EventRecord {
    pub id: String,
    pub timestamp: u128,
    pub source_id: Option<String>,
    pub event_type: String,
    pub provider: Option<String>,
    pub summary: String,
    pub objects: Vec<String>,
    pub confidence: Option<f64>,
    pub latency_ms: Option<u64>,
    pub metadata: serde_json::Value,
}
impl EventRecord {
    pub fn simple(
        event_type: impl Into<String>,
        source_id: Option<String>,
        summary: impl Into<String>,
    ) -> Self {
        Self {
            id: String::new(),
            timestamp: now_ms(),
            source_id,
            event_type: event_type.into(),
            provider: None,
            summary: summary.into(),
            objects: Vec::new(),
            confidence: None,
            latency_ms: None,
            metadata: serde_json::json!({}),
        }
    }
    fn to_event(&self) -> Event {
        Event {
            kind: self.event_type.clone(),
            source_id: self.source_id.clone(),
            message: self.summary.clone(),
        }
    }
}

#[derive(Clone)]
pub struct EventStore {
    capacity: usize,
    events: Arc<RwLock<VecDeque<EventRecord>>>,
    next_id: Arc<AtomicU64>,
}
impl EventStore {
    pub fn new(capacity: usize) -> Self {
        assert!(
            capacity > 0,
            "event store capacity must be greater than zero"
        );
        Self {
            capacity,
            events: Arc::new(RwLock::new(VecDeque::with_capacity(capacity))),
            next_id: Arc::new(AtomicU64::new(1)),
        }
    }
    pub async fn push(&self, mut event: EventRecord) -> EventRecord {
        event.id = format!(
            "evt-{}-{}",
            event.timestamp,
            self.next_id.fetch_add(1, Ordering::Relaxed)
        );
        let mut events = self.events.write().await;
        if events.len() == self.capacity {
            events.pop_front();
        }
        events.push_back(event.clone());
        event
    }
    pub async fn latest(&self) -> Option<EventRecord> {
        self.events.read().await.back().cloned()
    }
    pub async fn recent(&self, count: usize) -> Vec<EventRecord> {
        self.events
            .read()
            .await
            .iter()
            .rev()
            .take(count)
            .cloned()
            .collect()
    }
    pub async fn get(&self, id: &str) -> Option<EventRecord> {
        self.events
            .read()
            .await
            .iter()
            .find(|event| event.id == id)
            .cloned()
    }
    pub async fn len(&self) -> usize {
        self.events.read().await.len()
    }
    pub async fn is_empty(&self) -> bool {
        self.len().await == 0
    }
    pub fn capacity(&self) -> usize {
        self.capacity
    }
    pub async fn prometheus(&self) -> String {
        let events = self.events.read().await;
        let total = events.len();
        let vision = events
            .iter()
            .filter(|event| {
                event.event_type.starts_with("vision.") || event.event_type == "scene.observed"
            })
            .count();
        let source = events
            .iter()
            .filter(|event| event.event_type.starts_with("source."))
            .count();
        format!("sentinel_events_total {}\nsentinel_events_store_size {}\nsentinel_events_store_capacity {}\nsentinel_vision_events_total {}\nsentinel_source_events_total {}\n", total, total, self.capacity, vision, source)
    }
}

#[derive(Clone)]
pub struct EventBus {
    sender: broadcast::Sender<Event>,
    store: EventStore,
}
impl EventBus {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(128);
        Self {
            sender,
            store: EventStore::new(capacity),
        }
    }
    pub fn publish(&self, event: Event) {
        let store = self.store.clone();
        let sender = self.sender.clone();
        tokio::spawn(async move {
            let record = store
                .push(EventRecord::simple(
                    event.kind,
                    event.source_id,
                    event.message,
                ))
                .await;
            let _ = sender.send(record.to_event());
        });
    }
    pub fn publish_record(&self, record: EventRecord) {
        let store = self.store.clone();
        let sender = self.sender.clone();
        tokio::spawn(async move {
            let record = store.push(record).await;
            let _ = sender.send(record.to_event());
        });
    }
    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.sender.subscribe()
    }
    pub fn store(&self) -> EventStore {
        self.store.clone()
    }
}
fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

#[allow(dead_code)]
pub trait VisionEngine: Send + Sync {
    fn process(&self, _frame: &Frame) {}
}
#[allow(dead_code)]
pub trait SceneUnderstanding: Send + Sync {}
#[allow(dead_code)]
pub trait EventPublisher: Send + Sync {
    fn publish(&self, _event: &str) {}
}
