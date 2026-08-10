use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::{net::SocketAddr, sync::Arc, time::Duration};
use tokio::{net::UdpSocket, time::timeout};
use url::Url;

const DEFAULT_DISCOVERY_ADDRESS: &str = "239.255.255.250:3702";
const PROBE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<e:Envelope xmlns:e="http://www.w3.org/2003/05/soap-envelope" xmlns:w="http://schemas.xmlsoap.org/ws/2004/08/addressing" xmlns:d="http://schemas.xmlsoap.org/ws/2005/04/discovery" xmlns:dn="http://www.onvif.org/ver10/network/wsdl">
  <e:Header><w:MessageID>uuid:sentinel-discovery</w:MessageID><w:To>urn:schemas-xmlsoap-org:ws:2005:04:discovery</w:To><w:Action>http://schemas.xmlsoap.org/ws/2005/04/discovery/Probe</w:Action></e:Header>
  <e:Body><d:Probe><d:Types>dn:NetworkVideoTransmitter</d:Types></d:Probe></e:Body>
</e:Envelope>"#;

#[derive(Clone, Debug, Serialize, PartialEq, Eq, Default)]
pub struct PtzCapabilities {
    pub supported: bool,
    pub pan: bool,
    pub tilt: bool,
    pub zoom: bool,
    pub continuous_move: bool,
    pub relative_move: bool,
    pub absolute_move: bool,
    pub presets: bool,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq, Default)]
