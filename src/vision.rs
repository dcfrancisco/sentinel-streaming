use crate::{
    events::{EventBus, EventRecord},
    frame::Frame,
    frame_buffer::FrameBuffer,
};
use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Instant,
};
use tokio::sync::{watch, RwLock};

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
        format!("sentinel_vision_requests {}\nsentinel_vision_successful_analyses {}\nsentinel_vision_failed_analyses {}\nsentinel_vision_average_latency_ms {}\nsentinel_vision_average_frames_analyzed {}\nsentinel_vision_observation_count {}\nsentinel_vision_last_analysis_timestamp {}\n", requests, successes, self.failures.load(Ordering::Relaxed), average_latency, average_frames, successes, self.last_timestamp.load(Ordering::Relaxed))
    }
}

#[async_trait]
pub trait VisionProvider: Send + Sync {
    fn name(&self) -> &'static str;
    async fn analyze(&self, frames: Vec<Arc<Frame>>) -> Result<SceneAnalysis, String>;
}
pub struct OpenAiVisionProvider {
    client: reqwest::Client,
    api_key: String,
    model: &'static str,
}
impl OpenAiVisionProvider {
    pub fn from_env() -> Option<Self> {
        let api_key = match std::env::var("OPENAI_API_KEY") {
            Ok(key) if !key.trim().is_empty() => key,
            _ => {
                tracing::warn!("OPENAI_API_KEY is missing; vision disabled");
                return None;
            }
        };
        Some(Self {
            client: reqwest::Client::new(),
            api_key,
            model: "gpt-5.4-mini",
        })
    }
}
#[async_trait]
impl VisionProvider for OpenAiVisionProvider {
    fn name(&self) -> &'static str {
        "OpenAI"
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
                .map_err(|e| e.to_string())?;
            content.push(serde_json::json!({"type":"input_image","image_url":format!("data:image/jpeg;base64,{}", STANDARD.encode(jpeg)),"detail":"low"}));
        }
        let body = serde_json::json!({"model":self.model,"instructions":"Analyze the sequence as a temporal scene observation.","input":[{"role":"user","content":content}],"text":{"format":{"type":"json_schema","name":"scene_observation","strict":true,"schema":{"type":"object","properties":{"summary":{"type":"string"},"changes":{"type":"array","items":{"type":"string"}},"activities":{"type":"array","items":{"type":"string"}},"objects":{"type":"array","items":{"type":"string"}}},"required":["summary","changes","activities","objects"],"additionalProperties":false}}}});
        let response = self
            .client
            .post("https://api.openai.com/v1/responses")
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        let status = response.status();
        let value: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;
        if !status.is_success() {
            return Err(value.to_string());
        }
        let text = value
            .get("output_text")
            .and_then(|text| text.as_str())
            .ok_or_else(|| "Responses API returned no output_text".to_string())?;
        serde_json::from_str(text).map_err(|e| format!("invalid scene observation JSON: {e}"))
    }
}

pub struct VisionScheduler;
impl VisionScheduler {
    pub fn spawn(
        buffer: FrameBuffer,
        state: VisionState,
        metrics: VisionMetrics,
        selector: FrameSelector,
        interval_seconds: u64,
        shutdown: watch::Receiver<bool>,
        events: EventBus,
    ) -> Option<tokio::task::JoinHandle<()>> {
        let provider = OpenAiVisionProvider::from_env()?;
        let provider: Arc<dyn VisionProvider> = Arc::new(provider);
        let interval = std::time::Duration::from_secs(interval_seconds.max(1));
        Some(tokio::spawn(async move {
            let mut shutdown = shutdown;
            let mut ticker = tokio::time::interval(interval);
            loop {
                tokio::select! { _ = ticker.tick() => { let frames = selector.select(&buffer); if frames.is_empty() { continue; } let frame_count = frames.len(); metrics.request(); let started = Instant::now(); match provider.analyze(frames).await { Ok(analysis) => { let latency = started.elapsed().as_millis() as u64; let timestamp = now_ms(); metrics.success(latency, timestamp, frame_count); state.set(LatestAnalysis { timestamp, analysis: analysis.clone(), provider: provider.name().into(), latency_ms: latency, frames_analyzed: frame_count }).await; let metadata = serde_json::json!({"changes":analysis.changes,"activities":analysis.activities,"frames_analyzed":frame_count}); events.publish_record(EventRecord { id:String::new(), timestamp, source_id:Some("builtin".into()), event_type:"vision.completed".into(), provider:Some(provider.name().into()), summary:analysis.summary.clone(), objects:analysis.objects.clone(), confidence:None, latency_ms:Some(latency), metadata:metadata.clone() }); events.publish_record(EventRecord { id:String::new(), timestamp, source_id:Some("builtin".into()), event_type:"scene.observed".into(), provider:Some(provider.name().into()), summary:analysis.summary.clone(), objects:analysis.objects.clone(), confidence:None, latency_ms:Some(latency), metadata }); tracing::info!(provider=provider.name(), latency_ms=latency, frames_analyzed=frame_count, summary=%analysis.summary, changes=?analysis.changes, activities=?analysis.activities, objects=?analysis.objects, "Vision Observation"); }, Err(error) => { metrics.failure(); events.publish_record(EventRecord { id:String::new(), timestamp:now_ms(), source_id:Some("builtin".into()), event_type:"vision.failed".into(), provider:Some(provider.name().into()), summary:error.clone(), objects:Vec::new(), confidence:None, latency_ms:Some(started.elapsed().as_millis() as u64), metadata:serde_json::json!({"frames_analyzed":frame_count}) }); tracing::warn!(provider=provider.name(), error=%error, frames_analyzed=frame_count, "vision observation failed"); } } }, changed = shutdown.changed() => { if changed.is_err() || *shutdown.borrow() { break; } } }
            }
        }))
    }
}
fn now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}
