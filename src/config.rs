use crate::sources::{RtspCredentials, SourceOptions};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    net::SocketAddr,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Serialize)]
pub struct Config {
    pub bind: String,
    pub fps: u32,
    pub recovery: RecoveryConfig,
    pub pipeline: PipelineConfig,
    pub buffer: BufferConfig,
    pub vision: VisionConfig,
    pub events: EventsConfig,
    pub sources: Vec<ConfiguredSource>,
    pub logging: LoggingConfig,
    pub metrics: MetricsConfig,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConfiguredSource {
    pub id: String,
    pub name: Option<String>,
    #[serde(rename = "type")]
    pub kind: String,
    pub uri: Option<String>,
    pub path: Option<String>,
    pub transport: Option<String>,
    pub credentials: Option<RtspCredentials>,
    #[serde(rename = "loop")]
    pub loop_playback: Option<bool>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub fps: Option<u32>,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}
impl ConfiguredSource {
    pub fn options(&self) -> SourceOptions {
        SourceOptions {
            name: self.name.clone(),
            path: self.path.clone(),
            uri: self.uri.clone(),
            transport: self.transport.clone(),
            credentials: self.credentials.clone(),
            loop_playback: self.loop_playback,
            width: self.width,
            height: self.height,
            fps: self.fps,
        }
    }
}
fn default_enabled() -> bool {
    true
}

#[derive(Clone, Debug, Serialize)]
pub struct LoggingConfig {
    pub level: String,
}
impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".into(),
        }
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct MetricsConfig {
    pub enabled: bool,
}
impl Default for MetricsConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
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
    pub provider: String,
    pub interval_seconds: u64,
    pub frames: usize,
    pub spacing_seconds: u64,
}
impl Default for VisionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            provider: "openai".into(),
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

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("could not read configuration file {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("could not parse configuration file {path}: {source}")]
    Parse {
        path: PathBuf,
        source: serde_yaml::Error,
    },
    #[error("invalid configuration: {0}")]
    Invalid(String),
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
            sources: vec![ConfiguredSource {
                id: "builtin".into(),
                name: Some("Built-in camera".into()),
                kind: "built-in-camera".into(),
                uri: None,
                path: None,
                transport: None,
                credentials: None,
                loop_playback: None,
                width: None,
                height: None,
                fps: None,
                enabled: true,
            }],
            logging: LoggingConfig::default(),
            metrics: MetricsConfig::default(),
        }
    }
}

#[derive(Debug, Default, Deserialize)]
struct FileConfig {
    server: Option<ServerPatch>,
    bind: Option<String>,
    fps: Option<u32>,
    recovery: Option<RecoveryPatch>,
    pipeline: Option<PipelinePatch>,
    buffer: Option<BufferPatch>,
    vision: Option<VisionPatch>,
    events: Option<EventsPatch>,
    sources: Option<Vec<ConfiguredSource>>,
    logging: Option<LoggingPatch>,
    metrics: Option<MetricsPatch>,
}
#[derive(Debug, Default, Deserialize)]
struct ServerPatch {
    bind: Option<String>,
}
#[derive(Debug, Default, Deserialize)]
struct LoggingPatch {
    level: Option<String>,
}
#[derive(Debug, Default, Deserialize)]
struct MetricsPatch {
    enabled: Option<bool>,
}
#[derive(Debug, Default, Deserialize)]
struct EventsPatch {
    capacity: Option<usize>,
}
#[derive(Debug, Default, Deserialize)]
struct BufferPatch {
    enabled: Option<bool>,
    capacity: Option<usize>,
}
#[derive(Debug, Default, Deserialize)]
struct PipelinePatch {
    preview: Option<bool>,
    buffer: Option<bool>,
    recording: Option<bool>,
    vision: Option<bool>,
    events: Option<bool>,
}
#[derive(Debug, Default, Deserialize)]
struct VisionPatch {
    enabled: Option<bool>,
    provider: Option<String>,
    interval: Option<String>,
    interval_seconds: Option<u64>,
    frames: Option<usize>,
    spacing: Option<String>,
    spacing_seconds: Option<u64>,
}
#[derive(Debug, Default, Deserialize)]
struct RecoveryPatch {
    camera: Option<ReconnectPatch>,
    vision: Option<VisionRecoveryPatch>,
    mjpeg: Option<MjpegRecoveryPatch>,
}
#[derive(Debug, Default, Deserialize)]
struct ReconnectPatch {
    enabled: Option<bool>,
    retry_forever: Option<bool>,
    initial_delay_ms: Option<u64>,
    max_delay_seconds: Option<u64>,
    jitter: Option<bool>,
}
#[derive(Debug, Default, Deserialize)]
struct VisionRecoveryPatch {
    enabled: Option<bool>,
    retry_count: Option<u32>,
    cooldown_seconds: Option<u64>,
}
#[derive(Debug, Default, Deserialize)]
struct MjpegRecoveryPatch {
    cleanup_interval_seconds: Option<u64>,
}

