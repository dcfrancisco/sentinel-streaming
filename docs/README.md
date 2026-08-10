# Sentinel Streaming Documentation

Sentinel Streaming is a Rust video-intelligence infrastructure service with its
own setup and operations web console for acquiring, processing, buffering,
observing, and administering video sources. It is a platform component, not a
complete homeowner, campus, or building security product.

## Documentation map

- [Architecture](architecture.md) — runtime, module boundaries, data flow, and invariants.
- [Technical design](technical-design.md) — source adapters, frame capture, and runtime contracts.
- [Administration API](api.md) — REST, MJPEG, SSE, authentication, and CLI behavior.
- [RTSP validation](RTSP.md) — bounded protocol validation and normalized failures.
- [MediaGateway](MEDIA_GATEWAY.md) — MediaMTX registration and normalized media health.
- [Browser streaming](BROWSER_STREAMING.md) — WebRTC-preferred and HLS-fallback playback.
- [Camera lifecycle](CAMERA_LIFECYCLE.md) — source and validation state semantics.
- [Health and recovery](HEALTH_AND_RECOVERY.md) — source health fields and recovery boundary.
- [Testing](testing.md) — automated, runtime, and SS-WP-003 validation checks.
- [Work package status](WORK_PACKAGES.md) — SS-WP-003 scope and exclusions.
- [Admin/setup console](admin-console.md) — zero-friction onboarding and infrastructure operations UX.
- [Configuration](configuration.md) — YAML, environment, CLI precedence, and validation.
- [Operations](operations.md) — deployment, camera permissions, startup, shutdown, and observability.
- [Observability](observability.md) — open-standard logs, metrics, tracing, and platform integration.
- [Self-healing runtime](self-healing.md) — health states, recovery actions, events, and policies.
- [Testing and validation](testing.md) — automated checks, runtime tests, and endurance plans.
- [Human functional test guide](human-functional-test-guide.md) — step-by-step real hardware and network validation.
- [Compatibility matrix](compatibility-matrix.md) — verified source and camera records.
- [Test checklist](test-checklist.md) — reusable manual execution checklist.
- [Troubleshooting](troubleshooting.md) — camera, network, codec, and recovery diagnosis.
- [Camera certification process](camera-certification.md) — how to qualify new models and providers.
- [Roadmap](roadmap.md) — completed milestones, current gaps, and planned work.
- [Commercial readiness](COMMERCIAL_READINESS.md) — CR-1 through CR-10 and evidence classes.
- [Architecture decision records](adr/) — decisions that constrain future implementation.

## Current release state

The current implementation provides:

- A single headless `serve` runtime.
- A built-in camera source through Nokhwa.
- Synthetic, image-sequence, video-file, and RTSP/TCP source adapters through the same boundary.
- A manager-owned source lifecycle.
- A single processing pipeline for captured frames.
- A bounded in-memory `FrameBuffer`.
- JPEG preview and live MJPEG diagnostics.
- Bounded single-frame JPEG capture for downstream observation analysis.
- Health, status, source, event, vision, metrics, and shutdown APIs.
- An API-backed CLI.
- Optional temporal scene observation through the OpenAI Responses API.
- A bounded in-memory event store and Server-Sent Events feed.
- ONVIF discovery, normalized capabilities, capability-driven PTZ, and presets.
- MediaGateway/MediaMTX browser playback and local moving-video verification.

Commercial readiness is not yet certified. See [Commercial readiness](COMMERCIAL_READINESS.md),
[Roadmap](roadmap.md), and [Testing and validation](testing.md) for known gaps
and required evidence.
