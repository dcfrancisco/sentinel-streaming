# Administration API and CLI

The default server address is `0.0.0.0:8080`. A local development server can
use `127.0.0.1:8081` when port 8080 is occupied.

## Authentication

When `SENTINEL_API_TOKEN` is set on the server, all routes except the following
are protected:

- `GET /health/live`
- `GET /health/ready`
- `GET /api/v1/version`

Protected requests use:

```http
Authorization: Bearer <token>
```

The CLI can store a token through its profile/auth workflow. Do not put tokens
in shell history, source files, or committed configuration.

## Endpoints

| Method | Path | Purpose |
|---|---|---|
| GET | `/health/live` | Process liveness |
| GET | `/health/ready` | Runtime readiness |
| GET | `/api/v1/status` | Runtime, metrics, and buffer snapshot |
| GET | `/api/v1/version` | Service name and version |
| POST | `/api/v1/stop` | Request graceful shutdown |
| GET | `/api/v1/config` | Effective configuration |
| GET | `/api/v1/sources` | List registered sources |
| GET | `/api/v1/sources/{id}` | Source metadata and state |
| POST | `/api/v1/sources` | Register a supported source definition |
| POST | `/api/v1/sources/{id}/start` | Start a source |
| POST | `/api/v1/sources/{id}/stop` | Stop a source |
| POST | `/api/v1/sources/{id}/restart` | Restart a source |
| DELETE | `/api/v1/sources/{id}` | Remove a source |
| GET | `/api/v1/preview` | Latest JPEG frame |
| GET | `/api/v1/streams/{source_id}/mjpeg` | Continuous multipart MJPEG |
| GET | `/api/v1/vision/latest` | Latest successful scene observation |
| GET | `/api/v1/events` | Recent bounded events |
| GET | `/api/v1/events/latest` | Most recent event |
| GET | `/api/v1/events/{id}` | Event lookup |
| GET | `/api/v1/events/stream` | Real-time Server-Sent Events |
| GET | `/metrics` | Prometheus-compatible text metrics |

Unsupported source adapters return `501 Not Implemented`.

The configuration response includes the effective recovery policy. Recovery
state is included in `/api/v1/status`; recovery events are returned by the
existing event endpoints.

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
sentinel-streaming serve [--bind <address>] [--source builtin|synthetic]
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
