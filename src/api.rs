use crate::{
    auth::{Authenticator, BearerAuthenticator},
    config::Config,
    events::{Event, EventBus},
    frame_buffer::FrameBuffer,
    health::Health,
    metrics::Metrics,
    mjpeg::MjpegStream,
    preview::Preview,
    runtime::RuntimeStatus,
    sources::{AddSource, SourceManagerError, VideoSourceManager},
    vision::{VisionMetrics, VisionState},
};
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, Request, StatusCode},
    middleware::{self, Next},
    response::{
        sse::{Event as SseEvent, KeepAlive, Sse},
        IntoResponse, Response,
    },
    routing::{get, post},
    Json, Router,
};
use serde_json::json;
use std::{convert::Infallible, sync::Arc};
use tokio_stream::{wrappers::BroadcastStream, StreamExt};

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub health: Health,
    pub metrics: Metrics,
    pub sources: VideoSourceManager,
    pub events: EventBus,
    pub authenticator: BearerAuthenticator,
    pub preview: Preview,
    pub runtime: RuntimeStatus,
    pub frame_buffer: FrameBuffer,
    pub vision: VisionState,
    pub vision_metrics: VisionMetrics,
    pub mjpeg: MjpegStream,
    pub shutdown: tokio::sync::watch::Sender<bool>,
}
impl AppState {
    pub fn new(
        config: Config,
        frame_buffer: FrameBuffer,
        shutdown: tokio::sync::watch::Sender<bool>,
    ) -> Self {
        let events = EventBus::new(config.events.capacity);
        let mjpeg = MjpegStream::new(frame_buffer.clone());
        Self {
            config: config.clone(),
            health: Health::default(),
            metrics: Metrics::new(),
            sources: VideoSourceManager::new(config.fps, events.clone()),
            events,
            authenticator: BearerAuthenticator::from_env(),
            preview: Preview::new(),
            runtime: RuntimeStatus::default(),
            frame_buffer,
            vision: VisionState::default(),
            vision_metrics: VisionMetrics::default(),
            mjpeg,
            shutdown,
        }
    }
}

