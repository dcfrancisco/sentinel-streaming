# Sentinel Streaming — Human Functional Test Guide

This guide is for manually validating Sentinel Streaming with physical cameras,
real networks, local media, and deterministic simulators. It is intentionally
not an automated test plan. Record the evidence requested in each test so a
camera can be certified instead of merely reported as “working.”

## 1. Test scope and safety

Sentinel Streaming acquires frames, sends them through the processing pipeline,
stores recent frames in the bounded `FrameBuffer`, and exposes operational
interfaces. It observes video; it does not make security decisions.

Use a test camera, an isolated network where possible, and non-production
credentials. Do not expose the unauthenticated development API to the public
Internet. If credentials are needed, use environment variables or a local
secret manager. Never paste passwords into logs, screenshots, tickets, or
committed YAML.

The current runtime has one active `FrameProvider` stream. Multiple sources may
be registered, but use one active source at a time unless the deployment has
explicitly been extended for multi-provider execution.

## 2. Tester workstation prerequisites

Install:

- Rust toolchain and Cargo.
- FFmpeg on `PATH`, or set `SENTINEL_FFMPEG` to its full path.
- `curl` and a browser capable of displaying multipart MJPEG.
- Docker Desktop for the MediaMTX simulator, unless using a local MediaMTX binary.
- Network access to the camera on the same LAN for RTSP/ONVIF tests.
- Camera privacy permission for Terminal/IDE on macOS when testing built-in or USB cameras.

Supported development environments are macOS, Linux, and other platforms
supported by the selected camera backend. Native camera permissions and device
enumeration are platform-specific; network and virtual-source tests do not
require physical camera access.

From the Streaming repository:

```bash
cargo build
cargo test
cargo run -- version
```

Start a camera-free baseline in Terminal A:

```bash
cargo run -- serve --bind 127.0.0.1:8081 --source synthetic
```

Set a variable for the remaining examples:

```bash
export SENTINEL=http://127.0.0.1:8081
```

If the API token is enabled, add `-H "Authorization: Bearer $SENTINEL_API_TOKEN"`
to protected requests. The health and version endpoints remain available for
startup checks.

## 3. Baseline verification

Perform this before every source test.

1. Confirm the process starts without a fatal error.
2. Run:

   ```bash
   curl -fsS "$SENTINEL/health/live"
   curl -fsS "$SENTINEL/health/ready"
   curl -fsS "$SENTINEL/api/v1/version"
   curl -fsS "$SENTINEL/api/v1/sources"
   ```

3. Confirm liveness is successful, readiness reports the runtime state, and
   the expected source appears in the source list.
4. Confirm logs contain startup, source initialization, pipeline initialization,
   HTTP server startup, and no unexplained repeated error loop.
5. Check metrics:

   ```bash
   curl -fsS "$SENTINEL/metrics" > /tmp/sentinel-metrics.txt
   rg "sentinel_(source|frames|mjpeg|buffer|recovery)" /tmp/sentinel-metrics.txt
   ```

## 4. Common source registration workflow

The preferred manual workflow is test first, register second, start third.

### Test without registering

Send a `POST /api/v1/sources/test` request. The request accepts `kind` and the
same source options used for registration. For a synthetic source:

```bash
curl -fsS -X POST "$SENTINEL/api/v1/sources/test" \
  -H 'Content-Type: application/json' \
  -d '{"kind":"synthetic","name":"QA Pattern","width":640,"height":360,"fps":15}'
```

Success returns `success: true`, a resolution, measured FPS, latency, and a
diagnostic message. A failed test should return a useful message and must not
register or leave a worker running.

### Register and start

```bash
curl -fsS -X POST "$SENTINEL/api/v1/sources" \
  -H 'Content-Type: application/json' \
  -d '{"id":"qa-pattern","kind":"synthetic","name":"QA Pattern","width":640,"height":360,"fps":15}'

curl -fsS -X POST "$SENTINEL/api/v1/sources/qa-pattern/start"
curl -fsS "$SENTINEL/api/v1/sources/qa-pattern"
```

Fetch a bounded JPEG:

```bash
curl -fsS "$SENTINEL/api/v1/streams/qa-pattern/frame" -o /tmp/qa-pattern.jpg
file /tmp/qa-pattern.jpg
```

Open the continuous browser stream:

```text
http://127.0.0.1:8081/api/v1/streams/qa-pattern/mjpeg
```

The source should become `Running`, `last_frame` should advance, and frame
metrics should increase. Stop it after the test:

```bash
curl -fsS -X POST "$SENTINEL/api/v1/sources/qa-pattern/stop"
```

## 5. Built-in camera

### Prerequisites

- A laptop or desktop camera.
- Camera permission granted to the terminal or IDE launching Sentinel.
- No other application exclusively holding the camera.

### Setup

1. Close applications that may own the camera, such as video conferencing tools.
2. On macOS, verify the device is visible:

   ```bash
   system_profiler SPCameraDataType
   ```

3. Select the device index if necessary:

   ```bash
   export SENTINEL_CAMERA_INDEX=0
   ```

