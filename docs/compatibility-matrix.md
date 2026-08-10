# Sentinel Streaming Compatibility Matrix

This is a validation record, not a claim that every model in a brand family is
compatible. Add a row only after completing the human functional test guide.

| Camera/source | Discovery | Live stream | Snapshot | Audio | PTZ / presets | Browser playback | AI ready | Status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Built-in macOS camera | Manual/device index | Yes, native provider | Yes | Not evaluated | Not applicable | MJPEG diagnostics | Yes, frame endpoint | Implemented; validate per host |
| USB/UVC webcam | Manual/device index | Yes, through built-in provider | Yes | Not evaluated | Not applicable | MJPEG diagnostics | Yes, frame endpoint | Implemented; validate model/OS |
| RTSP IP camera | Configured/manual | Yes, FFmpeg RTSP/TCP | Yes | Not evaluated | Not evaluated | MJPEG diagnostics | Yes, frame endpoint | Implemented; validate codec/auth |
| Wi-Fi camera with RTSP | Manual or product discovery | Yes, as RTSP | Yes | Not evaluated | Not evaluated | MediaGateway integration; certify per model | Depends on model and firmware | Physical validation required |
| Wi-Fi camera with ONVIF | WS-Discovery | Profile/RTSP inspection available | Capability-dependent | Capability-dependent | Capability-dependent | MediaGateway integration; certify per model | After stream setup | Physical validation required |
| HTTP/MJPEG camera | Manual URL | Yes, FFmpeg MJPEG source | Yes | Not evaluated | Not applicable | MJPEG diagnostics | Yes, frame endpoint | Implemented; validate endpoint/auth |
| Video file | Manual path | Yes, FFmpeg playback | Yes | Not applicable | Not applicable | MJPEG diagnostics | Yes, frame endpoint | Implemented |
| Synthetic camera | Built-in candidate | Yes | Yes | Not applicable | Not applicable | MJPEG diagnostics | Yes | Implemented; hardware-free control |
| Local MediaMTX test source | Manual RTSP URL | Yes, RTSP/TCP | Yes | Not evaluated | Not applicable | WebRTC endpoint + HLS fallback verified | Yes | `LOCAL_MEDIAMTX_INTEGRATION`; not physical certification |
| Cloud-only camera | Usually no local discovery | No without documented local adapter | No | Unknown | Unknown | No | No | Not compatible by default |

## Device certification fields

For a real model, add:

- manufacturer and exact model
- firmware version and region
- provider/source kind
- discovery method and result
- ONVIF profile and service capability results
- stream URI type, codec, resolution, FPS
- audio support and verification result
- PTZ operations and preset support, if applicable
- WebRTC and HLS/LL-HLS playback results through the media gateway
- authentication behavior
- reconnect behavior
- long-duration stability and recovery result
- test date and Sentinel version
- tester and evidence location
- known limitations

Emulator and physical-camera results must be recorded separately. A passing
MediaMTX or ONVIF emulator test is not physical-camera certification.