pub struct CameraCapabilities {
    pub video: bool,
    pub audio: bool,
    pub snapshot: bool,
    pub events: bool,
    pub ptz: PtzCapabilities,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct MediaProfile {
    pub name: String,
    pub encoding: Option<String>,
    pub resolution: Option<String>,
    pub frame_rate: Option<f32>,
    pub audio: bool,
    pub rtsp_uri: Option<String>,
    #[serde(skip)]
    pub(crate) token: Option<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct OnvifDevice {
    pub id: String,
    pub address: String,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub capabilities: CameraCapabilities,
    pub profiles: Vec<MediaProfile>,
}

#[derive(Clone, Deserialize)]
pub struct OnvifDiscoveryRequest {
    pub address: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub timeout_ms: Option<u64>,
}

#[derive(Clone, Deserialize)]
pub struct OnvifInspectRequest {
    pub endpoint: String,
    pub username: Option<String>,
    pub password: Option<String>,
    pub timeout_ms: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PtzMoveMode {
    Continuous,
    Relative,
    Absolute,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct PtzMoveRequest {
    pub mode: PtzMoveMode,
    pub pan: Option<f32>,
    pub tilt: Option<f32>,
    pub zoom: Option<f32>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct PtzPreset {
    pub id: String,
    pub name: String,
    #[serde(skip)]
    pub(crate) token: Option<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct PtzOperationResult {
    pub operation: String,
    pub success: bool,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OnvifFailureCode {
    DiscoveryTimeout,
    AuthenticationFailed,
    MalformedSoap,
    DeviceUnreachable,
    ProtocolError,
    Unsupported,
    Unknown,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct OnvifFailure {
    pub code: OnvifFailureCode,
    pub message: String,
    pub technical_detail: Option<String>,
}

impl std::fmt::Display for OnvifFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}: {}",
            serde_json::to_string(&self.code).unwrap_or_default(),
            self.message
        )
    }
}

impl std::error::Error for OnvifFailure {}

#[derive(Clone, Debug)]
pub(crate) struct DiscoveredEndpoint {
    pub endpoint: String,
    pub name: Option<String>,
}

#[derive(Clone)]
pub(crate) struct OnvifInspection {
    pub device: OnvifDevice,
    pub selected_rtsp_uri: Option<String>,
    pub ptz_session: Option<PtzSession>,
}

#[derive(Clone)]
pub(crate) struct PtzSession {
    pub service_endpoint: String,
    pub profile_token: String,
    pub username: Option<String>,
    pub password: Option<String>,
}

#[derive(Clone, Debug)]
pub(crate) enum PtzOperation {
    Continuous { pan: f32, tilt: f32, zoom: f32 },
    Relative { pan: f32, tilt: f32, zoom: f32 },
    Absolute { pan: f32, tilt: f32, zoom: f32 },
    Stop,
    GetPresets,
    GotoPreset { token: String },
}

#[derive(Clone, Debug)]
pub(crate) enum PtzResponse {
    Ack,
    Presets(Vec<PtzPreset>),
}

#[async_trait]
pub(crate) trait OnvifBackend: Send + Sync {
    async fn discover(
        &self,
        address: Option<&str>,
        timeout: Duration,
    ) -> Result<Vec<DiscoveredEndpoint>, OnvifFailure>;
    async fn inspect(
        &self,
        endpoint: &str,
        username: Option<&str>,
        password: Option<&str>,
        timeout: Duration,
    ) -> Result<OnvifInspection, OnvifFailure>;
    async fn ptz(
        &self,
        session: &PtzSession,
        operation: PtzOperation,
        timeout: Duration,
    ) -> Result<PtzResponse, OnvifFailure>;
}

#[derive(Clone)]
pub struct OnvifClient {
    backend: Arc<dyn OnvifBackend>,
}

impl Default for OnvifClient {
    fn default() -> Self {
        Self {
            backend: Arc::new(HttpOnvifBackend),
        }
    }
}

impl OnvifClient {
    #[allow(dead_code)]
    pub(crate) fn with_backend(mut self, backend: Arc<dyn OnvifBackend>) -> Self {
        self.backend = backend;
        self
    }

    pub async fn discover(
        &self,
        request: &OnvifDiscoveryRequest,
    ) -> Result<Vec<OnvifDevice>, OnvifFailure> {
        let timeout = bounded_timeout(request.timeout_ms);
        let endpoints = self
            .backend
            .discover(request.address.as_deref(), timeout)
            .await?;
        let mut devices = Vec::new();
        for endpoint in endpoints {
            let inspection = self
                .backend
                .inspect(
                    &endpoint.endpoint,
                    request.username.as_deref(),
                    request.password.as_deref(),
                    timeout,
                )
                .await?;
            let mut device = inspection.device;
            device.address = redact_endpoint(&device.address);
            for profile in &mut device.profiles {
                profile.rtsp_uri = profile.rtsp_uri.as_deref().map(redact_endpoint);
            }
            if device.manufacturer.is_none() {
                device.manufacturer = endpoint.name.clone();
            }
            if devices
                .iter()
                .all(|existing: &OnvifDevice| existing.id != device.id)
            {
                devices.push(device);
            }
        }
        Ok(devices)
    }

    pub(crate) async fn inspect(
        &self,
        request: &OnvifInspectRequest,
    ) -> Result<OnvifInspection, OnvifFailure> {
        let timeout = bounded_timeout(request.timeout_ms);
        let mut inspection = self
            .backend
            .inspect(
                &request.endpoint,
                request.username.as_deref(),
                request.password.as_deref(),
                timeout,
            )
            .await?;
        inspection.device.address = redact_endpoint(&inspection.device.address);
        inspection.device.profiles.iter_mut().for_each(|profile| {
            profile.rtsp_uri = profile.rtsp_uri.as_deref().map(redact_endpoint);
        });
        inspection.selected_rtsp_uri = inspection.selected_rtsp_uri.as_deref().map(redact_endpoint);
        Ok(inspection)
    }

    pub(crate) async fn ptz(
        &self,
        session: &PtzSession,
        operation: PtzOperation,
        timeout: Duration,
    ) -> Result<PtzResponse, OnvifFailure> {
        self.backend.ptz(session, operation, timeout).await
    }
}

fn bounded_timeout(timeout_ms: Option<u64>) -> Duration {
    Duration::from_millis(timeout_ms.unwrap_or(3000).clamp(100, 60_000))
}

fn redact_endpoint(value: &str) -> String {
    Url::parse(value)
        .map(|mut url| {
            let _ = url.set_username("");
            let _ = url.set_password(None);
            url.to_string()
        })
        .unwrap_or_else(|_| value.to_owned())
}

#[derive(Default)]
struct HttpOnvifBackend;

#[async_trait]
impl OnvifBackend for HttpOnvifBackend {
    async fn discover(
        &self,
        address: Option<&str>,
        duration: Duration,
    ) -> Result<Vec<DiscoveredEndpoint>, OnvifFailure> {
        let target = address.unwrap_or(DEFAULT_DISCOVERY_ADDRESS);
        let target_addr: SocketAddr = target.parse::<SocketAddr>().map_err(|error| {
            failure(
                OnvifFailureCode::ProtocolError,
                "The ONVIF discovery address is invalid.",
                Some(error.to_string()),
            )
        })?;
        let socket = UdpSocket::bind("0.0.0.0:0").await.map_err(|error| {
            failure(
                OnvifFailureCode::DeviceUnreachable,
                "ONVIF discovery could not open a network socket.",
                Some(error.to_string()),
            )
        })?;
        socket
            .send_to(PROBE.as_bytes(), target_addr)
            .await
            .map_err(|error| {
                failure(
                    OnvifFailureCode::DeviceUnreachable,
                    "ONVIF discovery request could not be sent.",
                    Some(error.to_string()),
                )
            })?;
        let deadline = tokio::time::Instant::now() + duration;
        let mut buffer = [0u8; 16 * 1024];
        let mut endpoints = Vec::new();
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                break;
            }
            let received = timeout(remaining, socket.recv_from(&mut buffer)).await;
            let Ok(Ok((length, _address))) = received else {
                break;
            };
            let xml = std::str::from_utf8(&buffer[..length]).map_err(|error| {
                failure(
                    OnvifFailureCode::MalformedSoap,
                    "ONVIF discovery returned malformed XML.",
                    Some(error.to_string()),
                )
            })?;
            let Some(endpoint) = tag_value(xml, "XAddrs") else {
                continue;
            };
            let name = tag_value(xml, "Scopes").and_then(|scopes| scope_name(&scopes));
            let endpoint = endpoint
                .split_whitespace()
                .next()
                .unwrap_or(&endpoint)
                .to_owned();
            if endpoints
                .iter()
                .all(|item: &DiscoveredEndpoint| item.endpoint != endpoint)
            {
                endpoints.push(DiscoveredEndpoint { endpoint, name });
            }
        }
        Ok(endpoints)
    }

    async fn inspect(
        &self,
        endpoint: &str,
        username: Option<&str>,
        password: Option<&str>,
        duration: Duration,
    ) -> Result<OnvifInspection, OnvifFailure> {
        let client = Client::builder()
            .timeout(duration)
            .build()
            .map_err(|error| {
                failure(
                    OnvifFailureCode::ProtocolError,
                    "ONVIF client could not be initialized.",
                    Some(error.to_string()),
                )
            })?;
        let device_info = soap_post(
            &client,
            endpoint,
            username,
            password,
            duration,
            "GetDeviceInformation",
            "<tds:GetDeviceInformation/>",
        )
        .await?;
        let manufacturer = tag_value(&device_info, "Manufacturer");
        let model = tag_value(&device_info, "Model");
        if manufacturer.is_none() && model.is_none() {
            return Err(failure(
                OnvifFailureCode::MalformedSoap,
                "ONVIF device information was incomplete.",
                None,
            ));
        }
        let capabilities_xml = soap_post(
            &client,
            endpoint,
            username,
            password,
            duration,
            "GetCapabilities",
            "<tds:GetCapabilities><tds:Category>All</tds:Category></tds:GetCapabilities>",
        )
        .await?;
        let media_endpoint = tag_value(&capabilities_xml, "MediaXAddr")
            .or_else(|| tag_value(&capabilities_xml, "XAddr"))
            .unwrap_or_else(|| endpoint.to_owned());
        let media_xml = soap_post(
            &client,
            &media_endpoint,
            username,
            password,
            duration,
            "GetProfiles",
            "<trt:GetProfiles/>",
        )
        .await?;
        let mut profiles = parse_profiles(&media_xml);
        if profiles.is_empty() {
            return Err(failure(
                OnvifFailureCode::MalformedSoap,
                "ONVIF returned no usable media profiles.",
                None,
            ));
        }
        for profile in &mut profiles {
            let token = profile.token.clone();
            if let Some(token) = token {
                let body = format!("<trt:GetStreamUri><trt:StreamSetup><tt:Stream>RTP-Unicast</tt:Stream><tt:Transport><tt:Protocol>RTSP</tt:Protocol></tt:Transport></trt:StreamSetup><trt:ProfileToken>{token}</trt:ProfileToken></trt:GetStreamUri>");
                if let Ok(uri_xml) = soap_post(
                    &client,
                    &media_endpoint,
                    username,
                    password,
                    duration,
                    "GetStreamUri",
                    &body,
                )
                .await
                {
                    profile.rtsp_uri = tag_value(&uri_xml, "Uri");
                }
            }
        }
        let mut capabilities = normalize_capabilities(&capabilities_xml, &media_xml, &profiles);
        if let Ok(snapshot_xml) = soap_post(
            &client,
            &media_endpoint,
            username,
            password,
            duration,
            "GetSnapshotUri",
            "<trt:GetSnapshotUri/>",
        )
        .await
        {
            capabilities.snapshot = tag_value(&snapshot_xml, "Uri").is_some();
        }
        let address = redact_endpoint(endpoint);
        let id = format!("onvif-{:x}", stable_hash(&address));
        let selected_rtsp_uri = profiles.iter().find_map(|profile| profile.rtsp_uri.clone());
        let ptz_session = if capabilities.ptz.supported {
            profiles
                .iter()
                .find(|profile| profile.rtsp_uri.is_some())
                .and_then(|profile| profile.token.clone())
                .map(|profile_token| PtzSession {
                    service_endpoint: tag_value(&capabilities_xml, "PTZXAddr")
                        .unwrap_or_else(|| endpoint.to_owned()),
                    profile_token,
                    username: username.map(str::to_owned),
                    password: password.map(str::to_owned),
                })
        } else {
            None
        };
        Ok(OnvifInspection {
            device: OnvifDevice {
                id,
                address,
                manufacturer,
                model,
                capabilities,
                profiles,
            },
            selected_rtsp_uri,
            ptz_session,
        })
    }

    async fn ptz(
        &self,
        session: &PtzSession,
        operation: PtzOperation,
        duration: Duration,
    ) -> Result<PtzResponse, OnvifFailure> {
        let client = Client::builder()
            .timeout(duration)
            .build()
            .map_err(|error| {
                failure(
                    OnvifFailureCode::ProtocolError,
                    "ONVIF client could not be initialized.",
                    Some(error.to_string()),
                )
            })?;
        let (action, body) = match operation {
            PtzOperation::Continuous { pan, tilt, zoom } => ("ContinuousMove", format!("<tptz:ContinuousMove><tptz:ProfileToken>{}</tptz:ProfileToken><tptz:Velocity><tt:PanTilt x=\"{pan}\" y=\"{tilt}\"/><tt:Zoom x=\"{zoom}\"/></tptz:Velocity></tptz:ContinuousMove>", xml_escape(&session.profile_token))),
            PtzOperation::Relative { pan, tilt, zoom } => ("RelativeMove", format!("<tptz:RelativeMove><tptz:ProfileToken>{}</tptz:ProfileToken><tptz:Translation><tt:PanTilt x=\"{pan}\" y=\"{tilt}\"/><tt:Zoom x=\"{zoom}\"/></tptz:Translation></tptz:RelativeMove>", xml_escape(&session.profile_token))),
            PtzOperation::Absolute { pan, tilt, zoom } => ("AbsoluteMove", format!("<tptz:AbsoluteMove><tptz:ProfileToken>{}</tptz:ProfileToken><tptz:Position><tt:PanTilt x=\"{pan}\" y=\"{tilt}\"/><tt:Zoom x=\"{zoom}\"/></tptz:Position></tptz:AbsoluteMove>", xml_escape(&session.profile_token))),
            PtzOperation::Stop => ("Stop", format!("<tptz:Stop><tptz:ProfileToken>{}</tptz:ProfileToken><tptz:PanTilt>true</tptz:PanTilt><tptz:Zoom>true</tptz:Zoom></tptz:Stop>", xml_escape(&session.profile_token))),
            PtzOperation::GetPresets => ("GetPresets", format!("<tptz:GetPresets><tptz:ProfileToken>{}</tptz:ProfileToken></tptz:GetPresets>", xml_escape(&session.profile_token))),
            PtzOperation::GotoPreset { token } => ("GotoPreset", format!("<tptz:GotoPreset><tptz:ProfileToken>{}</tptz:ProfileToken><tptz:PresetToken>{}</tptz:PresetToken></tptz:GotoPreset>", xml_escape(&session.profile_token), xml_escape(&token))),
        };
        let response = soap_post_with_ptz(
            &client,
            &session.service_endpoint,
            session.username.as_deref(),
            session.password.as_deref(),
            duration,
            action,
            &body,
        )
        .await?;
        if action == "GetPresets" {
            Ok(PtzResponse::Presets(parse_presets(&response)))
        } else {
            Ok(PtzResponse::Ack)
        }
    }
}

async fn soap_post(
    client: &Client,
    endpoint: &str,
    username: Option<&str>,
    password: Option<&str>,
    duration: Duration,
    action: &str,
    body: &str,
) -> Result<String, OnvifFailure> {
    let envelope = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope" xmlns:tds="http://www.onvif.org/ver10/device/wsdl" xmlns:trt="http://www.onvif.org/ver10/media/wsdl" xmlns:tt="http://www.onvif.org/ver10/schema"><s:Body>{body}</s:Body></s:Envelope>"#
    );
    let mut request = client
        .post(endpoint)
        .header("Content-Type", "application/soap+xml")
        .header("SOAPAction", action)
        .body(envelope);
    if let (Some(user), Some(pass)) = (username, password) {
        request = request.basic_auth(user, Some(pass));
    }
    let response = timeout(duration, request.send())
        .await
        .map_err(|_| {
            failure(
                OnvifFailureCode::DiscoveryTimeout,
                "ONVIF request timed out.",
                None,
            )
        })?
        .map_err(|error| {
            failure(
                OnvifFailureCode::DeviceUnreachable,
                "ONVIF device could not be reached.",
                Some(error.to_string()),
            )
        })?;
    if response.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Err(failure(
            OnvifFailureCode::AuthenticationFailed,
            "The camera rejected the supplied ONVIF credentials.",
            None,
        ));
    }
    if !response.status().is_success() {
        return Err(failure(
            OnvifFailureCode::ProtocolError,
            "The ONVIF service rejected the request.",
            Some(response.status().to_string()),
        ));
    }
    let text = response.text().await.map_err(|error| {
        failure(
            OnvifFailureCode::MalformedSoap,
            "ONVIF returned an unreadable response.",
            Some(error.to_string()),
        )
    })?;
    if !text.contains("Envelope") || !text.contains("Body") {
        return Err(failure(
            OnvifFailureCode::MalformedSoap,
            "ONVIF returned malformed SOAP/XML.",
            None,
        ));
    }
    Ok(text)
}

