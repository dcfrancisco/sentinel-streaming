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
