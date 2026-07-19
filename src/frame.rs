use image::{codecs::jpeg::JpegEncoder, ColorType, ImageEncoder};
use serde::Serialize;
use std::{sync::Arc, time::SystemTime};

#[derive(Clone, Debug, Serialize)]
pub struct Frame {
    pub sequence: u64,
    pub captured_at_ms: u128,
    pub width: u32,
    pub height: u32,
    #[serde(skip_serializing)]
    pub data: Arc<[u8]>,
}
impl Frame {
    /// Deterministic frame fixture used by hardware-independent tests.
    #[allow(dead_code)]
    pub fn blank(sequence: u64, width: u32, height: u32) -> Self {
        Self {
            sequence,
            captured_at_ms: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis(),
            width,
            height,
            data: Arc::from(vec![0; (width * height * 3) as usize]),
        }
    }

    pub fn from_rgb(sequence: u64, width: u32, height: u32, data: Vec<u8>) -> Self {
        Self {
            sequence,
            captured_at_ms: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis(),
            width,
            height,
            data: Arc::from(data),
        }
    }

    pub fn jpeg(&self, quality: u8) -> Result<Vec<u8>, image::ImageError> {
        let mut output = Vec::new();
        let encoder = JpegEncoder::new_with_quality(&mut output, quality);
        encoder.write_image(&self.data, self.width, self.height, ColorType::Rgb8.into())?;
        Ok(output)
    }
}
