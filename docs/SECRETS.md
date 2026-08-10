# Secrets

Camera credentials use the existing credential boundary. Preferred
configuration stores environment references (`username_env` and
`password_env`); transient values are used only for protocol calls.

Passwords and tokens are excluded from serialized source/config responses,
custom `Debug` implementations, browser playback URLs, and operational event
metadata. RTSP and ONVIF endpoints returned to clients are redacted where
necessary.

Standalone deployments may use environment variables or an OS-managed secret
store. Sentinel Streaming does not require Vault, KMS, SQLite, PostgreSQL, or
another external persistence system for this boundary. Larger deployments can
replace the provider without changing RTSP/ONVIF code.
