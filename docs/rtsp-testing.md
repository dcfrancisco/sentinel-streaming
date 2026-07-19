# RTSP test profile

The RTSP adapter uses FFmpeg as its decoder process. Install FFmpeg locally
and run a disposable MediaMTX server for camera-free validation:

```bash
docker run --rm --name sentinel-mediamtx -p 8554:8554 bluenviron/mediamtx:latest
```

In another terminal, publish a deterministic H.264 test pattern:

```bash
ffmpeg -re -f lavfi -i testsrc2=size=640x360:rate=15 \
  -c:v libx264 -pix_fmt yuv420p -f rtsp \
  rtsp://127.0.0.1:8554/front-gate
```

Start Sentinel with a synthetic source so no physical camera is required, then
register and start the RTSP source:

```bash
cargo run -- serve --bind 127.0.0.1:8081 --source synthetic
cargo run -- source add --id front-gate --type rtsp \
  --uri rtsp://127.0.0.1:8554/front-gate --width 640 --height 360 --fps 15
cargo run -- source start front-gate
curl -N http://127.0.0.1:8081/api/v1/streams/front-gate/mjpeg
```

Stopping and restarting the FFmpeg publisher exercises the source disconnect,
reconnect, recovery event, and source downtime paths. The source manager keeps
only one active worker because the current processing pipeline has one
`FrameProvider` stream; switching sources stops the previous active worker.

RTSP credentials should be supplied through environment references in the API
payload. They are resolved only inside the worker and are never returned by
source status or written to logs.
# RTSP video simulator

The repository includes a deterministic local video fixture and scripts for
publishing it as a looping RTSP camera. From the workspace root:

```bash
./scripts/start-rtsp-simulator.sh
```

The publisher uses FFmpeg and sends the MP4 to MediaMTX at:

```text
rtsp://127.0.0.1:8554/front-gate
```

MediaMTX is started with `ops/rtsp/docker-compose.yml` when Docker is
available. A local `mediamtx` binary is also supported. Stop the simulator
with `./scripts/stop-rtsp-simulator.sh`.

To connect Streaming to the simulator, configure an `rtsp` source with TCP
transport and start Streaming. For development without MediaMTX, use the
`video-file-demo.yaml` configuration; it decodes the same MP4 directly through
FFmpeg while exercising the identical FrameProvider, pipeline, buffer, MJPEG,
and frame-capture boundaries.
