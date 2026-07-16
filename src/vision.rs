use crate::{frame::Frame, frame_buffer::FrameBuffer};
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
    pub objects: Vec<String>,
}
#[derive(Clone, Debug, Serialize)]
pub struct LatestAnalysis {
    pub timestamp: u128,
    pub analysis: SceneAnalysis,
    pub provider: String,
    pub latency_ms: u64,
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

#[derive(Clone, Default)]
pub struct VisionMetrics {
    requests: Arc<AtomicU64>,
    successes: Arc<AtomicU64>,
    failures: Arc<AtomicU64>,
    latency_total_ms: Arc<AtomicU64>,
    last_timestamp: Arc<AtomicU64>,
}
impl VisionMetrics {
    fn request(&self) {
        self.requests.fetch_add(1, Ordering::Relaxed);
    }
    fn success(&self, latency: u64, timestamp: u128) {
        self.successes.fetch_add(1, Ordering::Relaxed);
        self.latency_total_ms.fetch_add(latency, Ordering::Relaxed);
        self.last_timestamp
            .store(timestamp.min(u64::MAX as u128) as u64, Ordering::Relaxed);
    }
    fn failure(&self) {
        self.failures.fetch_add(1, Ordering::Relaxed);
    }
    pub fn prometheus(&self) -> String {
        let requests = self.requests.load(Ordering::Relaxed);
        let successes = self.successes.load(Ordering::Relaxed);
        let average = if successes == 0 {
            0
        } else {
            self.latency_total_ms.load(Ordering::Relaxed) / successes
        };
        format!("sentinel_vision_requests {}\nsentinel_vision_successful_analyses {}\nsentinel_vision_failed_analyses {}\nsentinel_vision_average_latency_ms {}\nsentinel_vision_last_analysis_timestamp {}\n", requests, successes, self.failures.load(Ordering::Relaxed), average, self.last_timestamp.load(Ordering::Relaxed))
    }
}

#[async_trait]
pub trait VisionProvider: Send + Sync {
    fn name(&self) -> &'static str;
    async fn analyze(&self, frame: Arc<Frame>) -> Result<SceneAnalysis, String>;
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
    async fn analyze(&self, frame: Arc<Frame>) -> Result<SceneAnalysis, String> {
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
        let image_url = format!("data:image/jpeg;base64,{}", STANDARD.encode(jpeg));
        let body = serde_json::json!({"model": self.model, "instructions": "Describe what is happening in this scene. Be factual. Do not speculate. Do not generate alarms. Return concise JSON.", "input": [{"role":"user","content":[{"type":"input_text","text":"Describe this scene."},{"type":"input_image","image_url": image_url, "detail":"low"}]}], "text":{"format":{"type":"json_schema","name":"scene_analysis","strict":true,"schema":{"type":"object","properties":{"summary":{"type":"string"},"objects":{"type":"array","items":{"type":"string"}}},"required":["summary","objects"],"additionalProperties":false}}}});
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
        serde_json::from_str(text).map_err(|e| format!("invalid scene analysis JSON: {e}"))
    }
}

pub struct VisionScheduler;
impl VisionScheduler {
    pub fn spawn(
        buffer: FrameBuffer,
        state: VisionState,
        metrics: VisionMetrics,
        interval_seconds: u64,
        shutdown: watch::Receiver<bool>,
    ) -> Option<tokio::task::JoinHandle<()>> {
        let provider = OpenAiVisionProvider::from_env()?;
        let provider: Arc<dyn VisionProvider> = Arc::new(provider);
        let interval = std::time::Duration::from_secs(interval_seconds.max(1));
        Some(tokio::spawn(async move {
            let mut shutdown = shutdown;
            let mut ticker = tokio::time::interval(interval);
            loop {
                tokio::select! { _ = ticker.tick() => { let Some(frame) = buffer.latest() else { continue; }; metrics.request(); let started = Instant::now(); match provider.analyze(frame).await { Ok(analysis) => { let latency = started.elapsed().as_millis() as u64; let timestamp = now_ms(); metrics.success(latency, timestamp); state.set(LatestAnalysis { timestamp, analysis: analysis.clone(), provider: provider.name().into(), latency_ms: latency }).await; tracing::info!(provider=provider.name(), latency_ms=latency, summary=%analysis.summary, objects=?analysis.objects, "Vision Analysis"); }, Err(error) => { metrics.failure(); tracing::warn!(provider=provider.name(), error=%error, "vision analysis failed"); } } }, changed = shutdown.changed() => { if changed.is_err() || *shutdown.borrow() { break; } } }
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