async fn soap_post_with_ptz(
    client: &Client,
    endpoint: &str,
    username: Option<&str>,
    password: Option<&str>,
    duration: Duration,
    action: &str,
    body: &str,
) -> Result<String, OnvifFailure> {
    let envelope = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope" xmlns:tptz="http://www.onvif.org/ver20/ptz/wsdl" xmlns:tt="http://www.onvif.org/ver10/schema"><s:Body>{body}</s:Body></s:Envelope>"#
    );
    let mut request = client
        .post(endpoint)
        .header("Content-Type", "application/soap+xml")
        .header("SOAPAction", action)
        .body(envelope);
    if let (Some(user), Some(pass)) = (username, password) {
        request = request.basic_auth(user, Some(pass));
    }
    let response = timeout(duration, request.send())
        .await
        .map_err(|_| {
            failure(
                OnvifFailureCode::DiscoveryTimeout,
                "ONVIF PTZ request timed out.",
                None,
            )
        })?
        .map_err(|error| {
            failure(
                OnvifFailureCode::DeviceUnreachable,
                "The ONVIF PTZ service could not be reached.",
                Some(error.to_string()),
            )
        })?;
    if response.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Err(failure(
            OnvifFailureCode::AuthenticationFailed,
            "The camera rejected the supplied ONVIF credentials.",
            None,
        ));
    }
    if !response.status().is_success() {
        return Err(failure(
            OnvifFailureCode::ProtocolError,
            "The ONVIF PTZ service rejected the request.",
            Some(response.status().to_string()),
        ));
    }
    let text = response.text().await.map_err(|error| {
        failure(
            OnvifFailureCode::MalformedSoap,
            "ONVIF returned an unreadable PTZ response.",
            Some(error.to_string()),
        )
    })?;
    if !text.contains("Envelope") || !text.contains("Body") {
        return Err(failure(
            OnvifFailureCode::MalformedSoap,
            "ONVIF returned malformed PTZ SOAP/XML.",
            None,
        ));
    }
    Ok(text)
}

