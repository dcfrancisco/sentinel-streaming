# Self-Healing Runtime

Sentinel Streaming follows the principle: **recover before reporting**.

The runtime attempts local corrective action before a product or operator must
interpret a failure. This service does not send notifications. Recovery and
failure events are stored for future consumers such as Sentinel Home or
Watchtower.

## Health model

The `HealthMonitor` tracks component states:

- `healthy`
- `degraded`
- `recovering`
- `failed`

The monitor covers the runtime components that participate in recovery:

- camera source
- pipeline
- Vision provider
- future MJPEG and event-engine supervisors

Current component health, state transitions, and consecutive failures are
included in `/api/v1/status`.

## Recovery engine

`RecoveryEngine` centralizes recovery bookkeeping and emits structured recovery
events. It records attempts, successful recoveries, failed attempts, latency,
and the number of currently degraded components.

Recovery event names include:

- `source.reconnecting`
- `source.recovered`
- `source.reconnect_failed`
- `vision.recovering`
- `vision.recovered`
- `pipeline.recovering`
- `pipeline.recovered`

These events use the existing bounded Event Store and are available through the
existing event REST and SSE interfaces.

## Current recovery behavior

### Camera

The source manager transitions a failed camera to `Disconnected`, then
`Reconnecting`. It retries camera worker creation with bounded exponential
backoff and optional jitter until the camera recovers, the source is stopped, or
the runtime shuts down.

### Pipeline

The runtime supervises the pipeline task. A pipeline error starts a recovery
cycle, waits for a retry interval, recreates the pipeline stages, and resumes
processing unless shutdown has been requested.

### Vision

Vision provider errors are isolated from capture and pipeline execution. The
scheduler continues on its configured interval, marks Vision as recovering, and
marks it healthy after a successful provider response.

### MJPEG

MJPEG clients are independently scoped. A disconnected client drops its stream
guard and is removed from the active viewer count without affecting capture or
other viewers. More advanced stalled-stream cleanup remains a future hardening
task.

## Policy configuration

The effective runtime configuration exposes recovery policy settings:

```yaml
recovery:
  camera:
    enabled: true
    retry_forever: true
    initial_delay_ms: 500
    max_delay_seconds: 30
    jitter: true
  vision:
    enabled: true
    retry_count: 3
    cooldown_seconds: 30
  mjpeg:
    cleanup_interval_seconds: 30
```

The current runtime uses camera backoff settings and Vision's scheduled retry
behavior. Configuration-file loading and full Vision retry/cooldown policy
overrides remain planned work.

## Metrics and logs

Recovery metrics are included in `/metrics`:

- `sentinel_recovery_attempts`
- `sentinel_recovery_successes`
- `sentinel_recovery_failures`
- `sentinel_recovery_average_latency_ms`
- `sentinel_recovery_degraded_components`

Recovery starts, attempts, completions, and failures are logged with component,
message, attempt count, and recovery latency fields where applicable.
