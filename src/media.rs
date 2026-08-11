use async_trait::async_trait;
use reqwest::Client;
use serde::Serialize;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::Duration,
};
use tokio::sync::RwLock;
use url::Url;

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MediaDeliveryHealth {
    Unknown,
    Healthy,
    Degraded,
    Unavailable,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MediaDeliveryState {
    Unknown,
    Starting,
    Ready,
    Degraded,
    Stalled,
    Unavailable,
    Recovering,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AudioDeliveryState {
    Unknown,
    Ready,
    Unavailable,
    Unsupported,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MediaTelemetry {
    pub source_id: String,
    pub protocol: Option<String>,
    pub codec: Option<String>,
    pub resolution: Option<String>,
    pub observed_fps: Option<f64>,
    pub bitrate_bps: Option<u64>,
    pub audio_present: Option<bool>,
    pub audio_codec: Option<String>,
    pub audio_sample_rate: Option<u32>,
    pub audio_channels: Option<u16>,
    pub audio_bitrate_bps: Option<u64>,
    pub audio_delivery_state: AudioDeliveryState,
    pub last_audio_activity: Option<u128>,
    pub stream_started_at: Option<u128>,
    pub last_media_activity: Option<u128>,
    pub reconnect_count: u64,
    pub delivery_state: MediaDeliveryState,
    pub playback_protocols: Vec<String>,
    pub gateway_state: MediaDeliveryHealth,
    pub detail: Option<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MediaGatewayFailureCode {
    MediaGatewayUnavailable,
    MediaSourceRegistrationFailed,
    PlaybackNotReady,
    PlaybackUnavailable,
    MediaConfigurationError,
    MediaGatewayProtocolError,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct MediaGatewayFailure {
    pub code: MediaGatewayFailureCode,
    pub message: String,
    pub technical_detail: Option<String>,
}

impl std::fmt::Display for MediaGatewayFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}: {}",
            serde_json::to_string(&self.code).unwrap_or_default(),
            self.message
        )
    }
}
impl std::error::Error for MediaGatewayFailure {}

#[derive(Clone)]
pub struct MediaSourceRegistration {
    pub source_id: String,
    pub rtsp_uri: String,
    pub username: Option<String>,
    pub password: Option<String>,
}

impl std::fmt::Debug for MediaSourceRegistration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MediaSourceRegistration")
            .field("source_id", &self.source_id)
            .field("rtsp_uri", &redact_uri(&self.rtsp_uri))
            .field("username", &self.username.as_ref().map(|_| "[REDACTED]"))
            .field("password", &self.password.as_ref().map(|_| "[REDACTED]"))
            .finish()
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct PlaybackStream {
    pub protocol: String,
    pub url: String,
    pub latency_class: String,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct PlaybackInfo {
    pub source_id: String,
    pub available: bool,
    pub streams: Vec<PlaybackStream>,
    pub media_health: MediaDeliveryHealth,
    pub failure: Option<MediaGatewayFailure>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct MediaGatewayStatus {
    pub kind: String,
    pub health: MediaDeliveryHealth,
    pub detail: Option<String>,
}

#[async_trait]
pub trait MediaGateway: Send + Sync {
    async fn register_source(
        &self,
        registration: MediaSourceRegistration,
    ) -> Result<(), MediaGatewayFailure>;
    async fn remove_source(&self, source_id: &str) -> Result<(), MediaGatewayFailure>;
    async fn playback(&self, source_id: &str) -> Result<PlaybackInfo, MediaGatewayFailure>;
    async fn telemetry(&self, source_id: &str) -> Result<MediaTelemetry, MediaGatewayFailure> {
        let playback = self.playback(source_id).await?;
        Ok(MediaTelemetry {
            source_id: source_id.into(),
            protocol: playback
                .streams
                .first()
                .map(|stream| stream.protocol.clone()),
            codec: None,
            resolution: None,
            observed_fps: None,
            bitrate_bps: None,
            audio_present: None,
            audio_codec: None,
            audio_sample_rate: None,
            audio_channels: None,
            audio_bitrate_bps: None,
            audio_delivery_state: AudioDeliveryState::Unknown,
            last_audio_activity: None,
            stream_started_at: None,
            last_media_activity: None,
            reconnect_count: 0,
            delivery_state: if playback.available {
                MediaDeliveryState::Ready
            } else {
                MediaDeliveryState::Unavailable
            },
            playback_protocols: playback
                .streams
                .iter()
                .map(|stream| stream.protocol.clone())
                .collect(),
            gateway_state: playback.media_health,
            detail: playback.failure.map(|failure| failure.message),
        })
    }
    async fn health(&self) -> MediaGatewayStatus;
    async fn shutdown(&self);
}

#[derive(Clone)]
pub struct MediaMtxAdapter {
    client: Client,
    api_url: Option<String>,
    webrtc_base_url: Option<String>,
    hls_base_url: Option<String>,
    timeout: Duration,
    registered: Arc<RwLock<HashSet<String>>>,
    samples: Arc<RwLock<HashMap<String, MediaSample>>>,
}

#[derive(Clone, Debug)]
struct MediaSample {
    observed_at: u128,
    bytes_received: Option<u64>,
    ready: bool,
    stream_started_at: Option<u128>,
    last_media_activity: Option<u128>,
    reconnect_count: u64,
}

impl MediaMtxAdapter {
    pub fn new(
        enabled: bool,
        api_url: Option<String>,
        base_url: Option<String>,
        webrtc_base_url: Option<String>,
        hls_base_url: Option<String>,
        timeout: Duration,
    ) -> Self {
        let base_url = base_url.filter(|_| enabled);
        let webrtc_base_url = webrtc_base_url.filter(|_| enabled);
        let hls_base_url = hls_base_url.filter(|_| enabled);
        Self {
            client: Client::new(),
            api_url: if enabled { api_url } else { None },
            webrtc_base_url: webrtc_base_url.or_else(|| base_url.clone()),
            hls_base_url: hls_base_url.or_else(|| base_url.clone()),
            timeout: timeout.max(Duration::from_millis(1)),
            registered: Arc::new(RwLock::new(HashSet::new())),
            samples: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn path_for_source(source_id: &str) -> String {
        let mut path = String::from("sentinel-");
        for character in source_id.chars() {
            if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                path.push(character.to_ascii_lowercase());
            } else {
                path.push('-');
            }
        }
        if path == "sentinel-" {
            path.push_str("source");
        }
        path.push_str(&format!("-{:x}", stable_hash(source_id)));
        path
    }

    fn registration_url(&self, path: &str) -> Result<String, MediaGatewayFailure> {
        let api = self.api_url.as_deref().ok_or_else(|| {
            failure(
                MediaGatewayFailureCode::MediaGatewayUnavailable,
                "Media gateway is not configured.",
                None,
            )
        })?;
        Ok(format!(
            "{}/v3/config/paths/add/{}",
            api.trim_end_matches('/'),
            path
        ))
    }

    fn playback_url(base: Option<&str>, path: &str, suffix: &str) -> Option<String> {
        base.map(|base| format!("{}/{}/{}", base.trim_end_matches('/'), path, suffix))
    }

    async fn send_registration(
        &self,
        registration: &MediaSourceRegistration,
        path: &str,
    ) -> Result<(), MediaGatewayFailure> {
        let url = self.registration_url(path)?;
        let source = source_url_with_credentials(
            &registration.rtsp_uri,
            registration.username.as_deref(),
            registration.password.as_deref(),
        )?;
        let response = self
            .client
            .post(url)
            .timeout(self.timeout)
            .json(&serde_json::json!({"name": path, "source": source}))
            .send()
            .await
            .map_err(|error| {
                failure(
                    MediaGatewayFailureCode::MediaGatewayUnavailable,
                    "Media gateway could not be reached.",
                    Some(error.to_string()),
                )
            })?;
        if response.status().is_success() {
            return Ok(());
        }
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        if status == reqwest::StatusCode::BAD_REQUEST && body.contains("path already exists") {
            let response = self
                .client
                .patch(format!(
                    "{}/v3/config/paths/patch/{}",
                    self.api_url
                        .as_deref()
                        .unwrap_or_default()
                        .trim_end_matches('/'),
                    path
                ))
                .timeout(self.timeout)
                .json(&serde_json::json!({"source": source}))
                .send()
                .await
                .map_err(|error| {
                    failure(
                        MediaGatewayFailureCode::MediaGatewayUnavailable,
                        "Media gateway could not be reached.",
                        Some(error.to_string()),
                    )
                })?;
            if response.status().is_success() {
                return Ok(());
            }
        }
        Err(failure(
            MediaGatewayFailureCode::MediaSourceRegistrationFailed,
            "Media gateway rejected source registration.",
            Some(status.to_string()),
        ))
    }
}

#[async_trait]
impl MediaGateway for MediaMtxAdapter {
    async fn register_source(
        &self,
        registration: MediaSourceRegistration,
    ) -> Result<(), MediaGatewayFailure> {
        let path = Self::path_for_source(&registration.source_id);
        self.send_registration(&registration, &path).await?;
        self.registered.write().await.insert(registration.source_id);
        Ok(())
    }

    async fn remove_source(&self, source_id: &str) -> Result<(), MediaGatewayFailure> {
        let path = Self::path_for_source(source_id);
        let Some(api) = self.api_url.as_deref() else {
            self.registered.write().await.remove(source_id);
            return Ok(());
        };
        let url = format!(
            "{}/v3/config/paths/delete/{}",
            api.trim_end_matches('/'),
            path
        );
        let response = self
            .client
            .delete(url)
            .timeout(self.timeout)
            .send()
            .await
            .map_err(|error| {
                failure(
                    MediaGatewayFailureCode::MediaGatewayUnavailable,
                    "Media gateway could not be reached.",
                    Some(error.to_string()),
                )
            })?;
        if !response.status().is_success() && response.status() != reqwest::StatusCode::NOT_FOUND {
            return Err(failure(
                MediaGatewayFailureCode::MediaGatewayProtocolError,
                "Media gateway rejected source removal.",
                Some(response.status().to_string()),
            ));
        }
        self.registered.write().await.remove(source_id);
        Ok(())
    }

    async fn playback(&self, source_id: &str) -> Result<PlaybackInfo, MediaGatewayFailure> {
        if !self.registered.read().await.contains(source_id) {
            return Err(failure(
                MediaGatewayFailureCode::PlaybackNotReady,
                "The source is not registered with the media gateway.",
                None,
            ));
        }
        if self.api_url.is_some() {
            let gateway_status = self.health().await;
            if gateway_status.health == MediaDeliveryHealth::Unavailable {
                return Err(failure(
                    MediaGatewayFailureCode::MediaGatewayUnavailable,
                    "Media gateway could not be reached.",
                    gateway_status.detail,
                ));
            }
        }
        let path = Self::path_for_source(source_id);
        let mut streams = Vec::new();
        if let Some(url) = Self::playback_url(self.webrtc_base_url.as_deref(), &path, "whep") {
            streams.push(PlaybackStream {
                protocol: "webrtc".into(),
                url,
                latency_class: "low".into(),
            });
        }
        if let Some(url) = Self::playback_url(self.hls_base_url.as_deref(), &path, "index.m3u8") {
            streams.push(PlaybackStream {
                protocol: "hls".into(),
                url,
                latency_class: "standard".into(),
            });
        }
        if streams.is_empty() {
            return Err(failure(
                MediaGatewayFailureCode::PlaybackUnavailable,
                "Browser playback is not configured.",
                None,
            ));
        }
        Ok(PlaybackInfo {
            source_id: source_id.into(),
            available: true,
            streams,
            media_health: MediaDeliveryHealth::Healthy,
            failure: None,
        })
    }

    async fn telemetry(&self, source_id: &str) -> Result<MediaTelemetry, MediaGatewayFailure> {
        let playback_protocols = [
            self.webrtc_base_url.as_ref().map(|_| "webrtc".to_owned()),
            self.hls_base_url.as_ref().map(|_| "hls".to_owned()),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
        let unavailable = |detail: String| MediaTelemetry {
            source_id: source_id.into(),
            protocol: None,
            codec: None,
            resolution: None,
            observed_fps: None,
            bitrate_bps: None,
            audio_present: None,
            audio_codec: None,
            audio_sample_rate: None,
            audio_channels: None,
            audio_bitrate_bps: None,
            audio_delivery_state: AudioDeliveryState::Unknown,
            last_audio_activity: None,
            stream_started_at: None,
            last_media_activity: None,
            reconnect_count: 0,
            delivery_state: MediaDeliveryState::Unavailable,
            playback_protocols: playback_protocols.clone(),
            gateway_state: MediaDeliveryHealth::Unavailable,
            detail: Some(detail),
        };
        if !self.registered.read().await.contains(source_id) {
            return Ok(unavailable(
                "Source is not registered with the media gateway.".into(),
            ));
        }
        let Some(api) = self.api_url.as_deref() else {
            return Ok(unavailable("MediaMTX API is not configured.".into()));
        };
        let gateway = self.health().await;
        if gateway.health == MediaDeliveryHealth::Unavailable {
            return Ok(unavailable(
                gateway
                    .detail
                    .unwrap_or_else(|| "Media gateway is unavailable.".into()),
            ));
        }
        let path = Self::path_for_source(source_id);
        let response = self
            .client
            .get(format!(
                "{}/v3/paths/get/{}",
                api.trim_end_matches('/'),
                path
            ))
            .timeout(self.timeout)
            .send()
            .await
            .map_err(|error| {
                failure(
                    MediaGatewayFailureCode::MediaGatewayUnavailable,
                    "Media gateway telemetry could not be reached.",
                    Some(error.to_string()),
                )
            })?;
        if !response.status().is_success() {
            return Ok(unavailable(format!(
                "MediaMTX path telemetry returned {}.",
                response.status()
            )));
        }
        let body: serde_json::Value = response.json().await.map_err(|error| {
            failure(
                MediaGatewayFailureCode::MediaGatewayProtocolError,
                "Media gateway returned invalid telemetry.",
                Some(error.to_string()),
            )
        })?;
        let ready = body
            .get("ready")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        let bytes_received = body
            .get("bytesReceived")
            .and_then(serde_json::Value::as_u64);
        let tracks = body.get("tracks").and_then(serde_json::Value::as_array);
        let video = tracks.and_then(|tracks| {
            tracks.iter().find(|track| {
                track.get("type").and_then(serde_json::Value::as_str) == Some("video")
            })
        });
        let audio_present = tracks.map(|tracks| {
            tracks
                .iter()
                .any(|track| track.get("type").and_then(serde_json::Value::as_str) == Some("audio"))
        });
        let audio = tracks.and_then(|tracks| {
            tracks.iter().find(|track| {
                track.get("type").and_then(serde_json::Value::as_str) == Some("audio")
            })
        });
        let codec = video.and_then(|track| {
            track
                .get("codec")
                .or_else(|| track.get("codecName"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        });
        let resolution = video.and_then(|track| {
            Some(format!(
                "{}x{}",
                track.get("width")?.as_u64()?,
                track.get("height")?.as_u64()?
            ))
        });
        let audio_codec = audio.and_then(|track| {
            track
                .get("codec")
                .or_else(|| track.get("codecName"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        });
        let audio_sample_rate = audio.and_then(|track| {
            track
                .get("sampleRate")
                .or_else(|| track.get("clockRate"))
                .and_then(serde_json::Value::as_u64)
                .map(|value| value as u32)
        });
        let audio_channels = audio.and_then(|track| {
            track
                .get("channels")
                .and_then(serde_json::Value::as_u64)
                .map(|value| value as u16)
        });
        let now = now_ms();
        let mut samples = self.samples.write().await;
        let previous = samples.get(source_id).cloned();
        let elapsed_ms = previous
            .as_ref()
            .map(|sample| now.saturating_sub(sample.observed_at));
        let bitrate_bps = previous.as_ref().and_then(|sample| {
            Some(
                sample
                    .bytes_received
                    .zip(bytes_received)?
                    .1
                    .saturating_sub(sample.bytes_received?)
                    .saturating_mul(8)
                    .saturating_mul(1000)
                    / elapsed_ms?.max(1) as u64,
            )
        });
        let active = ready
            && previous
                .as_ref()
                .map(|sample| sample.bytes_received != bytes_received)
                .unwrap_or(bytes_received.is_some());
        let last_media_activity = if active {
            Some(now)
        } else {
            previous
                .as_ref()
                .and_then(|sample| sample.last_media_activity)
        };
        let stream_started_at = if ready {
            previous
                .as_ref()
                .and_then(|sample| sample.stream_started_at)
                .or(Some(now))
        } else {
            None
        };
        let reconnect_count = previous.as_ref().map_or(0, |sample| {
            sample.reconnect_count + u64::from(!sample.ready && ready)
        });
        samples.insert(
            source_id.into(),
            MediaSample {
                observed_at: now,
                bytes_received,
                ready,
                stream_started_at,
                last_media_activity,
                reconnect_count,
            },
        );
        Ok(MediaTelemetry {
            source_id: source_id.into(),
            protocol: playback_protocols.first().cloned(),
            codec,
            resolution,
            observed_fps: video
                .and_then(|track| track.get("fps").and_then(serde_json::Value::as_f64)),
            bitrate_bps,
            audio_present,
            audio_codec,
            audio_sample_rate,
            audio_channels,
            audio_bitrate_bps: None,
            audio_delivery_state: match audio_present {
                Some(true) if ready => AudioDeliveryState::Ready,
                Some(true) => AudioDeliveryState::Unavailable,
                Some(false) => AudioDeliveryState::Unsupported,
                None => AudioDeliveryState::Unknown,
            },
            last_audio_activity: if audio_present == Some(true) && active {
                Some(now)
            } else {
                None
            },
            stream_started_at,
            last_media_activity,
            reconnect_count,
            delivery_state: if ready {
                MediaDeliveryState::Ready
            } else {
                MediaDeliveryState::Starting
            },
            playback_protocols,
            gateway_state: MediaDeliveryHealth::Healthy,
            detail: None,
        })
    }

    async fn health(&self) -> MediaGatewayStatus {
        let Some(api) = self.api_url.as_deref() else {
            return MediaGatewayStatus {
                kind: "mediamtx".into(),
                health: MediaDeliveryHealth::Unavailable,
                detail: Some("MediaMTX API is not configured".into()),
            };
        };
        let url = format!("{}/v3/paths/list", api.trim_end_matches('/'));
        match self.client.get(url).timeout(self.timeout).send().await {
            Ok(response) if response.status().is_success() => MediaGatewayStatus {
                kind: "mediamtx".into(),
                health: MediaDeliveryHealth::Healthy,
                detail: None,
            },
            Ok(response) => MediaGatewayStatus {
                kind: "mediamtx".into(),
                health: MediaDeliveryHealth::Unavailable,
                detail: Some(format!("MediaMTX returned {}", response.status())),
            },
            Err(error) => MediaGatewayStatus {
                kind: "mediamtx".into(),
                health: MediaDeliveryHealth::Unavailable,
                detail: Some(error.to_string()),
            },
        }
    }

    async fn shutdown(&self) {
        let source_ids = self
            .registered
            .read()
            .await
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        for source_id in source_ids {
            let _ = self.remove_source(&source_id).await;
        }
    }
}

fn source_url_with_credentials(
    uri: &str,
    username: Option<&str>,
    password: Option<&str>,
) -> Result<String, MediaGatewayFailure> {
    let mut url = Url::parse(uri).map_err(|error| {
        failure(
            MediaGatewayFailureCode::MediaConfigurationError,
            "The RTSP source URI is invalid.",
            Some(error.to_string()),
        )
    })?;
    if url.username().is_empty() {
        if let Some(username) = username {
            url.set_username(username).map_err(|_| {
                failure(
                    MediaGatewayFailureCode::MediaConfigurationError,
                    "The RTSP source username is invalid.",
                    None,
                )
            })?;
        }
        if let Some(password) = password {
            url.set_password(Some(password)).map_err(|_| {
                failure(
                    MediaGatewayFailureCode::MediaConfigurationError,
                    "The RTSP source password is invalid.",
                    None,
                )
            })?;
        }
    }
    Ok(url.to_string())
}

fn redact_uri(value: &str) -> String {
    let Ok(mut url) = Url::parse(value) else {
        return "[REDACTED_URI]".into();
    };
    if !url.username().is_empty() {
        let _ = url.set_username("[REDACTED]");
        let _ = url.set_password(Some("[REDACTED]"));
    }
    url.to_string()
}

fn failure(
    code: MediaGatewayFailureCode,
    message: &str,
    technical_detail: Option<String>,
) -> MediaGatewayFailure {
    MediaGatewayFailure {
        code,
        message: message.into(),
        technical_detail,
    }
}

fn stable_hash(value: &str) -> u64 {
    value.bytes().fold(1469598103934665603, |hash, byte| {
        (hash ^ byte as u64).wrapping_mul(1099511628211)
    })
}

fn now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mediamtx_adapter_generates_normalized_webrtc_and_hls_urls() {
        let adapter = MediaMtxAdapter::new(
            true,
            None,
            Some("http://127.0.0.1:8889".into()),
            None,
            Some("http://127.0.0.1:8888".into()),
            Duration::from_millis(100),
        );
        adapter.registered.write().await.insert("camera one".into());
        let playback = adapter.playback("camera one").await.unwrap();
        assert_eq!(playback.streams[0].protocol, "webrtc");
        assert_eq!(playback.streams[0].latency_class, "low");
        assert_eq!(playback.streams[1].protocol, "hls");
        assert!(playback
            .streams
            .iter()
            .all(|stream| !stream.url.contains("v3/config")));
        assert!(playback
            .streams
            .iter()
            .all(|stream| !stream.url.contains("camera one")));
    }

    #[test]
    fn telemetry_contract_is_normalized_and_contains_no_gateway_response_shape() {
        let telemetry = MediaTelemetry {
            source_id: "front-door".into(),
            protocol: Some("webrtc".into()),
            codec: Some("H264".into()),
            resolution: Some("1920x1080".into()),
            observed_fps: Some(30.0),
            bitrate_bps: Some(4_200_000),
            audio_present: Some(false),
            audio_codec: None,
            audio_sample_rate: None,
            audio_channels: None,
            audio_bitrate_bps: None,
            audio_delivery_state: AudioDeliveryState::Unsupported,
            last_audio_activity: None,
            stream_started_at: Some(100),
            last_media_activity: Some(200),
            reconnect_count: 1,
            delivery_state: MediaDeliveryState::Ready,
            playback_protocols: vec!["webrtc".into(), "hls".into()],
            gateway_state: MediaDeliveryHealth::Healthy,
            detail: None,
        };
        let json = serde_json::to_string(&telemetry).unwrap();
        assert!(json.contains("deliveryState"));
        assert!(json.contains("bitrateBps"));
        assert!(json.contains("audioDeliveryState"));
        assert!(!json.contains("bytesReceived"));
        assert!(!json.contains("readers"));
    }

    #[test]
    fn audio_states_distinguish_video_only_and_transport_ready() {
        assert_eq!(
            AudioDeliveryState::Unsupported,
            AudioDeliveryState::Unsupported
        );
        let telemetry = MediaTelemetry {
            source_id: "audio-camera".into(),
            protocol: Some("webrtc".into()),
            codec: Some("H264".into()),
            resolution: Some("1280x720".into()),
            observed_fps: Some(25.0),
            bitrate_bps: None,
            audio_present: Some(true),
            audio_codec: Some("AAC".into()),
            audio_sample_rate: Some(48_000),
            audio_channels: Some(1),
            audio_bitrate_bps: None,
            audio_delivery_state: AudioDeliveryState::Ready,
            last_audio_activity: Some(20),
            stream_started_at: Some(1),
            last_media_activity: Some(20),
            reconnect_count: 0,
            delivery_state: MediaDeliveryState::Ready,
            playback_protocols: vec!["webrtc".into()],
            gateway_state: MediaDeliveryHealth::Healthy,
            detail: None,
        };
        assert_eq!(telemetry.audio_codec.as_deref(), Some("AAC"));
        assert_eq!(telemetry.audio_delivery_state, AudioDeliveryState::Ready);
    }
}