impl Config {
    pub fn load(
        path: impl AsRef<Path>,
        cli_bind: Option<&str>,
        cli_source: Option<&str>,
    ) -> Result<Self, ConfigError> {
        let path = path.as_ref().to_path_buf();
        let file = if path.exists() {
            let text = fs::read_to_string(&path).map_err(|source| ConfigError::Read {
                path: path.clone(),
                source,
            })?;
            serde_yaml::from_str::<FileConfig>(&text).map_err(|source| ConfigError::Parse {
                path: path.clone(),
                source,
            })?
        } else {
            FileConfig::default()
        };
        let mut config = Config::default();
        config.apply_file(file)?;
        config.apply_env()?;
        if let Some(bind) = cli_bind {
            config.bind = bind.into();
        }
        if let Some(source) = cli_source {
            config.sources = vec![cli_source_config(source)?];
        }
        config.validate()?;
        Ok(config)
    }

    fn apply_file(&mut self, file: FileConfig) -> Result<(), ConfigError> {
        if let Some(server) = file.server {
            if let Some(bind) = server.bind {
                self.bind = bind;
            }
        }
        if let Some(bind) = file.bind {
            self.bind = bind;
        }
        if let Some(fps) = file.fps {
            self.fps = fps;
        }
        if let Some(p) = file.pipeline {
            merge_pipeline(&mut self.pipeline, p);
        }
        if let Some(p) = file.buffer {
            if let Some(v) = p.enabled {
                self.buffer.enabled = v
            }
            if let Some(v) = p.capacity {
                self.buffer.capacity = v
            }
        }
        if let Some(p) = file.events {
            if let Some(v) = p.capacity {
                self.events.capacity = v
            }
        }
        if let Some(p) = file.logging {
            if let Some(v) = p.level {
                self.logging.level = v
            }
        }
        if let Some(p) = file.metrics {
            if let Some(v) = p.enabled {
                self.metrics.enabled = v
            }
        }
        if let Some(p) = file.vision {
            if let Some(v) = p.enabled {
                self.vision.enabled = v
            }
            if let Some(v) = p.provider {
                self.vision.provider = v
            }
            if let Some(v) = p.interval_seconds {
                self.vision.interval_seconds = v
            }
            if let Some(v) = p.frames {
                self.vision.frames = v
            }
            if let Some(v) = p.spacing_seconds {
                self.vision.spacing_seconds = v
            }
            if let Some(v) = p.interval {
                self.vision.interval_seconds = parse_duration(&v)?
            }
            if let Some(v) = p.spacing {
                self.vision.spacing_seconds = parse_duration(&v)?
            }
        }
        if let Some(p) = file.recovery {
            merge_recovery(&mut self.recovery, p);
        }
        if let Some(sources) = file.sources {
            self.sources = sources;
        }
        Ok(())
    }

