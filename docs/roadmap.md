# Roadmap

## Completed milestones

### Milestone 1 — Streaming foundation

- Headless Rust daemon.
- Built-in camera acquisition.
- Frame abstraction and JPEG preview.
- REST health, status, version, and metrics foundations.
- CLI foundation.

### Milestone 2 — Processing pipeline and runtime

- Single frame-processing pipeline.
- Preview stage and placeholder stages.
- Central application state.
- Graceful shutdown.
- Video source manager and lifecycle metadata.

### Milestone 4 — Ring buffer

- Bounded thread-safe `FrameBuffer`.
- Recent-frame and sequence access.
- Preview and higher-level consumers read buffered frames.

### Milestone 5 — Video source manager

- Source registration and lifecycle API.
- Built-in source start/stop/restart.
- Source state, frame counts, FPS, uptime, and reconnect metadata.

### Milestone 6 — Vision engine

- Provider abstraction.
- OpenAI Responses API provider.
- Optional startup when the API key is absent.
- Temporal frame selection and scene observations.

### Milestone 7 — Event engine

- Strongly typed event records.
- Bounded event store.
- Source and Vision events.
- REST event lookup and SSE event feed.

### Milestone 8 — MJPEG diagnostics

- Multipart MJPEG endpoint.
- Multiple viewers.
- Stream metrics and disconnect handling.

### Milestone 9 — Runtime reliability and test foundation

- Clean format, check, test, and Clippy gates with warnings denied.
- Continuous camera reconnect with bounded exponential backoff, jitter, state,
  attempt count, and downtime reporting.
- Cross-platform Unix process CPU and resident-memory metrics.
- Frame-buffer size, capacity, utilization, and eviction metrics.
- MJPEG byte-rate metrics.
- Hardware-independent integration tests for REST, MJPEG, events, and mock Vision.
- Synthetic endurance command with machine-readable JSON reports.

Configuration-file loading is intentionally deferred because a YAML parser and
configuration precedence model need to be introduced and tested separately.

### Milestone 10 — Self-healing runtime

- Component health monitor with healthy, degraded, recovering, and failed states.
- Recovery engine with structured events and recovery metrics.
- Continuous camera recovery with observable retry attempts.
- Supervised pipeline restart after processing failure.
- Vision recovery state tracking while preserving provider isolation.
- MJPEG client cleanup through independent stream ownership.

Remaining hardening includes full Vision retry/cooldown policy enforcement,
stalled-client timeout cleanup, and a persistent recovery supervisor for future
multi-process deployments.

## Current hardening priorities

1. Add continuous reconnect retry with bounded exponential backoff and clear
   source state transitions.
2. Add API integration tests and a repeatable camera-free test source.
3. Add CPU, portable memory, frame-buffer occupancy, and network bandwidth
   metrics.
4. Make health readiness reflect source and pipeline state precisely.
5. Remove or narrowly annotate intentional dead-code warnings so Clippy can be a
   release gate.
6. Add service-manager packaging and restart policy examples for macOS/Linux.
7. Add structured configuration loading and validation.
8. Add endurance and recovery test tooling.
9. Add correlation IDs and OpenTelemetry spans without coupling the runtime to a
   specific observability vendor.
10. Add validated YAML configuration loading with CLI > environment > file >
    defaults precedence.

## Future capabilities

### Sources

- USB camera adapter.
- RTSP camera adapter.
- ONVIF discovery and control.
- Video-file source.

### Processing and storage

- Ring-buffer persistence options.
- Recording stage.
- Snapshot API.
- Motion and scene-change stages.

### Vision and events

- Provider implementations for Qwen-VL, Gemini, Ollama, and OIP.
- Provider timeouts, retries, circuit breakers, and cost controls.
- Event retention policies and durable event storage.
- WebSocket event transport.

### Streaming protocols and platform integration

- WebRTC.
- HLS or other production delivery protocols.
- Authentication and profile hardening.
- Sentinel Home integration.
- Multi-source and clustered deployment support.

Future features must preserve the source-manager, pipeline, and frame-buffer
boundaries established by the architecture.
