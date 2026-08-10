use crate::{
    admin,
    auth::{Authenticator, Authority, BearerAuthenticator, Principal, PtzAuthority},
    config::Config,
    events::{Event, EventBus, EventRecord},
    frame_buffer::FrameBuffer,
    health::Health,
    media::MediaMtxAdapter,
    metrics::Metrics,
    mjpeg::MjpegStream,
    onboarding::{OnboardingCompleteRequest, OnboardingInspectRequest},
    onvif::{OnvifDiscoveryRequest, OnvifInspectRequest, PtzMoveRequest},
    preview::Preview,
    recovery::{HealthMonitor, RecoveryEngine},
    runtime::RuntimeStatus,
    sources::{
        AddSource, ConnectionTestRequest, PtzAuditContext, SourceManagerError, VideoSourceManager,
    },
    vision::{VisionMetrics, VisionState},
};
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, HeaderMap, Request, StatusCode},
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
use tower_http::cors::CorsLayer;

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub health: Health,
    pub metrics: Metrics,
    pub sources: VideoSourceManager,
    pub events: EventBus,
    pub authenticator: BearerAuthenticator,
    pub ptz_authority: PtzAuthority,
    pub preview: Preview,
    pub runtime: RuntimeStatus,
    pub frame_buffer: FrameBuffer,
    pub vision: VisionState,
    pub vision_metrics: VisionMetrics,
    pub mjpeg: MjpegStream,
    pub shutdown: tokio::sync::watch::Sender<bool>,
    pub health_monitor: HealthMonitor,
    pub recovery: RecoveryEngine,
}
impl AppState {
    pub fn new(
        config: Config,
        frame_buffer: FrameBuffer,
        shutdown: tokio::sync::watch::Sender<bool>,
        shutdown_receiver: tokio::sync::watch::Receiver<bool>,
    ) -> Self {
        let events = EventBus::new(config.events.capacity);
        let recovery = RecoveryEngine::new(events.clone());
        let mjpeg = MjpegStream::new(frame_buffer.clone());
        let media_gateway = Arc::new(MediaMtxAdapter::new(
            config.media_gateway.enabled,
            config.media_gateway.api_url.clone(),
            config.media_gateway.base_url.clone(),
            config.media_gateway.webrtc_base_url.clone(),
            config.media_gateway.hls_base_url.clone(),
            std::time::Duration::from_millis(config.media_gateway.timeout_ms),
        ));
        Self {
            config: config.clone(),
            health: Health::default(),
            metrics: Metrics::new(),
            sources: VideoSourceManager::new(
                config.fps,
                events.clone(),
                config.recovery.camera.clone(),
                shutdown_receiver,
                recovery.clone(),
            )
            .with_validation_timeout(std::time::Duration::from_millis(
                config.rtsp_validation_timeout_ms,
            ))
            .with_health_config(config.recovery.health.clone())
            .with_media_gateway(media_gateway),
            events,
            authenticator: BearerAuthenticator::from_env(),
            ptz_authority: PtzAuthority,
            preview: Preview::new(),
            runtime: RuntimeStatus::default(),
            frame_buffer,
            vision: VisionState::default(),
            vision_metrics: VisionMetrics::default(),
            mjpeg,
            shutdown,
            health_monitor: recovery.monitor.clone(),
            recovery,
        }
    }
}