    fn apply_env(&mut self) -> Result<(), ConfigError> {
        if let Some(v) = env_string("SENTINEL_BIND") {
            self.bind = v;
        }
        if let Some(v) = env_u32("SENTINEL_FPS")? {
            self.fps = v;
        }
        if let Some(v) = env_bool("SENTINEL_VISION_ENABLED")? {
            self.vision.enabled = v;
        }
        if let Some(v) = env_string("SENTINEL_VISION_PROVIDER") {
            self.vision.provider = v;
        }
        if let Some(v) = env_string("SENTINEL_VISION_INTERVAL") {
            self.vision.interval_seconds = parse_duration(&v)?;
        }
        if let Some(v) = env_u64("SENTINEL_BUFFER_CAPACITY")? {
            self.buffer.capacity = v as usize;
        }
        Ok(())
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        self.bind.parse::<SocketAddr>().map_err(|_| {
            ConfigError::Invalid(format!("server.bind is not a valid address: {}", self.bind))
        })?;
        if !(1..=240).contains(&self.fps) {
            return Err(ConfigError::Invalid("fps must be between 1 and 240".into()));
        }
        if self.buffer.capacity == 0 {
            return Err(ConfigError::Invalid(
                "buffer.capacity must be greater than zero".into(),
            ));
        }
        if self.vision.provider != "openai" {
            return Err(ConfigError::Invalid(format!(
                "unsupported vision provider: {}",
                self.vision.provider
            )));
        }
        if self.vision.interval_seconds == 0 || self.vision.frames == 0 {
            return Err(ConfigError::Invalid(
                "vision interval and frames must be greater than zero".into(),
            ));
        }
        let mut ids = std::collections::BTreeSet::new();
        for source in &self.sources {
            if source.id.trim().is_empty() || !ids.insert(&source.id) {
                return Err(ConfigError::Invalid(format!(
                    "duplicate or empty source id: {}",
                    source.id
                )));
            }
            if !matches!(
                source.kind.as_str(),
                "built-in-camera" | "synthetic" | "image-sequence" | "rtsp"
            ) {
                return Err(ConfigError::Invalid(format!(
                    "unsupported source type '{}'",
                    source.kind
                )));
            }
            if let Some(fps) = source.fps {
                if !(1..=240).contains(&fps) {
                    return Err(ConfigError::Invalid(format!(
                        "source '{}' fps must be between 1 and 240",
                        source.id
                    )));
                }
            }
            if let Some(width) = source.width {
                if !(1..=8192).contains(&width) {
                    return Err(ConfigError::Invalid(format!(
                        "source '{}' width is invalid",
                        source.id
                    )));
                }
            }
            if let Some(height) = source.height {
                if !(1..=8192).contains(&height) {
                    return Err(ConfigError::Invalid(format!(
                        "source '{}' height is invalid",
                        source.id
                    )));
                }
            }
            if source.kind == "rtsp" {
                let uri = source.uri.as_deref().ok_or_else(|| {
                    ConfigError::Invalid(format!("RTSP source '{}' requires uri", source.id))
                })?;
                if !uri.starts_with("rtsp://") {
                    return Err(ConfigError::Invalid(format!(
                        "RTSP source '{}' has a malformed uri",
                        source.id
                    )));
                }
                let authority = uri
                    .trim_start_matches("rtsp://")
                    .split('/')
                    .next()
                    .unwrap_or_default();
                if authority.contains('@') {
                    return Err(ConfigError::Invalid(format!(
                        "RTSP source '{}' must use environment credential references",
                        source.id
                    )));
                }
                if source.transport.as_deref().unwrap_or("tcp") != "tcp" {
                    return Err(ConfigError::Invalid(
                        "only RTSP TCP transport is supported".into(),
                    ));
                }
            }
            if source.kind == "image-sequence" && source.path.is_none() {
                return Err(ConfigError::Invalid(format!(
                    "image-sequence source '{}' requires path",
                    source.id
                )));
            }
        }
        Ok(())
    }
}

