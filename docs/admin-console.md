# Sentinel Streaming Admin and Setup Console

## Current implementation and operation

Start the service and open `http://127.0.0.1:8080/admin`. In
`LOCAL_ADMIN_AUTH` mode configure `SENTINEL_ADMIN_TOKEN` in the service
environment and sign in with that administrator token. Do not share it. The
default is `LOCAL_ADMIN_AUTH`. The opt-in `OPEN_LOCAL_TEST` mode bypasses
authentication and is restricted by configuration validation to loopback; use
it only in a trusted local test environment. Bearer tokens are held in browser session storage until the tab
session ends; there is no server-side session or cookie.

The current console provides a status overview, ONVIF onboarding, a manual
source registration form, source start/stop/removal, current-frame retrieval,
RTSP playback and capability-dependent PTZ, recent events, component health,
support bundle download, and runtime-only OpenAI vision settings. Camera
registration and vision changes made through this interface are not persisted.
Some capabilities described below remain targets rather than completed
features; consult [the implementation inventory](reviews/streaming-admin-inventory.md).
The responsive embedded console uses a dark, card-based operations layout;
camera inventory and empty states are rendered from live API data rather than
mock camera records.

Vision reads `OPENAI_API_KEY` only from the service environment; the console
shows only whether a nonempty key is present. `OPENAI_VISION_MODEL` defaults to
`gpt-5.4-mini`. Set both `pipeline.vision` and `vision.enabled` for startup
vision, or enable it for this process in the console. Stopping vision cancels
in-flight work; changes revert on restart. A failed provider does not stop
capture. Refresh the overview to see the latest observation and errors.

Source and vision mutations are authorized on the server. Browser mutations
with an `Origin` must be same-origin, and cross-origin access is not enabled by
default. Existing external browser clients need a same-origin gateway or an
explicitly designed and secured deployment policy. Token-bearing plain HTTP
should be used only on loopback; secure remote access requires TLS and a
trusted deployment boundary.

Sentinel Streaming owns a first-party web interface for infrastructure setup
and operations. This console is separate from Sentinel Home, Sentinel Campus,
and Sentinel Buildings. It must not become another homeowner, campus, or
building surveillance application.

## Views and routine tasks

The embedded console has five views:

- **Overview** reports streamer identity/status, registered/active/disconnected
  camera counts, active stream count, AI state, and actionable recent alerts.
- **Cameras** is the primary workspace. Registered sources appear as compact
  cards with friendly names, connection/stream state, a real frame preview
  when the source is running and the frame API has data, plus details/actions.
  An unavailable preview states that it is unavailable; the UI does not
  substitute sample footage.
- **Discovery** starts bounded ONVIF discovery, lists devices returned by the
  service, collects camera credentials only for inspection, and allows a
  verified camera to be registered. Manual RTSP setup is also available.
- **AI Vision** changes runtime-only vision settings, reports whether a
  credential is configured (never its value), shows provider failure counters,
  and displays the latest available observation.
- **Settings** is read-only and shows selected configuration, security mode,
  media-gateway status, health details, and a support-bundle download.

To add an ONVIF camera, open **Add camera**, leave the discovery address blank
for the default local-network scan, start discovery, select a result, enter
credentials if needed, and choose **Verify camera**. Review the returned
capability checks, set a friendly name, and save. Saving registers the source;
it does not start it. Open **Cameras**, select the new source, and choose
**Start camera** to begin streaming and make a real frame preview available.
If the scan returns no ONVIF devices, use manual RTSP setup; an empty discovery
result is not evidence that no cameras are connected.

For manual setup, provide a unique camera ID, name, and RTSP URL. Put
credentials in the separate fields, not in the URL. Query strings are allowed
in the RTSP path. After adding, start the source from **Cameras**. RTSP
playback, capture, and PTZ controls appear only where their source/API
capabilities support them.

## Discovery outcomes and troubleshooting

The UI distinguishes a completed search with no ONVIF responders from
unsupported discovery, permission denial, timeout, and network failure. Check
that Sentinel and the camera are on the same local network, that the operating
system allows local-network access, and that the camera supports ONVIF. If not,
use the manual RTSP flow. A registered camera that is stopped must be started;
an offline camera or missing frame is shown as unavailable rather than as a
placeholder.

If a camera will not start, open its details and use **Verify connection**
where offered. Check the source status and Settings diagnostics. For browser
playback, a running ingest and available browser playback gateway are separate
conditions; the current-frame preview can work even if an RTSP browser player
is unavailable. Support bundles can contain operational information, so
review and redact them before sharing.

To enable OpenAI Vision, set `OPENAI_API_KEY` in the service environment, then
enable vision in startup configuration or in **AI Vision**. The model field
defaults to the documented image-capable `gpt-5.4-mini` and can be overridden
through `OPENAI_VISION_MODEL` at startup. The key is never editable or
retrievable in the console. Vision settings changed in the UI are not
persistent and revert after restart. A provider failure is reported in
diagnostics and does not stop camera capture.

## Runtime and product boundaries

The current decoded pipeline has **one active source at a time**. The inventory
can contain multiple cameras, but starting a different source switches the
active processing path; simultaneous independent streams/previews and
per-camera concurrent AI are not implemented. Vision configuration is global
and applies to that active source. Camera edits, durable configuration
mutation, viewer counts, latency and several other operational metrics are
not available; the console avoids inventing them.

The console is an optional surface over Sentinel Streaming's existing Rust
services. It manages cameras, streaming resources, and AI observations; it
does not implement homeowner intrusion policies, alarms, or other Sentinel
Home Security workflows.

## Security and verification

Authenticated mode is the default. The `LOCAL_ADMIN_AUTH` mode requires an
administrator bearer token configured through the service environment. The
explicit `OPEN_LOCAL_TEST` mode bypasses authentication and is accepted only
for loopback binding; use it only for local development. Do not expose it on a
network interface. Browser mutations are same-origin checked, server
authorization protects administrative operations, and RTSP credentials are
entered separately from the stream URL. The browser does not receive or store
the OpenAI API key.

For a real-camera smoke test, add and verify a camera through Discovery, save
it, start it from Cameras, and confirm the preview updates with actual camera
frames. Then optionally configure `OPENAI_API_KEY`, enable vision, and confirm
an observation and vision metrics appear while frames continue updating. This
procedure is not evidence that a physical camera or live OpenAI request was
tested in a given build; report only tests actually performed.
