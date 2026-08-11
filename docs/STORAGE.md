# Artifact storage

`MediaArtifactStore` is the storage boundary for snapshot and clip content.
The default implementation is a local filesystem store selected by
`SENTINEL_MEDIA_ARTIFACT_ROOT` (default `sentinel-artifacts`). Writes use a
temporary file and atomic rename. Metadata contains a SHA-256 checksum,
provenance, actor, correlation ID, capture mechanism, and retention hooks.

Filesystem paths, RTSP credentials, and FFmpeg diagnostics are not public
artifact references. A future durable backend may replace this implementation
without changing the API contract. Sentinel Streaming supports pluggable
persistence; SQLite is optional for standalone deployments and is not required
by the core runtime.
