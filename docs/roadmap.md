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

## Current product roadmap

SS-WP-003 through SS-WP-013 are complete in the current repository. The next
planned implementation package is SS-WP-014.

### SS-WP-014 — Standalone packaging and service installation

Provide supported-machine packaging, local service installation, configuration
validation, update instructions, and deployment profiles without mandatory
Docker.

### SS-WP-015 — Operational observability and diagnostics bundle

Add operator-facing diagnostics, support-bundle export, structured logs, metrics,
failure attribution, and actionable health/readiness reporting.

### SS-WP-016 — Long-duration stability and soak testing

Track 24-hour, 72-hour, and 7-day tests across source, MediaGateway, browser
delivery, recovery, resource usage, and representative physical cameras.

### SS-WP-017 — Upgrade, migration, versioning, and compatibility policy

Define semantic/versioned releases, `/api/v1` compatibility guarantees,
configuration migration, persistence migration, deprecation, and rollback policy.

### SS-WP-018 — Commercial readiness review and certification

Review CR-1 through CR-10, publish evidence by testing class, identify release
blockers, and certify the supported deployment/camera matrix.

## Virtual camera lab backlog

Future project-local tooling may provide a generated RTSP source, ONVIF device and
media services, PTZ-capable and fixed-camera personalities, visible virtual PTZ,
presets, authentication modes, failure injection, audio, and deterministic AI
scenarios. Keep fast in-process fixtures, standalone emulator tooling, and
physical-device certification as separate evidence classes. Do not implement the
full lab outside an explicitly started work package.

AI/video intelligence remains a future capability area, but streaming reliability,
setup, security, interoperability, and operations take priority.
