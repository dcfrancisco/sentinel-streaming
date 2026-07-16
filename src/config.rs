use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct Config {
    pub bind: String,
    pub fps: u32,
    pub recovery: RecoveryConfig,
    pub pipeline: PipelineConfig,
    pub buffer: BufferConfig,
    pub vision: VisionConfig,
    pub events: EventsConfig,
}
#[derive(Clone, Debug, Serialize)]
pub struct ReconnectConfig {
    pub enabled: bool,
    pub retry_forever: bool,
    pub initial_delay_ms: u64,
    pub max_delay_seconds: u64,
    pub jitter: bool,
}
impl Default for ReconnectConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            retry_forever: true,
            initial_delay_ms: 500,
            max_delay_seconds: 30,
            jitter: true,
        }
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct VisionRecoveryConfig {
    pub enabled: bool,
    pub retry_count: u32,
    pub cooldown_seconds: u64,
}
impl Default for VisionRecoveryConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            retry_count: 3,
            cooldown_seconds: 30,
        }
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct MjpegRecoveryConfig {
    pub cleanup_interval_seconds: u64,
}
impl Default for MjpegRecoveryConfig {
    fn default() -> Self {
        Self {
            cleanup_interval_seconds: 30,
        }
    }
}
#[derive(Clone, Debug, Default, Serialize)]
pub struct RecoveryConfig {
    pub camera: ReconnectConfig,
    pub vision: VisionRecoveryConfig,
    pub mjpeg: MjpegRecoveryConfig,
}
#[derive(Clone, Debug, Serialize)]
pub struct EventsConfig {
    pub capacity: usize,
}
impl Default for EventsConfig {
    fn default() -> Self {
        Self { capacity: 1000 }
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct VisionConfig {
    pub enabled: bool,
    pub interval_seconds: u64,
    pub frames: usize,
    pub spacing_seconds: u64,
}
impl Default for VisionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            interval_seconds: 5,
            frames: 5,
            spacing_seconds: 2,
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
            recovery: RecoveryConfig::default(),
            pipeline: PipelineConfig::default(),
            buffer: BufferConfig::default(),
            vision: VisionConfig::default(),
            events: EventsConfig::default(),
        }
    }
}