fn parse_presets(xml: &str) -> Vec<PtzPreset> {
    let mut presets = Vec::new();
    let mut cursor = 0;
    while let Some(start) = find_open_tag(xml, "Preset", cursor) {
        let Some(open_end) = xml[start..].find('>').map(|offset| start + offset + 1) else {
            break;
        };
        let Some(close_end) = find_close_tag(xml, open_end, "Preset") else {
            break;
        };
        let block = &xml[start..close_end];
        let token = attribute(&xml[start..open_end], "token");
        let name = tag_value(block, "Name").unwrap_or_else(|| "Preset".into());
        if let Some(token) = token {
            let id = format!("preset-{:x}", stable_hash(&token));
            presets.push(PtzPreset {
                id,
                name,
                token: Some(token),
            });
        }
        cursor = close_end;
    }
    presets
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn normalize_capabilities(
    capabilities: &str,
    media: &str,
    profiles: &[MediaProfile],
) -> CameraCapabilities {
    let ptz_supported = contains_any(capabilities, &["PTZ", "PTZXAddr"]);
    let ptz = PtzCapabilities {
        supported: ptz_supported,
        pan: ptz_supported && contains_any(capabilities, &["PanTilt", "Pan"]),
        tilt: ptz_supported && contains_any(capabilities, &["PanTilt", "Tilt"]),
        zoom: ptz_supported && contains_any(capabilities, &["Zoom"]),
        continuous_move: ptz_supported && contains_any(capabilities, &["Continuous"]),
        relative_move: ptz_supported && contains_any(capabilities, &["Relative"]),
        absolute_move: ptz_supported && contains_any(capabilities, &["Absolute"]),
        presets: ptz_supported && contains_any(capabilities, &["Preset"]),
    };
    CameraCapabilities {
        video: !profiles.is_empty(),
        audio: profiles.iter().any(|profile| profile.audio)
            || contains_any(media, &["AudioSourceConfiguration"]),
        snapshot: false,
        events: contains_any(capabilities, &["Events", "EventXAddr"]),
        ptz,
    }
}

fn parse_profiles(xml: &str) -> Vec<MediaProfile> {
    let mut profiles = Vec::new();
    let mut cursor = 0;
    while let Some(start) = find_open_tag(xml, "Profile", cursor) {
        let Some(end_start) = xml[start..].find('>') else {
            break;
        };
        let open_end = start + end_start + 1;
        let Some(close_offset) = find_close_tag(xml, open_end, "Profile") else {
            break;
        };
        let close_end = close_offset;
        let block = &xml[start..close_end];
        if block.starts_with("<")
            && !block.starts_with("</")
            && !xml[start..open_end].contains("Profiles")
        {
            let name = tag_value(block, "Name").unwrap_or_else(|| "ONVIF profile".into());
            let encoding = tag_value(block, "Encoding");
            let width = tag_value(block, "Width");
            let height = tag_value(block, "Height");
            let resolution = width.zip(height).map(|(w, h)| format!("{w}x{h}"));
            let frame_rate =
                tag_value(block, "FrameRateLimit").and_then(|value| value.parse().ok());
            profiles.push(MediaProfile {
                name,
                encoding,
                resolution,
                frame_rate,
                audio: contains_any(
                    block,
                    &["AudioSourceConfiguration", "AudioEncoderConfiguration"],
                ),
                rtsp_uri: None,
                token: attribute(&xml[start..open_end], "token"),
            });
        }
        cursor = close_end;
    }
    profiles
}

fn find_close_tag(xml: &str, from: usize, name: &str) -> Option<usize> {
    let mut cursor = from;
    while let Some(offset) = xml[cursor..].find("</") {
        let index = cursor + offset;
        let end = xml[index..].find('>')? + index;
        let local_name = xml[index + 2..end].trim().rsplit(':').next()?;
        if local_name == name {
            return Some(end + 1);
        }
        cursor = end + 1;
    }
    None
}

fn find_open_tag(xml: &str, name: &str, from: usize) -> Option<usize> {
    let mut cursor = from;
    while let Some(offset) = xml[cursor..].find('<') {
        let index = cursor + offset;
        let rest = &xml[index..];
        if rest.starts_with("</") {
            cursor = index + 2;
            continue;
        }
        let local_name = rest
            .strip_prefix('<')
            .and_then(|value| {
                value
                    .split(|c: char| c.is_ascii_whitespace() || c == '>' || c == '/')
                    .next()
            })
            .and_then(|value| value.rsplit(':').next());
        if local_name == Some(name) {
            return Some(index);
        }
        cursor = index + 1;
    }
    None
}

fn attribute(open_tag: &str, name: &str) -> Option<String> {
    let marker = format!("{name}=\"");
    let start = open_tag.find(&marker)? + marker.len();
    Some(open_tag[start..].split('"').next()?.to_owned())
}

fn tag_value(xml: &str, name: &str) -> Option<String> {
    let open = find_open_tag(xml, name, 0)?;
    let end = xml[open..].find('>')? + open + 1;
    let close = format!("</{name}>");
    let close_index = xml[end..]
        .find(&close)
        .map(|value| end + value)
        .or_else(|| {
            let suffix = xml[end..].find("</")?;
            Some(end + suffix)
        })?;
    let value = xml[end..close_index].trim();
    (!value.is_empty()).then(|| html_unescape(value))
}

fn contains_any(value: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| value.contains(needle))
}

