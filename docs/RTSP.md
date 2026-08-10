# RTSP Validation

SS-WP-003 adds explicit validation for registered `rtsp` sources through:

```text
POST /api/v1/sources/{id}/validate
```

The validator is a small Rust/Tokio RTSP protocol adapter. It establishes a
bounded TCP connection, sends `OPTIONS`, then `DESCRIBE`, and checks the RTSP
status responses. It does not invoke FFmpeg and does not decode media frames.

## What validation proves

A successful result proves that the configured RTSP endpoint responded to the
protocol handshake and accepted the requested resource sufficiently for
validation.

It does not prove:

- sustained video decoding;
- frame rate or resolution stability;
- audio availability;
- ONVIF support;
- PTZ support;
- browser playback;
- AI readiness;
- long-duration reliability.

## Normalized failures

- `AUTHENTICATION_FAILED` — the endpoint returned an authentication rejection.
- `SOURCE_UNREACHABLE` — the connection could not be established.
- `STREAM_NOT_FOUND` — the requested RTSP resource was not found.
- `CONNECTION_TIMEOUT` — connect, write, or response read exceeded the timeout.
- `PROTOCOL_ERROR` — the response was not a valid or expected RTSP response.
- `INVALID_SOURCE` — the configured address is not a valid credential-free
  `rtsp://` source identity.
- `UNKNOWN` — an otherwise unclassified transport failure.

Technical details are retained only as diagnostics. The source model and user
messages use the stable normalized category/message.

## Timeouts and credentials

The default validation timeout is five seconds. Configure it with
`rtsp_validation_timeout_ms` or `SENTINEL_RTSP_VALIDATION_TIMEOUT_MS` between
100 and 60000 milliseconds.

Credentials are supplied through the existing credential configuration and are
never included in `SourceInfo`, validation responses, admin HTML, or validation
messages. RTSP URLs containing embedded credentials are rejected by validation.

## Testing classification

The deterministic tests use an injectable validation backend:

- protocol/unit behavior: normalized success and failure mapping;
- integration behavior: source lifecycle/health and API wiring;
- no physical-camera or sustained-media claims.
