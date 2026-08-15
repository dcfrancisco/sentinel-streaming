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
    pub instance_id: String,
    pub deployment_profile: String,
    pub security: SecurityConfig,
    pub fps: u32,
    pub rtsp_validation_timeout_ms: u64,
    pub media_gateway: MediaGatewayConfig,
    pub media_supervision: MediaSupervisionConfig,
    pub recovery: RecoveryConfig,
    pub pipeline: PipelineConfig,
    pub buffer: BufferConfig,
    pub vision: VisionConfig,
    pub events: EventsConfig,
    pub sources: Vec<ConfiguredSource>,
    pub logging: LoggingConfig,
    pub metrics: MetricsConfig,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct SecurityConfig {
    pub mode: SecurityMode,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SecurityMode {
    OpenLocalTest,
    LocalAdminAuth,
    ExternalIdentity,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            mode: SecurityMode::OpenLocalTest,
        }
    }
}

impl SecurityMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::OpenLocalTest => "OPEN_LOCAL_TEST",
            Self::LocalAdminAuth => "LOCAL_ADMIN_AUTH",
            Self::ExternalIdentity => "EXTERNAL_IDENTITY",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct MediaGatewayConfig {
    pub enabled: bool,
    pub kind: String,
    pub api_url: Option<String>,
    pub base_url: Option<String>,
    pub webrtc_base_url: Option<String>,
    pub hls_base_url: Option<String>,
    pub timeout_ms: u64,
}
impl Default for MediaGatewayConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            kind: "mediamtx".into(),
            api_url: None,
            base_url: None,
            webrtc_base_url: None,
            hls_base_url: None,
            timeout_ms: 3000,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct MediaSupervisionConfig {
    pub enabled: bool,
    pub interval_ms: u64,
    pub stall_timeout_ms: u64,
    pub startup_timeout_ms: u64,
}
impl Default for MediaSupervisionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            interval_ms: 1000,
            stall_timeout_ms: 5000,
            startup_timeout_ms: 10000,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ConfiguredSource {
    pub id: String,
    pub name: Option<String>,
    pub vendor: Option<String>,
    pub host: Option<String>,
    pub port: Option<u16>,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(serialize_with = "serialize_safe_uri")]
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

impl std::fmt::Debug for ConfiguredSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConfiguredSource")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("vendor", &self.vendor)
            .field("host", &self.host)
            .field("port", &self.port)
            .field("kind", &self.kind)
            .field("uri", &self.uri.as_deref().map(redact_uri_for_debug))
            .field("path", &self.path)
            .field("transport", &self.transport)
            .field("credentials", &self.credentials)
            .field("enabled", &self.enabled)
            .finish()
    }
}

fn serialize_safe_uri<S>(value: &Option<String>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    value
        .as_deref()
        .map(redact_uri_for_debug)
        .serialize(serializer)
}

