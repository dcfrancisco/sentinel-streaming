# Physical-device certification report

## Classification

- Evidence class: `PHYSICAL_DEVICE`
- Result: `PASS` / `PASS_WITH_LIMITATIONS` / `FAIL` / `NOT_SUPPORTED` / `NOT_TESTED`
- Certification date (UTC):
- Tester:
- Sentinel Streaming version:
- Sentinel Streaming commit:
- Evidence directory:

## Hardware

- Vendor:
- Model:
- Hardware revision:
- Firmware version:
- Region/variant:
- Network transport:
- Test account: least-privilege account used; do not record its secret

## Certification matrix

| Capability | Result | Evidence/notes |
| --- | --- | --- |
| Discovery |  |  |
| Authentication |  |  |
| Device information |  |  |
| ONVIF profile(s) |  |  |
| Media profiles |  |  |
| RTSP URI retrieval |  |  |
| RTSP validation |  |  |
| Video codec |  |  |
| Resolution(s) |  |  |
| FPS |  |  |
| Audio presence |  |  |
| Snapshot |  |  |
| Events |  |  |
| PTZ |  |  |
| Continuous move |  |  |
| Relative move |  |  |
| Absolute move |  |  |
| Zoom |  |  |
| Presets |  |  |
| WebRTC playback |  |  |
| HLS playback |  |  |
| Onboarding flow |  |  |
| Health monitoring |  |  |
| Recovery after disconnect |  |  |
| Media telemetry |  |  |
| Reconnect behavior |  |  |
| Long-running stability |  |  |

## Onboarding evidence

- Automatically discovered: `yes` / `no`
- Credentials requested only when needed: `yes` / `no`
- Best media profile selected automatically: `yes` / `no`
- RTSP validated automatically: `yes` / `no`
- Browser preview ready before completion: `yes` / `no`
- PTZ correctly detected: `yes` / `no` / `not applicable`
- Readiness result matched observed capability: `yes` / `no`
- Manual RTSP URL required: `yes` / `no`

## Controlled failures

Record each scenario independently:

- camera network disconnect;
- camera power-cycle;
- MediaMTX stop/restart;
- blocked network path;
- Sentinel restart.

For each scenario record source health, RTSP/validation state, media-gateway
state, browser result, detection time, recovery attempts, recovery time, and
whether operator intervention was required.

## Security checks

- credentials absent from API output;
- credentials absent from logs/events;
- browser URLs contain no RTSP credentials;
- PTZ denied without authority and allowed with authority;
- admin/API authentication enforced.

## Limitations and conclusion

Known quirks, firmware-specific behavior, unsupported advertised capabilities,
and the final compatibility conclusion belong here. Do not generalize this
result to other models, firmware versions, or vendors.
