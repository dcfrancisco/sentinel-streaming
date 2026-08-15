# Security boundaries

SS-WP-009 adds a standalone bearer-token boundary without coupling the runtime
to an enterprise identity provider. The explicit `security.mode` setting
controls the active boundary:

- `OPEN_LOCAL_TEST` allows unauthenticated local validation and requires a
  loopback bind. The Admin UI labels this development/test mode.
- `LOCAL_ADMIN_AUTH` requires the configured bearer-token credentials and fails
  closed when none are configured.
- `EXTERNAL_IDENTITY` is reserved for the future ADR 0009 integration boundary
  and is rejected until implemented.

For `LOCAL_ADMIN_AUTH`, configure `SENTINEL_VIEWER_TOKEN`,
`SENTINEL_OPERATOR_TOKEN`, `SENTINEL_ADMIN_TOKEN`, or the legacy-compatible
`SENTINEL_API_TOKEN`. `SENTINEL_BOOTSTRAP_TOKEN` is an explicitly supplied,
temporary administrator token for first run. Sentinel ships no default
credential.

Liveness, readiness, and version are public. In `OPEN_LOCAL_TEST`, all local
Admin/API operations are available without a token. In `LOCAL_ADMIN_AUTH`, all
other API operations require `Authorization: Bearer <token>` and missing
credentials produce a configuration error rather than an open service.

PTZ is consequential device control. All PTZ operations pass through the
Sentinel API and the explicit `PtzAuthority` boundary; the admin page never
calls ONVIF directly. `CONTROL_PTZ` is required and authorization is checked
before an ONVIF command is issued.

PTZ operational events use the existing event store and contain only safe
normalized request data, source ID, operation, actor, outcome, and correlation
ID. Passwords, bearer tokens, authorization headers, SOAP bodies, profile
tokens, and device credentials are not recorded. Authentication and
authorization failures use stable API codes and produce security events.

Playback API access requires `VIEW_STREAM`, and returned browser URLs never
contain camera credentials or MediaMTX admin credentials. The current adapter
does not yet provision per-viewer MediaMTX JWT/auth policies, so deployments
must keep MediaMTX listeners on a trusted network or behind an authenticated
TLS reverse proxy. This is an explicit remaining CR-5 gap.

Sentinel Streaming supports pluggable persistence. SQLite is an optional
embedded backend for standalone deployments; it is not a mandatory dependency
of the runtime or security implementation.
# Media artifact security

Snapshots and clips are consequential operational outputs. Capture, viewing,
and deletion use separate authorities and are enforced server-side; browser
JavaScript cannot bypass the service boundary. Events retain actor, source,
artifact, and correlation information without credentials or raw FFmpeg/RTSP
secrets. Artifact metadata exposes logical references only.
## Audio privacy boundary

Audio is transport-only in SS-WP-013. The existing role model includes the
explicit `VIEW_AUDIO` authority so deployments can separate audio access from
other stream access when policy requires it. Admin playback starts muted and
requires an operator action to unmute. No microphone, talk-back, or transport
credential is exposed to the browser.
