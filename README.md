# Sentinel Streaming

Sentinel Streaming is a standalone and embeddable video streaming platform for
discovering, connecting, controlling, monitoring, and delivering IP camera
streams through web-friendly APIs and interfaces. It can run as a standalone
web-accessible product with its own setup and live-view experience, or as a
streaming service consumed by Sentinel Home, Campus, Buildings, and other
clients.

It is designed as a commercial platform component for edge devices, standalone
deployments, and larger distributed architectures. Sentinel Streaming owns
camera integration, media delivery, stream health, recovery, operational APIs,
and administration. Product-specific policy, alarms, notifications, and domain
workflows belong in downstream products.

## Product capabilities

| Capability | Status | Description |
| --- | --- | --- |
| Rust streaming daemon | Implemented | Long-running service with structured logging and graceful shutdown. |
| Admin/setup web console | Implemented foundation | Sentinel Streaming-owned console for discovery, validation, health, and PTZ testing. |
| Built-in camera | Implemented | Native macOS camera capture through Nokhwa. |
| Synthetic source | Implemented | Hardware-free animated test source for demos, CI, and endurance testing. |
| Image-sequence source | Implemented | Image file or directory playback with configurable FPS and looping. |
| Video-file source | Implemented | MP4/video playback through FFmpeg with looping and real-time pacing. |
| RTSP source | Implemented | RTSP/TCP ingestion and H.264-to-RGB decoding through FFmpeg. |
| MJPEG source | Implemented | HTTP/MJPEG ingestion through the FFmpeg source boundary. |
| Camera provider metadata | Implemented | Provider capabilities, discovery results, and test-connection API. |
| ONVIF and capability-driven PTZ | Implemented | WS-Discovery, SOAP profile inspection, normalized capabilities, and gated PTZ control. |
| MediaGateway / MediaMTX browser playback | Implemented foundation | Validated RTSP registration, normalized WebRTC/HLS playback, separate media health, and admin live view. |
| Local MediaMTX verification | Development tooling | FFmpeg moving RTSP test source and Docker-free local integration runbook; not production deployment or physical-camera certification. |
| Processing pipeline | Implemented | Every captured frame enters the shared pipeline. |
| Frame buffer | Implemented | Bounded, thread-safe recent-frame store shared by consumers. |
| JPEG preview | Implemented | Latest-frame preview endpoint. |
| Live MJPEG | Implemented | Multiple browser/debug viewers per running source. |
| Vision Engine | Optional / implemented | Temporal scene descriptions through the OpenAI Responses API. |
| Event Engine | Implemented | Bounded observation and source-lifecycle event store with SSE feed. |
| Health and metrics | Implemented | Liveness, readiness, runtime status, and Prometheus-compatible metrics. |
| Self-healing runtime | Implemented | Reconnect and recovery paths with backoff, events, and metrics. |
| API-backed CLI | Implemented | Runtime, source, metrics, configuration, authentication, and profile commands. |

## Current implementation boundary

The current release is a strong streaming and operations foundation. The
following capabilities are intentionally not implemented yet:

- Persistent recording and evidence storage.
- Ring-buffer persistence beyond process memory.
- Production-grade media supervision, browser compatibility certification, and
  automated MediaMTX orchestration beyond the verified local integration slice.
- Proprietary vendor-specific camera control and cloud integrations.
- Physical-camera PTZ/media certification and vendor-specific ONVIF extensions.
- H.265-specific support.
- AI alerts, intrusion detection, face recognition, object-detection policy, or
  automated security decisions.
- Notifications, user management, Sentinel Home integration, and product policy.
- Sentinel Home, Sentinel Campus, and Sentinel Buildings frontends or domain workflows.
- Distributed clustering and multi-node coordination.

Vision is observation only. If `OPENAI_API_KEY` is unavailable, the service
continues operating with Vision disabled.

## Quick start

Run a complete camera-free instance:

```bash
cargo run -- serve --bind 127.0.0.1:8081 --source synthetic
```

Open the live stream at:

```text
http://127.0.0.1:8081/api/v1/streams/synthetic/mjpeg
```

Or inspect it from the command line:

