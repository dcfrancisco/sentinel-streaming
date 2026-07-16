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

The built-in source uses the native camera backend provided by Nokhwa and captures device index `0`. USB, RTSP, ONVIF, and video-file adapters remain isolated extension points for later milestones. The latest captured frame is JPEG-encoded by a pipeline output and exposed at `/api/v1/preview` for browser inspection.

AI is not implemented in this milestone. Placeholder interfaces exist for `VisionEngine`, `SceneUnderstanding`, and `EventPublisher`.

## Build

```bash
cargo fmt --all
cargo test
cargo run -- version
```

## License

Apache 2.0
