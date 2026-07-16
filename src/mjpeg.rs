use crate::frame_buffer::FrameBuffer;
use axum::body::Bytes;
use image::codecs::jpeg::JpegEncoder;
use std::{
    io,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Instant,
};

#[derive(Clone)]
pub struct MjpegMetrics {
    viewers: Arc<AtomicU64>,
    frames: Arc<AtomicU64>,
    bytes: Arc<AtomicU64>,
    errors: Arc<AtomicU64>,
    started: Arc<Instant>,
}
impl Default for MjpegMetrics {
    fn default() -> Self {
        Self {
            viewers: Arc::new(AtomicU64::new(0)),
            frames: Arc::new(AtomicU64::new(0)),
            bytes: Arc::new(AtomicU64::new(0)),
            errors: Arc::new(AtomicU64::new(0)),
            started: Arc::new(Instant::now()),
        }
    }
}
impl MjpegMetrics {
    pub fn viewer_count(&self) -> u64 {
        self.viewers.load(Ordering::Relaxed)
    }
    pub fn prometheus(&self) -> String {
        let frames = self.frames.load(Ordering::Relaxed);
        let fps = frames as f64 / self.started.elapsed().as_secs_f64().max(1.0);
        format!("sentinel_mjpeg_connected_viewers {}\nsentinel_mjpeg_frames_streamed {}\nsentinel_mjpeg_bytes_transmitted {}\nsentinel_mjpeg_stream_errors {}\nsentinel_mjpeg_average_fps {}\n", self.viewer_count(), frames, self.bytes.load(Ordering::Relaxed), self.errors.load(Ordering::Relaxed), fps)
    }
}

#[derive(Clone)]
pub struct MjpegStream {
    buffer: FrameBuffer,
    pub metrics: MjpegMetrics,
}
impl MjpegStream {
    pub fn new(buffer: FrameBuffer) -> Self {
        Self {
            buffer,
            metrics: MjpegMetrics {
                started: Arc::new(Instant::now()),
                ..MjpegMetrics::default()
            },
        }
    }
    pub fn stream(
        &self,
        source_id: String,
    ) -> impl tokio_stream::Stream<Item = Result<Bytes, io::Error>> + Send + 'static {
        let buffer = self.buffer.clone();
        let metrics = self.metrics.clone();
        async_stream::stream! {
            metrics.viewers.fetch_add(1, Ordering::Relaxed);
            tracing::info!(source=%source_id, viewers=metrics.viewer_count(), "MJPEG client connected");
            let _guard = ViewerGuard { metrics: metrics.clone(), source_id: source_id.clone() };
            let mut last_sequence = 0;
            loop {
                let Some(frame) = buffer.latest() else { tokio::time::sleep(std::time::Duration::from_millis(50)).await; continue; };
                if frame.sequence == last_sequence { tokio::time::sleep(std::time::Duration::from_millis(20)).await; continue; }
                let mut jpeg = Vec::new(); let mut encoder = JpegEncoder::new(&mut jpeg);
                if let Err(error) = encoder.encode(frame.data.as_ref(), frame.width, frame.height, image::ExtendedColorType::Rgb8) { metrics.errors.fetch_add(1, Ordering::Relaxed); tracing::warn!(source=%source_id, error=%error, "MJPEG encoding error"); tokio::time::sleep(std::time::Duration::from_millis(100)).await; continue; }
                last_sequence = frame.sequence;
                let mut multipart = format!("--frame\r\nContent-Type: image/jpeg\r\nContent-Length: {}\r\n\r\n", jpeg.len()).into_bytes(); multipart.extend_from_slice(&jpeg); multipart.extend_from_slice(b"\r\n");
                metrics.frames.fetch_add(1, Ordering::Relaxed); metrics.bytes.fetch_add(multipart.len() as u64, Ordering::Relaxed);
                yield Ok(Bytes::from(multipart));
            }
        }
    }
}
struct ViewerGuard {
    metrics: MjpegMetrics,
    source_id: String,
}
impl Drop for ViewerGuard {
    fn drop(&mut self) {
        self.metrics.viewers.fetch_sub(1, Ordering::Relaxed);
        tracing::info!(source=%self.source_id, viewers=self.metrics.viewer_count(), "MJPEG client disconnected");
    }
}
