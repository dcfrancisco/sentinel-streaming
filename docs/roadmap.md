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

## Current product roadmap

### SS-WP-008 — Zero-friction camera onboarding

Discover, select, authenticate only when required, inspect capabilities, select a
usable media profile automatically, validate RTSP, register media delivery,
preview in the browser, test PTZ when supported, assign name/location, and
report readiness. Manual RTSP remains an Advanced path.

### SS-WP-009 — Authentication, roles, secrets, and playback security

Add product-grade operator/admin roles, secret handling, TLS deployment guidance,
secure playback access, audit policy, correlation, and rate limiting where
justified.

### SS-WP-010 — Media telemetry and stream supervision

Add codec, resolution, FPS, bitrate, delivery-state telemetry, watchdogs,
degraded-media detection, and bounded media recovery/re-registration.

### SS-WP-011 — Physical-camera interoperability and certification

Build a vendor/model/firmware compatibility harness and certify ONVIF, RTSP,
audio, PTZ, presets, browser playback, reconnect, recovery, and long-running
behavior against real devices.

### SS-WP-012 — Snapshot/recording foundation

Add only if product requirements justify it, behind explicit storage and
retention boundaries.

### SS-WP-013 — Audio transport and support

Add camera-compatible audio transport and validation separately from future audio
intelligence.

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
