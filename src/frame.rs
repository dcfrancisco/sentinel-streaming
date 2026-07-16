use serde::Serialize;
use std::time::SystemTime;

#[derive(Clone, Debug, Serialize)]
pub struct Frame {
    pub sequence: u64,
    pub captured_at_ms: u128,
    pub width: u32,
    pub height: u32,
    #[serde(skip_serializing)]
    pub data: Vec<u8>,
}
impl Frame {
    pub fn blank(sequence: u64, width: u32, height: u32) -> Self {
        Self {
            sequence,
            captured_at_ms: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis(),
            width,
            height,
            data: vec![0; (width * height * 3) as usize],
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
            data,
        }
    }
}
