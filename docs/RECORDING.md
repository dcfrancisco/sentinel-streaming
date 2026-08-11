# Recording scope

Sentinel Streaming currently supports bounded snapshots and short clips.
Continuous NVR recording is not implemented.

The current capture API is intended for diagnostics and operator-requested
evidence capture. Clips are bounded to a maximum of 60 seconds by default and
are encoded through an FFmpeg adapter. Storage and metadata are separated so
local filesystem, NAS, or object storage backends can be added later.

When an RTSP source exposes a compatible audio track, bounded clips optionally
preserve it using FFmpeg stream copy. Artifact metadata records audio presence
and observed audio metadata. Video-only clips remain supported.
