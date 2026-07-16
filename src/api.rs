use crate::{
    auth::{Authenticator, BearerAuthenticator},
    config::Config,
    events::{Event, EventBus},
    health::Health,
    metrics::Metrics,
    preview::Preview,
    sources::{AddSource, SourceRegistry},
};
use axum::{
    extract::{Path, State},
    http::{header, Request, StatusCode},
    middleware::{self, Next},
    response::{
        sse::{Event as SseEvent, KeepAlive, Sse},
        IntoResponse,
    },
    routing::{delete, get, post},
    Json, Router,
};
use serde_json::json;
use std::{convert::Infallible, sync::Arc};
use tokio_stream::{wrappers::BroadcastStream, StreamExt};

#[derive(Clone)]
pub struct AppState {
    pub health: Health,
    pub metrics: Metrics,
    pub sources: SourceRegistry,
    pub events: EventBus,
    pub authenticator: BearerAuthenticator,
    pub preview: Preview,
}
impl AppState {
    pub fn new() -> Self {
        Self {
            health: Health::default(),
            metrics: Metrics::new(),
            sources: SourceRegistry::new(),
            events: EventBus::new(),
            authenticator: BearerAuthenticator::from_env(),
            preview: Preview::new(),
        }
    }
}

pub async fn serve(config: Config, state: AppState) -> Result<(), std::io::Error> {
    let shared = Arc::new(state);
    let app = Router::new()
        .route("/health/live", get(live))
        .route("/health/ready", get(ready))
        .route("/metrics", get(metrics))
        .route("/api/v1/status", get(status))
        .route("/api/v1/version", get(version))
        .route("/api/v1/sources", get(list_sources).post(add_source))
        .route("/api/v1/sources/:id/start", post(start_source))
        .route("/api/v1/sources/:id/stop", post(stop_source))
        .route("/api/v1/sources/:id", delete(remove_source))
        .route("/api/v1/config", get(show_config))
        .route("/api/v1/events", get(events))
        .route("/api/v1/auth/whoami", get(whoami))
        .route("/api/v1/preview", get(preview))
        .layer(middleware::from_fn_with_state(shared.clone(), require_auth))
        .with_state(shared);
    let listener = tokio::net::TcpListener::bind(&config.bind).await?;
    tracing::info!(address=%config.bind, "administration API listening");
    axum::serve(listener, app).await
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
    (
        [(header::CONTENT_TYPE, "text/plain; version=0.0.4")],
        state.metrics.prometheus(),
    )
}
async fn status(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    Json(state.metrics.snapshot())
}
async fn version() -> impl IntoResponse {
    Json(json!({"name":"sentinel-streaming","version":env!("CARGO_PKG_VERSION")}))
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
        Err(error) => (StatusCode::CONFLICT, Json(json!({"error": error}))),
    }
}
async fn start_source(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    set_source_state(id, state, true).await
}
async fn stop_source(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    set_source_state(id, state, false).await
}
async fn set_source_state(
    id: String,
    state: Arc<AppState>,
    running: bool,
) -> (StatusCode, Json<serde_json::Value>) {
    match state.sources.set_running(&id, running).await {
        Some(source) => {
            state.events.publish(Event {
                kind: if running {
                    "stream_started"
                } else {
                    "stream_stopped"
                }
                .into(),
                source_id: Some(id),
                message: "source state changed".into(),
            });
            (StatusCode::OK, Json(json!(source)))
        }
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({"error":"source not found"})),
        ),
    }
}
async fn remove_source(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    if state.sources.remove(&id).await {
        state.events.publish(Event {
            kind: "source_removed".into(),
            source_id: Some(id),
            message: "source removed".into(),
        });
        StatusCode::NO_CONTENT
    } else {
        StatusCode::NOT_FOUND
    }
}
async fn show_config(State(_state): State<Arc<AppState>>) -> impl IntoResponse {
    Json(json!({"bind":"0.0.0.0:8080","fps":30}))
}
async fn events(
    State(state): State<Arc<AppState>>,
) -> Sse<impl tokio_stream::Stream<Item = Result<SseEvent, Infallible>>> {
    let stream = BroadcastStream::new(state.events.subscribe()).filter_map(|item| {
        item.ok()
            .and_then(|event| serde_json::to_string(&event).ok())
            .map(|data| Ok(SseEvent::default().data(data)))
    });
    Sse::new(stream).keep_alive(KeepAlive::default())
}
