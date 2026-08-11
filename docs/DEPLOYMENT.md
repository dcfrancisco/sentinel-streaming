# Deployment

Sentinel Streaming remains usable without MediaMTX. In that mode RTSP
validation, ONVIF discovery, PTZ, health, and operational APIs continue to work;
browser playback reports `MEDIA_GATEWAY_UNAVAILABLE`.

MediaMTX can run as an external local service, a separately managed process, a
sidecar/package, or a container. Sentinel does not assume a Docker runtime or a
hard-coded executable path.

Example configuration:

```yaml
media_gateway:
  enabled: true
  kind: mediamtx
  api_url: http://127.0.0.1:9997
  webrtc_base_url: http://127.0.0.1:8889
  hls_base_url: http://127.0.0.1:8888
  timeout_ms: 3000
```

Sentinel supports pluggable persistence. SQLite is an optional embedded backend
for standalone deployments; it is not a mandatory dependency of this runtime.

## Authentication and TLS

Set `SENTINEL_ADMIN_TOKEN` for a standalone administrator and optionally set
`SENTINEL_OPERATOR_TOKEN` and `SENTINEL_VIEWER_TOKEN` for least-privilege
access. For first run, set a high-entropy `SENTINEL_BOOTSTRAP_TOKEN`, use it
once to reach the admin console, then replace it with a deployment-managed
administrator token. Never use a known example token remotely.

Sentinel does not claim secure remote operation over plain HTTP. Terminate TLS
at a trusted reverse proxy or on a Rust HTTP stack configured by the deployment,
forward `Authorization` safely, and keep MediaMTX admin/playback listeners on a
private interface or behind equivalent authorization. Token rotation, durable
sessions, and per-viewer MediaMTX policy are later hardening work.

## Local playback verification

For a Docker-free local check, run a pinned MediaMTX binary as a separate
process, then run `./tools/rtsp-test-source/start.sh`. The publisher, MediaMTX,
and Sentinel are three separate processes. This is a
`LOCAL_MEDIAMTX_INTEGRATION` test profile only; it does not certify physical
cameras or production deployment orchestration.
# Media artifact deployment

Install FFmpeg when short-clip capture is required. Set
`SENTINEL_FFMPEG` when the executable is not on `PATH`. Set
`SENTINEL_MEDIA_ARTIFACT_ROOT` to a writable local filesystem location with
appropriate operator-only permissions. This work package does not add a
database, continuous recording service, backup worker, or object-storage
adapter.