```bash
curl -N http://127.0.0.1:8081/api/v1/streams/synthetic/mjpeg
```

Use `Ctrl+C` or the API-backed `sentinel-streaming stop` command to shut down
gracefully.

For repeatable deployments, place configuration in `sentinel.yaml` or pass
`--config`. See [docs/configuration.md](docs/configuration.md) for the complete
schema and precedence rules.

## Administration

The administration surface is an infrastructure setup and operations console,
not a homeowner, campus, or building surveillance product. Its planned guided
camera flow is: discover camera, select device, provide credentials, discover
capabilities, verify connectivity and playback, preview video, name the camera,
and save. Manual RTSP configuration remains available under an Advanced path.

See [docs/admin-console.md](docs/admin-console.md) for the target console
experience.

Protected deployments use bearer roles configured with environment tokens;
see [docs/AUTHENTICATION.md](docs/AUTHENTICATION.md) and
[docs/SECURITY.md](docs/SECURITY.md).

PTZ control is exposed only for inspected sources whose normalized ONVIF
capabilities advertise the requested operation. See [docs/PTZ.md](docs/PTZ.md)
and [docs/SECURITY.md](docs/SECURITY.md). Sentinel Streaming supports pluggable
persistence; SQLite is an optional embedded backend for standalone deployments,
not a mandatory runtime dependency.

MediaMTX is optional. Without it, Sentinel continues RTSP, ONVIF, PTZ, and
health operations while browser playback reports normalized media-gateway
unavailability.

For real local playback verification, see [docs/MEDIAMTX.md](docs/MEDIAMTX.md)
and run the development-only [RTSP test source](tools/rtsp-test-source/README.md).

Commercial readiness status and quality gates are tracked in
[docs/COMMERCIAL_READINESS.md](docs/COMMERCIAL_READINESS.md).

Run the daemon with `cargo run -- serve`. The administration API listens on `0.0.0.0:8080` by default and exposes:

- `GET /health/live`
- `GET /health/ready`
- `GET /api/v1/status`
- `GET /api/v1/version`
- `POST /api/v1/stop`
- `GET /api/v1/sources`
- `GET /api/v1/sources/providers`
- `GET /api/v1/sources/discover`
- `POST /api/v1/sources/test`
- `POST /api/v1/sources`
- `POST /api/v1/sources/{id}/start`
- `POST /api/v1/sources/{id}/stop`
- `DELETE /api/v1/sources/{id}`
- `GET /api/v1/config`
- `GET /metrics`
- `GET /api/v1/events`
- `GET /api/v1/events/latest`
- `GET /api/v1/events/{id}`
- `GET /api/v1/events/stream` (Server-Sent Events)
- `GET /api/v1/preview` (latest JPEG frame)
- `GET /api/v1/streams/{source_id}/frame` (bounded JPEG capture for analysis)
- `GET /api/v1/vision/latest`
- `GET /api/v1/streams/{source_id}/mjpeg`

When `SENTINEL_API_TOKEN` is set, all administration endpoints except liveness,
readiness, and version require `Authorization: Bearer <token>`.

The CLI is API-backed and includes `serve`, `status`, `stop`, `version`,
`endurance`, `source list`, `source add`, `source remove`, `source start`,
`source stop`, `source restart`, `config show`, and `metrics`.

## How it works

The built-in source uses the native camera backend provided by Nokhwa. It captures
device index `0` by default; select another camera with
`SENTINEL_CAMERA_INDEX`. Every frame passes through the configurable processing
pipeline. Preview and buffering are enabled by default; recording remains a
placeholder stage while Vision and Event Engine operate as frame-buffer
consumers. The manager also supports a
hardware-independent synthetic source, image-sequence source, video-file source,
and RTSP source. USB/UVC cameras use the built-in camera provider; ONVIF discovery
is available through WS-Discovery and returns stream-profile hints. The latest captured frame is JPEG-encoded by the
preview stage and exposed at `/api/v1/preview`.

The `VideoSourceManager` owns live source implementations. The pipeline consumes the manager through the `FrameProvider` abstraction and does not depend on concrete camera types. The `serve` runtime coordinates configuration, logging, metrics, source initialization, pipeline startup, HTTP serving, signal handling, and graceful shutdown.

