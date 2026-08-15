# Standalone installation (macOS)

SS-WP-014 officially supports macOS on the development architecture used to
produce the release bundle. The first supported mode is a user-local launchd
agent; it does not require Docker, PostgreSQL, or root access.

## Fast path

From a packaged directory containing `bin/`, `config/`, and `third-party/`:

```bash
./packaging/macos/install.sh
```

The installer creates the runtime, configuration, state, artifact, log, and
LaunchAgent locations, preserves an existing configuration, validates it, and
starts the service. It prints the one-time bootstrap token and Admin URL.

The packaged default is camera-free. A physical camera is not required for
installation or control-plane readiness; cameras can be discovered and added
from the Admin setup workflow after the service starts.

The packaged developer/test configuration explicitly uses `OPEN_LOCAL_TEST`.
It is loopback-only and intentionally not production-safe. Set
`security.mode: LOCAL_ADMIN_AUTH` and configure an administrator token before
using a non-local interface.

The default readiness path is:

```text
http://127.0.0.1:8081/admin
```

Sign in with the printed bootstrap token. Replace the bootstrap environment
secret with a deployment-managed `SENTINEL_ADMIN_TOKEN` before exposing the
service beyond the local machine.

## Build a bundle

From a source checkout:

```bash
./packaging/macos/package.sh
```

This builds the locked release binary and creates a versioned tarball under
`dist/`. MediaMTX is not downloaded silently. To include a reviewed executable
supplied by the operator, use `MEDIAMTX_BIN=/path/to/mediamtx`.

## Service operations

```bash
launchctl print "gui/$(id -u)/com.sentinel.streaming"
launchctl kickstart -k "gui/$(id -u)/com.sentinel.streaming"
launchctl bootout "gui/$(id -u)/com.sentinel.streaming"
```

The service is configured with `RunAtLoad` and `KeepAlive`, so it starts after
login and restarts after an unexpected exit. Sentinel handles graceful
shutdown and does not own a MediaMTX child process in this first packaging
slice.

## Uninstall

```bash
./packaging/macos/uninstall.sh
```

This removes binaries and the service definition but intentionally preserves
configuration, state, artifacts, and logs. Review and remove those directories
manually only when data deletion is intended.
