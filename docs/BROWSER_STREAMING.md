# Browser streaming

`GET /api/v1/sources/{id}/playback` returns normalized playback streams. WebRTC
is listed first as the low-latency interactive option. HLS is listed as a
compatibility fallback with standard, higher latency characteristics.

Sources must first be RTSP-validated and registered with the media gateway:

1. `POST /api/v1/sources/{id}/validate`
2. `POST /api/v1/sources/{id}/playback/register`
3. `GET /api/v1/sources/{id}/playback`

The admin console prefers WebRTC when the browser supports `RTCPeerConnection`
and falls back to HLS. MediaMTX remains responsible for WebRTC/HLS transport;
Sentinel does not implement signaling or a media server.

Browser URLs contain no camera credentials. Playback availability reflects
media-delivery health and does not replace RTSP source health.

Playback API calls require `VIEW_STREAM`. The adapter does not yet provision
per-viewer MediaMTX authorization; keep direct MediaMTX WebRTC/HLS listeners
private or place them behind an authenticated TLS reverse proxy.

## Local moving-video verification

SS-WP-007A uses `tools/rtsp-test-source/start.sh`, which publishes a moving
`testsrc2` pattern at `rtsp://127.0.0.1:8554/front-gate`. It is a standalone
development fixture, not a production source and not a physical-camera test.
Start MediaMTX first, start the wrapper second, then validate/register the
source and open `http://127.0.0.1:8080/admin`. A successful visual check is
actual moving video in the browser, not merely a successful JSON response.
