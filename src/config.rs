use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct Config {
    pub bind: String,
    pub fps: u32,
    pub pipeline: PipelineConfig,
    pub buffer: BufferConfig,
    pub vision: VisionConfig,
}
#[derive(Clone, Debug, Serialize)]
pub struct VisionConfig {
    pub enabled: bool,
    pub interval_seconds: u64,
}
impl Default for VisionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            interval_seconds: 5,
        }
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct BufferConfig {
    pub enabled: bool,
    pub capacity: usize,
}
impl Default for BufferConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            capacity: 300,
        }
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct PipelineConfig {
    pub preview: bool,
    pub buffer: bool,
    pub recording: bool,
    pub vision: bool,
    pub events: bool,
}
impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            preview: true,
            buffer: true,
            recording: false,
            vision: false,
            events: false,
        }
    }
}
impl Default for Config {
    fn default() -> Self {
        Self {
            bind: "0.0.0.0:8080".into(),
            fps: 30,
            pipeline: PipelineConfig::default(),
            buffer: BufferConfig::default(),
            vision: VisionConfig::default(),
        }
    }
}
