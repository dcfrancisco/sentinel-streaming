use crate::frame::Frame;
use crate::preview::Preview;
use crate::{api::AppState, metrics::Metrics, sources::VideoSource};

pub trait FrameOutput: Send + Sync {
    fn accept(&self, frame: Frame);
}

/// Default sink for the MVP. Future cache, recording, and AI consumers attach here.
#[derive(Default)]
pub struct NullOutput;
impl FrameOutput for NullOutput {
    fn accept(&self, _frame: Frame) {}
}

pub struct PreviewOutput {
    preview: Preview,
}
impl PreviewOutput {
    pub fn new(preview: Preview) -> Self {
        Self { preview }
    }
}
impl FrameOutput for PreviewOutput {
    fn accept(&self, frame: Frame) {
        let mut bytes = Vec::new();
        let mut encoder = image::codecs::jpeg::JpegEncoder::new(&mut bytes);
        if encoder
            .encode(
                &frame.data,
                frame.width,
                frame.height,
                image::ExtendedColorType::Rgb8,
            )
            .is_ok()
        {
            self.preview.set(bytes);
        }
    }
}

pub struct Pipeline {
    metrics: Metrics,
    output: Box<dyn FrameOutput>,
}
impl Pipeline {
    pub fn new(metrics: Metrics, preview: Preview) -> Self {
        Self {
            metrics,
            output: Box::new(PreviewOutput::new(preview)),
        }
    }
    pub async fn run<S: VideoSource>(
        &self,
        mut source: S,
        state: AppState,
    ) -> Result<(), crate::errors::SourceError> {
        self.metrics.connected(1);
        loop {
            let frame = source.next_frame().await?;
            self.output.accept(frame);
            self.metrics.frame();
            state
                .health
                .ready
                .store(true, std::sync::atomic::Ordering::Relaxed);
        }
    }
}
