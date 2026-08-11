# Media telemetry and supervision

SS-WP-010 keeps camera/source health separate from media-delivery health. A
source may be RTSP-healthy while MediaMTX is unavailable, or a reachable
gateway may report a stalled path.

`MediaTelemetry` is the provider-neutral contract returned by
`GET /api/v1/sources/{id}/media` and included in source list/detail responses.
It can report protocol, codec, resolution, observed FPS, bitrate, audio
presence, stream start, last media activity, reconnect count, delivery state,
available playback protocols, gateway state, audio codec, sample rate, channel
count, audio delivery state, and last audio activity. Values are `null`, empty, or
`UNKNOWN` when the gateway cannot observe them; Sentinel does not invent media
measurements.

Delivery states are `UNKNOWN`, `STARTING`, `READY`, `DEGRADED`, `STALLED`,
`UNAVAILABLE`, and `RECOVERING`. The bounded `MediaSupervisor` periodically
checks registered/expected media paths, applies startup and stall thresholds,
updates telemetry, and emits normalized operational events. It uses the
existing bounded semaphore and an in-flight source set so network I/O is not
performed under repository locks and duplicate checks do not run concurrently.

The MediaMTX adapter uses structured `/v3/paths/get/{path}` data and the
structured paths health API. It does not scrape logs or expose MediaMTX path,
reader, or response structures through Sentinel APIs. Byte deltas can produce
an observed bitrate; FPS and exact codec metadata remain unknown when MediaMTX
does not provide them.

Configuration:

```text
SENTINEL_MEDIA_SUPERVISION_ENABLED
SENTINEL_MEDIA_SUPERVISION_INTERVAL_MS
SENTINEL_MEDIA_STALL_TIMEOUT_MS
SENTINEL_MEDIA_STARTUP_TIMEOUT_MS
```

This work package does not implement recording, physical-camera certification,
or per-viewer MediaMTX authorization. Audio telemetry is transport metadata;
it is not speech or sound-event intelligence.
