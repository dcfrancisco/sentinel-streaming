use async_trait::async_trait;
use sentinel_streaming::{
    config::Config,
    events::EventBus,
    media::{
        MediaDeliveryHealth, MediaDeliveryState, MediaGateway, MediaGatewayFailure,
        MediaGatewayFailureCode, MediaGatewayStatus, MediaMtxAdapter, MediaSourceRegistration,
        MediaTelemetry, PlaybackInfo, PlaybackStream,
    },
    recovery::RecoveryEngine,
    rtsp::{RtspFailure, RtspValidationBackend, RtspValidationRequest, RtspValidator},
    sources::{AddSource, SourceOptions, StreamHealth, VideoSourceManager},
};
use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
    time::Duration,
};

#[derive(Clone)]
struct FakeGateway {
    registrations: Arc<Mutex<Vec<MediaSourceRegistration>>>,
    registered: Arc<Mutex<HashSet<String>>>,
    fail: bool,
    telemetry: Arc<Mutex<Option<MediaTelemetry>>>,
}

#[async_trait]
impl MediaGateway for FakeGateway {
    async fn register_source(
        &self,
        registration: MediaSourceRegistration,
    ) -> Result<(), MediaGatewayFailure> {
        if self.fail {
            return Err(MediaGatewayFailure {
                code: MediaGatewayFailureCode::MediaGatewayUnavailable,
                message: "fixture unavailable".into(),
                technical_detail: None,
            });
        }
        self.registered
            .lock()
            .unwrap()
            .insert(registration.source_id.clone());
        self.registrations.lock().unwrap().push(registration);
        Ok(())
    }
    async fn remove_source(&self, source_id: &str) -> Result<(), MediaGatewayFailure> {
        self.registered.lock().unwrap().remove(source_id);
        Ok(())
    }
    async fn playback(&self, source_id: &str) -> Result<PlaybackInfo, MediaGatewayFailure> {
        if !self.registered.lock().unwrap().contains(source_id) {
            return Err(MediaGatewayFailure {
                code: MediaGatewayFailureCode::PlaybackNotReady,
                message: "not registered".into(),
                technical_detail: None,
            });
        }
        Ok(PlaybackInfo {
            source_id: source_id.into(),
            available: true,
            streams: vec![
                PlaybackStream {
                    protocol: "webrtc".into(),
                    url: format!("https://playback.test/{source_id}/whep"),
                    latency_class: "low".into(),
                },
                PlaybackStream {
                    protocol: "hls".into(),
                    url: format!("https://playback.test/{source_id}/index.m3u8"),
                    latency_class: "standard".into(),
                },
            ],
            media_health: MediaDeliveryHealth::Healthy,
            failure: None,
        })
    }
    async fn health(&self) -> MediaGatewayStatus {
        MediaGatewayStatus {
            kind: "fixture".into(),
            health: if self.fail {
                MediaDeliveryHealth::Unavailable
            } else {
                MediaDeliveryHealth::Healthy
            },
            detail: None,
        }
    }
    async fn telemetry(&self, source_id: &str) -> Result<MediaTelemetry, MediaGatewayFailure> {
        if let Some(telemetry) = self.telemetry.lock().unwrap().clone() {
            return Ok(telemetry);
        }
        self.playback(source_id)
            .await
            .map(|playback| MediaTelemetry {
                source_id: source_id.into(),
                protocol: Some("webrtc".into()),
                codec: None,
                resolution: None,
                observed_fps: None,
                bitrate_bps: None,
                audio_present: None,
                audio_codec: None,
                audio_sample_rate: None,
                audio_channels: None,
                audio_bitrate_bps: None,
                audio_delivery_state: sentinel_streaming::media::AudioDeliveryState::Unknown,
                last_audio_activity: None,
                stream_started_at: None,
                last_media_activity: Some(1),
                reconnect_count: 0,
                delivery_state: MediaDeliveryState::Ready,
                playback_protocols: playback
                    .streams
                    .iter()
                    .map(|stream| stream.protocol.clone())
                    .collect(),
                gateway_state: playback.media_health,
                detail: None,
            })
    }
    async fn shutdown(&self) {
        self.registered.lock().unwrap().clear();
    }
}

