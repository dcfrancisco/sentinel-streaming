# Operations Guide

## Build and run

Development build:

```bash
cargo fmt --all
cargo test
cargo run -- serve
```

Release build:

```bash
cargo build --release
./target/release/sentinel-streaming version
```

Hardware-independent endurance validation:

```bash
./target/release/sentinel-streaming endurance \
  --duration 60s \
  --source synthetic \
  --viewers 5 \
  --vision mock \
  --report endurance-report.json
```

The command exits non-zero when its configured minimum FPS or internal error
thresholds are violated. It does not require a camera, OpenAI key, or running
HTTP server.

Run a local release instance with the FaceTime camera:

```bash
SENTINEL_API_TOKEN=dev-token \
SENTINEL_CAMERA_INDEX=0 \
./target/release/sentinel-streaming serve --bind 127.0.0.1:8081
```

## Camera selection

The default camera index is `0`. Override it with:

```bash
SENTINEL_CAMERA_INDEX=1
```

On macOS, inspect cameras with:

```bash
system_profiler SPCameraDataType
```

Grant Camera permission to the terminal or IDE launching Sentinel. Close other
camera applications such as FaceTime, Photo Booth, Zoom, or browser camera tabs
when troubleshooting `AVCaptureDeviceInput ... Rejected`.

## Shutdown

Preferred shutdown paths are:

```text
Ctrl+C
sentinel-streaming stop --endpoint http://127.0.0.1:8081/api/v1/stop
```

Both signal the same runtime shutdown channel and allow pipeline, vision, HTTP,
and camera tasks to exit cleanly.

## Observability

Start with structured logs and inspect:

```bash
curl http://127.0.0.1:8081/health/live
curl http://127.0.0.1:8081/health/ready
curl -H 'Authorization: Bearer dev-token' \
  http://127.0.0.1:8081/api/v1/status
curl -H 'Authorization: Bearer dev-token' \
  http://127.0.0.1:8081/metrics
```

Important current metric families include uptime, connected sources, aggregate
FPS, dropped frames, memory bytes, source counts, source frame counts, Vision
requests/successes/failures/latency, event counts, and MJPEG viewers/frames/
bytes/errors/FPS.

## Vision operations

Vision is optional. If `OPENAI_API_KEY` is absent, the scheduler logs a warning
and the streaming service continues. If enabled, it periodically selects frames
from the buffer and sends them to the configured OpenAI provider.

```bash
OPENAI_API_KEY='use-a-secret-manager-or-shell-secret-store' \
SENTINEL_API_TOKEN=dev-token \
./target/release/sentinel-streaming serve --bind 127.0.0.1:8081
```

Camera frames sent to an external provider may contain sensitive content and
may incur API costs. Never commit or paste API keys.

## Failure behavior

| Failure | Current behavior | Operational expectation |
|---|---|---|
| Camera capture error | Worker closes; manager marks failed and attempts reconnect | Monitor source state and reconnect metrics |
| Vision provider error | Analysis failure is logged and counted; runtime continues | Check Vision failure metrics and latest analysis timestamp |
| MJPEG client disconnect | Client task ends and viewer metric decrements | Other viewers and pipeline continue |
| HTTP client/network interruption | Request/stream terminates independently | Reconnect the client; no process restart should be needed |
| Process termination | Shutdown channel stops managed tasks | Use a supervisor for restart policy |

Host-level resource limits and service-manager integration remain
production-hardening work; see the roadmap. Source retry uses configurable
initial delay, maximum delay, and optional jitter from the runtime defaults.
Configuration-file overrides are planned but not yet implemented.
