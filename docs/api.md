# Administration API and CLI

The default server address is `0.0.0.0:8080`. A local development server can
use `127.0.0.1:8081` when port 8080 is occupied.

## Authentication

Protected routes require `Authorization: Bearer <token>`. Configure
`SENTINEL_VIEWER_TOKEN`, `SENTINEL_OPERATOR_TOKEN`, `SENTINEL_ADMIN_TOKEN`, or
the legacy-compatible `SENTINEL_API_TOKEN`. A first-run administrator may use
an explicitly configured `SENTINEL_BOOTSTRAP_TOKEN`; Sentinel has no default
credential. All routes except the following are protected:

- `GET /health/live`
- `GET /health/ready`
- `GET /api/v1/version`

Protected requests use:

```http
Authorization: Bearer <token>
```

The CLI can store a token through its profile/auth workflow. Do not put tokens
in shell history, source files, or committed configuration.

With no token configured the service preserves an explicitly local,
camera-free development mode for backwards-compatible quick starts. Do not
bind that mode to a shared or remote network.

`GET /api/v1/auth/whoami` returns the authenticated principal and role. Viewer
accounts can read sources and playback; operators can validate, inspect
diagnostics, and control capability-authorized PTZ; administrators can manage
sources and onboarding. Authorization is enforced server-side.

## Endpoints

| Method | Path | Purpose |
|---|---|---|
| GET | `/health/live` | Process liveness |
| GET | `/health/ready` | Runtime readiness |
| GET | `/api/v1/status` | Runtime, metrics, and buffer snapshot |
| GET | `/api/v1/version` | Service name and version |
| POST | `/api/v1/stop` | Request graceful shutdown |
| GET | `/api/v1/config` | Read-only effective configuration with secrets masked |
| GET | `/api/v1/sources` | List registered sources |
| GET | `/api/v1/sources/providers` | List camera provider capabilities |
| GET | `/api/v1/sources/discover` | Discover local and configured cameras |
| POST | `/api/v1/sources/test` | Test a source and capture one frame without registering it |
| GET | `/api/v1/sources/{id}` | Source metadata and state |
| GET | `/api/v1/sources/{id}/capabilities` | Normalized camera/device capabilities |
| POST | `/api/v1/onboarding/discover` | Start an in-memory discovery session |
| GET | `/api/v1/onboarding/sessions/{id}` | Read onboarding progress |
| POST | `/api/v1/onboarding/sessions/{id}/inspect` | Inspect selected camera and choose a usable profile |
| POST | `/api/v1/onboarding/sessions/{id}/complete` | Validate, register playback, and finalize camera setup |
| POST | `/api/v1/onvif/discover` | Discover and inspect ONVIF devices |
| POST | `/api/v1/sources/{id}/onvif/inspect` | Inspect ONVIF endpoint, retain capabilities, validate RTSP handoff |
| GET | `/api/v1/sources/{id}/playback` | Normalized WebRTC/HLS playback contract |
| POST | `/api/v1/sources/{id}/playback/register` | Register validated RTSP source with the media gateway |
| DELETE | `/api/v1/sources/{id}/playback/register` | Remove source from the media gateway |
| GET | `/api/v1/media-gateway/health` | Normalized media-delivery health |
| GET | `/api/v1/sources/{id}/ptz` | PTZ capability and supported operations |
| POST | `/api/v1/sources` | Register a supported source definition |
| POST | `/api/v1/sources/{id}/start` | Start a source |
| POST | `/api/v1/sources/{id}/stop` | Stop a source |
| POST | `/api/v1/sources/{id}/restart` | Restart a source |
| DELETE | `/api/v1/sources/{id}` | Remove a source |
| POST | `/api/v1/sources/{id}/ptz/move` | Capability-authorized PTZ movement |
| POST | `/api/v1/sources/{id}/ptz/stop` | Stop PTZ movement |
| GET | `/api/v1/sources/{id}/ptz/presets` | List supported presets |
| POST | `/api/v1/sources/{id}/ptz/presets/{preset_id}/goto` | Invoke a listed preset |
| GET | `/api/v1/preview` | Latest JPEG frame |
| GET | `/api/v1/streams/{source_id}/mjpeg` | Continuous multipart MJPEG |
| GET | `/api/v1/vision/latest` | Latest successful scene observation |
| GET | `/api/v1/events` | Recent bounded events |
| GET | `/api/v1/events/latest` | Most recent event |
| GET | `/api/v1/events/{id}` | Event lookup |
| GET | `/api/v1/events/stream` | Real-time Server-Sent Events |
| GET | `/metrics` | Prometheus-compatible text metrics |

