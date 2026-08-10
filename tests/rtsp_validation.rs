use async_trait::async_trait;
use axum::{body::Body, http::Request};
use sentinel_streaming::{
    api::{self, AppState},
    config::{Config, HealthConfig},
    events::EventBus,
    frame_buffer::FrameBuffer,
    recovery::RecoveryEngine,
    rtsp::{
        RtspFailure, RtspFailureCode, RtspValidationBackend, RtspValidationRequest, RtspValidator,
    },
    sources::{
        AddSource, RecoveryState, SourceOptions, StreamHealth, ValidationState, VideoSourceManager,
    },
};
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tower::ServiceExt;

#[derive(Clone, Copy)]
enum BackendResult {
    Success,
    Auth,
    Missing,
    Timeout,
    Unreachable,
}

struct FakeBackend(BackendResult);

fn failure(result: BackendResult) -> Result<(u16, u16), RtspFailure> {
    match result {
        BackendResult::Success => Ok((200, 200)),
        BackendResult::Auth => Err(RtspFailure {
            code: RtspFailureCode::AuthenticationFailed,
            message: "The camera rejected the supplied credentials.".into(),
            technical_detail: Some("OPTIONS returned RTSP status 401".into()),
        }),
        BackendResult::Missing => Err(RtspFailure {
            code: RtspFailureCode::StreamNotFound,
            message: "The RTSP stream was not found.".into(),
            technical_detail: Some("DESCRIBE returned RTSP status 404".into()),
        }),
        BackendResult::Timeout => Err(RtspFailure {
            code: RtspFailureCode::ConnectionTimeout,
            message: "The RTSP connection timed out.".into(),
            technical_detail: None,
        }),
        BackendResult::Unreachable => Err(RtspFailure {
            code: RtspFailureCode::SourceUnreachable,
            message: "The RTSP source could not be reached.".into(),
            technical_detail: Some("connection refused".into()),
        }),
    }
}

#[async_trait]
impl RtspValidationBackend for FakeBackend {
    async fn validate(
        &self,
        _request: &RtspValidationRequest,
        _timeout: Duration,
    ) -> Result<(u16, u16), RtspFailure> {
        failure(self.0)
    }
}

struct SequenceBackend {
    results: Mutex<Vec<BackendResult>>,
    calls: AtomicUsize,
    active: AtomicUsize,
    max_active: AtomicUsize,
    delay: Duration,
}

#[async_trait]
impl RtspValidationBackend for SequenceBackend {
    async fn validate(
        &self,
        _request: &RtspValidationRequest,
        _timeout: Duration,
    ) -> Result<(u16, u16), RtspFailure> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
        self.max_active.fetch_max(active, Ordering::SeqCst);
        tokio::time::sleep(self.delay).await;
        let result = self
            .results
            .lock()
            .unwrap()
            .pop()
            .unwrap_or(BackendResult::Success);
        self.active.fetch_sub(1, Ordering::SeqCst);
        failure(result)
    }
}

fn validator(result: BackendResult) -> RtspValidator {
    RtspValidator::default().with_backend(Arc::new(FakeBackend(result)))
}

fn manager(timeout: Duration, result: BackendResult) -> VideoSourceManager {
    let events = EventBus::new(32);
    let (_, receiver) = tokio::sync::watch::channel(false);
    VideoSourceManager::new(
        30,
        events.clone(),
        Config::default().recovery.camera,
        receiver,
        RecoveryEngine::new(events),
    )
    .with_validation_timeout(timeout)
    .with_validator(validator(result))
}

#[tokio::test]
async fn protocol_success_validates_options_and_describe() {
    let result = validator(BackendResult::Success)
        .validate(RtspValidationRequest {
            uri: "rtsp://camera/stream1".into(),
            username: None,
            password: None,
        })
        .await;
    assert!(result.valid);
    assert_eq!(result.details.options_status, Some(200));
    assert_eq!(result.details.describe_status, Some(200));
}