The manager now owns source lifecycle and runtime metadata for built-in, synthetic,
video-file, image-sequence, and RTSP sources. Start a camera-free server with
`cargo run -- serve --source synthetic`, or register a source through
`POST /api/v1/sources` using `kind: synthetic`, `kind: video-file`,
`kind: image-sequence`, or `kind: rtsp`. All source types use the same frame provider, pipeline, frame
buffer, MJPEG, metrics, and health paths. RTSP requires an FFmpeg executable;
set `SENTINEL_FFMPEG` when it is not available on `PATH`.

Vision is an optional background consumer of the `FrameBuffer`. When
`OPENAI_API_KEY` is available, it analyzes buffered frames every five seconds
through the OpenAI Responses API and stores the latest structured scene
observation at `/api/v1/vision/latest`. Without the key, vision is disabled and
streaming continues normally. Enabling vision sends camera frames to OpenAI and
may incur API usage costs.

Vision now performs temporal observation by selecting five frames spaced two seconds apart by default. Observations include summaries, changes, activities, and objects; selection and spacing are configurable through `vision.frames` and `vision.spacing_seconds`.

The Event Engine stores bounded observation records (default capacity: 1000). Source lifecycle events and vision results are available through `/api/v1/events`, `/api/v1/events/latest`, and `/api/v1/events/{id}`. The existing real-time feed is available at `/api/v1/events/stream`.

Live browser diagnostics are available at `/api/v1/streams/builtin/mjpeg`. The MJPEG consumer reads the latest frames from the `FrameBuffer`, supports multiple clients, and reports viewer, frame, byte, error, and delivery-FPS metrics.

The bounded `FrameBuffer` is the canonical frame store. It defaults to 300 frames, evicts the oldest frame when full, stores RGB payloads behind `Arc`, and exposes latest, previous, sequence lookup, and recent-frame accessors. Preview reads its source frame from this buffer.

Placeholder interfaces remain for future providers and processing stages. The
current OpenAI Vision provider performs scene observation only; it does not make
security decisions, generate alerts, or implement face recognition or object
detection policies.

## Build

```bash
cargo fmt --all
cargo test
cargo clippy --all-targets --all-features -- -D warnings
cargo run -- version
```

Run a short hardware-independent endurance validation:

```bash
cargo run -- endurance \
  --duration 60s \
  --source synthetic \
  --viewers 5 \
  --vision mock \
  --report endurance-report.json
```

## Local camera test

On macOS, grant Camera permission to Terminal or the IDE running the service.
If port `8080` is occupied, use another port such as `8081`:

```bash
SENTINEL_API_TOKEN=dev-token \
SENTINEL_CAMERA_INDEX=0 \
cargo run -- serve --bind 127.0.0.1:8081
```

Check health and source status:

```bash
curl http://127.0.0.1:8081/health/live
curl http://127.0.0.1:8081/health/ready
curl -H "Authorization: Bearer dev-token" \
  http://127.0.0.1:8081/api/v1/sources
```

Save and view a JPEG preview:

```bash
curl -H "Authorization: Bearer dev-token" \
  http://127.0.0.1:8081/api/v1/preview --output preview.jpg
open preview.jpg
```

Test the live MJPEG stream:

```bash
curl -N -H "Authorization: Bearer dev-token" \
  http://127.0.0.1:8081/api/v1/streams/builtin/mjpeg
```

To enable optional OpenAI vision, provide `OPENAI_API_KEY` when starting the
service. Do not commit API keys or paste them into logs or shell transcripts.

Stop a running release server gracefully from another terminal:

```bash
./target/release/sentinel-streaming stop \
  --endpoint http://127.0.0.1:8081/api/v1/stop
```

## Acknowledgements

Special thanks to Eric Son for practical engineering feedback that influenced
Sentinel Streaming's MediaMTX/browser-streaming direction, ONVIF
capability-driven PTZ, camera onboarding priorities, and real-device
interoperability strategy.

See [ACKNOWLEDGEMENTS.md](docs/ACKNOWLEDGEMENTS.md) for full credits.

## License

Apache 2.0
