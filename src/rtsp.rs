use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::Serialize;
use std::{io, sync::Arc, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    time::{timeout, Instant},
};
use url::Url;

const MAX_RESPONSE_BYTES: usize = 64 * 1024;

#[derive(Clone)]
pub struct RtspValidationRequest {
    pub uri: String,
    pub username: Option<String>,
    pub password: Option<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RtspFailureCode {
    AuthenticationFailed,
    SourceUnreachable,
    StreamNotFound,
    ConnectionTimeout,
    ProtocolError,
    InvalidSource,
    UnsupportedSource,
    Unknown,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct RtspFailure {
    pub code: RtspFailureCode,
    pub message: String,
    pub technical_detail: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct RtspValidationDetails {
    pub rtsp_reachable: bool,
    pub options_status: Option<u16>,
    pub describe_status: Option<u16>,
}

#[derive(Clone, Debug, Serialize)]
pub struct RtspValidationResult {
    pub valid: bool,
    pub checked_at: u128,
    pub latency_ms: u64,
    pub details: RtspValidationDetails,
    pub failure: Option<RtspFailure>,
}

#[async_trait]
pub trait RtspValidationBackend: Send + Sync {
    async fn validate(
        &self,
        request: &RtspValidationRequest,
        timeout: Duration,
    ) -> Result<(u16, u16), RtspFailure>;
}

#[derive(Default)]
struct TcpRtspValidationBackend;

#[async_trait]
impl RtspValidationBackend for TcpRtspValidationBackend {
    async fn validate(
        &self,
        request: &RtspValidationRequest,
        timeout: Duration,
    ) -> Result<(u16, u16), RtspFailure> {
        validate_inner(timeout, request).await
    }
}

#[derive(Clone)]
pub struct RtspValidator {
    timeout: Duration,
    backend: Arc<dyn RtspValidationBackend>,
}

impl Default for RtspValidator {
    fn default() -> Self {
        Self::new(Duration::from_secs(5))
    }
}

impl RtspValidator {
    pub fn new(timeout: Duration) -> Self {
        Self {
            timeout: timeout.max(Duration::from_millis(1)),
            backend: Arc::new(TcpRtspValidationBackend),
        }
    }

    pub fn with_backend(mut self, backend: Arc<dyn RtspValidationBackend>) -> Self {
        self.backend = backend;
        self
    }

    pub async fn validate(&self, request: RtspValidationRequest) -> RtspValidationResult {
        let started = Instant::now();
        let checked_at = now_ms();
        let details = RtspValidationDetails {
            rtsp_reachable: false,
            options_status: None,
            describe_status: None,
        };
        let result = match self.backend.validate(&request, self.timeout).await {
            Ok((options_status, describe_status)) => RtspValidationResult {
                valid: true,
                checked_at,
                latency_ms: started.elapsed().as_millis() as u64,
                details: RtspValidationDetails {
                    rtsp_reachable: true,
                    options_status: Some(options_status),
                    describe_status: Some(describe_status),
                },
                failure: None,
            },
            Err(failure) => {
                let reachable = matches!(
                    failure.code,
                    RtspFailureCode::AuthenticationFailed
                        | RtspFailureCode::StreamNotFound
                        | RtspFailureCode::ProtocolError
                );
                RtspValidationResult {
                    valid: false,
                    checked_at,
                    latency_ms: started.elapsed().as_millis() as u64,
                    details: RtspValidationDetails {
                        rtsp_reachable: reachable,
                        ..details
                    },
                    failure: Some(failure),
                }
            }
        };
        result
    }
}

async fn validate_inner(
    request_timeout: Duration,
    request: &RtspValidationRequest,
) -> Result<(u16, u16), RtspFailure> {
    let url = Url::parse(&request.uri).map_err(|error| {
        failure(
            RtspFailureCode::InvalidSource,
            "The RTSP source address is malformed.",
            Some(error.to_string()),
        )
    })?;
    if url.scheme() != "rtsp" || url.host_str().is_none() {
        return Err(failure(
            RtspFailureCode::InvalidSource,
            "The source must be a valid rtsp:// address.",
            None,
        ));
    }
    if url.username() != "" || url.password().is_some() {
        return Err(failure(
            RtspFailureCode::InvalidSource,
            "RTSP credentials must be supplied through credential configuration.",
            None,
        ));
    }
    let host = url.host_str().ok_or_else(|| {
        failure(
            RtspFailureCode::InvalidSource,
            "The RTSP source has no host.",
            None,
        )
    })?;
    let port = url.port().unwrap_or(554);
    let address = format!("{host}:{port}");
    let stream = timeout(request_timeout, TcpStream::connect(address))
        .await
        .map_err(|_| {
            failure(
                RtspFailureCode::ConnectionTimeout,
                "The RTSP connection timed out.",
                None,
            )
        })?
        .map_err(classify_io_error)?;
    let request_uri = url.to_string();
    let authorization = basic_authorization(request);
    let options = send_request(
        stream,
        "OPTIONS",
        &request_uri,
        authorization.as_deref(),
        request_timeout,
    )
    .await?;
    if options.status == 401 || options.status == 403 {
        return Err(failure(
            RtspFailureCode::AuthenticationFailed,
            "The camera rejected the supplied credentials.",
            Some(format!(
                "OPTIONS returned HTTP-like RTSP status {}",
                options.status
            )),
        ));
    }
    if !(200..300).contains(&options.status) {
        return Err(classify_status(options.status, "OPTIONS"));
    }

    let stream = options.stream;
    let describe = send_request(
        stream,
        "DESCRIBE",
        &request_uri,
        authorization.as_deref(),
        request_timeout,
    )
    .await?;
    if describe.status == 401 || describe.status == 403 {
        return Err(failure(
            RtspFailureCode::AuthenticationFailed,
            "The camera rejected the supplied credentials.",
            Some(format!("DESCRIBE returned RTSP status {}", describe.status)),
        ));
    }
    if describe.status == 404 || describe.status == 454 {
        return Err(failure(
            RtspFailureCode::StreamNotFound,
            "The RTSP stream was not found.",
            Some(format!("DESCRIBE returned RTSP status {}", describe.status)),
        ));
    }
    if !(200..300).contains(&describe.status) {
        return Err(classify_status(describe.status, "DESCRIBE"));
    }
    Ok((options.status, describe.status))
}

struct RtspResponse {
    status: u16,
    stream: TcpStream,
}

async fn send_request(
    mut stream: TcpStream,
    method: &str,
    uri: &str,
    authorization: Option<&str>,
    request_timeout: Duration,
) -> Result<RtspResponse, RtspFailure> {
    let cseq = if method == "OPTIONS" { 1 } else { 2 };
    let mut request =
        format!("{method} {uri} RTSP/1.0\r\nCSeq: {cseq}\r\nUser-Agent: sentinel-streaming\r\n");
    if let Some(authorization) = authorization {
        request.push_str(&format!("Authorization: {authorization}\r\n"));
    }
    request.push_str("\r\n");
    timeout(request_timeout, stream.write_all(request.as_bytes()))
        .await
        .map_err(|_| {
            failure(
                RtspFailureCode::ConnectionTimeout,
                "The RTSP request timed out.",
                None,
            )
        })?
        .map_err(classify_io_error)?;

    let mut response = Vec::with_capacity(1024);
    let mut byte = [0u8; 1];
    loop {
        let read = timeout(request_timeout, stream.read(&mut byte))
            .await
            .map_err(|_| {
                failure(
                    RtspFailureCode::ConnectionTimeout,
                    "The RTSP response timed out.",
                    None,
                )
            })?
            .map_err(classify_io_error)?;
        if read == 0 {
            return Err(failure(
                RtspFailureCode::ProtocolError,
                "The RTSP endpoint closed the connection before responding.",
                None,
            ));
        }
        response.push(byte[0]);
        if response.len() > MAX_RESPONSE_BYTES {
            return Err(failure(
                RtspFailureCode::ProtocolError,
                "The RTSP response was too large.",
                None,
            ));
        }
        if response.ends_with(b"\r\n\r\n") {
            break;
        }
    }
    let text = String::from_utf8_lossy(&response);
    let mut parts = text.lines().next().unwrap_or_default().split_whitespace();
    let protocol = parts.next().unwrap_or_default();
    let status = parts
        .next()
        .unwrap_or_default()
        .parse::<u16>()
        .map_err(|_| {
            failure(
                RtspFailureCode::ProtocolError,
                "The RTSP endpoint returned an invalid status line.",
                None,
            )
        })?;
    if protocol != "RTSP/1.0" {
        return Err(failure(
            RtspFailureCode::ProtocolError,
            "The endpoint did not return a valid RTSP response.",
            None,
        ));
    }
    Ok(RtspResponse { status, stream })
}

fn basic_authorization(request: &RtspValidationRequest) -> Option<String> {
    match (&request.username, &request.password) {
        (Some(username), Some(password)) => Some(format!(
            "Basic {}",
            STANDARD.encode(format!("{username}:{password}"))
        )),
        _ => None,
    }
}

fn classify_status(status: u16, method: &str) -> RtspFailure {
    let (code, message) = if status == 404 || status == 454 {
        (
            RtspFailureCode::StreamNotFound,
            "The RTSP stream was not found.",
        )
    } else if (400..500).contains(&status) {
        (
            RtspFailureCode::ProtocolError,
            "The RTSP endpoint rejected the request.",
        )
    } else {
        (
            RtspFailureCode::ProtocolError,
            "The RTSP endpoint returned an unexpected response.",
        )
    };
    failure(
        code,
        message,
        Some(format!("{method} returned RTSP status {status}")),
    )
}

fn classify_io_error(error: io::Error) -> RtspFailure {
    let code = match error.kind() {
        io::ErrorKind::TimedOut => RtspFailureCode::ConnectionTimeout,
        io::ErrorKind::ConnectionRefused
        | io::ErrorKind::ConnectionReset
        | io::ErrorKind::ConnectionAborted
        | io::ErrorKind::NotFound
        | io::ErrorKind::AddrNotAvailable
        | io::ErrorKind::HostUnreachable
        | io::ErrorKind::NetworkUnreachable => RtspFailureCode::SourceUnreachable,
        _ => RtspFailureCode::Unknown,
    };
    failure(
        code,
        "The RTSP source could not be reached.",
        Some(error.to_string()),
    )
}

fn failure(
    code: RtspFailureCode,
    message: impl Into<String>,
    technical_detail: Option<String>,
) -> RtspFailure {
    RtspFailure {
        code,
        message: message.into(),
        technical_detail,
    }
}

fn now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}
