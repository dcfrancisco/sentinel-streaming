# Admin Console

The current SS-WP-003 admin surface is intentionally minimal and is served by
Sentinel Streaming at `/admin`. It lists registered sources and displays their
safe name, type, lifecycle, validation, and health state.

For RTSP sources it provides **Validate RTSP**, media-gateway registration, a
browser live view, and media-delivery health. After ONVIF inspection it also
provides capability-gated PTZ test controls and preset actions. Unsupported PTZ
axes and operations are disabled or hidden.

This is not the later full setup/operations console. It does not implement
zero-friction onboarding, audio intelligence, recording, or domain application
workflows.

The console serves a credential-free login shell and sends the selected bearer
token to protected APIs. It does not receive or display camera passwords.

Source cards also show source health separately from media delivery state,
protocol, codec, resolution, observed FPS, bitrate, last media activity, and
product-language degradation details such as “Video stream appears stalled.”
# Capture operations

The source card provides operator capture actions when an RTSP source has a
decoded frame or valid source configuration. `Capture snapshot` saves a JPEG;
`Capture 10s clip` requests a bounded clip and provides a protected download
link. Technical storage paths and camera credentials are not shown.
## Audio display

Source operations views show advertised/observed audio, codec, sample rate,
channels, and audio delivery state when available. The live player requests an
audio track only when reported, starts muted, and provides a user-controlled
mute/unmute action. Browser autoplay restrictions are not reported as camera
audio failures.