pub async fn serve(
    state: AppState,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
) -> Result<(), std::io::Error> {
    let shared = Arc::new(state);
    let app = router(shared.clone());
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

pub fn router(shared: Arc<AppState>) -> Router {
    Router::new()
        .route("/health/live", get(live))
        .route("/health/ready", get(ready))
        .route("/metrics", get(metrics))
        .route("/api/v1/status", get(status))
        .route("/api/v1/version", get(version))
        .route("/api/v1/stop", post(stop))
        .route("/api/v1/sources", get(list_sources).post(add_source))
        .route("/api/v1/sources/providers", get(source_providers))
        .route("/api/v1/sources/discover", get(discover_sources))
        .route("/api/v1/onvif/discover", post(discover_onvif))
        .route("/api/v1/onboarding/discover", post(onboarding_discover))
        .route("/api/v1/onboarding/sessions/:id", get(onboarding_session))
        .route(
            "/api/v1/onboarding/sessions/:id/inspect",
            post(onboarding_inspect),
        )
        .route(
            "/api/v1/onboarding/sessions/:id/complete",
            post(onboarding_complete),
        )
        .route("/api/v1/sources/test", post(test_source))
        .route("/api/v1/sources/:id", get(get_source).delete(remove_source))
        .route("/api/v1/sources/:id/validate", post(validate_source))
        .route("/api/v1/sources/:id/onvif/inspect", post(inspect_onvif))
        .route("/api/v1/sources/:id/capabilities", get(source_capabilities))
        .route("/api/v1/sources/:id/ptz", get(ptz_capabilities))
        .route("/api/v1/sources/:id/ptz/move", post(ptz_move))
        .route("/api/v1/sources/:id/ptz/stop", post(ptz_stop))
        .route("/api/v1/sources/:id/ptz/presets", get(ptz_presets))
        .route(
            "/api/v1/sources/:id/ptz/presets/:preset_id/goto",
            post(ptz_goto_preset),
        )
        .route("/api/v1/sources/:id/playback", get(source_playback))
        .route(
            "/api/v1/sources/:id/playback/register",
            post(register_playback).delete(remove_playback),
        )
        .route("/api/v1/media-gateway/health", get(media_gateway_health))
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
        .route("/api/v1/streams/:source_id/frame", get(frame))
        .route("/admin", get(admin_page))
        .route("/admin/", get(admin_page))
        .layer(middleware::from_fn_with_state(shared.clone(), require_auth))
        .layer(CorsLayer::permissive())
        .with_state(shared)
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
    let recovery_metrics = state
        .recovery
        .metrics
        .prometheus(state.health_monitor.degraded_count().await);
    let buffer_metrics = format!(
        "sentinel_frame_buffer_size {}\nsentinel_frame_buffer_capacity {}\nsentinel_frame_buffer_utilization {}\nsentinel_frame_buffer_evictions {}\n",
        state.frame_buffer.len(),
        state.frame_buffer.capacity(),
        state.frame_buffer.utilization(),
        state.frame_buffer.evictions()
    );
    (
        [(header::CONTENT_TYPE, "text/plain; version=0.0.4")],
        format!(
            "{}{}{}{}{}{}{}",
            state.metrics.prometheus(),
            source_metrics,
            vision_metrics,
            event_metrics,
            mjpeg_metrics,
            buffer_metrics,
            recovery_metrics
        ),
    )
}
async fn status(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let event_store = state.events.store();
    Json(
        json!({"version": env!("CARGO_PKG_VERSION"), "metrics": state.metrics.snapshot(), "runtime": state.runtime.snapshot().await, "health": state.health_monitor.snapshot().await, "buffer": {"length": state.frame_buffer.len(), "capacity": state.frame_buffer.capacity(), "utilization": state.frame_buffer.utilization(), "evictions": state.frame_buffer.evictions()}, "events": {"length": event_store.len().await, "capacity": event_store.capacity()}}),
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
async fn whoami(
    State(state): State<Arc<AppState>>,
    principal: Option<axum::extract::Extension<Principal>>,
) -> impl IntoResponse {
    Json(json!({
        "authenticationConfigured": state.authenticator.configured(),
        "principal": principal.map(|value| value.0),
    }))
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
    match state.sources.get(&source_id).await {
        Ok(source) if matches!(source.status, crate::sources::SourceState::Running) => {}
        Ok(_) => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({"error":"source is not running"})),
            )
                .into_response()
        }
        Err(_) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error":"source not found"})),
            )
                .into_response()
        }
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
async fn frame(
    Path(source_id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    match state.sources.get(&source_id).await {
        Ok(source) if matches!(source.status, crate::sources::SourceState::Running) => {}
        Ok(_) => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({"error":"source is not running"})),
            )
                .into_response()
        }
        Err(_) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error":"source not found"})),
            )
                .into_response()
        }
    }
    match state.frame_buffer.latest() {
        Some(frame) => match frame.jpeg(82) {
            Ok(bytes) => (
                [
                    (header::CONTENT_TYPE, "image/jpeg"),
                    (header::ACCESS_CONTROL_ALLOW_ORIGIN, "*"),
                ],
                bytes,
            )
                .into_response(),
            Err(error) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": error.to_string()})),
            )
                .into_response(),
        },
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({"error":"no frame available"})),
        )
            .into_response(),
    }
}
async fn require_auth(
    State(state): State<Arc<AppState>>,
    mut request: Request<axum::body::Body>,
    next: Next,
) -> impl IntoResponse {
    // Preserve the camera-free developer experience when no credentials are
    // configured. Deployment documentation treats this as an explicit local
    // development mode and requires tokens for shared/remote installations.
    if !state.authenticator.configured() {
        return next.run(request).await;
    }
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
    let Some(principal) = state.authenticator.authenticate(token) else {
        // The HTML shell contains no source data or controls. It is allowed to
        // load so the built-in console can present its login/bootstrap form;
        // every data/API operation remains protected.
        if matches!(request.uri().path(), "/admin" | "/admin/") {
            return next.run(request).await;
        }
        publish_security_event(
            &state,
            "AUTHENTICATION_FAILED",
            None,
            "Authentication failed",
            None,
            request
                .headers()
                .get("x-request-id")
                .and_then(|v| v.to_str().ok()),
        );
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"code":"AUTHENTICATION_REQUIRED","error":"Sign in is required to use Sentinel Streaming."})),
        ).into_response();
    };
    if let Some(authority) = required_authority(request.uri().path(), request.method()) {
        if !principal.allows(authority.clone()) {
            publish_security_event(
                &state,
                "AUTHORIZATION_DENIED",
                source_id_from_path(request.uri().path()),
                "The authenticated principal is not authorized for this operation.",
                Some(&principal),
                request
                    .headers()
                    .get("x-request-id")
                    .and_then(|v| v.to_str().ok()),
            );
            return (
                StatusCode::FORBIDDEN,
                Json(json!({"code":"AUTHORIZATION_DENIED","error":"Your account is not authorized for this operation."})),
            ).into_response();
        }
    }
    publish_security_event(
        &state,
        "AUTHENTICATION_SUCCEEDED",
        None,
        "Authentication succeeded",
        Some(&principal),
        request
            .headers()
            .get("x-request-id")
            .and_then(|v| v.to_str().ok()),
    );
    request.extensions_mut().insert(principal);
    next.run(request).await
}