struct SuccessfulRtsp;
#[async_trait]
impl RtspValidationBackend for SuccessfulRtsp {
    async fn validate(
        &self,
        _request: &RtspValidationRequest,
        _timeout: Duration,
    ) -> Result<(u16, u16), RtspFailure> {
        Ok((200, 200))
    }
}

async fn manager(gateway: Arc<FakeGateway>) -> VideoSourceManager {
    let events = EventBus::new(32);
    let (sender, receiver) = tokio::sync::watch::channel(false);
    std::mem::forget(sender);
    let manager = VideoSourceManager::new(
        30,
        events,
        Config::default().recovery.camera,
        receiver,
        RecoveryEngine::new(EventBus::new(32)),
    )
    .with_validator(RtspValidator::default().with_backend(Arc::new(SuccessfulRtsp)))
    .with_media_gateway(gateway);
    manager
        .add(AddSource {
            id: "front gate/1".into(),
            kind: "rtsp".into(),
            options: SourceOptions {
                uri: Some("rtsp://admin:secret@camera/stream".into()),
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
    manager
}

#[tokio::test]
async fn validated_source_registers_with_normalized_browser_playback() {
    let gateway = Arc::new(FakeGateway {
        registrations: Arc::new(Mutex::new(Vec::new())),
        registered: Arc::new(Mutex::new(HashSet::new())),
        fail: false,
        telemetry: Arc::new(Mutex::new(None)),
    });
    let manager = manager(gateway.clone()).await;
    manager.validate("front gate/1").await.unwrap();
    let playback = manager.register_playback("front gate/1").await.unwrap();
    assert!(playback.available);
    assert_eq!(playback.streams[0].protocol, "webrtc");
    assert_eq!(playback.streams[1].protocol, "hls");
    let registration = gateway.registrations.lock().unwrap().pop().unwrap();
    assert_eq!(registration.password.as_deref(), Some("secret"));
    let source = manager.get("front gate/1").await.unwrap();
    assert_eq!(source.health, StreamHealth::Healthy);
    assert_eq!(source.media_health, MediaDeliveryHealth::Healthy);
    let json = serde_json::to_string(&source).unwrap();
    assert!(!json.contains("secret"));
}

#[tokio::test]
async fn media_gateway_failure_does_not_make_camera_unhealthy() {
    let gateway = Arc::new(FakeGateway {
        registrations: Arc::new(Mutex::new(Vec::new())),
        registered: Arc::new(Mutex::new(HashSet::new())),
        fail: true,
        telemetry: Arc::new(Mutex::new(None)),
    });
    let manager = manager(gateway).await;
    manager.validate("front gate/1").await.unwrap();
    assert!(manager.register_playback("front gate/1").await.is_err());
    let source = manager.get("front gate/1").await.unwrap();
    assert_eq!(source.health, StreamHealth::Healthy);
    assert_eq!(source.media_health, MediaDeliveryHealth::Unavailable);
}

#[tokio::test]
async fn source_removal_and_gateway_shutdown_reconcile_registrations() {
    let gateway = Arc::new(FakeGateway {
        registrations: Arc::new(Mutex::new(Vec::new())),
        registered: Arc::new(Mutex::new(HashSet::new())),
        fail: false,
        telemetry: Arc::new(Mutex::new(None)),
    });
    let manager = manager(gateway.clone()).await;
    manager.validate("front gate/1").await.unwrap();
    manager.register_playback("front gate/1").await.unwrap();
    manager.remove_playback("front gate/1").await.unwrap();
    assert!(gateway.registered.lock().unwrap().is_empty());
    manager.register_playback("front gate/1").await.unwrap();
    manager.shutdown_media_gateway().await;
    assert!(gateway.registered.lock().unwrap().is_empty());
}

#[tokio::test]
async fn disabled_mediamtx_reports_unavailable_without_exposing_admin_urls() {
    let adapter = MediaMtxAdapter::new(
        false,
        Some("http://admin:secret@127.0.0.1:9997".into()),
        Some("http://127.0.0.1:8889".into()),
        None,
        None,
        Duration::from_millis(10),
    );
    let health = adapter.health().await;
    assert_eq!(health.health, MediaDeliveryHealth::Unavailable);
    let json = serde_json::to_string(&health).unwrap();
    assert!(!json.contains("9997"));
    assert!(!json.contains("secret"));
    assert!(MediaMtxAdapter::path_for_source("../camera secret")
        .chars()
        .all(|character| character.is_ascii_alphanumeric()
            || character == '-'
            || character == '_'));
}

#[tokio::test]
async fn media_supervisor_keeps_source_health_separate_from_media_failure() {
    let gateway = Arc::new(FakeGateway {
        registrations: Arc::new(Mutex::new(Vec::new())),
        registered: Arc::new(Mutex::new(HashSet::new())),
        fail: false,
        telemetry: Arc::new(Mutex::new(Some(MediaTelemetry {
            source_id: "front gate/1".into(),
            protocol: None,
            codec: None,
            resolution: None,
            observed_fps: None,
            bitrate_bps: None,
            audio_present: None,
            audio_codec: None,
            audio_sample_rate: None,
            audio_channels: None,
            audio_bitrate_bps: None,
            audio_delivery_state: sentinel_streaming::media::AudioDeliveryState::Unknown,
            last_audio_activity: None,
            stream_started_at: None,
            last_media_activity: None,
            reconnect_count: 2,
            delivery_state: MediaDeliveryState::Unavailable,
            playback_protocols: vec!["webrtc".into(), "hls".into()],
            gateway_state: MediaDeliveryHealth::Unavailable,
            detail: Some("Media gateway is unavailable.".into()),
        }))),
    });
    let manager = manager(gateway.clone()).await;
    manager.validate("front gate/1").await.unwrap();
    manager.register_playback("front gate/1").await.unwrap();
    manager.supervise_media_once().await;
    let source = manager.get("front gate/1").await.unwrap();
    assert_eq!(source.health, StreamHealth::Healthy);
    assert_eq!(source.media_health, MediaDeliveryHealth::Unavailable);
    assert_eq!(source.media_telemetry.reconnect_count, 2);
}

#[tokio::test]
async fn media_supervisor_marks_stale_ready_media_as_stalled() {
    let gateway = Arc::new(FakeGateway {
        registrations: Arc::new(Mutex::new(Vec::new())),
        registered: Arc::new(Mutex::new(HashSet::new())),
        fail: false,
        telemetry: Arc::new(Mutex::new(Some(MediaTelemetry {
            source_id: "front gate/1".into(),
            protocol: Some("webrtc".into()),
            codec: Some("H264".into()),
            resolution: Some("1920x1080".into()),
            observed_fps: Some(30.0),
            bitrate_bps: Some(4_000_000),
            audio_present: Some(false),
            audio_codec: None,
            audio_sample_rate: None,
            audio_channels: None,
            audio_bitrate_bps: None,
            audio_delivery_state: sentinel_streaming::media::AudioDeliveryState::Unsupported,
            last_audio_activity: None,
            stream_started_at: None,
            last_media_activity: Some(sentinel_streaming::events::now_ms() - 10_000),
            reconnect_count: 1,
            delivery_state: MediaDeliveryState::Ready,
            playback_protocols: vec!["webrtc".into()],
            gateway_state: MediaDeliveryHealth::Healthy,
            detail: None,
        }))),
    });
    let manager = manager(gateway).await.with_media_supervision_config(
        sentinel_streaming::config::MediaSupervisionConfig {
            enabled: true,
            interval_ms: 100,
            stall_timeout_ms: 10,
            startup_timeout_ms: 100,
        },
    );
    manager.validate("front gate/1").await.unwrap();
    manager.register_playback("front gate/1").await.unwrap();
    manager.supervise_media_once().await;
    let source = manager.get("front gate/1").await.unwrap();
    assert_eq!(
        source.media_telemetry.delivery_state,
        MediaDeliveryState::Stalled
    );
    assert_eq!(source.health, StreamHealth::Healthy);
}

#[tokio::test]
async fn media_supervisor_allows_only_one_runtime_loop() {
    let gateway = Arc::new(FakeGateway {
        registrations: Arc::new(Mutex::new(Vec::new())),
        registered: Arc::new(Mutex::new(HashSet::new())),
        fail: false,
        telemetry: Arc::new(Mutex::new(None)),
    });
    let manager = manager(gateway).await;
    let first = manager.spawn_media_supervisor();
    assert!(first.is_some());
    assert!(manager.spawn_media_supervisor().is_none());
    first.unwrap().abort();
}
