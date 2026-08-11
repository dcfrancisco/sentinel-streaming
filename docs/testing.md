# Testing and Validation

## Automated checks

Run before a release:

```bash
cargo fmt --all -- --check
cargo check
cargo test
cargo clippy --all-targets --all-features -- -D warnings
cargo build --release
```

The current automated test coverage includes bounded frame-buffer eviction.
Clippy with warnings denied is a release gate and currently passes. Integration
coverage includes REST health/version startup, MJPEG multipart framing, bounded
event storage, and a mock Vision provider that updates latest analysis and emits
events.

## Smoke test

1. Start the service on an available local port.
2. Verify liveness and readiness.
3. Verify the built-in source reaches `running`.
4. Fetch `/api/v1/preview` and confirm it is a valid JPEG.
5. Connect one MJPEG client and confirm multipart frames arrive.
6. Connect multiple MJPEG clients and confirm the source FPS remains stable.
7. Query `/metrics` while clients are connected.
8. Stop and restart the source through the API.
9. Stop the service through the CLI and confirm graceful shutdown logs.

## Vision smoke test

With a valid provider key in a secure environment:

1. Start the service.
2. Wait for at least one scheduler interval.
3. Query `/api/v1/vision/latest`.
4. Confirm `vision_requests`, success/failure counters, latency, and last
   analysis timestamp change.
5. Disconnect or invalidate the provider and confirm the server remains live.

## Recovery tests

The following tests should be automated or run as controlled operational tests:

- Disconnect and reconnect the camera.
- Stop and restart the source through the API.
- Interrupt an MJPEG client while other viewers remain connected.
- Simulate Vision timeout, HTTP error, malformed response, and missing key.
- Fill the frame buffer and verify bounded eviction.
- Fill the event store and verify bounded eviction.
- Send repeated stop requests and verify idempotent shutdown behavior.

## Endurance testing

Release candidates should run on representative hardware for:

- 24 hours minimum for smoke endurance.
- 72 hours for release confidence.
- Longer runs for deployment-specific qualification.

Record at startup and at regular intervals:

- process RSS and heap trend
- CPU utilization
- camera FPS and frame age
- pipeline FPS and dropped frames
- buffer occupancy and eviction rate
- MJPEG viewers, FPS, latency, bytes, and errors
- Vision request rate, latency, failures, and last success
- event throughput and store size
- restarts, reconnect attempts, and error logs

The result should include start/end snapshots and a conclusion about memory,
CPU, FPS, and recovery stability. A long-duration run is evidence, not a
replacement for automated tests.

The repeatable short-run baseline is:

```bash
cargo run -- endurance --duration 2s --source synthetic \
  --viewers 5 --vision mock --report /tmp/endurance-report.json
```

## SS-WP-003 RTSP validation tests

The RTSP validation tests are deterministic and do not require a physical
camera, FFmpeg, MediaMTX, Docker, or a local listening socket. They use an
injectable protocol backend to cover successful validation, authentication
failure, missing stream, unreachable source, bounded timeout, malformed source,
source health transitions, API wiring, and credential redaction.

These are unit/protocol/integration tests. They are not physical-device tests
and do not claim sustained decoded media, audio, or browser playback.

## SS-WP-005 ONVIF tests

The ONVIF test fixtures are deterministic emulator/protocol tests. They cover
device discovery, empty discovery, timeout/authentication/malformed-response
categories, device and profile normalization, multiple profiles, RTSP URI
redaction, PTZ-supported and fixed-camera capability results, and handoff to
the existing RTSP validator. They are not physical-camera tests.

## SS-WP-004 health and recovery tests

The same deterministic suite also covers automatic recovery after a retryable
failure, no retry for authentication failure, and bounded concurrent health
checks. The injected backend is a protocol fixture; these tests are classified
as unit/protocol/integration tests and are not physical-camera verification.

## SS-WP-006 PTZ tests

The PTZ tests use the deterministic ONVIF emulator/protocol fixture. They
verify continuous, relative, absolute, stop, zoom, preset listing, and
go-to-preset SOAP operation receipt, capability rejection for fixed cameras,
failure propagation, credential/profile-token redaction, correlation-bearing
events, and RTSP/source-session integration. They are emulator/protocol tests,
not physical-device validation.

## SS-WP-007 media gateway tests

MediaGateway tests use deterministic fake gateway fixtures and adapter-level
URL generation. They cover validated-source registration, removal, gateway
unavailability, registration failure, normalized WebRTC/HLS playback, safe
source paths, credential redaction, separate camera/media health, and shutdown
reconciliation. They do not require Docker or a real MediaMTX process.

### LOCAL_MEDIAMTX_INTEGRATION — SS-WP-007A

This classification uses a real locally running MediaMTX process plus the
project-local FFmpeg `testsrc2` publisher. It verifies RTSP input, MediaMTX
registration, normalized WebRTC/HLS playback endpoints, MediaMTX failure, and
restart/re-registration behavior. The strongest acceptance check is opening
`/admin` and seeing moving video. It is not a physical-device test.

The pinned macOS x86_64 verification command is documented in
`docs/MEDIAMTX.md`. The test source does not yet provide ONVIF, PTZ, scenarios,
audio, or synchronized A/V.

This evidence is recorded as `LOCAL_MEDIAMTX_INTEGRATION`; it does not certify
any physical camera vendor, model, or firmware combination.

## SS-WP-008 onboarding tests

Onboarding unit coverage verifies discovery-session creation, automatic
selection of the highest-scoring usable H.264 profile, and redaction of
credentials/profile tokens from the session response. The full browser flow
still requires a local ONVIF/RTSP fixture or physical camera and is not physical
device certification by itself.

## SS-WP-009 security tests

Security unit coverage verifies cumulative role authorities, PTZ denial without
`CONTROL_PTZ`, PTZ allowance for an Operator principal, and credential-safe
serialization/debug boundaries. API middleware uses deterministic bearer-token
configuration and emits normalized authentication/authorization events.
These tests are local unit/API tests, not an enterprise identity or physical
deployment certification. Direct MediaMTX authorization remains a documented
deployment boundary.

## SS-WP-010 media supervision tests

Media tests cover normalized telemetry serialization, source-health/media-health
separation, unavailable gateways, stale-media-to-stalled transitions,
reconnect accounting, route authority classification, and configuration of
bounded supervision thresholds. Fake-gateway tests are deterministic unit and
integration evidence; they are not physical-camera or browser certification.

## SS-WP-011 physical-device certification

Physical certification is classified `PHYSICAL_DEVICE` and requires a completed
report from `docs/certification/TEMPLATE.md`, a matrix entry matching
`MATRIX_SCHEMA.yaml`, and sanitized API/browser/failure evidence. The capture
helper reads normalized source, capability, media, gateway-health, and playback
endpoints but never sends PTZ or destructive device commands. PTZ movement,
power-cycle, network-disconnect, and soak observations require explicit human
operator execution and recording.
# Media artifact tests

Artifact-store tests are `UNIT` tests and use temporary local directories.
Snapshot behavior uses deterministic frame-buffer fixtures. Clip capture is
bounded and uses the FFmpeg process boundary; tests that invoke FFmpeg should
be classified as local integration tests. These tests do not certify a
physical camera or continuous recording behavior.
