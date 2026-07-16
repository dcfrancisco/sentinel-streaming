use crate::{
    api::AppState, config::PipelineConfig, errors::PipelineError, frame::Frame,
    frame_buffer::FrameBuffer, metrics::Metrics, preview::Preview, sources::FrameProvider,
};

pub trait PipelineStage: Send {
    fn name(&self) -> &'static str;
    fn process(&mut self, frame: &mut Frame) -> Result<(), PipelineError>;
}

pub struct Pipeline {
    metrics: Metrics,
    stages: Vec<Box<dyn PipelineStage>>,
}
impl Pipeline {
    pub fn new(
        metrics: Metrics,
        preview: Preview,
        buffer: FrameBuffer,
        config: PipelineConfig,
    ) -> Self {
        let mut stages: Vec<Box<dyn PipelineStage>> = Vec::new();
        if config.buffer {
            stages.push(Box::new(crate::stages::BufferStage::new(buffer.clone())));
        }
        if config.preview {
            stages.push(Box::new(crate::stages::PreviewStage::new(preview, buffer)));
        }
        if config.recording {
            stages.push(Box::new(crate::stages::RecordingStage));
        }
        if config.vision {
            stages.push(Box::new(crate::stages::VisionStage));
        }
        if config.events {
            stages.push(Box::new(crate::stages::EventPublisherStage));
        }
        tracing::info!(stages = ?stages.iter().map(|stage| stage.name()).collect::<Vec<_>>(), "processing pipeline configured");
        Self { metrics, stages }
    }
    pub async fn run<S: FrameProvider>(
        &mut self,
        mut source: S,
        state: AppState,
        mut shutdown: tokio::sync::watch::Receiver<bool>,
    ) -> Result<(), PipelineError> {
        self.metrics.connected(1);
        loop {
            let mut frame = tokio::select! {
                changed = shutdown.changed() => {
                    if changed.is_err() || *shutdown.borrow() { break; }
                    continue;
                }
                frame = source.next_frame() => frame.map_err(|error| PipelineError(error.to_string()))?,
            };
            for stage in &mut self.stages {
                stage.process(&mut frame)?;
            }
            self.metrics.frame();
            state
                .health
                .ready
                .store(true, std::sync::atomic::Ordering::Relaxed);
        }
        Ok(())
    }
}