fn source_id_from_path(path: &str) -> Option<String> {
    let mut parts = path.trim_matches('/').split('/');
    while let Some(part) = parts.next() {
        if part == "sources" {
            return parts.next().map(str::to_owned);
        }
    }
    None
}

fn publish_security_event(
    state: &AppState,
    event_type: &str,
    source_id: Option<String>,
    summary: &str,
    principal: Option<&Principal>,
    correlation_id: Option<&str>,
) {
    state.events.publish_record(EventRecord {
        id: String::new(),
        timestamp: crate::events::now_ms(),
        source_id,
        event_type: event_type.into(),
        provider: Some("auth".into()),
        summary: summary.into(),
        objects: Vec::new(),
        confidence: None,
        latency_ms: None,
        metadata: json!({
            "principal": principal.map(|p| &p.id),
            "role": principal.map(|p| &p.role),
            "correlationId": correlation_id,
            "outcome": if event_type == "AUTHENTICATION_FAILED" || event_type == "AUTHORIZATION_DENIED" { "failure" } else { "success" }
        }),
    });
}

fn required_authority(path: &str, method: &axum::http::Method) -> Option<Authority> {
    if path == "/admin" || path == "/admin/" {
        return Some(Authority::ViewSource);
    }
    if path == "/metrics"
        || path == "/api/v1/status"
        || path == "/api/v1/config"
        || path.starts_with("/api/v1/events")
        || path.starts_with("/api/v1/vision")
    {
        return Some(Authority::ViewDiagnostics);
    }
    if path == "/api/v1/auth/whoami" {
        return None;
    }
    if path.contains("/ptz") {
        return Some(Authority::ControlPtz);
    }
    if path.starts_with("/api/v1/onboarding")
        || path == "/api/v1/onvif/discover"
        || path.ends_with("/onvif/inspect")
    {
        return Some(Authority::RunOnboarding);
    }
    if path == "/api/v1/sources" && method == axum::http::Method::GET
        || path.ends_with("/capabilities")
    {
        return Some(Authority::ViewSource);
    }
    if path.contains("/playback") {
        return Some(if method == axum::http::Method::GET {
            Authority::ViewStream
        } else {
            Authority::ManageSource
        });
    }
    if path == "/api/v1/sources"
        || path.ends_with("/validate")
        || path.ends_with("/start")
        || path.ends_with("/stop")
        || path.ends_with("/restart")
        || path == "/api/v1/stop"
    {
        return Some(if method == axum::http::Method::GET {
            Authority::ViewSource
        } else {
            Authority::ManageSource
        });
    }
    Some(Authority::ViewSource)
}

