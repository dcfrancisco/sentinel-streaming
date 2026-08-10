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
