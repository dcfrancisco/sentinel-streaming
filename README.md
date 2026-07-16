# Sentinel Streaming

Sentinel Streaming is a headless, extensible Rust streaming engine for the Sentinel Platform. It acquires frames through `VideoSource`, sends every frame through one `Pipeline`, and delivers them to `FrameOutput` extension points.

## Administration

Run the daemon with `cargo run -- serve`. The administration API listens on `0.0.0.0:8080` and exposes:

- `GET /health/live`
- `GET /health/ready`
- `GET /api/v1/status`
- `GET /api/v1/version`
- `GET /api/v1/sources`
- `POST /api/v1/sources`
- `POST /api/v1/sources/{id}/start`
- `POST /api/v1/sources/{id}/stop`
- `DELETE /api/v1/sources/{id}`
- `GET /api/v1/config`
- `GET /metrics`
- `GET /api/v1/events` (Server-Sent Events)
- `GET /api/v1/preview` (latest JPEG camera frame)

The CLI is API-backed and includes `serve`, `status`, `version`, `source list`, `source add`, `source remove`, `source start`, `source stop`, `config show`, and `metrics`.

## MVP architecture

The built-in source uses the native camera backend provided by Nokhwa and captures device index `0`. Every frame then passes through the configurable processing pipeline. Preview is enabled by default; buffer, recording, vision, and event publisher stages are present as disabled no-op extension points. USB, RTSP, ONVIF, and video-file adapters remain isolated extension points for later milestones. The latest captured frame is JPEG-encoded by the preview stage and exposed at `/api/v1/preview` for browser inspection.

The `VideoSourceManager` owns live source implementations. The pipeline consumes the manager through the `FrameProvider` abstraction and does not depend on concrete camera types. The `serve` runtime coordinates configuration, logging, metrics, source initialization, pipeline startup, HTTP serving, signal handling, and graceful shutdown.

The manager now owns source lifecycle and runtime metadata. Built-in camera management supports start, stop, restart, failure tracking, reconnect attempts, frame counters, resolution, FPS, uptime, and internal lifecycle events. Other source types return `501 Not Implemented` until their adapters are added.

Vision is an optional background consumer of the `FrameBuffer`. When `OPENAI_API_KEY` is available, it analyzes the latest frame every five seconds through the OpenAI Responses API and stores the latest structured scene description at `/api/v1/vision/latest`. Without the key, vision logs a warning and streaming continues normally.

The Event Engine stores bounded observation records (default capacity: 1000). Source lifecycle events and vision results are available through `/api/v1/events`, `/api/v1/events/latest`, and `/api/v1/events/{id}`. The existing real-time feed is available at `/api/v1/events/stream`.

The bounded `FrameBuffer` is the canonical frame store. It defaults to 300 frames, evicts the oldest frame when full, stores RGB payloads behind `Arc`, and exposes latest, previous, sequence lookup, and recent-frame accessors. Preview reads its source frame from this buffer.

AI is not implemented in this milestone. Placeholder interfaces exist for `VisionEngine`, `SceneUnderstanding`, and `EventPublisher`.

## Build

```bash
cargo fmt --all
cargo test
cargo run -- version
```

## License

Apache 2.0