pub(crate) fn redact_uri_for_debug(value: &str) -> String {
    url::Url::parse(value)
        .map(|mut url| {
            if !url.username().is_empty() {
                let _ = url.set_username("[REDACTED]");
                let _ = url.set_password(Some("[REDACTED]"));
            }
            url.to_string()
        })
        .unwrap_or_else(|_| "[REDACTED_URI]".into())
}
impl ConfiguredSource {
    pub fn options(&self) -> SourceOptions {
        SourceOptions {
            name: self.name.clone(),
            location: None,
            vendor: self.vendor.clone(),
            host: self.host.clone(),
            port: self.port,
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
#[derive(Clone, Debug, Serialize)]
pub struct HealthConfig {
    pub enabled: bool,
    pub interval_seconds: u64,
    pub max_concurrent_checks: usize,
    pub max_attempts: u32,
    pub initial_backoff_ms: u64,
    pub max_backoff_seconds: u64,
}
impl Default for HealthConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            interval_seconds: 30,
            max_concurrent_checks: 4,
            max_attempts: 3,
            initial_backoff_ms: 500,
            max_backoff_seconds: 30,
        }
    }
}
#[derive(Clone, Debug, Default, Serialize)]
pub struct RecoveryConfig {
    pub camera: ReconnectConfig,
    pub health: HealthConfig,
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
            bind: "127.0.0.1:8080".into(),
            instance_id: "local-instance".into(),
            deployment_profile: "standalone".into(),
            security: SecurityConfig::default(),
            fps: 30,
            rtsp_validation_timeout_ms: 5000,
            media_gateway: MediaGatewayConfig::default(),
            media_supervision: MediaSupervisionConfig::default(),
            recovery: RecoveryConfig::default(),
            pipeline: PipelineConfig::default(),
            buffer: BufferConfig::default(),
            vision: VisionConfig::default(),
            events: EventsConfig::default(),
            sources: vec![ConfiguredSource {
                id: "builtin".into(),
                name: Some("Built-in camera".into()),
                vendor: None,
                host: None,
                port: None,
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
    instance_id: Option<String>,
    deployment_profile: Option<String>,
    security: Option<SecurityPatch>,
    fps: Option<u32>,
    rtsp_validation_timeout_ms: Option<u64>,
    media_gateway: Option<MediaGatewayPatch>,
    media_supervision: Option<MediaSupervisionPatch>,
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
struct SecurityPatch {
    mode: Option<SecurityModeFile>,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum SecurityModeFile {
    OpenLocalTest,
    LocalAdminAuth,
    ExternalIdentity,
}
#[derive(Debug, Default, Deserialize)]
struct MediaGatewayPatch {
    enabled: Option<bool>,
    kind: Option<String>,
    api_url: Option<String>,
    base_url: Option<String>,
    webrtc_base_url: Option<String>,
    hls_base_url: Option<String>,
    timeout_ms: Option<u64>,
}
#[derive(Debug, Default, Deserialize)]
struct MediaSupervisionPatch {
    enabled: Option<bool>,
    interval_ms: Option<u64>,
    stall_timeout_ms: Option<u64>,
    startup_timeout_ms: Option<u64>,
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
    health: Option<HealthPatch>,
    vision: Option<VisionRecoveryPatch>,
    mjpeg: Option<MjpegRecoveryPatch>,
}
#[derive(Debug, Default, Deserialize)]
struct HealthPatch {
    enabled: Option<bool>,
    interval_seconds: Option<u64>,
    max_concurrent_checks: Option<usize>,
    max_attempts: Option<u32>,
    initial_backoff_ms: Option<u64>,
    max_backoff_seconds: Option<u64>,
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
        if let Some(instance_id) = file.instance_id {
            self.instance_id = instance_id;
        }
        if let Some(deployment_profile) = file.deployment_profile {
            self.deployment_profile = deployment_profile;
        }
        if let Some(security) = file.security {
            if let Some(mode) = security.mode {
                self.security.mode = match mode {
                    SecurityModeFile::OpenLocalTest => SecurityMode::OpenLocalTest,
                    SecurityModeFile::LocalAdminAuth => SecurityMode::LocalAdminAuth,
                    SecurityModeFile::ExternalIdentity => SecurityMode::ExternalIdentity,
                };
            }
        }
        if let Some(fps) = file.fps {
            self.fps = fps;
        }
        if let Some(timeout) = file.rtsp_validation_timeout_ms {
            self.rtsp_validation_timeout_ms = timeout;
        }
        if let Some(media_gateway) = file.media_gateway {
            merge_media_gateway(&mut self.media_gateway, media_gateway);
        }
        if let Some(media_supervision) = file.media_supervision {
            merge_media_supervision(&mut self.media_supervision, media_supervision);
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
        if let Some(v) = env_string("SENTINEL_INSTANCE_ID") {
            self.instance_id = v;
        }
        if let Some(v) = env_string("SENTINEL_DEPLOYMENT_PROFILE") {
            self.deployment_profile = v;
        }
        if let Some(v) = env_string("SENTINEL_SECURITY_MODE") {
            self.security.mode = parse_security_mode(&v)?;
        }
        if let Some(v) = env_u32("SENTINEL_FPS")? {
            self.fps = v;
        }
        if let Some(v) = env_u64("SENTINEL_RTSP_VALIDATION_TIMEOUT_MS")? {
            self.rtsp_validation_timeout_ms = v;
        }
        if let Some(v) = env_bool("SENTINEL_MEDIA_GATEWAY_ENABLED")? {
            self.media_gateway.enabled = v;
        }
        if let Some(v) = env_string("SENTINEL_MEDIA_GATEWAY") {
            self.media_gateway.kind = v;
        }
        if let Some(v) = env_string("SENTINEL_MEDIAMTX_API_URL") {
            self.media_gateway.api_url = Some(v);
        }
        if let Some(v) = env_string("SENTINEL_MEDIAMTX_BASE_URL") {
            self.media_gateway.base_url = Some(v);
        }
        if let Some(v) = env_string("SENTINEL_MEDIAMTX_WEBRTC_BASE_URL") {
            self.media_gateway.webrtc_base_url = Some(v);
        }
        if let Some(v) = env_string("SENTINEL_MEDIAMTX_HLS_BASE_URL") {
            self.media_gateway.hls_base_url = Some(v);
        }
        if let Some(v) = env_bool("SENTINEL_MEDIA_SUPERVISION_ENABLED")? {
            self.media_supervision.enabled = v;
        }
        if let Some(v) = env_u64("SENTINEL_MEDIA_SUPERVISION_INTERVAL_MS")? {
            self.media_supervision.interval_ms = v;
        }
        if let Some(v) = env_u64("SENTINEL_MEDIA_STALL_TIMEOUT_MS")? {
            self.media_supervision.stall_timeout_ms = v;
        }
        if let Some(v) = env_u64("SENTINEL_MEDIA_STARTUP_TIMEOUT_MS")? {
            self.media_supervision.startup_timeout_ms = v;
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
        let bind = self.bind.parse::<SocketAddr>().map_err(|_| {
            ConfigError::Invalid(format!("server.bind is not a valid address: {}", self.bind))
        })?;
        if self.security.mode == SecurityMode::OpenLocalTest && !bind.ip().is_loopback() {
            return Err(ConfigError::Invalid(
                "OPEN_LOCAL_TEST requires a loopback server.bind address".into(),
            ));
        }
        if self.security.mode == SecurityMode::ExternalIdentity {
            return Err(ConfigError::Invalid(
                "EXTERNAL_IDENTITY is reserved for the future identity integration boundary".into(),
            ));
        }
        if self.instance_id.trim().is_empty() {
            return Err(ConfigError::Invalid("instance_id must not be empty".into()));
        }
        if self.deployment_profile.trim().is_empty() {
            return Err(ConfigError::Invalid(
                "deployment_profile must not be empty".into(),
            ));
        }
        if !(1..=240).contains(&self.fps) {
            return Err(ConfigError::Invalid("fps must be between 1 and 240".into()));
        }
        if !(100..=60_000).contains(&self.rtsp_validation_timeout_ms) {
            return Err(ConfigError::Invalid(
                "rtsp_validation_timeout_ms must be between 100 and 60000".into(),
            ));
        }
        if self.media_gateway.timeout_ms == 0 || self.media_gateway.timeout_ms > 60_000 {
            return Err(ConfigError::Invalid(
                "media_gateway.timeout_ms must be between 1 and 60000".into(),
            ));
        }
        if self.media_supervision.interval_ms == 0
            || self.media_supervision.stall_timeout_ms == 0
            || self.media_supervision.startup_timeout_ms == 0
        {
            return Err(ConfigError::Invalid(
                "media supervision interval and timeouts must be greater than zero".into(),
            ));
        }
        if self.media_gateway.kind != "mediamtx" {
            return Err(ConfigError::Invalid(format!(
                "unsupported media gateway: {}",
                self.media_gateway.kind
            )));
        }
        let health = &self.recovery.health;
        if health.interval_seconds == 0
            || health.max_concurrent_checks == 0
            || health.initial_backoff_ms == 0
            || health.max_backoff_seconds == 0
        {
            return Err(ConfigError::Invalid(
                "recovery.health interval, concurrency, backoff, and max delay must be greater than zero".into(),
            ));
        }
        if health.max_attempts > 100 {
            return Err(ConfigError::Invalid(
                "recovery.health max_attempts must be 100 or less".into(),
            ));
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
                "built-in-camera" | "synthetic" | "image-sequence" | "video-file" | "rtsp"
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
            if matches!(source.kind.as_str(), "image-sequence" | "video-file")
                && source.path.is_none()
            {
                return Err(ConfigError::Invalid(format!(
                    "{} source '{}' requires path",
                    source.kind, source.id
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
        vendor: None,
        host: None,
        port: None,
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
    if let Some(v) = p.health {
        if let Some(x) = v.enabled {
            dst.health.enabled = x
        }
        if let Some(x) = v.interval_seconds {
            dst.health.interval_seconds = x
        }
        if let Some(x) = v.max_concurrent_checks {
            dst.health.max_concurrent_checks = x
        }
        if let Some(x) = v.max_attempts {
            dst.health.max_attempts = x
        }
        if let Some(x) = v.initial_backoff_ms {
            dst.health.initial_backoff_ms = x
        }
        if let Some(x) = v.max_backoff_seconds {
            dst.health.max_backoff_seconds = x
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
fn merge_media_gateway(dst: &mut MediaGatewayConfig, p: MediaGatewayPatch) {
    if let Some(value) = p.enabled {
        dst.enabled = value;
    }
    if let Some(value) = p.kind {
        dst.kind = value;
    }
    if let Some(value) = p.api_url {
        dst.api_url = Some(value);
    }
    if let Some(value) = p.base_url {
        dst.base_url = Some(value);
    }
    if let Some(value) = p.webrtc_base_url {
        dst.webrtc_base_url = Some(value);
    }
    if let Some(value) = p.hls_base_url {
        dst.hls_base_url = Some(value);
    }
    if let Some(value) = p.timeout_ms {
        dst.timeout_ms = value;
    }
}
fn merge_media_supervision(dst: &mut MediaSupervisionConfig, p: MediaSupervisionPatch) {
    if let Some(value) = p.enabled {
        dst.enabled = value;
    }
    if let Some(value) = p.interval_ms {
        dst.interval_ms = value;
    }
    if let Some(value) = p.stall_timeout_ms {
        dst.stall_timeout_ms = value;
    }
    if let Some(value) = p.startup_timeout_ms {
        dst.startup_timeout_ms = value;
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

fn parse_security_mode(value: &str) -> Result<SecurityMode, ConfigError> {
    match value.trim().to_ascii_uppercase().as_str() {
        "OPEN_LOCAL_TEST" => Ok(SecurityMode::OpenLocalTest),
        "LOCAL_ADMIN_AUTH" => Ok(SecurityMode::LocalAdminAuth),
        "EXTERNAL_IDENTITY" => Ok(SecurityMode::ExternalIdentity),
        _ => Err(ConfigError::Invalid(
            "SENTINEL_SECURITY_MODE must be OPEN_LOCAL_TEST, LOCAL_ADMIN_AUTH, or EXTERNAL_IDENTITY"
                .to_string(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    static CONFIG_ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

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
        let _guard = CONFIG_ENV_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap();
        let path = temp_yaml("server:\n  bind: 127.0.0.1:9000\nvision:\n  interval: 7s\nsources:\n  - id: demo\n    type: synthetic\n    width: 320\n    height: 200\n    fps: 15\n");
        let config = Config::load(&path, Some("127.0.0.1:9100"), None).unwrap();
        assert_eq!(config.bind, "127.0.0.1:9100");
        assert_eq!(config.vision.interval_seconds, 7);
        assert_eq!(config.sources[0].kind, "synthetic");
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn environment_overrides_yaml() {
        let _guard = CONFIG_ENV_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap();
        let path = temp_yaml("server:\n  bind: 127.0.0.1:9000\n");
        std::env::set_var("SENTINEL_BIND", "127.0.0.1:9200");
        let config = Config::load(&path, None, None).unwrap();
        std::env::remove_var("SENTINEL_BIND");
        assert_eq!(config.bind, "127.0.0.1:9200");
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn open_local_test_requires_loopback() {
        let path = temp_yaml("bind: 0.0.0.0:8080\nsecurity:\n  mode: OPEN_LOCAL_TEST\n");
        let error = Config::load(&path, None, None).unwrap_err().to_string();
        assert!(error.contains("OPEN_LOCAL_TEST requires a loopback"));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn local_admin_auth_is_explicitly_configurable() {
        let path = temp_yaml("bind: 0.0.0.0:8080\nsecurity:\n  mode: LOCAL_ADMIN_AUTH\n");
        let config = Config::load(&path, None, None).unwrap();
        assert_eq!(config.security.mode, SecurityMode::LocalAdminAuth);
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

    #[test]
    fn media_supervision_environment_thresholds_are_loaded() {
        let _guard = CONFIG_ENV_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap();
        std::env::set_var("SENTINEL_MEDIA_SUPERVISION_INTERVAL_MS", "250");
        std::env::set_var("SENTINEL_MEDIA_STALL_TIMEOUT_MS", "1500");
        std::env::set_var("SENTINEL_MEDIA_STARTUP_TIMEOUT_MS", "3000");
        let config = Config::load(
            PathBuf::from("/tmp/sentinel-missing-config.yaml"),
            None,
            Some("synthetic"),
        )
        .unwrap();
        std::env::remove_var("SENTINEL_MEDIA_SUPERVISION_INTERVAL_MS");
        std::env::remove_var("SENTINEL_MEDIA_STALL_TIMEOUT_MS");
        std::env::remove_var("SENTINEL_MEDIA_STARTUP_TIMEOUT_MS");
        assert_eq!(config.media_supervision.interval_ms, 250);
        assert_eq!(config.media_supervision.stall_timeout_ms, 1500);
        assert_eq!(config.media_supervision.startup_timeout_ms, 3000);
    }
}