#[tokio::test]
async fn protocol_authentication_failure_is_normalized() {
    let result = validator(BackendResult::Auth)
        .validate(RtspValidationRequest {
            uri: "rtsp://camera/stream1".into(),
            username: Some("admin".into()),
            password: Some("secret".into()),
        })
        .await;
    assert_eq!(
        result.failure.as_ref().map(|failure| &failure.code),
        Some(&RtspFailureCode::AuthenticationFailed)
    );
}

#[tokio::test]
async fn protocol_missing_stream_is_normalized() {
    let result = validator(BackendResult::Missing)
        .validate(RtspValidationRequest {
            uri: "rtsp://camera/stream1".into(),
            username: None,
            password: None,
        })
        .await;
    assert_eq!(
        result.failure.as_ref().map(|failure| &failure.code),
        Some(&RtspFailureCode::StreamNotFound)
    );
}

#[tokio::test]
async fn timeout_is_bounded() {
    let result = validator(BackendResult::Timeout)
        .validate(RtspValidationRequest {
            uri: "rtsp://camera/stream1".into(),
            username: None,
            password: None,
        })
        .await;
    assert_eq!(
        result.failure.as_ref().map(|failure| &failure.code),
        Some(&RtspFailureCode::ConnectionTimeout)
    );
}

#[tokio::test]
async fn unreachable_source_is_normalized() {
    let result = validator(BackendResult::Unreachable)
        .validate(RtspValidationRequest {
            uri: "rtsp://camera/stream1".into(),
            username: None,
            password: None,
        })
        .await;
    assert_eq!(
        result.failure.as_ref().map(|failure| &failure.code),
        Some(&RtspFailureCode::SourceUnreachable)
    );
}

#[tokio::test]
async fn malformed_source_is_rejected_without_network_access() {
    let result = RtspValidator::default()
        .validate(RtspValidationRequest {
            uri: "http://camera/stream".into(),
            username: None,
            password: None,
        })
        .await;
    assert_eq!(
        result.failure.as_ref().map(|failure| &failure.code),
        Some(&RtspFailureCode::InvalidSource)
    );
}

#[tokio::test]
async fn manager_updates_validation_lifecycle_and_health() {
    let manager = manager(Duration::from_secs(1), BackendResult::Success);
    manager
        .add(AddSource {
            id: "front-gate".into(),
            kind: "rtsp".into(),
            options: SourceOptions {
                uri: Some("rtsp://camera/stream1".into()),
                ..Default::default()
            },
        })
        .await
        .unwrap();
    let result = manager.validate("front-gate").await.unwrap();
    assert!(result.valid);
    let source = manager.get("front-gate").await.unwrap();
    assert_eq!(source.validation, ValidationState::Validated);
    assert_eq!(source.health, StreamHealth::Healthy);
    assert_eq!(source.consecutive_validation_failures, 0);
}