fn scope_name(scopes: &str) -> Option<String> {
    scopes.split_whitespace().find_map(|scope| {
        scope
            .strip_prefix("onvif://www.onvif.org/name/")
            .map(|value| value.replace('%', " "))
    })
}

fn html_unescape(value: &str) -> String {
    value
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
}

fn stable_hash(value: &str) -> u64 {
    value.bytes().fold(1469598103934665603, |hash, byte| {
        (hash ^ byte as u64).wrapping_mul(1099511628211)
    })
}

fn failure(
    code: OnvifFailureCode,
    message: &str,
    technical_detail: Option<String>,
) -> OnvifFailure {
    OnvifFailure {
        code,
        message: message.into(),
        technical_detail,
    }
}

#[cfg(test)]
fn ptz_envelope(operation: &PtzOperation) -> String {
    let body = match operation {
        PtzOperation::Continuous { .. } => "<tptz:ContinuousMove/>",
        PtzOperation::Relative { .. } => "<tptz:RelativeMove/>",
        PtzOperation::Absolute { .. } => "<tptz:AbsoluteMove/>",
        PtzOperation::Stop => "<tptz:Stop/>",
        PtzOperation::GetPresets => "<tptz:GetPresets/>",
        PtzOperation::GotoPreset { .. } => "<tptz:GotoPreset/>",
    };
    format!(
        r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope" xmlns:tptz="http://www.onvif.org/ver20/ptz/wsdl"><s:Body>{body}</s:Body></s:Envelope>"#
    )
}

