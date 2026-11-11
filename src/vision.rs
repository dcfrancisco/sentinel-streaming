use crate::{
    events::{EventBus, EventRecord},
    frame::Frame,
    frame_buffer::FrameBuffer,
    recovery::RecoveryEngine,
};
use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, Instant, SystemTime},
};
use tokio::sync::{watch, Mutex, RwLock};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SceneAnalysis {
    pub summary: String,
    pub changes: Vec<String>,
    pub activities: Vec<String>,
    pub objects: Vec<String>,
}
#[derive(Clone, Debug, Serialize)]
pub struct LatestAnalysis {
    pub timestamp: u128,
    pub analysis: SceneAnalysis,
    pub provider: String,
    pub latency_ms: u64,
    pub frames_analyzed: usize,
}
#[derive(Clone, Default)]
pub struct VisionState(Arc<RwLock<Option<LatestAnalysis>>>);
impl VisionState {
    pub async fn latest(&self) -> Option<LatestAnalysis> {
        self.0.read().await.clone()
    }
    async fn set(&self, analysis: LatestAnalysis) {
        *self.0.write().await = Some(analysis);
    }
}

#[derive(Clone, Copy)]
pub struct FrameSelector {
    pub count: usize,
    pub spacing_seconds: u64,
}
impl FrameSelector {
    pub fn new(count: usize, spacing_seconds: u64) -> Self {
        Self {
            count: count.max(1),
            spacing_seconds: spacing_seconds.max(1),
        }
    }
    pub fn select(&self, buffer: &FrameBuffer) -> Vec<Arc<Frame>> {
        let Some(latest) = buffer.latest() else {
            return Vec::new();
        };
        let candidates = buffer.recent(buffer.len());
        let mut selected = vec![latest.clone()];
        for index in 1..self.count {
            let target = latest
                .captured_at_ms
                .saturating_sub(index as u128 * self.spacing_seconds as u128 * 1000);
            if let Some(frame) = candidates
                .iter()
                .find(|frame| frame.captured_at_ms <= target)
            {
                selected.push(frame.clone());
            }
        }
        selected.sort_by_key(|frame| frame.captured_at_ms);
        selected.dedup_by_key(|frame| frame.sequence);
        selected
    }
}

#[derive(Clone, Default)]
pub struct VisionMetrics {
    requests: Arc<AtomicU64>,
    successes: Arc<AtomicU64>,
    failures: Arc<AtomicU64>,
    latency_total_ms: Arc<AtomicU64>,
    frames_total: Arc<AtomicU64>,
    last_timestamp: Arc<AtomicU64>,
    http_attempts: Arc<AtomicU64>,
    rate_limits: Arc<AtomicU64>,
    http_errors: Arc<AtomicU64>,
    input_tokens: Arc<AtomicU64>,
    output_tokens: Arc<AtomicU64>,
    usage_responses: Arc<AtomicU64>,
}
#[derive(Serialize)]
pub struct VisionMetricsSnapshot {
    pub requests: u64,
    pub successes: u64,
    pub failures: u64,
    pub average_latency_ms: Option<u64>,
    pub http_attempts: u64,
    pub http_errors: u64,
    pub rate_limits: u64,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
}
impl VisionMetrics {
    fn request(&self) {
        self.requests.fetch_add(1, Ordering::Relaxed);
    }
    fn success(&self, latency: u64, timestamp: u128, frames: usize) {
        self.successes.fetch_add(1, Ordering::Relaxed);
        self.latency_total_ms.fetch_add(latency, Ordering::Relaxed);
        self.frames_total
            .fetch_add(frames as u64, Ordering::Relaxed);
        self.last_timestamp
            .store(timestamp.min(u64::MAX as u128) as u64, Ordering::Relaxed);
    }
    fn failure(&self) {
        self.failures.fetch_add(1, Ordering::Relaxed);
    }
    pub fn configuration_failure(&self) {
        self.failure();
    }
    pub fn snapshot(&self) -> VisionMetricsSnapshot {
        let successes = self.successes.load(Ordering::Relaxed);
        VisionMetricsSnapshot {
            requests: self.requests.load(Ordering::Relaxed),
            successes,
            failures: self.failures.load(Ordering::Relaxed),
            average_latency_ms: if successes == 0 {
                None
            } else {
                Some(self.latency_total_ms.load(Ordering::Relaxed) / successes)
            },
            http_attempts: self.http_attempts.load(Ordering::Relaxed),
            http_errors: self.http_errors.load(Ordering::Relaxed),
            rate_limits: self.rate_limits.load(Ordering::Relaxed),
            input_tokens: (self.usage_responses.load(Ordering::Relaxed) > 0)
                .then(|| self.input_tokens.load(Ordering::Relaxed)),
            output_tokens: (self.usage_responses.load(Ordering::Relaxed) > 0)
                .then(|| self.output_tokens.load(Ordering::Relaxed)),
        }
    }
    pub fn prometheus(&self) -> String {
        let requests = self.requests.load(Ordering::Relaxed);
        let successes = self.successes.load(Ordering::Relaxed);
        let average_latency = if successes == 0 {
            0
        } else {
            self.latency_total_ms.load(Ordering::Relaxed) / successes
        };
        let average_frames = if successes == 0 {
            0
        } else {
            self.frames_total.load(Ordering::Relaxed) / successes
        };
        format!("sentinel_vision_requests {}\nsentinel_vision_successful_analyses {}\nsentinel_vision_failed_analyses {}\nsentinel_vision_average_latency_ms {}\nsentinel_vision_average_frames_analyzed {}\nsentinel_vision_observation_count {}\nsentinel_vision_last_analysis_timestamp {}\nsentinel_vision_http_attempts {}\nsentinel_vision_http_errors {}\nsentinel_vision_rate_limits {}\nsentinel_vision_input_tokens {}\nsentinel_vision_output_tokens {}\nsentinel_vision_usage_responses {}\n", requests, successes, self.failures.load(Ordering::Relaxed), average_latency, average_frames, successes, self.last_timestamp.load(Ordering::Relaxed), self.http_attempts.load(Ordering::Relaxed), self.http_errors.load(Ordering::Relaxed), self.rate_limits.load(Ordering::Relaxed), self.input_tokens.load(Ordering::Relaxed), self.output_tokens.load(Ordering::Relaxed), self.usage_responses.load(Ordering::Relaxed))
    }
}

