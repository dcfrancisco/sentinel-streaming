use crate::{errors::PipelineError, frame::Frame, frame_buffer::FrameBuffer, preview::Preview};

pub struct PreviewStage {
    preview: Preview,
    buffer: FrameBuffer,
}
impl PreviewStage {
    pub fn new(preview: Preview, buffer: FrameBuffer) -> Self {
        Self { preview, buffer }
    }
}
impl super::pipeline::PipelineStage for PreviewStage {
    fn name(&self) -> &'static str {
        "preview"
    }
    fn process(&mut self, _frame: &mut Frame) -> Result<(), PipelineError> {
        let frame = self
            .buffer
            .latest()
            .ok_or_else(|| PipelineError("preview has no buffered frame".into()))?;
        let mut bytes = Vec::new();
        let mut encoder = image::codecs::jpeg::JpegEncoder::new(&mut bytes);
        encoder
            .encode(
                frame.data.as_ref(),
                frame.width,
                frame.height,
                image::ExtendedColorType::Rgb8,
            )
            .map_err(|error| PipelineError(error.to_string()))?;
        self.preview.set(bytes);
        Ok(())
    }
}

pub struct BufferStage {
    buffer: FrameBuffer,
}
impl BufferStage {
    pub fn new(buffer: FrameBuffer) -> Self {
        Self { buffer }
    }
}
impl super::pipeline::PipelineStage for BufferStage {
    fn name(&self) -> &'static str {
        "buffer"
    }
    fn process(&mut self, frame: &mut Frame) -> Result<(), PipelineError> {
        self.buffer.push(frame.clone());
        Ok(())
    }
}
pub struct RecordingStage;
impl super::pipeline::PipelineStage for RecordingStage {
    fn name(&self) -> &'static str {
        "recording"
    }
    fn process(&mut self, _frame: &mut Frame) -> Result<(), PipelineError> {
        Ok(())
    }
}
pub struct VisionStage;
impl super::pipeline::PipelineStage for VisionStage {
    fn name(&self) -> &'static str {
        "vision"
    }
    fn process(&mut self, _frame: &mut Frame) -> Result<(), PipelineError> {
        Ok(())
    }
}
pub struct EventPublisherStage;
impl super::pipeline::PipelineStage for EventPublisherStage {
    fn name(&self) -> &'static str {
        "events"
    }
    fn process(&mut self, _frame: &mut Frame) -> Result<(), PipelineError> {
        Ok(())
    }
}
