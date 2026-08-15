# ADR 0012: Secure Playback Delegation

- Status: Proposed
- Date: 2026-08-15

## Context

Sentinel Home needs to request playback without learning camera credentials,
ONVIF profile tokens, RTSP URLs, MediaMTX paths, or MediaMTX administration
details. The current normalized playback response does not yet provide complete
per-viewer gateway authorization.

## Proposed decision

Streaming issues short-lived, source-scoped playback grants after authenticating
and authorizing a caller. The public contract identifies the grant, camera,
expiry, and normalized preferred/fallback playback methods. MediaGateway-specific
realization remains private to the adapter.

Conceptually:

```text
POST /api/v1/sources/{cameraId}/playback-grants
```

The response must not expose camera credentials, gateway administrative
credentials, internal path names, or ONVIF details. Grant revocation, expiry,
viewer limits, audit evidence, and gateway capability degradation are explicit
contract behaviors.

## Consequences

- Domain products depend on a Sentinel-owned playback contract rather than
  MediaMTX.
- Replacing `MediaGateway` does not change Home integration semantics.
- Trusted-listener deployment remains an interim limitation, not the final
  embedded playback security model.
- Implementation requires gateway enforcement support and security testing.