#[async_trait]
pub trait VisionProvider: Send + Sync {
    fn name(&self) -> &'static str;
    async fn analyze(&self, frames: Vec<Arc<Frame>>) -> Result<SceneAnalysis, String>;
    fn is_permanent_failure(&self, _error: &str) -> bool {
        false
    }
}
pub struct OpenAiVisionProvider {
    client: reqwest::Client,
    api_key: String,
    model: String,
    endpoint: String,
    next_allowed: Arc<Mutex<Option<tokio::time::Instant>>>,
    metrics: VisionMetrics,
}
impl OpenAiVisionProvider {
    pub fn from_env() -> Result<Self, &'static str> {
        Self::from_values(
            std::env::var("OPENAI_API_KEY").ok(),
            std::env::var("OPENAI_VISION_MODEL").ok(),
            "https://api.openai.com/v1/responses".into(),
        )
    }

    fn from_env_with_model(model: String) -> Result<Self, &'static str> {
        Self::from_values(
            std::env::var("OPENAI_API_KEY").ok(),
            Some(model),
            "https://api.openai.com/v1/responses".into(),
        )
    }

    fn from_values(
        api_key: Option<String>,
        model: Option<String>,
        endpoint: String,
    ) -> Result<Self, &'static str> {
        let api_key = api_key
            .filter(|key| !key.trim().is_empty())
            .ok_or("OPENAI_API_KEY is missing; vision unavailable")?;
        let model = model.unwrap_or_else(|| "gpt-5.4-mini".into());
        if model.trim().is_empty() {
            return Err("OPENAI_VISION_MODEL is empty; vision unavailable");
        }
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| "could not initialize OpenAI HTTP client")?;
        Ok(Self {
            client,
            api_key,
            model,
            endpoint,
            next_allowed: Arc::new(Mutex::new(Some(tokio::time::Instant::now()))),
            metrics: VisionMetrics::default(),
        })
    }
}

fn retry_after(headers: &reqwest::header::HeaderMap) -> Option<Duration> {
    let value = headers.get(reqwest::header::RETRY_AFTER)?.to_str().ok()?;
    if let Ok(seconds) = value.trim().parse::<u64>() {
        return Some(Duration::from_secs(seconds));
    }
    let date = httpdate::parse_http_date(value).ok()?;
    Some(date.duration_since(SystemTime::now()).unwrap_or_default())
}

fn backoff(attempt: u32) -> Duration {
    let base_ms = 500_u64 * (1 << attempt);
    let jitter = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos() as u64
        % (base_ms / 4 + 1);
    Duration::from_millis(base_ms + jitter)
}

