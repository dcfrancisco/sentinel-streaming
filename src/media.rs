use async_trait::async_trait;
use reqwest::Client;
use serde::Serialize;
use std::{collections::HashSet, sync::Arc, time::Duration};
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
}