#[cfg(test)]
pub(crate) mod emulator {
    use super::*;
    use std::sync::Mutex;

    #[derive(Clone)]
    pub struct OnvifEmulator {
        pub devices: Arc<Mutex<Vec<OnvifInspection>>>,
        pub failure: Option<OnvifFailure>,
        pub soap_operations: Arc<Mutex<Vec<String>>>,
        pub presets: Arc<Mutex<Vec<PtzPreset>>>,
    }

    impl OnvifEmulator {
        pub fn new(devices: Vec<OnvifInspection>) -> Self {
            Self {
                devices: Arc::new(Mutex::new(devices)),
                failure: None,
                soap_operations: Arc::new(Mutex::new(Vec::new())),
                presets: Arc::new(Mutex::new(Vec::new())),
            }
        }
        pub fn failing(failure: OnvifFailure) -> Self {
            Self {
                devices: Arc::new(Mutex::new(Vec::new())),
                failure: Some(failure),
                soap_operations: Arc::new(Mutex::new(Vec::new())),
                presets: Arc::new(Mutex::new(Vec::new())),
            }
        }
    }

    #[async_trait]
    impl OnvifBackend for OnvifEmulator {
        async fn discover(
            &self,
            _address: Option<&str>,
            _timeout: Duration,
        ) -> Result<Vec<DiscoveredEndpoint>, OnvifFailure> {
            if let Some(failure) = &self.failure {
                return Err(failure.clone());
            }
            Ok(self
                .devices
                .lock()
                .unwrap()
                .iter()
                .map(|device| DiscoveredEndpoint {
                    endpoint: device.device.address.clone(),
                    name: device.device.manufacturer.clone(),
                })
                .collect())
        }
        async fn inspect(
            &self,
            endpoint: &str,
            _username: Option<&str>,
            _password: Option<&str>,
            _timeout: Duration,
        ) -> Result<OnvifInspection, OnvifFailure> {
            if let Some(failure) = &self.failure {
                return Err(failure.clone());
            }
            self.devices
                .lock()
                .unwrap()
                .iter()
                .find(|device| device.device.address == endpoint)
                .cloned()
                .ok_or_else(|| {
                    failure(
                        OnvifFailureCode::DeviceUnreachable,
                        "The emulator device was not found.",
                        None,
                    )
                })
        }