fn parse_response(value: &serde_json::Value) -> Result<SceneAnalysis, String> {
    match value.get("status").and_then(|status| status.as_str()) {
        Some("completed") => {}
        Some("incomplete") => return Err("OpenAI response was incomplete".into()),
        Some("failed") => return Err("OpenAI response failed".into()),
        _ => return Err("OpenAI response has an invalid or unfinished status".into()),
    }
    let output = value
        .get("output")
        .and_then(|output| output.as_array())
        .ok_or("OpenAI response has no output")?;
    let mut text = String::new();
    for item in output {
        if let Some(content) = item.get("content").and_then(|content| content.as_array()) {
            for part in content {
                match part.get("type").and_then(|kind| kind.as_str()) {
                    Some("refusal") => return Err("OpenAI refused the vision request".into()),
                    Some("output_text") => text.push_str(
                        part.get("text")
                            .and_then(|text| text.as_str())
                            .ok_or("OpenAI output_text is invalid")?,
                    ),
                    _ => {}
                }
            }
        }
    }
    if text.is_empty() {
        return Err("OpenAI response has no output_text".into());
    }
    serde_json::from_str(&text).map_err(|_| "invalid scene observation JSON".into())
}
#[async_trait]
impl VisionProvider for OpenAiVisionProvider {
    fn name(&self) -> &'static str {
        "OpenAI"
    }
    fn is_permanent_failure(&self, error: &str) -> bool {
        matches!(
            error,
            "OpenAI request failed (HTTP 400)"
                | "OpenAI request failed (HTTP 401)"
                | "OpenAI request failed (HTTP 403)"
                | "OpenAI request failed (HTTP 404)"
                | "OpenAI request failed (HTTP 422)"
        )
    }
    async fn analyze(&self, frames: Vec<Arc<Frame>>) -> Result<SceneAnalysis, String> {
        let mut content = vec![
            serde_json::json!({"type":"input_text","text":"You are observing a sequence of images captured over time. Describe what changed, what people are doing, movement, objects entering or leaving, and interactions. Be factual. Do not speculate. Do not generate security alerts. Return concise JSON."}),
        ];
        for frame in frames {
            let mut jpeg = Vec::new();
            let mut encoder = image::codecs::jpeg::JpegEncoder::new(&mut jpeg);
            encoder
                .encode(
                    frame.data.as_ref(),
                    frame.width,
                    frame.height,
                    image::ExtendedColorType::Rgb8,
                )
                .map_err(|_| "could not encode vision frame as JPEG".to_string())?;
            content.push(serde_json::json!({"type":"input_image","image_url":format!("data:image/jpeg;base64,{}", STANDARD.encode(jpeg)),"detail":"low"}));
        }
        let body = serde_json::json!({"model":self.model,"instructions":"Analyze the sequence as a temporal scene observation.","input":[{"role":"user","content":content}],"text":{"format":{"type":"json_schema","name":"scene_observation","strict":true,"schema":{"type":"object","properties":{"summary":{"type":"string"},"changes":{"type":"array","items":{"type":"string"}},"activities":{"type":"array","items":{"type":"string"}},"objects":{"type":"array","items":{"type":"string"}}},"required":["summary","changes","activities","objects"],"additionalProperties":false}}}});
        let mut next_allowed = self.next_allowed.lock().await;
        const MAX_ATTEMPTS: u32 = 3;
        const MAX_INLINE_WAIT: Duration = Duration::from_secs(60);
        for attempt in 0..MAX_ATTEMPTS {
            let deadline = next_allowed.ok_or("OpenAI retry deadline is too far in the future")?;
            tokio::time::sleep_until(deadline).await;
            self.metrics.http_attempts.fetch_add(1, Ordering::Relaxed);
            let response = self
                .client
                .post(&self.endpoint)
                .bearer_auth(&self.api_key)
                .json(&body)
                .send()
                .await;
            let response = match response {
                Ok(response) => response,
                Err(_) if attempt + 1 < MAX_ATTEMPTS => {
                    self.metrics.http_errors.fetch_add(1, Ordering::Relaxed);
                    *next_allowed = Some(tokio::time::Instant::now() + backoff(attempt));
                    continue;
                }
                Err(_) => {
                    self.metrics.http_errors.fetch_add(1, Ordering::Relaxed);
                    return Err("OpenAI request failed or timed out".into());
                }
            };
            let status = response.status();
            if status.is_success() {
                let value: serde_json::Value = response
                    .json()
                    .await
                    .map_err(|_| "OpenAI response was not valid JSON".to_string())?;
                if let (Some(input), Some(output)) = (
                    value
                        .pointer("/usage/input_tokens")
                        .and_then(|value| value.as_u64()),
                    value
                        .pointer("/usage/output_tokens")
                        .and_then(|value| value.as_u64()),
                ) {
                    self.metrics
                        .input_tokens
                        .fetch_add(input, Ordering::Relaxed);
                    self.metrics
                        .output_tokens
                        .fetch_add(output, Ordering::Relaxed);
                    self.metrics.usage_responses.fetch_add(1, Ordering::Relaxed);
                }
                return parse_response(&value);
            }
            self.metrics.http_errors.fetch_add(1, Ordering::Relaxed);
            if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                self.metrics.rate_limits.fetch_add(1, Ordering::Relaxed);
            }
            if status == reqwest::StatusCode::TOO_MANY_REQUESTS || status.is_server_error() {
                let backoff = backoff(attempt);
                let delay = retry_after(response.headers())
                    .unwrap_or(backoff)
                    .max(backoff);
                *next_allowed = tokio::time::Instant::now().checked_add(delay);
                if attempt + 1 < MAX_ATTEMPTS && delay <= MAX_INLINE_WAIT {
                    continue;
                }
            }
            return Err(format!("OpenAI request failed (HTTP {})", status.as_u16()));
        }
        Err("OpenAI request retry limit reached".into())
    }
}

