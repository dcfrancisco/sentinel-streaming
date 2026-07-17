# Sentinel Streaming Documentation

Sentinel Streaming is a headless Rust infrastructure service for acquiring,
processing, buffering, observing, and administrating video sources. It is a
platform component, not a complete security product.

## Documentation map

- [Architecture](architecture.md) — runtime, module boundaries, data flow, and invariants.
- [Administration API](api.md) — REST, MJPEG, SSE, authentication, and CLI behavior.
- [Configuration](configuration.md) — YAML, environment, CLI precedence, and validation.
- [Operations](operations.md) — deployment, camera permissions, startup, shutdown, and observability.
- [Observability](observability.md) — open-standard logs, metrics, tracing, and platform integration.
- [Self-healing runtime](self-healing.md) — health states, recovery actions, events, and policies.
- [Testing and validation](testing.md) — automated checks, runtime tests, and endurance plans.
- [Roadmap](roadmap.md) — completed milestones, current gaps, and planned work.
- [Architecture decision records](adr/) — decisions that constrain future implementation.

## Current release state

The current implementation provides:

- A single headless `serve` runtime.
- A built-in camera source through Nokhwa.
- Synthetic, image-sequence, and RTSP/TCP source adapters through the same boundary.
- A manager-owned source lifecycle.
- A single processing pipeline for captured frames.
- A bounded in-memory `FrameBuffer`.
- JPEG preview and live MJPEG diagnostics.
- Health, status, source, event, vision, metrics, and shutdown APIs.
- An API-backed CLI.
- Optional temporal scene observation through the OpenAI Responses API.
- A bounded in-memory event store and Server-Sent Events feed.

Production readiness is not yet certified. See [Roadmap](roadmap.md) and
[Testing and validation](testing.md) for known gaps and required evidence.
