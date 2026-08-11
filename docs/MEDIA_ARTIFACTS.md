# Media artifacts

SS-WP-012 adds bounded on-demand snapshots and short clips. Artifacts are
metadata plus content managed through the `MediaArtifactStore` boundary. The
first backend stores files on the local filesystem and exposes only a logical
`artifacts/{id}` reference, checksum, provenance, and retention metadata.

Snapshots use the latest decoded frame. Clips use FFmpeg with bounded duration,
safe argument passing, and RTSP/TCP input. `SENTINEL_MEDIA_ARTIFACT_ROOT`
configures the filesystem root. `SENTINEL_MEDIA_CLIP_DEFAULT_DURATION_SECONDS`
defaults to 10 seconds, `SENTINEL_MEDIA_CLIP_MAX_DURATION_SECONDS` defaults to
60 seconds, and `SENTINEL_MEDIA_ARTIFACT_MAX_CONCURRENT` defaults to 2.

The store is pluggable and persistence-backend agnostic. SQLite is not required
and media bytes are not stored in a database. Retention fields are hooks for a
future policy worker; automatic deletion policy is not implemented here.

Continuous NVR recording, incident evidence workflows, playback authorization
redesign, audio capture, and AI interpretation are out of scope.
