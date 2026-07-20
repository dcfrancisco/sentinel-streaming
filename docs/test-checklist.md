# Human Functional Test Checklist

Copy this checklist into a test record for each source.

## Environment

- [ ] Tester and date recorded.
- [ ] Sentinel commit/version recorded.
- [ ] Operating system and architecture recorded.
- [ ] FFmpeg version recorded.
- [ ] Camera model and firmware recorded.
- [ ] Network/VLAN and camera IP recorded without exposing credentials.
- [ ] Test credentials are non-production.

## Startup and registration

- [ ] Sentinel starts cleanly.
- [ ] `/health/live` succeeds.
- [ ] `/health/ready` reports expected readiness.
- [ ] Provider list identifies the source capability correctly.
- [ ] Discovery result is correct, or manual setup is documented.
- [ ] Test connection succeeds before registration.
- [ ] Test connection failure is clear and leaves no registered worker.
- [ ] Source registration succeeds.
- [ ] Source start succeeds.

## Media path

- [ ] Source state becomes running.
- [ ] `last_frame` advances.
- [ ] JPEG frame endpoint returns a valid JPEG.
- [ ] Browser MJPEG stream displays real changing frames.
- [ ] Resolution and FPS are plausible.
- [ ] `/metrics` counters increase.
- [ ] Frame retrieval is available for downstream AI analysis.

## Recovery

- [ ] Wrong credentials produce an explicit authentication/connection failure.
- [ ] Camera or publisher interruption changes source health.
- [ ] Reconnect attempts are visible in logs and metrics.
- [ ] Restoring the camera resumes frames without restarting Sentinel.
- [ ] Repeated start/stop operations are safe.
- [ ] Stopping Sentinel exits cleanly.

## Evidence

- [ ] Live stream screenshot captured.
- [ ] Source JSON captured before and after recovery.
- [ ] Health/status/metrics captured.
- [ ] Relevant logs attached.
- [ ] Limitations and exact reproduction steps recorded.