        async fn ptz(
            &self,
            session: &PtzSession,
            operation: PtzOperation,
            _timeout: Duration,
        ) -> Result<PtzResponse, OnvifFailure> {
            if let Some(failure) = &self.failure {
                return Err(failure.clone());
            }
            let soap = ptz_envelope(&operation);
            self.soap_operations.lock().unwrap().push(soap.clone());
            if !soap.contains("tptz:") {
                return Err(failure(
                    OnvifFailureCode::MalformedSoap,
                    "PTZ emulator received malformed SOAP.",
                    None,
                ));
            }
            if let Some(device) = self
                .devices
                .lock()
                .unwrap()
                .iter()
                .find(|device| device.device.address == session.service_endpoint)
            {
                if !device.device.capabilities.ptz.supported {
                    return Err(failure(
                        OnvifFailureCode::Unsupported,
                        "The emulator device does not support PTZ.",
                        None,
                    ));
                }
            }
            match operation {
                PtzOperation::GetPresets => {
                    Ok(PtzResponse::Presets(self.presets.lock().unwrap().clone()))
                }
                _ => Ok(PtzResponse::Ack),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{emulator::OnvifEmulator, *};

    fn fixture(ptz: bool, address: &str) -> OnvifInspection {
        let capabilities = CameraCapabilities {
            video: true,
            audio: true,
            snapshot: false,
            events: true,
            ptz: PtzCapabilities {
                supported: ptz,
                pan: ptz,
                tilt: ptz,
                zoom: ptz,
                continuous_move: ptz,
                relative_move: ptz,
                absolute_move: ptz,
                presets: ptz,
            },
        };
        OnvifInspection {
            device: OnvifDevice {
                id: format!("onvif-{}", stable_hash(address)),
                address: address.into(),
                manufacturer: Some("Sentinel Emulator".into()),
                model: Some(if ptz { "PTZ-100" } else { "Fixed-100" }.into()),
                capabilities,
                profiles: vec![
                    MediaProfile {
                        name: "Main".into(),
                        encoding: Some("H264".into()),
                        resolution: Some("1920x1080".into()),
                        frame_rate: Some(25.0),
                        audio: true,
                        rtsp_uri: Some("rtsp://admin:secret@camera/main".into()),
                        token: Some("main-token".into()),
                    },
                    MediaProfile {
                        name: "Sub".into(),
                        encoding: Some("H264".into()),
                        resolution: Some("640x360".into()),
                        frame_rate: Some(15.0),
                        audio: false,
                        rtsp_uri: Some("rtsp://camera/sub".into()),
                        token: Some("sub-token".into()),
                    },
                ],
            },
            selected_rtsp_uri: Some("rtsp://admin:secret@camera/main".into()),
            ptz_session: ptz.then(|| PtzSession {
                service_endpoint: address.into(),
                profile_token: "main-token".into(),
                username: Some("admin".into()),
                password: Some("secret".into()),
            }),
        }
    }

    #[tokio::test]
    async fn emulator_discovery_normalizes_device_profiles_and_ptz() {
        let emulator = OnvifEmulator::new(vec![fixture(true, "http://camera-a/onvif")]);
        let client = OnvifClient::default().with_backend(Arc::new(emulator));
        let devices = client
            .discover(&OnvifDiscoveryRequest {
                address: Some("127.0.0.1:3702".into()),
                username: Some("admin".into()),
                password: Some("secret".into()),
                timeout_ms: Some(100),
            })
            .await
            .unwrap();
        assert_eq!(devices.len(), 1);
        assert_eq!(
            devices[0].manufacturer.as_deref(),
            Some("Sentinel Emulator")
        );
        assert_eq!(devices[0].profiles.len(), 2);
        assert!(devices[0].capabilities.video);
        assert!(devices[0].capabilities.audio);
        assert!(devices[0].capabilities.events);
        assert!(devices[0].capabilities.ptz.supported);
        let json = serde_json::to_string(&devices[0]).unwrap();
        assert!(!json.contains("main-token"));
        assert!(!json.contains("secret"));
    }

    #[tokio::test]
    async fn emulator_supports_fixed_camera_without_ptz() {
        let emulator = OnvifEmulator::new(vec![fixture(false, "http://fixed-camera/onvif")]);
        let client = OnvifClient::default().with_backend(Arc::new(emulator));
        let devices = client
            .discover(&OnvifDiscoveryRequest {
                address: None,
                username: None,
                password: None,
                timeout_ms: None,
            })
            .await
            .unwrap();
        assert!(!devices[0].capabilities.ptz.supported);
        assert!(!devices[0].capabilities.ptz.pan);
    }

    #[tokio::test]
    async fn emulator_returns_no_devices_deterministically() {
        let client = OnvifClient::default().with_backend(Arc::new(OnvifEmulator::new(Vec::new())));
        let devices = client
            .discover(&OnvifDiscoveryRequest {
                address: None,
                username: None,
                password: None,
                timeout_ms: Some(100),
            })
            .await
            .unwrap();
        assert!(devices.is_empty());
    }

    #[tokio::test]
    async fn emulator_receives_expected_soap_for_all_ptz_operations() {
        let emulator = Arc::new(OnvifEmulator::new(vec![fixture(true, "http://ptz/onvif")]));
        emulator.presets.lock().unwrap().push(PtzPreset {
            id: "preset-1".into(),
            name: "Entrance".into(),
            token: Some("token-1".into()),
        });
        let client = OnvifClient::default().with_backend(emulator.clone());
        let session = PtzSession {
            service_endpoint: "http://ptz/onvif".into(),
            profile_token: "profile-token".into(),
            username: Some("admin".into()),
            password: Some("secret".into()),
        };
        for operation in [
            PtzOperation::Continuous {
                pan: 0.5,
                tilt: 0.0,
                zoom: 0.0,
            },
            PtzOperation::Relative {
                pan: 0.1,
                tilt: -0.1,
                zoom: 0.0,
            },
            PtzOperation::Absolute {
                pan: 0.0,
                tilt: 0.1,
                zoom: 0.0,
            },
            PtzOperation::Stop,
            PtzOperation::GotoPreset {
                token: "token-1".into(),
            },
        ] {
            assert!(matches!(
                client
                    .ptz(&session, operation, Duration::from_millis(100))
                    .await
                    .unwrap(),
                PtzResponse::Ack
            ));
        }
        let presets = client
            .ptz(
                &session,
                PtzOperation::GetPresets,
                Duration::from_millis(100),
            )
            .await
            .unwrap();
        assert!(matches!(presets, PtzResponse::Presets(values) if values[0].id == "preset-1"));
        let operations = emulator.soap_operations.lock().unwrap().clone();
        for action in [
            "ContinuousMove",
            "RelativeMove",
            "AbsoluteMove",
            "Stop",
            "GotoPreset",
            "GetPresets",
        ] {
            assert!(operations
                .iter()
                .any(|soap| soap.contains(&format!("tptz:{action}"))));
        }
        assert!(operations
            .iter()
            .all(|soap| !soap.contains("secret") && !soap.contains("profile-token")));
    }

    #[tokio::test]
    async fn emulator_rejects_ptz_for_fixed_device_and_preserves_failures() {
        let emulator = Arc::new(OnvifEmulator::new(vec![fixture(
            false,
            "http://fixed/onvif",
        )]));
        let client = OnvifClient::default().with_backend(emulator);
        let error = client
            .ptz(
                &PtzSession {
                    service_endpoint: "http://fixed/onvif".into(),
                    profile_token: "token".into(),
                    username: None,
                    password: None,
                },
                PtzOperation::Stop,
                Duration::from_millis(100),
            )
            .await
            .unwrap_err();
        assert_eq!(error.code, OnvifFailureCode::Unsupported);
        let error = OnvifClient::default()
            .with_backend(Arc::new(OnvifEmulator::failing(OnvifFailure {
                code: OnvifFailureCode::AuthenticationFailed,
                message: "rejected".into(),
                technical_detail: None,
            })))
            .ptz(
                &PtzSession {
                    service_endpoint: "http://ptz".into(),
                    profile_token: "token".into(),
                    username: None,
                    password: None,
                },
                PtzOperation::Stop,
                Duration::from_millis(100),
            )
            .await
            .unwrap_err();
        assert_eq!(error.code, OnvifFailureCode::AuthenticationFailed);
    }

    #[tokio::test]
    async fn emulator_failure_categories_are_stable() {
        for code in [
            OnvifFailureCode::DiscoveryTimeout,
            OnvifFailureCode::AuthenticationFailed,
            OnvifFailureCode::MalformedSoap,
        ] {
            let client = OnvifClient::default().with_backend(Arc::new(OnvifEmulator::failing(
                OnvifFailure {
                    code: code.clone(),
                    message: "fixture failure".into(),
                    technical_detail: None,
                },
            )));
            let error = client
                .discover(&OnvifDiscoveryRequest {
                    address: None,
                    username: None,
                    password: None,
                    timeout_ms: Some(100),
                })
                .await
                .unwrap_err();
            assert_eq!(error.code, code);
        }
    }

    #[test]
    fn malformed_xml_does_not_create_capabilities_or_profiles() {
        assert!(parse_profiles("<Envelope><Body><Profiles>").is_empty());
        let capabilities = normalize_capabilities("<Envelope/>", "<Envelope/>", &[]);
        assert!(!capabilities.video);
        assert!(!capabilities.ptz.supported);
    }
}
