# Local RTSP test source

This development-only tool uses the locally installed FFmpeg executable and
its `testsrc2` filter to publish a moving, deterministic video pattern over
RTSP. It is intentionally not part of Sentinel production deployment and is
not the full Sentinel Virtual Camera Lab.

From the repository root:

```bash
./tools/rtsp-test-source/start.sh
```

The default source is:

```text
rtsp://127.0.0.1:8554/front-gate
```

Override the fixture without editing the tool:

```bash
SENTINEL_TEST_WIDTH=960 \
SENTINEL_TEST_HEIGHT=540 \
SENTINEL_TEST_FPS=30 \
SENTINEL_TEST_PATH=lab-camera \
./tools/rtsp-test-source/start.sh
```

MediaMTX must be listening on the selected host and port. Stop the publisher
with Ctrl-C. This verifies moving video transport only; it does not provide
ONVIF, PTZ, audio, scenarios, or physical-camera compatibility.