async fn list_sources(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    Json(state.sources.list().await)
}

async fn admin_page() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        admin::INDEX,
    )
}
async fn source_providers() -> impl IntoResponse {
    Json(VideoSourceManager::providers())
}
async fn discover_sources(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    Json(state.sources.discover().await)
}
async fn discover_onvif(
    State(state): State<Arc<AppState>>,
    Json(request): Json<OnvifDiscoveryRequest>,
) -> impl IntoResponse {
    match state.sources.onvif_discover(request).await {
        Ok(devices) => (StatusCode::OK, Json(json!({"devices": devices}))).into_response(),
        Err(error) => manager_error(error).into_response(),
    }
}
async fn onboarding_discover(
    State(state): State<Arc<AppState>>,
    Json(request): Json<OnvifDiscoveryRequest>,
) -> impl IntoResponse {
    match state.sources.onboarding_discover(request).await {
        Ok(session) => (StatusCode::OK, Json(session)).into_response(),
        Err(error) => onboarding_error("discovery", error),
    }
}
async fn onboarding_session(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    match state.sources.onboarding_session(&id).await {
        Ok(session) => (StatusCode::OK, Json(session)).into_response(),
        Err(error) => onboarding_error("session", error),
    }
}
async fn onboarding_inspect(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
    Json(request): Json<OnboardingInspectRequest>,
) -> impl IntoResponse {
    match state.sources.onboarding_inspect(&id, request).await {
        Ok(session) => (StatusCode::OK, Json(session)).into_response(),
        Err(error) => onboarding_error("onvif_inspection", error),
    }
}
async fn onboarding_complete(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
    Json(request): Json<OnboardingCompleteRequest>,
) -> impl IntoResponse {
    match state.sources.onboarding_complete(&id, request).await {
        Ok(result) => (StatusCode::OK, Json(result)).into_response(),
        Err(error) => onboarding_error("setup", error),
    }
}
async fn test_source(
    State(state): State<Arc<AppState>>,
    Json(request): Json<ConnectionTestRequest>,
) -> impl IntoResponse {
    let result = state.sources.test_connection(request).await;
    let status = if result.success {
        StatusCode::OK
    } else {
        StatusCode::BAD_GATEWAY
    };
    (status, Json(result))
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
async fn validate_source(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    match state.sources.validate(&id).await {
        Ok(result) => (StatusCode::OK, Json(result)).into_response(),
        Err(error) => manager_error(error).into_response(),
    }
}
async fn inspect_onvif(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
    Json(request): Json<OnvifInspectRequest>,
) -> impl IntoResponse {
    match state.sources.inspect_onvif(&id, request).await {
        Ok(result) => (StatusCode::OK, Json(result)).into_response(),
        Err(error) => manager_error(error).into_response(),
    }
}
async fn source_capabilities(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    match state.sources.capabilities(&id).await {
        Ok(capabilities) => (
            StatusCode::OK,
            Json(json!({"cameraId": id, "capabilities": capabilities})),
        )
            .into_response(),
        Err(error) => manager_error(error).into_response(),
    }
}
async fn ptz_capabilities(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    match state.sources.ptz_capabilities(&id).await {
        Ok(capabilities) => (
            StatusCode::OK,
            Json(json!({"cameraId": id, "ptz": capabilities})),
        )
            .into_response(),
        Err(error) => manager_error(error).into_response(),
    }
}
fn ptz_audit_context(
    state: &AppState,
    headers: &HeaderMap,
    principal: Option<&Principal>,
) -> Result<PtzAuditContext, (StatusCode, Json<serde_json::Value>)> {
    let actor = state
        .ptz_authority
        .authorize(principal)
        .map_err(|message| {
            (
                StatusCode::FORBIDDEN,
                Json(json!({"code":"PTZ_AUTHORIZATION_DENIED","error": message})),
            )
        })?;
    let correlation_id = headers
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| {
            format!(
                "ptz-{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis()
            )
        });
    Ok(PtzAuditContext {
        actor,
        correlation_id,
    })
}
async fn ptz_move(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    principal: Option<axum::extract::Extension<Principal>>,
    Json(request): Json<PtzMoveRequest>,
) -> impl IntoResponse {
    let audit = match ptz_audit_context(&state, &headers, principal.as_ref().map(|p| &p.0)) {
        Ok(audit) => audit,
        Err(response) => return response.into_response(),
    };
    match state.sources.ptz_move(&id, request, audit).await {
        Ok(result) => (StatusCode::OK, Json(result)).into_response(),
        Err(error) => manager_error(error).into_response(),
    }
}
async fn ptz_stop(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    principal: Option<axum::extract::Extension<Principal>>,
) -> impl IntoResponse {
    let audit = match ptz_audit_context(&state, &headers, principal.as_ref().map(|p| &p.0)) {
        Ok(audit) => audit,
        Err(response) => return response.into_response(),
    };
    match state.sources.ptz_stop(&id, audit).await {
        Ok(result) => (StatusCode::OK, Json(result)).into_response(),
        Err(error) => manager_error(error).into_response(),
    }
}
async fn ptz_presets(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    principal: Option<axum::extract::Extension<Principal>>,
) -> impl IntoResponse {
    let audit = match ptz_audit_context(&state, &headers, principal.as_ref().map(|p| &p.0)) {
        Ok(audit) => audit,
        Err(response) => return response.into_response(),
    };
    match state.sources.ptz_presets(&id, audit).await {
        Ok(presets) => (
            StatusCode::OK,
            Json(json!({"cameraId": id, "presets": presets})),
        )
            .into_response(),
        Err(error) => manager_error(error).into_response(),
    }
}
async fn ptz_goto_preset(
    Path((id, preset_id)): Path<(String, String)>,
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    principal: Option<axum::extract::Extension<Principal>>,
) -> impl IntoResponse {
    let audit = match ptz_audit_context(&state, &headers, principal.as_ref().map(|p| &p.0)) {
        Ok(audit) => audit,
        Err(response) => return response.into_response(),
    };
    match state.sources.ptz_goto_preset(&id, &preset_id, audit).await {
        Ok(result) => (StatusCode::OK, Json(result)).into_response(),
        Err(error) => manager_error(error).into_response(),
    }
}
async fn source_playback(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    match state.sources.playback(&id).await {
        Ok(playback) => (StatusCode::OK, Json(playback)).into_response(),
        Err(error) => media_error(error).into_response(),
    }
}
async fn register_playback(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    match state.sources.register_playback(&id).await {
        Ok(playback) => (StatusCode::OK, Json(playback)).into_response(),
        Err(error) => media_error(error).into_response(),
    }
}
async fn remove_playback(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    match state.sources.remove_playback(&id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => media_error(error).into_response(),
    }
}
async fn media_gateway_health(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    Json(state.sources.media_gateway_health().await)
}
fn media_error(error: SourceManagerError) -> (StatusCode, Json<serde_json::Value>) {
    let status = match &error {
        SourceManagerError::NotFound => StatusCode::NOT_FOUND,
        SourceManagerError::InvalidRequest(_) => StatusCode::BAD_REQUEST,
        SourceManagerError::MediaGateway(_) => StatusCode::BAD_GATEWAY,
        _ => StatusCode::BAD_GATEWAY,
    };
    let message = error.to_string();
    let code = if message.contains("MEDIA_SOURCE_REGISTRATION_FAILED") {
        "MEDIA_SOURCE_REGISTRATION_FAILED"
    } else if message.contains("PLAYBACK_NOT_READY") {
        "PLAYBACK_NOT_READY"
    } else if message.contains("PLAYBACK_UNAVAILABLE") {
        "PLAYBACK_UNAVAILABLE"
    } else if message.contains("MEDIA_CONFIGURATION_ERROR") {
        "MEDIA_CONFIGURATION_ERROR"
    } else if message.contains("MEDIA_GATEWAY_PROTOCOL_ERROR") {
        "MEDIA_GATEWAY_PROTOCOL_ERROR"
    } else if matches!(error, SourceManagerError::MediaGateway(_)) {
        "MEDIA_GATEWAY_UNAVAILABLE"
    } else {
        "PLAYBACK_UNAVAILABLE"
    };
    (status, Json(json!({"code": code, "error": message})))
}
fn manager_error(error: SourceManagerError) -> (StatusCode, Json<serde_json::Value>) {
    let status = match &error {
        SourceManagerError::NotFound => StatusCode::NOT_FOUND,
        SourceManagerError::AlreadyExists => StatusCode::CONFLICT,
        SourceManagerError::Unsupported(_) => StatusCode::NOT_IMPLEMENTED,
        SourceManagerError::Camera(_) => StatusCode::BAD_GATEWAY,
        SourceManagerError::Onvif(_) => StatusCode::BAD_GATEWAY,
        SourceManagerError::MediaGateway(_) => StatusCode::BAD_GATEWAY,
        SourceManagerError::PtzNotSupported | SourceManagerError::PtzOperationUnsupported(_) => {
            StatusCode::UNPROCESSABLE_ENTITY
        }
        SourceManagerError::InvalidRequest(_) => StatusCode::BAD_REQUEST,
    };
    let code = match &error {
        SourceManagerError::PtzNotSupported => Some("PTZ_NOT_SUPPORTED"),
        SourceManagerError::PtzOperationUnsupported(_) => Some("PTZ_OPERATION_UNSUPPORTED"),
        SourceManagerError::InvalidRequest(_) => Some("INVALID_REQUEST"),
        _ => None,
    };
    (
        status,
        Json(json!({"code": code, "error": error.to_string()})),
    )
}
fn onboarding_error(stage: &str, error: SourceManagerError) -> Response {
    let raw = error.to_string();
    let (code, message) = if raw.contains("AUTHENTICATION_FAILED") {
        (
            "CREDENTIALS_REJECTED",
            "Camera found, but the supplied credentials were rejected.",
        )
    } else if raw.contains("DISCOVERY_TIMEOUT") {
        (
            "DISCOVERY_TIMEOUT",
            "No camera responded before discovery timed out.",
        )
    } else if raw.contains("MALFORMED_SOAP") {
        (
            "CAMERA_RESPONSE_INVALID",
            "Camera returned an unreadable ONVIF response.",
        )
    } else if raw.contains("DEVICE_UNREACHABLE") {
        ("CAMERA_UNREACHABLE", "Camera could not be reached.")
    } else if raw.contains("Already") || raw.contains("already") {
        (
            "SOURCE_ALREADY_EXISTS",
            "That camera is already configured.",
        )
    } else {
        ("ONBOARDING_FAILED", "Camera setup could not be completed.")
    };
    let status = match error {
        SourceManagerError::Onvif(_) => StatusCode::BAD_GATEWAY,
        SourceManagerError::InvalidRequest(_) => StatusCode::BAD_REQUEST,
        _ => StatusCode::BAD_REQUEST,
    };
    (
        status,
        Json(json!({
            "stage": stage,
            "code": code,
            "message": message,
            "details": raw,
        })),
    )
        .into_response()
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

#[cfg(test)]
mod security_tests {
    use super::*;

    #[test]
    fn route_authorities_keep_read_and_control_paths_distinct() {
        assert_eq!(
            required_authority("/api/v1/sources/camera", &axum::http::Method::GET),
            Some(Authority::ViewSource)
        );
        assert_eq!(
            required_authority("/api/v1/sources/camera/playback", &axum::http::Method::GET),
            Some(Authority::ViewStream)
        );
        assert_eq!(
            required_authority("/api/v1/sources/camera/ptz/move", &axum::http::Method::POST),
            Some(Authority::ControlPtz)
        );
        assert_eq!(
            required_authority("/api/v1/onboarding/discover", &axum::http::Method::POST),
            Some(Authority::RunOnboarding)
        );
    }
}