4. Start Sentinel without `--source synthetic`:

   ```bash
   cargo run -- serve --bind 127.0.0.1:8081
   ```

### Verification

1. Query `/api/v1/sources` and confirm `builtin` is present and running.
2. Open `/api/v1/streams/builtin/mjpeg` in a browser.
3. Capture `/api/v1/streams/builtin/frame` and confirm it is a valid JPEG.
4. Move in front of the camera. Confirm the preview changes and `last_frame`
   advances.
5. Record the observed resolution, approximate FPS, startup latency, and any
   camera permission log entries.

### Failure tests

- Deny permission: startup should report a clear camera-open failure and the
  service should remain observable rather than panic.
- Open the camera in another application: expect a failed or disconnected source
  and recovery attempts where supported.
- Disconnect an external capture device: expect source degradation,
  reconnect attempts, and recovery after reconnecting.

## 6. USB/UVC webcam

### Prerequisites and setup

1. Plug the USB webcam directly into the test workstation.
2. Prefer a direct port over an unpowered hub.
3. Verify the operating system sees the device. On macOS use
   `system_profiler SPCameraDataType`; on Linux inspect `/dev/video*` and use
   `v4l2-ctl --list-devices` when available.
4. Grant camera permission to the process.
5. Start with `SENTINEL_CAMERA_INDEX=0`. If the wrong camera appears, try the
   next index and repeat.

### Verification

Use the built-in camera procedure. The USB/UVC webcam is expected to use the
same provider boundary and should appear as the built-in camera source. Confirm
the physical webcam image, not only a successful HTTP response.

### Failure tests

Test a second camera, an unplug/replug cycle, and a busy-device condition. The
service must not return frames from a different device without the source
metadata making that clear.

## 7. Synthetic camera

The synthetic source is the standard hardware-free control. It produces an
animated pattern and is required for repeatable diagnostics.

```bash
cargo run -- serve --bind 127.0.0.1:8081 --source synthetic
```

For a custom source, register:

```bash
curl -fsS -X POST "$SENTINEL/api/v1/sources" \
  -H 'Content-Type: application/json' \
  -d '{"id":"synthetic-720p","kind":"synthetic","width":1280,"height":720,"fps":30}'
curl -fsS -X POST "$SENTINEL/api/v1/sources/synthetic-720p/start"
```

Verify that the rectangle moves, frame sequence numbers advance in source
metadata, JPEG and MJPEG endpoints work, and the configured dimensions/FPS are
reported. This source is also the control case for network interruption,
MJPEG viewers, Vision mock mode, and endurance testing.

## 8. Video file source

### Prerequisites

- A local video file supported by FFmpeg, preferably H.264 MP4.
- Read permission for the Sentinel process.
- FFmpeg installed.

### Setup and registration

```bash
curl -fsS -X POST "$SENTINEL/api/v1/sources" \
  -H 'Content-Type: application/json' \
  -d '{"id":"office-demo","kind":"video-file","path":"/absolute/path/office.mp4","loop":true,"width":640,"height":360,"fps":15}'
curl -fsS -X POST "$SENTINEL/api/v1/sources/office-demo/start"
```

### Verification

Confirm the video plays, reaches end-of-file, loops when configured, and
continues producing frames. Check logs for `video file loop completed`. Set
`loop:false` and verify the source stops or becomes inactive at EOF according to
the source status.

### Failure tests

Use a missing path, unreadable file, corrupt file, and unsupported codec. The
source should fail with a diagnostic, increment failure/decode metrics, and not
take down the HTTP service.

## 9. RTSP camera

### Prerequisites

- A camera with RTSP enabled.
- Camera IP/hostname, port, stream path, and test credentials.
- Workstation reachability to the camera.
- FFmpeg installed.

### Setup

1. Give the camera a stable DHCP reservation or static address.
2. Enable RTSP in the camera application or web interface.
3. Record the vendor-provided stream path and whether TCP transport is required.
4. Verify reachability:

   ```bash
   ping CAMERA_HOST
   nc -vz CAMERA_HOST 554
   ```

5. Validate the stream independently with FFplay/VLC if available. Do not
   treat an open TCP port as proof that the stream is valid.

### Test connection

Use environment-backed credentials:

```bash
export FRONT_GATE_USERNAME='test-user'
export FRONT_GATE_PASSWORD='test-password'

curl -fsS -X POST "$SENTINEL/api/v1/sources/test" \
  -H 'Content-Type: application/json' \
  -d '{"kind":"rtsp","uri":"rtsp://CAMERA_HOST:554/STREAM_PATH","transport":"tcp","credentials":{"username_env":"FRONT_GATE_USERNAME","password_env":"FRONT_GATE_PASSWORD"},"width":640,"height":360,"fps":15}'
```

The response must not contain the password. A successful response must include
captured resolution and latency.

### Register and verify

Register the same payload with an `id`, start the source, view its MJPEG stream,
and observe `/api/v1/status`, `/api/v1/sources/{id}`, `/metrics`, and logs. Stop
the camera publisher or disconnect the network, wait for the source to become
disconnected/reconnecting, then restore it and verify recovery without a full
Sentinel restart.

