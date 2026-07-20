# Sentinel Streaming Compatibility Matrix

This is a validation record, not a claim that every model in a brand family is
compatible. Add a row only after completing the human functional test guide.

| Camera/source | Discovery | Live stream | Snapshot | AI ready | Status |
| --- | --- | --- | --- | --- | --- |
| Built-in macOS camera | Manual/device index | Yes, native provider | Yes | Yes, frame endpoint | Implemented; validate per host |
| USB/UVC webcam | Manual/device index | Yes, through built-in provider | Yes | Yes, frame endpoint | Implemented; validate model/OS |
| RTSP IP camera | Configured/manual | Yes, FFmpeg RTSP/TCP | Yes | Yes, frame endpoint | Implemented; validate codec/auth |
| Wi-Fi camera with RTSP | Manual or product discovery | Yes, as RTSP | Yes | Yes, frame endpoint | Depends on model and firmware |
| Wi-Fi camera with ONVIF | WS-Discovery | Profile may require RTSP setup | After stream setup | After stream setup | Discovery implemented; profile setup required |
| HTTP/MJPEG camera | Manual URL | Yes, FFmpeg MJPEG source | Yes | Yes, frame endpoint | Implemented; validate endpoint/auth |
| Video file | Manual path | Yes, FFmpeg playback | Yes | Yes, frame endpoint | Implemented |
| Synthetic camera | Built-in candidate | Yes | Yes | Yes | Implemented; hardware-free control |
| MediaMTX simulator | Manual RTSP URL | Yes, RTSP/TCP | Yes | Yes | Recommended RTSP regression fixture |
| Cloud-only camera | Usually no local discovery | No without documented local adapter | No | No | Not compatible by default |

## Device certification fields

For a real model, add:

- manufacturer and exact model
- firmware version and region
- provider/source kind
- discovery method and result
- stream URI type, codec, resolution, FPS
- authentication behavior
- reconnect behavior
- test date and Sentinel version
- tester and evidence location
- known limitations
