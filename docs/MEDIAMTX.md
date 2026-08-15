# MediaMTX integration

MediaMTX is an external, optional media distribution service. Sentinel does not
spawn it, assume Docker, or reimplement a media server. It may be installed as
an executable, managed externally, packaged beside Sentinel, or deployed as a
container in a larger environment.

Configuration uses `media_gateway` settings or environment variables:

- `SENTINEL_MEDIA_GATEWAY_ENABLED`
- `SENTINEL_MEDIA_GATEWAY`
- `SENTINEL_MEDIAMTX_API_URL`
- `SENTINEL_MEDIAMTX_BASE_URL`
- `SENTINEL_MEDIAMTX_WEBRTC_BASE_URL`
- `SENTINEL_MEDIAMTX_HLS_BASE_URL`

The MediaMTX API is used only internally for source registration/removal and
health. Downstream clients receive Sentinel playback contracts, not MediaMTX
configuration or API URLs.

The adapter uses safe Sentinel-derived path identifiers and sends credentials
only in the internal upstream registration request. Credentials are never
placed in browser playback URLs or returned by the API.

## Local MediaMTX verification (SS-WP-007A)

This verification uses an external MediaMTX process and FFmpeg; neither is a
Sentinel production dependency. The commands below use MediaMTX `v1.20.0` on
macOS x86_64:

```bash
mkdir -p /tmp/sentinel-mediamtx-1.20.0
curl -L https://github.com/bluenviron/mediamtx/releases/download/v1.20.0/mediamtx_v1.20.0_darwin_amd64.tar.gz \
  | tar -xz -C /tmp/sentinel-mediamtx-1.20.0
/tmp/sentinel-mediamtx-1.20.0/mediamtx \
  tools/rtsp-test-source/mediamtx.yml
```

In a second terminal, publish the moving local source:

```bash
./tools/rtsp-test-source/start.sh
```

Then configure Sentinel with `SENTINEL_MEDIA_GATEWAY_ENABLED=true`, the
MediaMTX API at `http://127.0.0.1:9997`, WebRTC at
`http://127.0.0.1:8889`, and HLS at `http://127.0.0.1:8888`. Validate the
source, register it with `POST /api/v1/sources/front-gate/playback/register`,
and open `/admin`. Stop MediaMTX to verify normalized gateway failure, then
restart it and repeat validation/registration; the current adapter does not
run an automatic re-registration supervisor.
## Packaging boundary

SS-WP-014 does not download or silently install MediaMTX. The macOS bundle can
include an operator-supplied reviewed binary, or MediaMTX can remain an
external sibling service. Its version and license must be recorded by the
deployment owner. Sentinel continues to use the provider-neutral `MediaGateway`
and `MediaMtxAdapter` rather than exposing MediaMTX administration through its
public contracts.
