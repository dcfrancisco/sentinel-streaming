# ONVIF discovery and capability normalization

SS-WP-005 implements ONVIF discovery and inspection behind `src/onvif.rs`.

## Flow

1. Sentinel sends a WS-Discovery `Probe` request.
2. Returned device service addresses are inspected with ONVIF SOAP requests.
3. Device information, service capabilities, media profiles, and stream URIs are retrieved.
4. Results are normalized into Sentinel device information, media profiles, and `CameraCapabilities`.
5. A selected media profile can be passed to the existing RTSP validator through the camera inspection API.

Discovery does not auto-register every device. It returns candidates for a later setup flow.

## API

`POST /api/v1/onvif/discover`

```json
{
  "discovery_address": "239.255.255.250:3702",
  "username": "admin",
  "password": "provided-at-request-time"
}
```

The address and credentials are optional. Credentials are not returned. Normal responses include manufacturer/model information, normalized capabilities, and media profile metadata. ONVIF profile tokens are kept internal and are omitted from serialized API responses.

`POST /api/v1/sources/{id}/onvif/inspect` inspects a known ONVIF endpoint, updates the existing source's normalized capabilities, and validates the first usable RTSP URI with the existing `RtspValidator`.

`GET /api/v1/sources/{id}/capabilities` returns the normalized capability model.

## Capability rules

Video and audio are derived from returned media profiles. Events and PTZ are exposed only when the corresponding ONVIF service is advertised. PTZ movement dimensions are populated only when the inspected capability response provides matching information; the presence of a camera model name never implies PTZ support.

SS-WP-005 detects and normalizes PTZ capability. SS-WP-006 adds capability-gated PTZ movement, stop, zoom, and preset operations through the same ONVIF adapter. It does not implement physical-camera certification or a full operator console.

## Security and limitations

SOAP/XML, service endpoints, and profile tokens are internal protocol details. Stream URIs and device endpoints are redacted before normal API serialization. Credentials are used only for the request and are not persisted by this work package.

The current implementation uses HTTP ONVIF services and WS-Discovery with HTTP
Basic authentication where provided. HTTPS certificate policy, WS-Security
username tokens, vendor-specific SOAP extensions, multicast-network quirks,
and physical-camera certification remain future work.

## Testing classification

- `UNIT`: XML normalization and capability parsing;
- `PROTOCOL`: raw WS-Discovery, HTTP/SOAP, and RTSP exchange behavior;
- `EMULATOR_INTEGRATION`: deterministic in-process UDP/TCP ONVIF emulator exercising discovery, authentication, profiles, stream URIs, PTZ variants, timeout, and RTSP handoff.

These tests are not physical-device validation.
