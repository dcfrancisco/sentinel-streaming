# Sentinel Streaming Troubleshooting Guide

## Camera not found

For built-in/USB sources, verify OS enumeration and camera permissions. Close
other camera applications and try the correct `SENTINEL_CAMERA_INDEX`. For
network cameras, verify the host is on the same LAN, resolve the hostname, and
test the expected port with `nc -vz`. For ONVIF, discovery requires multicast
traffic on the local broadcast domain; routed or isolated Wi-Fi often blocks it.

## Authentication failure

Confirm the local-camera username/password and that the account is permitted to
use RTSP/ONVIF. Confirm `username_env` and `password_env` point to variables
present in the Sentinel process environment. Do not place clear-text passwords
in YAML, shell history, logs, or screenshots. A test connection should fail
without taking down the service.

## No video or black frames

Test the exact stream independently with FFplay or VLC. Try the camera’s
substream, confirm the URI path, and check whether the codec is supported by
the installed FFmpeg. A successful TCP connection is not proof of a decodable
video stream. Inspect source decode-failure metrics and logs.

## Codec incompatibility

Start with H.264, YUV-compatible streams at a modest resolution. H.265,
proprietary codecs, unusual pixel formats, and vendor-specific encryption may
not be supported by the current adapter. Record the codec and FFmpeg error,
then retry with a supported camera substream.

## Firewall or network isolation

Allow the Sentinel host to reach the camera’s RTSP/MJPEG port. For ONVIF,
permit WS-Discovery multicast UDP 3702 and ensure the camera and host share a
broadcast domain. Corporate guest Wi-Fi commonly blocks peer-to-peer traffic;
move both devices to an approved IoT/LAN segment.

## Weak Wi-Fi or high latency

Measure signal quality at the camera, reduce stream resolution/FPS, prefer a
wired test, and compare camera latency with Sentinel frame latency. Do not
classify a camera as incompatible until it passes on a stable network.

## Reconnect failures

Confirm the camera returns to the same IP/hostname and that credentials remain
valid. Check recovery events, reconnect counters, backoff logs, and the last
frame timestamp. Restart only the source first; a full service restart is a
last-resort diagnostic. If the source never recovers, capture the failure
sequence and network state for certification.

## MJPEG endpoint unavailable

Confirm the source is running and has produced at least one frame. Verify the
source ID in `/api/v1/sources`. Use the bounded frame endpoint before testing a
continuous stream. If other viewers work, inspect client/network buffering;
the MJPEG stream must not block the capture pipeline.

## AI appears unavailable

AI is optional. Confirm a JPEG is available from the frame endpoint first, then
check `OPENAI_API_KEY`, Vision configuration, provider errors, and latency
metrics. A missing or unavailable OpenAI provider must not stop streaming.
