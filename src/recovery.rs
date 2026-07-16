use crate::events::{EventBus, EventRecord};
use serde::Serialize;
use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Instant,
};
use tokio::sync::RwLock;

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ComponentState {
    Healthy,
    Degraded,
    Recovering,
    Failed,
}

#[derive(Clone, Debug, Serialize)]
pub struct ComponentHealth {
    pub component: String,
    pub state: ComponentState,
    pub message: String,
    pub consecutive_failures: u64,
    pub updated_at_ms: u128,
}

#[derive(Clone, Default)]
pub struct HealthMonitor {
    components: Arc<RwLock<BTreeMap<String, ComponentHealth>>>,
}
impl HealthMonitor {
    pub fn new() -> Self {
        Self::default()
    }
    pub async fn set(
        &self,
        component: &str,
        state: ComponentState,
        message: impl Into<String>,
        failures: u64,
    ) {
        self.components.write().await.insert(
            component.into(),
            ComponentHealth {
                component: component.into(),
                state,
                message: message.into(),
                consecutive_failures: failures,
                updated_at_ms: now_ms(),
            },
        );
    }
    pub async fn snapshot(&self) -> Vec<ComponentHealth> {
        self.components.read().await.values().cloned().collect()
    }
    pub async fn degraded_count(&self) -> usize {
        self.components
            .read()
            .await
            .values()
            .filter(|health| health.state != ComponentState::Healthy)
            .count()
    }
    pub async fn state(&self, component: &str) -> Option<ComponentState> {
        self.components
            .read()
            .await
            .get(component)
            .map(|health| health.state.clone())
    }
}

#[derive(Clone, Default)]
pub struct RecoveryMetrics {
    attempts: Arc<AtomicU64>,
    successes: Arc<AtomicU64>,
    failures: Arc<AtomicU64>,
    latency_ms: Arc<AtomicU64>,
}
impl RecoveryMetrics {
    pub fn prometheus(&self, degraded: usize) -> String {
        let successes = self.successes.load(Ordering::Relaxed);
        let average = if successes == 0 {
            0
        } else {
            self.latency_ms.load(Ordering::Relaxed) / successes
        };
        format!("sentinel_recovery_attempts {}\nsentinel_recovery_successes {}\nsentinel_recovery_failures {}\nsentinel_recovery_average_latency_ms {}\nsentinel_recovery_degraded_components {}\n", self.attempts.load(Ordering::Relaxed), successes, self.failures.load(Ordering::Relaxed), average, degraded)
    }
}

#[derive(Clone)]
pub struct RecoveryEngine {
    pub monitor: HealthMonitor,
    pub metrics: RecoveryMetrics,
    events: EventBus,
}
impl RecoveryEngine {
    pub fn new(events: EventBus) -> Self {
        Self {
            monitor: HealthMonitor::new(),
            metrics: RecoveryMetrics::default(),
            events,
        }
    }
    pub async fn begin(&self, component: &str, message: impl Into<String>) -> Instant {
        self.metrics.attempts.fetch_add(1, Ordering::Relaxed);
        let message = message.into();
        self.monitor
            .set(component, ComponentState::Recovering, message.clone(), 0)
            .await;
        self.events.publish_record(EventRecord::simple(
            event_type(component, "recovering"),
            None,
            message.clone(),
        ));
        tracing::warn!(component, message=%message, "recovery started");
        Instant::now()
    }
    pub async fn recovered(&self, component: &str, started: Instant, message: impl Into<String>) {
        let message = message.into();
        self.metrics.successes.fetch_add(1, Ordering::Relaxed);
        self.metrics
            .latency_ms
            .fetch_add(started.elapsed().as_millis() as u64, Ordering::Relaxed);
        self.monitor
            .set(component, ComponentState::Healthy, message.clone(), 0)
            .await;
        self.events.publish_record(EventRecord::simple(
            event_type(component, "recovered"),
            None,
            message.clone(),
        ));
        tracing::info!(component, recovery_ms=started.elapsed().as_millis() as u64, message=%message, "recovery completed");
    }
    pub async fn failed(
        &self,
        component: &str,
        message: impl Into<String>,
        consecutive_failures: u64,
    ) {
        let message = message.into();
        self.metrics.failures.fetch_add(1, Ordering::Relaxed);
        self.monitor
            .set(
                component,
                ComponentState::Failed,
                message.clone(),
                consecutive_failures,
            )
            .await;
        self.events.publish_record(EventRecord::simple(
            event_type(component, "reconnect_failed"),
            None,
            message.clone(),
        ));
        tracing::error!(component, consecutive_failures, message=%message, "recovery failed");
    }
    pub async fn attempt_failed(
        &self,
        component: &str,
        message: impl Into<String>,
        consecutive_failures: u64,
    ) {
        let message = message.into();
        self.metrics.failures.fetch_add(1, Ordering::Relaxed);
        self.monitor
            .set(
                component,
                ComponentState::Recovering,
                message.clone(),
                consecutive_failures,
            )
            .await;
        self.events.publish_record(EventRecord::simple(
            event_type(component, "reconnect_failed"),
            None,
            message.clone(),
        ));
        tracing::warn!(component, consecutive_failures, message=%message, "recovery attempt failed");
    }
}
fn event_type(component: &str, action: &str) -> String {
    if component == "camera" {
        format!("source.{action}")
    } else {
        format!("{component}.{action}")
    }
}
fn now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}
