# Sentinel Streaming Admin and Setup Console

Sentinel Streaming owns a first-party web interface for infrastructure setup
and operations. This console is separate from Sentinel Home, Sentinel Campus,
and Sentinel Buildings. It must not become another homeowner, campus, or
building surveillance application.

## Primary modes

- **SETUP** — discover, install, verify, configure, and name cameras.
- **OPERATIONS** — inspect streams, health, recovery, AI processing, media
  gateway state, events/logs, configuration, and diagnostics.

## Add Camera wizard

The default path is designed for a non-technical operator:

1. Choose **Add Camera**.
2. Discover cameras using supported local mechanisms such as ONVIF.
3. Select a discovered device.
4. Request credentials only when required.
5. Discover ONVIF/device/media/PTZ capabilities.
6. Select an appropriate usable video profile automatically.
7. Verify connectivity, ingest, browser playback, and available capabilities.
8. Show a live preview before committing the camera.
9. Capture a friendly name and location.
10. Save the camera for authorized Sentinel API consumers.

Manual RTSP configuration remains available under **Advanced** for unsupported
or unusual devices. It is not the normal onboarding path.

## Verification result

The wizard should present normalized results for:

- camera connectivity
- ONVIF discovery/service access
- video profile
- RTSP ingest
- browser playback
- audio (`supported` or `unsupported`)
- PTZ and individual operations (`supported` or `unsupported`)
- AI (`ready` or `not configured`)
- health monitoring

Each check must distinguish unsupported, not configured, failed, and not
checked. Technical details remain available for operators without making them a
prerequisite for ordinary setup.

## Operations areas

The console should provide Dashboard, Cameras, Discovery, Streams, AI /
Processing, Media Gateway, Health, Events / Logs, System Configuration, and
About / Diagnostics areas. PTZ controls are visible and enabled only for
capabilities confirmed for the selected camera/profile, and PTZ test actions
use the protected API and produce operational evidence.

Playback uses Sentinel's normalized playback contract. The console and domain
applications do not depend on MediaMTX URLs, ports, profile tokens, codecs, or
WHIP/WHEP details.

## Camera onboarding

The Add Camera flow is discovery-first:

1. Find cameras.
2. Select a camera.
3. Enter credentials only when required.
4. Inspect ONVIF capabilities.
5. Automatically select the best usable media profile.
6. Validate RTSP.
7. Register browser playback.
8. Open live preview and expose PTZ testing only when supported.
9. Assign a friendly name and location.

The normal path presents product-language results such as “Camera rejected the
supplied credentials” or “No usable video profile was found.” Raw protocol
details remain under a technical-details view. Onboarding sessions are
in-memory and intentionally not a persistence or secrets-management subsystem;
those concerns are later product work.
