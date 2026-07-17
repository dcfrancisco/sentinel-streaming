use async_trait::async_trait;
use axum::{body::Body, http::Request};
use sentinel_streaming::{
    api::{self, AppState},
    config::Config,
    events::{EventRecord, EventStore},
    frame::Frame,
    frame_buffer::FrameBuffer,
    mjpeg::MjpegStream,
    sources::{AddSource, SourceOptions, SyntheticSource, VideoFileSource, VideoSourceManager},
    vision::{
        FrameSelector, SceneAnalysis, VisionJob, VisionMetrics, VisionProvider, VisionScheduler,
        VisionState,
    },
};
use std::sync::Arc;
use tokio_stream::StreamExt;
use tower::ServiceExt;

fn app_state() -> (AppState, tokio::sync::watch::Sender<bool>) {
    let (shutdown, receiver) = tokio::sync::watch::channel(false);
    let state = AppState::new(
        Config::default(),
        FrameBuffer::new(4),
        shutdown.clone(),
        receiver,
    );
    (state, shutdown)
}

#[tokio::test]
async fn health_and_version_routes_start_without_hardware() {
    let (state, _) = app_state();
    let router = api::router(Arc::new(state));
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/health/live")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let response = router
        .oneshot(
            Request::builder()
                .uri("/api/v1/version")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
}

#[tokio::test]
async fn mjpeg_emits_valid_multipart_frame() {
    let buffer = FrameBuffer::new(4);
    buffer.push(Frame::blank(1, 2, 2));
    let mut stream = Box::pin(MjpegStream::new(buffer).stream("synthetic".into()));
    let chunk = stream.next().await.unwrap().unwrap();
    let text = String::from_utf8_lossy(&chunk);
    assert!(text.starts_with("--frame\r\nContent-Type: image/jpeg\r\nContent-Length: "));
    assert!(chunk.ends_with(b"\r\n"));
}

#[tokio::test]
async fn event_store_is_bounded_and_ordered() {
    let store = EventStore::new(2);
    let first = store.push(EventRecord::simple("one", None, "first")).await;
    store.push(EventRecord::simple("two", None, "second")).await;
    store
        .push(EventRecord::simple("three", None, "third"))
        .await;
    assert!(store.get(&first.id).await.is_none());
    let recent = store.recent(2).await;
    assert_eq!(recent.len(), 2);
    assert_eq!(recent[0].event_type, "three");
}

struct MockVisionProvider;
#[async_trait]
impl VisionProvider for MockVisionProvider {
    fn name(&self) -> &'static str {
        "mock"
    }
    async fn analyze(&self, frames: Vec<Arc<Frame>>) -> Result<SceneAnalysis, String> {
        Ok(SceneAnalysis {
            summary: format!("{} synthetic frame(s)", frames.len()),
            ..Default::default()
        })
    }
}

#[tokio::test]
async fn mock_vision_updates_latest_analysis_and_events() {
    let buffer = FrameBuffer::new(4);
    buffer.push(Frame::blank(1, 2, 2));
    let state = VisionState::default();
    let events = sentinel_streaming::events::EventBus::new(8);
    let (shutdown, receiver) = tokio::sync::watch::channel(false);
    let task = VisionScheduler::spawn_with_provider(VisionJob {
        buffer,
        state: state.clone(),
        metrics: VisionMetrics::default(),
        selector: FrameSelector::new(1, 1),
        interval_seconds: 1,
        shutdown: receiver,
        events: events.clone(),
        provider: Arc::new(MockVisionProvider),
        recovery: sentinel_streaming::recovery::RecoveryEngine::new(events.clone()),
    });
    for _ in 0..20 {
        if state.latest().await.is_some() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert!(state.latest().await.is_some());
    shutdown.send(true).unwrap();
    task.await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    assert!(events.store().latest().await.is_some());
}

#[test]
fn synthetic_source_generates_configured_frames() {
    let mut source = SyntheticSource::new(32, 18, 10);
    let first = source.next_frame_sync();
    let second = source.next_frame_sync();
    assert_eq!((first.width, first.height), (32, 18));
    assert_eq!(first.data.len(), 32 * 18 * 3);
    assert_eq!(second.sequence, first.sequence + 1);
}

#[test]
fn video_file_source_loops_image_frames() {
    let path = std::env::temp_dir().join(format!("sentinel-frame-{}.jpg", std::process::id()));
    let image = image::RgbImage::from_pixel(4, 3, image::Rgb([12, 34, 56]));
    image.save(&path).unwrap();
    let mut source = VideoFileSource::open(&path, true, 5).unwrap();
    assert_eq!(source.next_frame_sync().unwrap().width, 4);
    assert_eq!(source.next_frame_sync().unwrap().sequence, 1);
    std::fs::remove_file(path).unwrap();
}

#[tokio::test]
async fn manager_starts_synthetic_source_without_hardware() {
    let events = sentinel_streaming::events::EventBus::new(8);
    let (shutdown, receiver) = tokio::sync::watch::channel(false);
    let recovery = sentinel_streaming::recovery::RecoveryEngine::new(events.clone());
    let manager = VideoSourceManager::new(
        10,
        events,
        Config::default().recovery.camera,
        receiver,
        recovery,
    );
    manager
        .add(AddSource {
            id: "synthetic".into(),
            kind: "synthetic".into(),
            options: SourceOptions {
                width: Some(16),
                height: Some(12),
                fps: Some(10),
                ..Default::default()
            },
        })
        .await
        .unwrap();
    manager.start("synthetic").await.unwrap();
    let mut provider = manager.clone();
    let frame = sentinel_streaming::sources::FrameProvider::next_frame(&mut provider)
        .await
        .unwrap();
    assert_eq!((frame.width, frame.height), (16, 12));
    manager.stop("synthetic").await.unwrap();
    shutdown.send(true).unwrap();
}
