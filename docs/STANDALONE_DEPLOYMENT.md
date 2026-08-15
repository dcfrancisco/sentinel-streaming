# Standalone deployment

The first supported standalone deployment is a macOS user-local LaunchAgent.
The application remains persistence-backend agnostic and uses no database for
installation. SQLite is not introduced by SS-WP-014.

Set `instance_id` to a stable deployment identifier when the instance will be
operated or integrated by another product. `deployment_profile` defaults to
`standalone`.

## Layout

The installer uses platform-appropriate macOS locations:

```text
~/Library/Application Support/Sentinel Streaming/runtime   binaries
~/Library/Application Support/Sentinel Streaming/config    sentinel.yaml
~/Library/Application Support/Sentinel Streaming/state     durable state
~/Library/Application Support/Sentinel Streaming/state/artifacts
~/Library/Logs/Sentinel Streaming                          logs
~/Library/LaunchAgents/com.sentinel.streaming.plist        service
```

`SENTINEL_INSTALL_ROOT` can override the application root for development or
test installations. Configuration is never overwritten by reinstall.

## Configuration precedence

Sentinel applies configuration in this order:

```text
built-in defaults -> YAML config -> SENTINEL_* environment -> CLI overrides
```

Use `sentinel-streaming check-config --config path` before starting a service.
Use `sentinel-streaming doctor --config path` for product-language deployment
diagnostics. Neither command performs network I/O or exposes secrets.

Export a sanitized running-instance bundle with:

```bash
sentinel-streaming support-bundle \
  --endpoint http://127.0.0.1:8081/api/v1/support/bundle \
  --output ./support-bundle \
  --logs "$HOME/Library/Logs/Sentinel Streaming/sentinel-streaming.log"
```

## MediaMTX strategy

MediaMTX is an optional sibling runtime, not part of Sentinel's domain model.
The bundle can include an operator-supplied pinned MediaMTX executable, or an
external installation can be configured. Sentinel does not download arbitrary
binaries and does not manage a MediaMTX child process in this work package.
Configure and validate MediaMTX separately, then enable the existing
`MediaMtxAdapter` endpoints in `sentinel.yaml`.

The public Sentinel API remains MediaMTX-neutral and the `MediaGateway` boundary
remains replaceable.

## Readiness

Basic installation readiness does not require a camera or MediaMTX:

1. `check-config` succeeds;
2. LaunchAgent is loaded;
3. `curl http://127.0.0.1:8081/health/live` returns `{"status":"ok"}`;
4. `curl http://127.0.0.1:8081/health/ready` returns HTTP 200 for the Sentinel
   control plane;
5. `/admin` loads and accepts the bootstrap token.

Camera and media readiness are reported separately in source and
MediaGateway status. The service may be ready for setup while no camera is
configured or available.

When MediaMTX is enabled, `/api/v1/media/health` must additionally report a
reachable gateway. Use the service log for startup failures.