pub struct VisionScheduler;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VisionSettings {
    pub enabled: bool,
    pub model: String,
    pub interval_seconds: u64,
    pub frames: usize,
    pub spacing_seconds: u64,
}

impl VisionSettings {
    fn validate(&self) -> Result<(), &'static str> {
        if self.model.is_empty()
            || self.model.len() > 128
            || !self.model.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':')
            })
        {
            return Err("invalid vision model name");
        }
        if !(1..=3600).contains(&self.interval_seconds)
            || !(1..=10).contains(&self.frames)
            || !(1..=3600).contains(&self.spacing_seconds)
        {
            return Err("vision interval, frames or spacing is outside allowed limits");
        }
        Ok(())
    }
}

struct VisionWorker {
    shutdown: watch::Sender<bool>,
    task: tokio::task::JoinHandle<()>,
}

struct VisionControlState {
    settings: VisionSettings,
    worker: Option<VisionWorker>,
}

pub struct VisionControl {
    inner: Mutex<VisionControlState>,
    next_allowed: Arc<Mutex<Option<tokio::time::Instant>>>,
}

impl VisionControl {
    pub fn new(config: &crate::config::Config) -> Self {
        Self {
            inner: Mutex::new(VisionControlState {
                settings: VisionSettings {
                    enabled: config.vision_enabled(),
                    model: std::env::var("OPENAI_VISION_MODEL")
                        .unwrap_or_else(|_| "gpt-5.4-mini".into()),
                    interval_seconds: config.vision.interval_seconds,
                    frames: config.vision.frames,
                    spacing_seconds: config.vision.spacing_seconds,
                },
                worker: None,
            }),
            next_allowed: Arc::new(Mutex::new(Some(tokio::time::Instant::now()))),
        }
    }

    pub async fn snapshot(&self) -> (VisionSettings, bool) {
        let inner = self.inner.lock().await;
        (
            inner.settings.clone(),
            inner
                .worker
                .as_ref()
                .is_some_and(|worker| !worker.task.is_finished()),
        )
    }

    pub async fn apply(
        &self,
        settings: VisionSettings,
        state: &crate::api::AppState,
    ) -> Result<(), &'static str> {
        settings.validate()?;
        if settings.enabled && !state.config.pipeline.vision {
            return Err("vision is disabled by pipeline.vision");
        }
        if settings.enabled && !state.config.buffer.enabled {
            return Err("vision requires the frame buffer to be enabled");
        }
        let provider = if settings.enabled {
            let mut provider = OpenAiVisionProvider::from_env_with_model(settings.model.clone())?;
            provider.next_allowed = self.next_allowed.clone();
            provider.metrics = state.vision_metrics.clone();
            Some(provider)
        } else {
            None
        };
        let mut inner = self.inner.lock().await;
        if let Some(worker) = inner.worker.take() {
            let _ = worker.shutdown.send(true);
            if let Err(error) = worker.task.await {
                tracing::error!(error = %error, "vision worker failed during shutdown");
            }
        }
        if let Some(provider) = provider {
            let (shutdown, receiver) = watch::channel(false);
            let task = VisionScheduler::spawn_with_provider(VisionJob {
                buffer: state.frame_buffer.clone(),
                state: state.vision.clone(),
                metrics: state.vision_metrics.clone(),
                selector: FrameSelector::new(settings.frames, settings.spacing_seconds),
                interval_seconds: settings.interval_seconds,
                shutdown: receiver,
                events: state.events.clone(),
                provider: Arc::new(provider),
                recovery: state.recovery.clone(),
            });
            inner.worker = Some(VisionWorker { shutdown, task });
            state
                .recovery
                .monitor
                .set(
                    "vision",
                    crate::recovery::ComponentState::Degraded,
                    "vision enabled; awaiting observation",
                    0,
                )
                .await;
        } else {
            state
                .recovery
                .monitor
                .set(
                    "vision",
                    crate::recovery::ComponentState::Healthy,
                    "vision disabled",
                    0,
                )
                .await;
        }
        inner.settings = settings;
        Ok(())
    }

    pub async fn stop(&self) {
        let mut inner = self.inner.lock().await;
        if let Some(worker) = inner.worker.take() {
            let _ = worker.shutdown.send(true);
            if let Err(error) = worker.task.await {
                tracing::error!(error = %error, "vision worker failed during shutdown");
            }
        }
    }
}