Unsupported source adapters return `501 Not Implemented`.

`/api/v1/sources/discover` includes existing sources, a hardware-free synthetic
camera, and ONVIF WS-Discovery results when cameras respond on the local network.
ONVIF discovery may return a device management address without a selected media
profile; the Observatory must guide the operator through stream-profile setup.

The capabilities response is normalized and provider-neutral. It records only
capabilities actually discovered from the camera/profile. PTZ
operations are consequential and require stronger operation-level authorization
than read-only metadata. Successful and failed PTZ attempts must produce
operational evidence through the existing event/logging path, including the
initiator, camera, requested operation, outcome, and request/correlation ID when
available.

The planned setup API should also expose a provider-neutral verification result
for the onboarding console. It should report the state of camera connectivity,
ONVIF, selected video profile, RTSP ingest, browser playback, audio, PTZ, AI
readiness, and health monitoring. The console may show technical diagnostics on
failure, but ordinary setup should not require RTSP URLs, profile tokens,
MediaMTX ports, codecs, or WHIP/WHEP details.

`POST /api/v1/sources/test` accepts the same source fields as source
registration. It validates the connection, captures one frame, and returns
resolution, FPS, latency, and a diagnostic message without storing credentials
in the response.

The configuration response includes the effective recovery policy. Recovery
state is included in `/api/v1/status`; recovery events are returned by the
existing event endpoints.

Playback registration requires a validated RTSP source. Playback responses are
provider-neutral and list WebRTC first for low latency, followed by HLS when
configured. Media-gateway health is independent from camera/RTSP health; a
gateway outage returns a normalized `MEDIA_GATEWAY_UNAVAILABLE` response.
Playback API access requires an authenticated principal with `VIEW_STREAM`.
Browser URLs contain no camera or MediaMTX admin credentials. Direct MediaMTX
listener protection still requires trusted listener binding or an authenticated
TLS reverse proxy until per-viewer MediaMTX authorization is provisioned.

## Examples

```bash
curl http://127.0.0.1:8081/health/live

curl -H 'Authorization: Bearer dev-token' \
  http://127.0.0.1:8081/api/v1/status

curl -H 'Authorization: Bearer dev-token' \
  -X POST http://127.0.0.1:8081/api/v1/stop

curl -N -H 'Authorization: Bearer dev-token' \
  http://127.0.0.1:8081/api/v1/streams/builtin/mjpeg
```

## CLI

```text
sentinel-streaming serve [--config <path>] [--bind <address>] [--source <type>]
sentinel-streaming status [--endpoint <url>]
sentinel-streaming stop [--endpoint <url>]
sentinel-streaming version
sentinel-streaming endurance [--duration <N>s|<N>m|<N>h] [--source synthetic] [--viewers <N>] [--vision mock]
sentinel-streaming source list|add|remove|start|stop|restart
sentinel-streaming config show
sentinel-streaming metrics
sentinel-streaming auth login|logout|status|whoami
sentinel-streaming profile list|use|add
```

The CLI uses the API for runtime operations. Source lifecycle and shutdown
logic are not duplicated inside the CLI.

RTSP sources use `kind: rtsp` with `uri`, optional `transport: tcp`, and
optional environment-backed credential references:

```json
{
  "id": "front-gate",
  "kind": "rtsp",
  "uri": "rtsp://camera-host/stream",
  "transport": "tcp",
  "credentials": {
    "username_env": "FRONT_GATE_USERNAME",
    "password_env": "FRONT_GATE_PASSWORD"
  }
}
```

Credentials are resolved only when the worker starts and are never included in
source status, metrics, or logs. The RTSP adapter currently uses an FFmpeg
process configured through `SENTINEL_FFMPEG` (default: `ffmpeg`) and emits raw
RGB frames into the existing pipeline.
