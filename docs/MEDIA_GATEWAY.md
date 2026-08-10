# MediaGateway

SS-WP-007 introduces the `MediaGateway` boundary. Sentinel owns source identity,
validation, lifecycle, health, authorization, and normalized playback contracts;
the gateway owns media transport and browser delivery.

`MediaMtxAdapter` is the first implementation. It registers validated RTSP
sources, removes them, returns normalized WebRTC/HLS playback information,
reports gateway health, and reconciles registrations during shutdown.

Media-gateway health is separate from camera health. A healthy RTSP source may
have an unavailable media gateway without being marked physically unhealthy.
When the gateway is unavailable, playback returns a stable media failure and
the service remains available for RTSP, ONVIF, PTZ, and health operations.

MediaMTX administrative URLs and path configuration never appear in normalized
playback responses.