## 10. Wi-Fi consumer camera

Wi-Fi is a network transport, not a Sentinel source kind. Test the camera as
RTSP, MJPEG, or ONVIF depending on what the device actually exposes.

### Identify support

1. Read the manufacturer specification and mobile-app settings.
2. Search for settings named `RTSP`, `NVR`, `ONVIF`, `NAS`, `Local access`, or
   `Third-party integration`.
3. Confirm whether local streaming is available without a cloud subscription.
4. Confirm the camera and test workstation are on the same LAN/VLAN.
5. Check whether the device requires a separate local-camera password.

### Enable RTSP

Enable RTSP in the camera UI, create a least-privilege test user, note the port
and main/substream path, then use the RTSP procedure above. Prefer the lower
resolution substream for initial setup and verify the stream independently.

### Identify ONVIF

Enable ONVIF if the camera provides the option, create ONVIF credentials, and
run:

```bash
curl -fsS "$SENTINEL/api/v1/sources/discover"
```

The device should appear with `onvif-discovery` and `stream-profile-discovery`
capabilities. The current implementation may return the device-management
address without selecting a media profile; use the vendor profile information
to complete a manual RTSP setup.

### Consumer-camera guidance

Many Tapo, Reolink, EZVIZ, Hikvision, Dahua/IMOU, Amcrest, and Annke models
offer some combination of RTSP or ONVIF, but support varies by model, firmware,
region, and subscription. Cloud-only Ring, Nest, Wyze, Arlo, and Blink models
may not expose a local stream. Do not infer compatibility from the brand alone.

For cloud-only cameras, document `Not compatible with local Streaming` unless a
documented local API or supported adapter is later added. Do not bypass vendor
security or reverse-engineer undocumented services.

## 11. ONVIF-discovered camera

### Setup

1. Put the camera and Sentinel host on the same broadcast domain.
2. Enable ONVIF and create test credentials.
3. Start Sentinel and call `/api/v1/sources/discover`.
4. Confirm the returned vendor, device address, capabilities, and stable ID.
5. Obtain the camera’s media profile/RTSP URI from the vendor UI or ONVIF tool.
6. Run the RTSP test-connection flow using that URI, then register it as an
   `rtsp` source.

### Verification and limitation

Discovery success proves WS-Discovery visibility and identity parsing. It does
not by itself prove authentication, media-profile negotiation, decoding, or
video availability. Those require the RTSP connection test and live-stream
verification. Record this distinction in the compatibility matrix.

## 12. MJPEG source

### Prerequisites and setup

- An HTTP multipart MJPEG endpoint, for example a compatible network camera or
  local test server.
- HTTP reachability from the Sentinel host.

Test it first:

```bash
curl -I http://CAMERA_HOST:PORT/STREAM_PATH
curl -fsS -X POST "$SENTINEL/api/v1/sources/test" \
  -H 'Content-Type: application/json' \
  -d '{"kind":"mjpeg","uri":"http://CAMERA_HOST:PORT/STREAM_PATH","width":640,"height":360,"fps":10}'
```

Register with the same `uri`, start it, and view the Sentinel MJPEG endpoint.
This verifies decode-and-re-encode through the common FrameProvider and buffer
path rather than directly proxying the camera.

## 13. MediaMTX simulator

Use this when no physical RTSP camera is available.

```bash
./scripts/start-rtsp-simulator.sh
```

If the helper is unavailable, use the documented Docker/FFmpeg procedure in
[RTSP testing](rtsp-testing.md). Register:

```bash
curl -fsS -X POST "$SENTINEL/api/v1/sources" \
  -H 'Content-Type: application/json' \
  -d '{"id":"front-gate","kind":"rtsp","uri":"rtsp://127.0.0.1:8554/front-gate","transport":"tcp","width":640,"height":360,"fps":15}'
curl -fsS -X POST "$SENTINEL/api/v1/sources/front-gate/start"
```

Verify the deterministic test pattern, then stop and restart the publisher.
Sentinel should report a source interruption, attempt recovery, and resume
frames after the publisher returns.

## 14. Required evidence for every source

Capture the following for each certification run:

- Date, tester, Sentinel commit/version, operating system, and FFmpeg version.
- Camera model, firmware, source kind, network topology, and resolution/FPS.
- A screenshot of the live browser MJPEG stream.
- The source JSON before, during, and after a disconnect test.
- Successful test-connection response, with credentials removed.
- `/health/live`, `/health/ready`, `/api/v1/status`, and relevant metrics.
- Relevant startup, connection, failure, reconnect, and recovery logs.
- Exact reproduction steps for any limitation or failure.

## 15. Expected success criteria

A source passes when it can be tested without registration, registered without
editing internal code, started through the manager, viewed through the common
JPEG/MJPEG endpoints, observed through status and metrics, and recovered after
the planned interruption. A source is only “AI ready” when a frame can be
retrieved from the bounded frame endpoint; AI provider availability is a
separate operational dependency.

See the [compatibility matrix](compatibility-matrix.md), [test checklist](test-checklist.md),
[troubleshooting guide](troubleshooting.md), and [camera certification process](camera-certification.md).
