# Sentinel Streaming

Sentinel Streaming is a headless video infrastructure service for the Sentinel Platform. It acquires video, processes every frame through one extensible pipeline, exposes operational APIs, and provides browser diagnostics and optional AI scene understanding.

It is designed as a reusable platform component for edge devices, home servers,
and future distributed deployments. Sentinel Streaming observes and transports
video; product-specific policy, alarms, notifications, and identity workflows
belong in downstream products.

## Product capabilities

| Capability | Status | Description |
| --- | --- | --- |
| Headless daemon | Implemented | Long-running Rust service with structured logging and graceful shutdown. |
| Built-in camera | Implemented | Native macOS camera capture through Nokhwa. |
| Synthetic source | Implemented | Hardware-free animated test source for demos, CI, and endurance testing. |
| Image-sequence source | Implemented | Image file or directory playback with configurable FPS and looping. |
| RTSP source | Implemented | RTSP/TCP ingestion and H.264-to-RGB decoding through FFmpeg. |
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
- WebRTC, HLS, RTSP rebroadcast, or other production output protocols.
- USB, ONVIF, and vendor-specific camera discovery or control.
- MP4/MOV/MKV container decoding; the current file adapter is image-based.
- H.265-specific support.
- AI alerts, intrusion detection, face recognition, object-detection policy, or
  automated security decisions.
- Notifications, user management, Sentinel Home integration, and product policy.
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

## Administration

Run the daemon with `cargo run -- serve`. The administration API listens on `0.0.0.0:8080` by default and exposes:

- `GET /health/live`
- `GET /health/ready`
- `GET /api/v1/status`
- `GET /api/v1/version`
- `POST /api/v1/stop`
- `GET /api/v1/sources`
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
hardware-independent synthetic source, image-sequence source, and RTSP source;
USB, ONVIF, and container decoding remain future work. The latest captured frame is JPEG-encoded by the
preview stage and exposed at `/api/v1/preview`.

The `VideoSourceManager` owns live source implementations. The pipeline consumes the manager through the `FrameProvider` abstraction and does not depend on concrete camera types. The `serve` runtime coordinates configuration, logging, metrics, source initialization, pipeline startup, HTTP serving, signal handling, and graceful shutdown.

The manager now owns source lifecycle and runtime metadata for built-in, synthetic,
and image-sequence video-file sources. Start a camera-free server with
`cargo run -- serve --source synthetic`, or register a source through
`POST /api/v1/sources` using `kind: synthetic`, `kind: image-sequence`, or
`kind: rtsp`. All source types use the same frame provider, pipeline, frame
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

## License

Apache 2.0
