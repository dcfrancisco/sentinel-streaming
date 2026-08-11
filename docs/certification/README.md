# Physical-device certification

This directory contains the repeatable `PHYSICAL_DEVICE` certification
framework for Sentinel Streaming. It is separate from unit, protocol,
emulator, and `LOCAL_MEDIAMTX_INTEGRATION` evidence.

## Procedure

1. Prepare the camera on an isolated test network and record vendor, exact
   model, hardware revision, firmware, region, and test date.
2. Create a least-privilege camera account and configure the Sentinel operator
   credentials through the deployment secret boundary.
3. Run discovery and the Sentinel onboarding flow.
4. Inspect ONVIF device/media/capability results and record the selected
   profile.
5. Validate RTSP, register MediaGateway playback, and verify WebRTC and HLS
   in a normal browser.
6. Record codec, resolution, FPS, bitrate, startup time, last activity, and
   delivery state from the normalized APIs.
7. For PTZ cameras, an operator explicitly confirms each movement/preset test.
   The harness never sends PTZ commands automatically.
8. Disconnect the camera, stop/restart MediaMTX, block the network path, and
   restart Sentinel one scenario at a time. Record source, gateway, browser,
   and recovery states separately.
9. Run the initial 30-minute qualification, then the applicable 24-hour,
   72-hour, or 7-day stability profile.
10. Complete `TEMPLATE.md`, attach sanitized evidence, assign a result state,
    and add the device to the compatibility matrix.

The procedure is detailed in [TEMPLATE.md](TEMPLATE.md). The capture helper is
`tools/physical-certification/capture.sh` and only reads normalized APIs. It
does not power-cycle cameras, move PTZ, change configuration, or store tokens.

Never commit passwords, bearer tokens, raw Authorization headers, or private
network details. Evidence must retain the `PHYSICAL_DEVICE` classification.
No physical device has been certified by this work package unless a completed
report appears under `docs/certification/results/`.
