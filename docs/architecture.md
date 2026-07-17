# Architecture

## Purpose and scope

Sentinel Streaming acquires video frames and exposes operational interfaces for
future platform products. The service observes and transports video. Product
policy, notifications, alarms, identity management, and security decisions are
outside this repository.

## System flow

```mermaid
flowchart TD
    Clients["Administration clients<br/>CLI / REST / automation"] --> API["HTTP API"]
    API --> State["Runtime / AppState"]
    API --> Ops["Operational APIs<br/>health, metrics, events"]
    State --> Manager["VideoSourceManager"]
    Manager --> Provider["FrameProvider"]
    Provider --> Pipeline["Processing Pipeline"]
    Pipeline --> Buffer["FrameBuffer"]
    Buffer --> Preview["Preview"]
    Buffer --> MJPEG["MJPEG"]
    Buffer --> Vision["Vision"]
    Buffer --> Future["Future stages"]
    Vision --> Events["Event Engine"]
```

The `HealthMonitor` and `RecoveryEngine` observe these runtime boundaries and
coordinate corrective actions without bypassing source, pipeline, or buffer
ownership rules.

Every captured frame enters through `FrameProvider` and is processed by the
pipeline. Higher-level consumers read from the `FrameBuffer`; they do not
communicate directly with camera implementations.

## Runtime lifecycle

The `serve` command owns the process lifecycle:

1. Initialize structured logging.
2. Load default/runtime configuration.
3. Create the shutdown channel and central `AppState`.
4. Create the bounded frame buffer.
5. Start the built-in source through `VideoSourceManager`.
6. Initialize the processing pipeline.
7. Start optional Vision scheduling.
8. Start the HTTP server.
9. Wait for Ctrl+C, an API shutdown request, or a subsystem failure.
10. Signal all tasks and camera workers to stop.
11. Wait for HTTP, pipeline, and vision tasks to exit.

The shutdown API and Ctrl+C use the same watch channel, keeping shutdown paths
consistent.

## Core boundaries

### Video source boundary

`VideoSourceManager` is the only owner of source registration and lifecycle.
Built-in camera, synthetic, image-sequence, and RTSP adapters are functional.
USB, ONVIF, and container decoding remain extension points. Every adapter
emits `Frame` values through the same manager-owned `FrameProvider`, so pipeline,
buffer, MJPEG, Vision, and event consumers do not branch on source type.

The macOS Nokhwa camera backend is not `Send`. It is therefore constructed and
used on a dedicated `sentinel-camera` OS thread. Frames cross into the async
runtime through a bounded Tokio channel.

Camera failure transitions to `Disconnected` and then `Reconnecting`. Reconnect
attempts use configurable bounded exponential backoff with optional jitter and
continue until the source reconnects, is explicitly stopped, or the service
shuts down.

### Pipeline boundary

The pipeline receives frames from `FrameProvider` and runs configured stages.
Preview is the current working stage. Buffer, recording, vision, and event
stages are represented as extension points; Vision currently also runs as a
dedicated consumer of the frame buffer.

### Frame buffer boundary

`FrameBuffer` is a bounded, thread-safe, timestamp/sequence-ordered store. It
keeps recent frames and evicts the oldest frame at capacity. Its contents are
shared through `Arc`, avoiding copies between preview, MJPEG, and Vision.

### Observation boundary

Vision produces scene observations. The Event Engine stores those observations
and source lifecycle events. Consumers receive facts; they do not receive
security decisions.

## Central application state

`AppState` shares the operational components required by API handlers:

- configuration
- health state
- runtime status
- metrics
- source manager
- event bus/store
- authentication validator
- latest preview
- frame buffer
- latest vision observation
- MJPEG metrics/stream access
- graceful-shutdown sender

## Module map

| Module | Responsibility |
|---|---|
| `main.rs` | CLI dispatch and runtime orchestration |
| `api.rs` | Axum routes, authentication middleware, handlers |
| `cli.rs` | API-backed command-line interface |
| `config.rs` | Runtime defaults and stage configuration |
| `sources.rs` | Source abstractions, camera worker, manager, lifecycle |
| `frame.rs` | Frame representation and RGB payload ownership |
| `frame_buffer.rs` | Bounded shared recent-frame store |
| `pipeline.rs` | Single frame-processing loop |
| `stages.rs` | Preview and placeholder pipeline stages |
| `preview.rs` | Latest JPEG preview storage |
| `mjpeg.rs` | Multipart MJPEG consumers and metrics |
| `vision.rs` | Temporal selection, OpenAI provider, scheduler, metrics |
| `events.rs` | Typed event records, bounded store, SSE broadcast |
| `health.rs` | Liveness/readiness state |
| `metrics.rs` | Core Prometheus-compatible metrics |
| `runtime.rs` | Lifecycle status snapshot |
| `logging.rs` | Structured tracing initialization |

## Design invariants

1. Camera implementations are owned by `VideoSourceManager`.
2. Frames do not bypass `FrameProvider` and the pipeline.
3. Higher-level consumers read from `FrameBuffer`, not cameras.
4. Memory used by frame and event stores is bounded by configuration.
5. Vision unavailability must not prevent video runtime startup.
6. Administration clients use the public API instead of duplicating business logic.
7. Products interpret observations; the streaming service does not make alarms or policy decisions.
