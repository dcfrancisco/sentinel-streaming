# Capability-driven PTZ

SS-WP-006 adds PTZ control through the existing ONVIF adapter. It does not create a second protocol stack and does not infer support from a camera model or manufacturer.

## API

- `GET /api/v1/sources/{id}/ptz` — normalized PTZ availability and controls;
- `POST /api/v1/sources/{id}/ptz/move` — continuous, relative, or absolute movement;
- `POST /api/v1/sources/{id}/ptz/stop` — stop supported movement axes;
- `GET /api/v1/sources/{id}/ptz/presets` — normalized preset IDs and names;
- `POST /api/v1/sources/{id}/ptz/presets/{presetId}/goto` — move to a listed preset.

Movement values are finite numbers in `[-1, 1]`. API requests contain normalized Sentinel fields and cannot submit raw SOAP.

Every operation checks the camera's normalized capabilities before network I/O. Unsupported operations return stable errors such as `PTZ_NOT_SUPPORTED`. ONVIF profile and preset tokens remain in the runtime adapter context and are never returned in normal API responses or operational event details.

## Runtime context and restart behavior

ONVIF inspection establishes an in-memory PTZ context containing the device service endpoint, selected media profile token, and request credentials. Credentials are not persisted. After restart, or after the context is lost, the camera must be inspected again before PTZ commands can be issued.

## Authority and evidence

PTZ commands pass through an explicit `PtzAuthority` boundary and require the
existing authenticated API operator path. The boundary currently identifies
the authenticated principal as `operator`; deployments can replace it with
actor-, role-, and policy-aware authorization without exposing ONVIF access to
clients.

Successful and failed commands create operational events containing the camera ID, operation, movement or normalized preset ID, timestamp, outcome, and correlation ID. PTZ events are separate from AI observations, incidents, and evidence.

## Testing scope

The deterministic ONVIF emulator confirms actual SOAP receipt for continuous, relative, absolute, stop, preset listing, and go-to-preset operations. Non-PTZ and unsupported-mode behavior is capability-gated before network I/O. These are emulator/protocol tests, not physical-device certification.

Physical PTZ compatibility remains a future work package.