pub struct VisionJob {
    pub buffer: FrameBuffer,
    pub state: VisionState,
    pub metrics: VisionMetrics,
    pub selector: FrameSelector,
    pub interval_seconds: u64,
    pub shutdown: watch::Receiver<bool>,
    pub events: EventBus,
    pub provider: Arc<dyn VisionProvider>,
    pub recovery: RecoveryEngine,
}
impl VisionScheduler {
    pub fn spawn_if_enabled(
        config: &crate::config::Config,
        create_job: impl FnOnce() -> Result<VisionJob, &'static str>,
    ) -> Result<Option<tokio::task::JoinHandle<()>>, &'static str> {
        if !config.vision_enabled() {
            return Ok(None);
        }
        Ok(Some(Self::spawn_with_provider(create_job()?)))
    }

    pub fn spawn_with_provider(job: VisionJob) -> tokio::task::JoinHandle<()> {
        let VisionJob {
            buffer,
            state,
            metrics,
            selector,
            interval_seconds,
            shutdown,
            events,
            provider,
            recovery,
        } = job;
        let interval = std::time::Duration::from_secs(interval_seconds.max(1));
        tokio::spawn(async move {
            let mut shutdown = shutdown;
            let mut ticker = tokio::time::interval(interval);
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            let mut last_frame = None;
            let mut cooldown_until = tokio::time::Instant::now();
            loop {
                tokio::select! {
                    _ = ticker.tick() => {}
                    changed = shutdown.changed() => {
                        if changed.is_err() || *shutdown.borrow() { break; }
                        continue;
                    }
                }
                if tokio::time::Instant::now() < cooldown_until {
                    continue;
                }
                let frames = selector.select(&buffer);
                let Some(latest) = frames.last() else {
                    continue;
                };
                let frame_id = (latest.sequence, latest.captured_at_ms);
                if last_frame == Some(frame_id) {
                    continue;
                }
                last_frame = Some(frame_id);
                let frame_count = frames.len();
                metrics.request();
                let started = Instant::now();
                let result = tokio::select! {
                    result = provider.analyze(frames) => result,
                    changed = shutdown.changed() => {
                        if changed.is_err() || *shutdown.borrow() { break; }
                        continue;
                    }
                };
                match result {
                    Ok(analysis) => {
                        let latency = started.elapsed().as_millis() as u64;
                        let timestamp = now_ms();
                        metrics.success(latency, timestamp, frame_count);
                        state
                            .set(LatestAnalysis {
                                timestamp,
                                analysis: analysis.clone(),
                                provider: provider.name().into(),
                                latency_ms: latency,
                                frames_analyzed: frame_count,
                            })
                            .await;
                        if recovery.monitor.state("vision").await
                            == Some(crate::recovery::ComponentState::Recovering)
                        {
                            recovery
                                .recovered("vision", started, "vision provider recovered")
                                .await;
                        } else {
                            recovery
                                .monitor
                                .set(
                                    "vision",
                                    crate::recovery::ComponentState::Healthy,
                                    "vision provider operational",
                                    0,
                                )
                                .await;
                        }
                        let metadata = serde_json::json!({"changes":analysis.changes,"activities":analysis.activities,"frames_analyzed":frame_count});
                        events.publish_record(EventRecord {
                            id: String::new(),
                            timestamp,
                            source_id: None,
                            event_type: "vision.completed".into(),
                            provider: Some(provider.name().into()),
                            summary: analysis.summary.clone(),
                            objects: analysis.objects.clone(),
                            confidence: None,
                            latency_ms: Some(latency),
                            metadata: metadata.clone(),
                        });
                        events.publish_record(EventRecord {
                            id: String::new(),
                            timestamp,
                            source_id: None,
                            event_type: "scene.observed".into(),
                            provider: Some(provider.name().into()),
                            summary: analysis.summary.clone(),
                            objects: analysis.objects.clone(),
                            confidence: None,
                            latency_ms: Some(latency),
                            metadata,
                        });
                        tracing::info!(
                            provider = provider.name(),
                            latency_ms = latency,
                            frames_analyzed = frame_count,
                            "vision observation completed"
                        );
                    }
                    Err(error) => {
                        cooldown_until = tokio::time::Instant::now() + Duration::from_secs(30);
                        metrics.failure();
                        let permanent = provider.is_permanent_failure(&error);
                        if permanent {
                            recovery.failed("vision", error.clone(), 1).await;
                        } else {
                            recovery.begin("vision", error.clone()).await;
                        }
                        events.publish_record(EventRecord {
                            id: String::new(),
                            timestamp: now_ms(),
                            source_id: None,
                            event_type: "vision.failed".into(),
                            provider: Some(provider.name().into()),
                            summary: error.clone(),
                            objects: Vec::new(),
                            confidence: None,
                            latency_ms: Some(started.elapsed().as_millis() as u64),
                            metadata: serde_json::json!({"frames_analyzed":frame_count}),
                        });
                        tracing::warn!(provider=provider.name(), error=%error, frames_analyzed=frame_count, "vision observation failed");
                        if permanent {
                            break;
                        }
                    }
                }
            }
        })
    }
}
fn now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{extract::State, http::StatusCode, routing::post, Json, Router};
    use std::{collections::VecDeque, sync::Mutex as StdMutex};

    struct MockReply {
        status: StatusCode,
        retry_after: Option<&'static str>,
        body: serde_json::Value,
        delay: Duration,
    }

    #[derive(Default)]
    struct MockState {
        replies: VecDeque<MockReply>,
        requests: Vec<(Instant, serde_json::Value)>,
    }

    async fn mock_response(
        State(state): State<Arc<StdMutex<MockState>>>,
        Json(body): Json<serde_json::Value>,
    ) -> (StatusCode, axum::http::HeaderMap, Json<serde_json::Value>) {
        let reply = {
            let mut state = state.lock().unwrap();
            state.requests.push((Instant::now(), body));
            state.replies.pop_front().expect("unexpected request")
        };
        tokio::time::sleep(reply.delay).await;
        let mut headers = axum::http::HeaderMap::new();
        if let Some(value) = reply.retry_after {
            headers.insert(axum::http::header::RETRY_AFTER, value.parse().unwrap());
        }
        (reply.status, headers, Json(reply.body))
    }

    fn reply(status: u16, body: serde_json::Value) -> MockReply {
        MockReply {
            status: StatusCode::from_u16(status).unwrap(),
            retry_after: None,
            body,
            delay: Duration::ZERO,
        }
    }

    fn observation() -> serde_json::Value {
        serde_json::json!({"summary":"A person walks", "changes":["movement"], "activities":["walking"], "objects":["person"]})
    }

    fn complete_response() -> serde_json::Value {
        serde_json::json!({"status":"completed","output":[
            {"type":"reasoning"},
            {"type":"message","content":[{"type":"output_text","text":observation().to_string()}]}
        ]})
    }

    async fn mock_provider(
        replies: Vec<MockReply>,
    ) -> (
        OpenAiVisionProvider,
        Arc<StdMutex<MockState>>,
        tokio::task::JoinHandle<()>,
    ) {
        let state = Arc::new(StdMutex::new(MockState {
            replies: replies.into(),
            requests: Vec::new(),
        }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}/v1/responses", listener.local_addr().unwrap());
        let router = Router::new()
            .route("/v1/responses", post(mock_response))
            .with_state(state.clone());
        let task = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        let provider = OpenAiVisionProvider::from_values(
            Some("test-only-placeholder".into()),
            Some("test-model".into()),
            endpoint,
        )
        .unwrap();
        (provider, state, task)
    }

    fn frame() -> Vec<Arc<Frame>> {
        vec![Arc::new(Frame::blank(1, 2, 2))]
    }

    #[tokio::test]
    async fn successful_response_sends_images_and_parses_nested_output() {
        let mut response = complete_response();
        response["usage"] = serde_json::json!({"input_tokens": 12, "output_tokens": 3});
        let (provider, server, task) = mock_provider(vec![reply(200, response)]).await;
        let result = provider.analyze(frame()).await.unwrap();
        assert_eq!(result.summary, "A person walks");
        assert_eq!(result.objects, ["person"]);
        assert_eq!(provider.metrics.snapshot().input_tokens, Some(12));
        assert_eq!(provider.metrics.snapshot().output_tokens, Some(3));
        let server = server.lock().unwrap();
        let body = &server.requests[0].1;
        assert_eq!(body["model"], "test-model");
        assert_eq!(body["text"]["format"]["type"], "json_schema");
        assert_eq!(body["input"][0]["content"][1]["type"], "input_image");
        assert!(body["input"][0]["content"][1]["image_url"]
            .as_str()
            .unwrap()
            .starts_with("data:image/jpeg;base64,"));
        task.abort();
    }

    #[test]
    fn concatenates_output_text_from_multiple_items() {
        let value = serde_json::json!({"status":"completed","output":[
            {"type":"message","content":[{"type":"output_text","text":"{\"summary\":\"ok\","}]},
            {"type":"message","content":[{"type":"output_text","text":"\"changes\":[],\"activities\":[],\"objects\":[]}"}]}
        ]});
        assert_eq!(parse_response(&value).unwrap().summary, "ok");
    }

    #[tokio::test]
    async fn request_timeout_is_retried_only_within_the_limit() {
        let replies = (0..3)
            .map(|_| {
                let mut result = reply(200, complete_response());
                result.delay = Duration::from_millis(250);
                result
            })
            .collect();
        let (mut provider, server, task) = mock_provider(replies).await;
        provider.client = reqwest::Client::builder()
            .timeout(Duration::from_millis(50))
            .build()
            .unwrap();
        assert_eq!(
            provider.analyze(frame()).await.unwrap_err(),
            "OpenAI request failed or timed out"
        );
        assert_eq!(server.lock().unwrap().requests.len(), 3);
        assert_eq!(provider.metrics.snapshot().http_errors, 3);
        task.abort();
    }

    #[tokio::test]
    async fn invalid_missing_incomplete_and_refused_outputs_fail_safely() {
        let cases = [
            serde_json::json!({"status":"completed","output":[]}),
            serde_json::json!({"status":"completed"}),
            serde_json::json!({"status":"completed","output":[{"content":[{"type":"output_text","text":"not json"}]}]}),
            serde_json::json!({"status":"completed","output":[{"content":[{"type":"output_text","text":"{}"}]}]}),
            serde_json::json!({"status":"incomplete","output":[]}),
            serde_json::json!({"status":"failed","output":[]}),
            serde_json::json!({"status":"in_progress","output":[]}),
            serde_json::json!({"status":"completed","output":[{"content":[{"type":"refusal","refusal":"sensitive content"}]}]}),
        ];
        let (provider, server, task) =
            mock_provider(cases.into_iter().map(|body| reply(200, body)).collect()).await;
        for _ in 0..8 {
            let error = provider.analyze(frame()).await.unwrap_err();
            assert!(!error.contains("sensitive content"));
        }
        assert_eq!(server.lock().unwrap().requests.len(), 8);
        task.abort();
    }

    #[tokio::test]
    async fn unauthorized_is_not_retried_and_response_body_is_not_exposed() {
        let (provider, server, task) = mock_provider(vec![reply(
            401,
            serde_json::json!({"error":{"message":"secret from remote"}}),
        )])
        .await;
        let error = provider.analyze(frame()).await.unwrap_err();
        assert_eq!(error, "OpenAI request failed (HTTP 401)");
        assert_eq!(server.lock().unwrap().requests.len(), 1);
        task.abort();
    }

    #[tokio::test]
    async fn rate_limit_honors_retry_after_and_recovers() {
        let mut limited = reply(429, serde_json::json!({}));
        limited.retry_after = Some("1");
        let (provider, server, task) =
            mock_provider(vec![limited, reply(200, complete_response())]).await;
        assert_eq!(
            provider.analyze(frame()).await.unwrap().summary,
            "A person walks"
        );
        let server = server.lock().unwrap();
        assert_eq!(server.requests.len(), 2);
        assert!(
            server.requests[1].0.duration_since(server.requests[0].0) >= Duration::from_secs(1)
        );
        task.abort();
    }

    #[tokio::test]
    async fn server_errors_retry_a_bounded_number_of_times() {
        let (provider, server, task) =
            mock_provider((0..3).map(|_| reply(503, serde_json::json!({}))).collect()).await;
        assert_eq!(
            provider.analyze(frame()).await.unwrap_err(),
            "OpenAI request failed (HTTP 503)"
        );
        assert_eq!(server.lock().unwrap().requests.len(), 3);
        task.abort();
    }

    #[test]
    fn retry_after_supports_http_dates() {
        let mut headers = reqwest::header::HeaderMap::new();
        let date = httpdate::fmt_http_date(SystemTime::now() + Duration::from_secs(30));
        headers.insert(reqwest::header::RETRY_AFTER, date.parse().unwrap());
        let delay = retry_after(&headers).unwrap();
        assert!(delay > Duration::from_secs(28) && delay <= Duration::from_secs(30));
    }

    #[test]
    fn missing_key_from_environment() {
        const MARKER: &str = "SENTINEL_TEST_MISSING_OPENAI_KEY";
        if std::env::var_os(MARKER).is_some() {
            assert!(matches!(
                OpenAiVisionProvider::from_env(),
                Err("OPENAI_API_KEY is missing; vision unavailable")
            ));
            return;
        }
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "vision::tests::missing_key_from_environment"])
            .env_remove("OPENAI_API_KEY")
            .env(MARKER, "1")
            .status()
            .unwrap();
        assert!(status.success());
        assert!(OpenAiVisionProvider::from_values(
            Some("  ".into()),
            None,
            "http://127.0.0.1".into()
        )
        .is_err());
    }

    #[tokio::test]
    async fn disabled_vision_never_constructs_or_calls_provider() {
        let mut config = crate::config::Config::default();
        for (pipeline, vision) in [(false, true), (true, false), (false, false)] {
            config.pipeline.vision = pipeline;
            config.vision.enabled = vision;
            let result = VisionScheduler::spawn_if_enabled(&config, || {
                panic!("disabled vision constructed a provider")
            })
            .unwrap();
            assert!(result.is_none());
        }
    }

    #[tokio::test]
    async fn runtime_vision_cannot_override_disabled_pipeline() {
        let mut config = crate::config::Config::default();
        config.pipeline.vision = false;
        config.vision.enabled = true;
        let (shutdown, receiver) = watch::channel(false);
        let state = crate::api::AppState::new(config, FrameBuffer::new(4), shutdown, receiver);
        let (mut settings, running) = state.vision_control.snapshot().await;
        assert!(!settings.enabled);
        assert!(!running);
        settings.enabled = true;
        assert_eq!(
            state.vision_control.apply(settings, &state).await,
            Err("vision is disabled by pipeline.vision")
        );
        assert!(!state.vision_control.snapshot().await.1);
    }

    #[tokio::test]
    async fn scheduler_reports_failure_without_blocking_buffer_or_shutdown() {
        let (provider, server, task) = mock_provider(vec![reply(401, serde_json::json!({}))]).await;
        let buffer = FrameBuffer::new(4);
        buffer.push(Frame::blank(1, 2, 2));
        let metrics = VisionMetrics::default();
        let events = EventBus::new(8);
        let recovery = RecoveryEngine::new(events.clone());
        let (shutdown, receiver) = watch::channel(false);
        let worker = VisionScheduler::spawn_with_provider(VisionJob {
            buffer: buffer.clone(),
            state: VisionState::default(),
            metrics: metrics.clone(),
            selector: FrameSelector::new(1, 1),
            interval_seconds: 1,
            shutdown: receiver,
            events,
            provider: Arc::new(provider),
            recovery: recovery.clone(),
        });
        for _ in 0..100 {
            if recovery.monitor.state("vision").await.is_some() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert_eq!(
            recovery.monitor.state("vision").await,
            Some(crate::recovery::ComponentState::Failed)
        );
        assert!(metrics
            .prometheus()
            .contains("sentinel_vision_failed_analyses 1"));
        buffer.push(Frame::blank(2, 2, 2));
        assert_eq!(buffer.latest().unwrap().sequence, 2);
        assert_eq!(server.lock().unwrap().requests.len(), 1);
        let _ = shutdown.send(true);
        tokio::time::timeout(Duration::from_secs(1), worker)
            .await
            .unwrap()
            .unwrap();
        task.abort();
    }

    #[tokio::test]
    async fn disabling_vision_cancels_inflight_rate_limit_wait() {
        let mut limited = reply(429, serde_json::json!({}));
        limited.retry_after = Some("120");
        let (mut provider, server, mock_task) = mock_provider(vec![limited]).await;
        let config = crate::config::Config::default();
        let buffer = FrameBuffer::new(4);
        buffer.push(Frame::blank(1, 2, 2));
        let (shutdown, receiver) = watch::channel(false);
        let state = crate::api::AppState::new(config, buffer.clone(), shutdown, receiver);
        provider.next_allowed = state.vision_control.next_allowed.clone();
        let (stop, receiver) = watch::channel(false);
        let worker = VisionScheduler::spawn_with_provider(VisionJob {
            buffer: buffer.clone(),
            state: state.vision.clone(),
            metrics: state.vision_metrics.clone(),
            selector: FrameSelector::new(1, 1),
            interval_seconds: 1,
            shutdown: receiver,
            events: state.events.clone(),
            provider: Arc::new(provider),
            recovery: state.recovery.clone(),
        });
        {
            let mut inner = state.vision_control.inner.lock().await;
            inner.settings.enabled = true;
            inner.worker = Some(VisionWorker {
                shutdown: stop,
                task: worker,
            });
        }
        for _ in 0..100 {
            if !server.lock().unwrap().requests.is_empty() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert_eq!(server.lock().unwrap().requests.len(), 1);
        let (mut settings, _) = state.vision_control.snapshot().await;
        settings.enabled = false;
        tokio::time::timeout(
            Duration::from_secs(1),
            state.vision_control.apply(settings, &state),
        )
        .await
        .unwrap()
        .unwrap();
        buffer.push(Frame::blank(2, 2, 2));
        tokio::time::sleep(Duration::from_millis(1200)).await;
        assert_eq!(server.lock().unwrap().requests.len(), 1);
        assert!(!state.vision_control.snapshot().await.1);
        mock_task.abort();
    }
}