#[tokio::test]
async fn manager_failure_is_visible_without_leaking_credentials() {
    let manager = manager(Duration::from_secs(1), BackendResult::Auth);
    let source = manager
        .add(AddSource {
            id: "secure-camera".into(),
            kind: "rtsp".into(),
            options: SourceOptions {
                uri: Some("rtsp://admin:secret@camera/stream1".into()),
                credentials: Some(sentinel_streaming::sources::RtspCredentials {
                    username: Some("admin".into()),
                    password: Some("secret".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
        })
        .await
        .unwrap();
    let output = serde_json::to_string(&source).unwrap();
    assert!(!output.contains("secret"));
    let result = manager.validate("secure-camera").await.unwrap();
    assert!(!result.valid);
    let source = manager.get("secure-camera").await.unwrap();
    assert_eq!(source.validation, ValidationState::Failed);
    assert_eq!(source.health, StreamHealth::Unhealthy);
}

fn recovery_manager(
    backend: Arc<SequenceBackend>,
    max_attempts: u32,
    concurrency: usize,
) -> VideoSourceManager {
    let events = EventBus::new(64);
    let (sender, receiver) = tokio::sync::watch::channel(false);
    std::mem::forget(sender);
    VideoSourceManager::new(
        30,
        events.clone(),
        Config::default().recovery.camera,
        receiver,
        RecoveryEngine::new(events),
    )
    .with_validator(RtspValidator::default().with_backend(backend))
    .with_health_config(HealthConfig {
        enabled: true,
        interval_seconds: 1,
        max_concurrent_checks: concurrency,
        max_attempts,
        initial_backoff_ms: 1,
        max_backoff_seconds: 1,
    })
}

async fn add_rtsp(manager: &VideoSourceManager, id: &str) {
    manager
        .add(AddSource {
            id: id.into(),
            kind: "rtsp".into(),
            options: SourceOptions {
                uri: Some(format!("rtsp://camera/{id}")),
                ..Default::default()
            },
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn health_monitor_recovers_after_bounded_retry() {
    let backend = Arc::new(SequenceBackend {
        results: Mutex::new(vec![BackendResult::Success, BackendResult::Unreachable]),
        calls: AtomicUsize::new(0),
        active: AtomicUsize::new(0),
        max_active: AtomicUsize::new(0),
        delay: Duration::ZERO,
    });
    let manager = recovery_manager(backend.clone(), 2, 1);
    add_rtsp(&manager, "recovering").await;
    manager.monitor_once().await;
    let source = manager.get("recovering").await.unwrap();
    assert_eq!(source.health, StreamHealth::Healthy);
    assert_eq!(source.recovery, RecoveryState::Idle);
    assert_eq!(source.recovery_attempts, 0);
    assert_eq!(backend.calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn authentication_failure_is_not_retried() {
    let backend = Arc::new(SequenceBackend {
        results: Mutex::new(vec![BackendResult::Auth]),
        calls: AtomicUsize::new(0),
        active: AtomicUsize::new(0),
        max_active: AtomicUsize::new(0),
        delay: Duration::ZERO,
    });
    let manager = recovery_manager(backend.clone(), 3, 1);
    add_rtsp(&manager, "auth-failure").await;
    manager.monitor_once().await;
    let source = manager.get("auth-failure").await.unwrap();
    assert_eq!(source.recovery, RecoveryState::Exhausted);
    assert_eq!(source.recovery_attempts, 0);
    assert_eq!(backend.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn health_checks_are_bounded_concurrently() {
    let backend = Arc::new(SequenceBackend {
        results: Mutex::new(vec![BackendResult::Success; 4]),
        calls: AtomicUsize::new(0),
        active: AtomicUsize::new(0),
        max_active: AtomicUsize::new(0),
        delay: Duration::from_millis(5),
    });
    let manager = recovery_manager(backend.clone(), 0, 2);
    for id in ["one", "two", "three", "four"] {
        add_rtsp(&manager, id).await;
    }
    manager.monitor_once().await;
    assert_eq!(backend.calls.load(Ordering::SeqCst), 4);
    assert!(backend.max_active.load(Ordering::SeqCst) <= 2);
}

#[tokio::test]
async fn api_exposes_validation_and_admin_page() {
    let (shutdown, receiver) = tokio::sync::watch::channel(false);
    let state = AppState::new(Config::default(), FrameBuffer::new(4), shutdown, receiver);
    state
        .sources
        .add(AddSource {
            id: "api-camera".into(),
            kind: "rtsp".into(),
            options: SourceOptions {
                uri: Some("not-an-rtsp-uri".into()),
                ..Default::default()
            },
        })
        .await
        .unwrap();
    let router = api::router(Arc::new(state));
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/sources/api-camera/validate")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let response = router
        .oneshot(
            Request::builder()
                .uri("/admin")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
}