fn cli_source_config(source: &str) -> Result<ConfiguredSource, ConfigError> {
    let kind = match source {
        "builtin" => "built-in-camera",
        "synthetic" => "synthetic",
        "image-sequence" => "image-sequence",
        "rtsp" => "rtsp",
        other => {
            return Err(ConfigError::Invalid(format!(
                "unsupported CLI source: {other}"
            )))
        }
    };
    Ok(ConfiguredSource {
        id: if kind == "built-in-camera" {
            "builtin".into()
        } else {
            kind.into()
        },
        name: None,
        kind: kind.into(),
        uri: None,
        path: None,
        transport: None,
        credentials: None,
        loop_playback: None,
        width: None,
        height: None,
        fps: None,
        enabled: true,
    })
}
fn merge_pipeline(dst: &mut PipelineConfig, p: PipelinePatch) {
    if let Some(v) = p.preview {
        dst.preview = v
    }
    if let Some(v) = p.buffer {
        dst.buffer = v
    }
    if let Some(v) = p.recording {
        dst.recording = v
    }
    if let Some(v) = p.vision {
        dst.vision = v
    }
    if let Some(v) = p.events {
        dst.events = v
    }
}
fn merge_recovery(dst: &mut RecoveryConfig, p: RecoveryPatch) {
    if let Some(v) = p.camera {
        if let Some(x) = v.enabled {
            dst.camera.enabled = x
        }
        if let Some(x) = v.retry_forever {
            dst.camera.retry_forever = x
        }
        if let Some(x) = v.initial_delay_ms {
            dst.camera.initial_delay_ms = x
        }
        if let Some(x) = v.max_delay_seconds {
            dst.camera.max_delay_seconds = x
        }
        if let Some(x) = v.jitter {
            dst.camera.jitter = x
        }
    }
    if let Some(v) = p.vision {
        if let Some(x) = v.enabled {
            dst.vision.enabled = x
        }
        if let Some(x) = v.retry_count {
            dst.vision.retry_count = x
        }
        if let Some(x) = v.cooldown_seconds {
            dst.vision.cooldown_seconds = x
        }
    }
    if let Some(v) = p.mjpeg {
        if let Some(x) = v.cleanup_interval_seconds {
            dst.mjpeg.cleanup_interval_seconds = x
        }
    }
}
fn parse_duration(value: &str) -> Result<u64, ConfigError> {
    let value = value.trim();
    let (number, multiplier) = value
        .strip_suffix('s')
        .map(|v| (v, 1))
        .or_else(|| value.strip_suffix('m').map(|v| (v, 60)))
        .or_else(|| value.strip_suffix('h').map(|v| (v, 3600)))
        .ok_or_else(|| ConfigError::Invalid(format!("invalid duration '{value}'")))?;
    number
        .parse::<u64>()
        .map(|n| n.saturating_mul(multiplier))
        .map_err(|_| ConfigError::Invalid(format!("invalid duration '{value}'")))
}
fn env_string(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.is_empty())
}
fn env_u32(name: &str) -> Result<Option<u32>, ConfigError> {
    env_string(name)
        .map(|v| {
            v.parse()
                .map_err(|_| ConfigError::Invalid(format!("{name} must be an integer")))
        })
        .transpose()
}
fn env_u64(name: &str) -> Result<Option<u64>, ConfigError> {
    env_string(name)
        .map(|v| {
            v.parse()
                .map_err(|_| ConfigError::Invalid(format!("{name} must be an integer")))
        })
        .transpose()
}
fn env_bool(name: &str) -> Result<Option<bool>, ConfigError> {
    env_string(name)
        .map(|v| {
            v.parse()
                .map_err(|_| ConfigError::Invalid(format!("{name} must be true or false")))
        })
        .transpose()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    fn temp_yaml(contents: &str) -> PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "sentinel-config-{}-{nonce}.yaml",
            std::process::id()
        ));
        fs::write(&path, contents).unwrap();
        path
    }

    #[test]
    fn yaml_and_cli_precedence_are_applied() {
        let path = temp_yaml("server:\n  bind: 127.0.0.1:9000\nvision:\n  interval: 7s\nsources:\n  - id: demo\n    type: synthetic\n    width: 320\n    height: 200\n    fps: 15\n");
        let config = Config::load(&path, Some("127.0.0.1:9100"), None).unwrap();
        assert_eq!(config.bind, "127.0.0.1:9100");
        assert_eq!(config.vision.interval_seconds, 7);
        assert_eq!(config.sources[0].kind, "synthetic");
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn environment_overrides_yaml() {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        let _guard = LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
        let path = temp_yaml("server:\n  bind: 127.0.0.1:9000\n");
        std::env::set_var("SENTINEL_BIND", "127.0.0.1:9200");
        let config = Config::load(&path, None, None).unwrap();
        std::env::remove_var("SENTINEL_BIND");
        assert_eq!(config.bind, "127.0.0.1:9200");
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn duplicate_sources_are_rejected() {
        let path = temp_yaml(
            "sources:\n  - id: same\n    type: synthetic\n  - id: same\n    type: synthetic\n",
        );
        let error = Config::load(&path, None, None).unwrap_err().to_string();
        assert!(error.contains("duplicate"));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn secrets_are_represented_only_as_references() {
        let path = temp_yaml("sources:\n  - id: front\n    type: rtsp\n    uri: rtsp://camera/stream\n    credentials:\n      username_env: FRONT_USER\n      password_env: FRONT_PASSWORD\n");
        let config = Config::load(&path, None, None).unwrap();
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("FRONT_PASSWORD"));
        assert!(!json.contains("actual-secret"));
        fs::remove_file(path).unwrap();
    }
}