pub async fn serve(
    state: AppState,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
) -> Result<(), std::io::Error> {
    let shared = Arc::new(state);
    let app = Router::new()
        .route("/health/live", get(live))
        .route("/health/ready", get(ready))
        .route("/metrics", get(metrics))
        .route("/api/v1/status", get(status))
        .route("/api/v1/version", get(version))
        .route("/api/v1/stop", post(stop))
        .route("/api/v1/sources", get(list_sources).post(add_source))
        .route("/api/v1/sources/:id", get(get_source).delete(remove_source))
        .route("/api/v1/sources/:id/start", post(start_source))
        .route("/api/v1/sources/:id/stop", post(stop_source))
        .route("/api/v1/sources/:id/restart", post(restart_source))
        .route("/api/v1/config", get(show_config))
        .route("/api/v1/events", get(list_events))
        .route("/api/v1/events/latest", get(latest_event))
        .route("/api/v1/events/:id", get(get_event))
        .route("/api/v1/events/stream", get(event_stream))
        .route("/api/v1/auth/whoami", get(whoami))
        .route("/api/v1/preview", get(preview))
        .route("/api/v1/vision/latest", get(vision_latest))
        .route("/api/v1/streams/:source_id/mjpeg", get(mjpeg))
        .layer(middleware::from_fn_with_state(shared.clone(), require_auth))
        .with_state(shared.clone());
    let listener = tokio::net::TcpListener::bind(&shared.config.bind).await?;
    shared.runtime.mark_http_started().await;
    tracing::info!(address=%shared.config.bind, "HTTP server started");
    let graceful = async move {
        let _ = shutdown.changed().await;
        tracing::info!("HTTP server shutting down");
    };
    axum::serve(listener, app)
        .with_graceful_shutdown(graceful)
        .await
}
async fn live() -> impl IntoResponse {
    Json(json!({"status":"ok"}))
}
async fn ready(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let ready = state
        .health
        .ready
        .load(std::sync::atomic::Ordering::Relaxed);
    (
        if ready {
            StatusCode::OK
        } else {
            StatusCode::SERVICE_UNAVAILABLE
        },
        Json(json!({"status": if ready {"ready"} else {"starting"}})),
    )
}
async fn metrics(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let source_metrics = state.sources.prometheus().await;
    let vision_metrics = state.vision_metrics.prometheus();
    let event_metrics = state.events.store().prometheus().await;
    let mjpeg_metrics = state.mjpeg.metrics.prometheus();
    (
        [(header::CONTENT_TYPE, "text/plain; version=0.0.4")],
        format!(
            "{}{}{}{}{}",
            state.metrics.prometheus(),
            source_metrics,
            vision_metrics,
            event_metrics,
            mjpeg_metrics
        ),
    )
}
async fn status(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    Json(
        json!({"version": env!("CARGO_PKG_VERSION"), "metrics": state.metrics.snapshot(), "runtime": state.runtime.snapshot().await, "buffer": {"length": state.frame_buffer.len(), "capacity": state.frame_buffer.capacity()}}),
    )
}
async fn version() -> impl IntoResponse {
    Json(json!({"name":"sentinel-streaming","version":env!("CARGO_PKG_VERSION")}))
}
async fn stop(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    state.runtime.mark_shutting_down().await;
    let _ = state.shutdown.send(true);
    Json(json!({"status":"stopping"}))
}
async fn whoami(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    Json(json!({"principal": state.authenticator.principal()}))
}
async fn preview(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.preview.get().await {
        Some(bytes) => ([(header::CONTENT_TYPE, "image/jpeg")], bytes).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}
async fn vision_latest(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.vision.latest().await {
        Some(analysis) => (StatusCode::OK, Json(json!(analysis))).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}
async fn mjpeg(
    Path(source_id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    if source_id != "builtin" {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error":"source not found"})),
        )
            .into_response();
    }
    let stream = state.mjpeg.stream(source_id);
    Response::builder()
        .status(StatusCode::OK)
        .header(
            header::CONTENT_TYPE,
            "multipart/x-mixed-replace; boundary=frame",
        )
        .body(Body::from_stream(stream))
        .expect("valid MJPEG response")
}
async fn require_auth(
    State(state): State<Arc<AppState>>,
    request: Request<axum::body::Body>,
    next: Next,
) -> impl IntoResponse {
    if matches!(
        request.uri().path(),
        "/health/live" | "/health/ready" | "/api/v1/version"
    ) {
        return next.run(request).await;
    }
    let token = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "));
    if state.authenticator.authenticate(token) {
        next.run(request).await
    } else {
        (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error":"authentication required"})),
        )
            .into_response()
    }
}
async fn list_sources(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    Json(state.sources.list().await)
}
async fn add_source(
    State(state): State<Arc<AppState>>,
    Json(request): Json<AddSource>,
) -> impl IntoResponse {
    match state.sources.add(request).await {
        Ok(source) => {
            state.events.publish(Event {
                kind: "source_added".into(),
                source_id: Some(source.id.clone()),
                message: "source registered".into(),
            });
            (StatusCode::CREATED, Json(json!(source)))
        }
        Err(error) => manager_error(error),
    }
}
async fn start_source(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    match state.sources.start(&id).await {
        Ok(source) => (StatusCode::OK, Json(json!(source))),
        Err(error) => manager_error(error),
    }
}
async fn stop_source(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    match state.sources.stop(&id).await {
        Ok(source) => (StatusCode::OK, Json(json!(source))),
        Err(error) => manager_error(error),
    }
}
async fn restart_source(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    match state.sources.restart(&id).await {
        Ok(source) => (StatusCode::OK, Json(json!(source))),
        Err(error) => manager_error(error),
    }
}
async fn get_source(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    match state.sources.get(&id).await {
        Ok(source) => (StatusCode::OK, Json(json!(source))),
        Err(error) => manager_error(error),
    }
}
fn manager_error(error: SourceManagerError) -> (StatusCode, Json<serde_json::Value>) {
    let status = match &error {
        SourceManagerError::NotFound => StatusCode::NOT_FOUND,
        SourceManagerError::AlreadyExists => StatusCode::CONFLICT,
        SourceManagerError::Unsupported(_) => StatusCode::NOT_IMPLEMENTED,
        SourceManagerError::Camera(_) => StatusCode::BAD_GATEWAY,
    };
    (status, Json(json!({"error": error.to_string()})))
}
async fn remove_source(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    match state.sources.remove(&id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => manager_error(error).into_response(),
    }
}
async fn show_config(State(_state): State<Arc<AppState>>) -> impl IntoResponse {
    Json(json!(_state.config))
}
async fn list_events(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    Json(state.events.store().recent(100).await)
}
async fn latest_event(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.events.store().latest().await {
        Some(event) => (StatusCode::OK, Json(event)).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}
async fn get_event(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    match state.events.store().get(&id).await {
        Some(event) => (StatusCode::OK, Json(event)).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}
async fn event_stream(
    State(state): State<Arc<AppState>>,
) -> Sse<impl tokio_stream::Stream<Item = Result<SseEvent, Infallible>>> {
    let stream = BroadcastStream::new(state.events.subscribe()).filter_map(|item| {
        item.ok()
            .and_then(|event| serde_json::to_string(&event).ok())
            .map(|data| Ok(SseEvent::default().data(data)))
    });
    Sse::new(stream).keep_alive(KeepAlive::default())
}
