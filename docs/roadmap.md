# Product roadmap

Sentinel Streaming is a commercial standalone and embeddable video streaming
platform. It owns camera integration, media delivery, stream health/recovery,
operational APIs, and its own setup/operations console. Sentinel Home, Campus,
and Buildings remain separate domain applications.

## Completed baseline

- Rust runtime, source manager, shared frame pipeline, bounded frame buffer,
  preview, MJPEG, events, metrics, and graceful shutdown.
- SS-WP-003: RTSP OPTIONS/DESCRIBE validation, normalized failures, source
  health, API/admin validation, and credential safety.
- SS-WP-004: HealthMonitor, bounded RecoveryEngine, capped backoff, retry policy,
  recovery history, and shutdown integration.
- SS-WP-005: ONVIF WS-Discovery, device/media inspection, profile and RTSP URI
  retrieval, normalized capabilities, and deterministic emulator coverage.
- SS-WP-006: capability-driven PTZ movement, stop, zoom, presets, authority,
  evidence, correlation, emulator verification, and admin controls.
- SS-WP-007: MediaGateway, MediaMtxAdapter, validated source registration,
  normalized WebRTC/HLS playback, separate media health, and admin live view.
- SS-WP-007A: real local MediaMTX v1.20.0 plus deterministic FFmpeg RTSP source;
  registration, WHEP/HLS endpoints, browser moving-video acceptance, failure,
  restart, and re-registration evidence.
- SS-WP-008: discovery-first onboarding with automatic profile selection, RTSP
  validation, MediaGateway registration, readiness checks, and Admin setup flow.
- SS-WP-009: bearer authentication, roles, authority checks, secret redaction,
  security events, and playback security boundaries.
- SS-WP-010: normalized media telemetry, MediaMTX inspection, bounded stream
  supervision, watchdog thresholds, and separate source/media health.
- SS-WP-011: physical-device certification schema, procedures, evidence
  templates, and non-destructive capture tooling; no physical result is claimed.
- SS-WP-012: bounded snapshots and clips, pluggable artifact storage, checksums,
  provenance, retention metadata, and protected artifact APIs.
- SS-WP-013: ONVIF/MediaMTX audio capability metadata, audio telemetry, muted
  browser audio controls, optional audio-preserving clips, and `VIEW_AUDIO`.
- SS-WP-014: macOS release bundle, user-local LaunchAgent installer, secure
  bootstrap, predictable filesystem layout, config validation, and safe
  uninstall; no clean-machine timing evidence is claimed yet.

## Current product roadmap

SS-WP-003 through SS-WP-014 are implementation-complete in the current
repository. SS-WP-014 remains product-gate-incomplete until the documented
clean-machine installation is measured under the 15-minute CR-1 target.
SS-WP-015 is now in implementation, beginning with its diagnostics contracts
and local operator slice.

### SS-WP-015 — Operational observability and diagnostics bundle

Add operator-facing diagnostics, support-bundle export, structured logs, metrics,
failure attribution, and actionable health/readiness reporting.

Diagnostics contracts must include stable instance/source identity, failure
codes, correlation/causation fields, dependency versions, and sanitized support
evidence suitable for standalone operators and embedded platform consumers.
`doctor`, support-bundle collection, and local smoke tests must honor
`OPEN_LOCAL_TEST` without requiring token setup while clearly reporting the
active security mode.

### Architecture decisions during SS-WP-015

- ADR 0007: accepted one-edge-instance-per-site topology;
- ADR 0008: accepted per-source runtime isolation, superseding ADR 0001;
- ADR 0009: proposed identity and authorization trust boundary;
- ADR 0010: proposed versioned integration event contract;
- ADR 0011: proposed durable state ownership;
- ADR 0012: proposed secure playback delegation.

The proposed contracts must be reviewed before their implementation packages
begin.

### SS-WP-015A — Embeddable Platform Contract

Define what Sentinel Home and other products may depend on: stable instance and
camera identity, external references, authentication seam, resource-scoped
authorization, API/error/event schemas, artifact references, playback grants,
compatibility rules, a reference client, and consumer-driven contract tests.

The API compatibility baseline needed by consumers is pulled forward from
SS-WP-017. Full release migration, deprecation, and rollback machinery remains
in SS-WP-017.

### SS-WP-015B — Concurrent Multi-Camera Runtime

Implement ADR 0008 with per-source pipelines, frame buffers, previews, optional
Vision state, capture context, health/recovery, resource accounting, and failure
isolation. A source-addressed API must never return another source's frame or
mutable runtime state.

### SS-WP-016 — Long-duration stability and soak testing

Track 24-hour, 72-hour, and 7-day tests across source, MediaGateway, browser
delivery, recovery, resource usage, and representative physical cameras.

Commercial multi-camera soak evidence requires SS-WP-015B. Deterministic lab
scenarios may supplement but never replace physical-device evidence.

### SS-WP-017 — Upgrade, migration, versioning, and compatibility policy

Define semantic/versioned releases, `/api/v1` compatibility guarantees,
configuration migration, persistence migration, deprecation, and rollback policy.

### SS-WP-018 — Commercial readiness review and certification

Review CR-1 through CR-10, publish evidence by testing class, identify release
blockers, and certify the supported deployment/camera matrix.

## Virtual camera lab backlog

### SS-TOOL-001A — Virtual Camera Lab Contract

Write the documentation-only architecture, fidelity classes, protocol matrix,
camera identity/lifecycle model, versioned personality and scenario schemas,
deterministic clock, ground-truth schema, fault taxonomy, security boundary,
headless API/CLI, evidence manifest, and acceptance tests.

The evidence ladder is `SIMULATED_PROTOCOL`,
`REAL_PROTOCOL_SYNTHETIC_MEDIA`, `FAULT_PROXY`, and
`PHYSICAL_DEVICE_REFERENCE`.

### SS-TOOL-001B — Deterministic Virtual Camera

Provide real generated media plus the first fixed and PTZ camera-facing
RTSP/ONVIF contracts. Sentinel must use camera-facing protocols only.

### SS-TOOL-001C — Scenario and Fault Engine

Provide deterministic timelines, layered failure injection, replay manifests,
ground truth, and evidence export before SS-WP-016 soak execution.

The polished lab dashboard follows stable control-plane contracts. Keep fast
in-process fixtures, standalone lab tooling, and physical-device certification
as separate evidence classes. Production products never depend on the lab
control API.

## Pre-Sentinel Home production gate

Sentinel Home must not take a production dependency on Streaming until stable
instance/camera identity, per-source runtime isolation, externally asserted
identity, resource-scoped authorization, versioned API/event contracts, secure
playback delegation, and independent compatibility/upgrade rules are available
and contract-tested. Home development may proceed independently against
versioned mocks while this gate remains open.

AI/video intelligence remains a future capability area, but streaming reliability,
setup, security, interoperability, and operations take priority.
