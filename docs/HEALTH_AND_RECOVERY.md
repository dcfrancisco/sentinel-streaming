# Stream Health and Recovery

SS-WP-004 extends the source-level health record with a periodic monitor and
bounded RTSP recovery. The existing `RecoveryEngine`/`HealthMonitor` remains
the operational component-health mechanism.

Health values are:

- `unknown` — no validation result exists;
- `healthy` — the latest explicit RTSP validation succeeded;
- `degraded` — reserved for later partial/runtime health conditions;
- `unhealthy` — the latest explicit validation failed.

Recovery values are:

- `idle` — no recovery loop is active;
- `recovering` — a bounded retry sequence is in progress;
- `exhausted` — retry attempts are exhausted, or the failure is not safe to
  retry automatically.

The source API exposes:

- validation state;
- health state;
- last validation attempt;
- last successful validation;
- last normalized failure;
- consecutive validation failures.
- recovery state and attempt count;
- recovery timestamps and the next scheduled retry time.

When enabled, the monitor checks registered RTSP sources at the configured
interval. Checks are bounded by a semaphore and only one recovery loop may run
for a source. Retryable failures are source-unreachable, connection-timeout,
and unknown failures. Authentication, malformed/configuration, missing-stream,
protocol, and unsupported-source failures are not retried automatically.
Retry delays grow progressively and are capped; maximum attempts are bounded
by configuration. Shutdown cancels a pending retry wait.

This work package does not supervise decoded media, restart FFmpeg, or provide
full RecoveryEngine orchestration for video/audio pipelines. RTSP validation
still proves endpoint control-plane usability only; it does not prove decoded
video, audio, browser playback, AI readiness, or physical-camera compatibility.
## Media delivery supervision

Media delivery is tracked separately from camera/source health. The
`MediaSupervisor` checks registered media paths on a bounded interval and can
report `STARTING`, `READY`, `DEGRADED`, `STALLED`, `UNAVAILABLE`, or
`RECOVERING` without changing the RTSP source health. It emits `MEDIA_READY`,
`MEDIA_DEGRADED`, `MEDIA_STALLED`, `MEDIA_GATEWAY_UNAVAILABLE`,
`MEDIA_GATEWAY_RESTORED`, and recovery events when state changes.

The supervisor uses bounded concurrency and one in-flight check per source.
Startup and stale-activity thresholds are configurable; it does not scrape
logs or start an unbounded retry loop. Source reconnection remains owned by the
existing source `HealthMonitor`/`RecoveryEngine`.
