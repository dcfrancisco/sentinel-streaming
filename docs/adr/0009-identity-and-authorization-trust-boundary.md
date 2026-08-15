# ADR 0009: Identity and Authorization Trust Boundary

- Status: Proposed
- Date: 2026-08-15

## Context

Standalone environment-backed tokens and instance-wide roles are useful for
local administration. Embedded clients need externally asserted identity and
resource-scoped authorization without importing a domain product's identity
model into Streaming.

## Proposed decision

Keep three identity concepts distinct:

- `StreamingInstanceId`: stable identity of a deployed Streaming instance;
- `CameraId`: immutable Streaming-owned identity of a camera;
- `ExternalReference`: optional consumer-owned mapping that is never the
  canonical Streaming identity.

Authentication answers who is calling. Authorization separately evaluates an
authority against a resource such as a camera, artifact, playback grant, or
system operation. Standalone roles remain a convenient policy adapter; they do
not define the complete platform authorization model.

The current explicit modes are `OPEN_LOCAL_TEST` for loopback-only human
validation and `LOCAL_ADMIN_AUTH` for bearer-token administration.
`EXTERNAL_IDENTITY` remains reserved for this proposed integration boundary.

Provide a replaceable authenticator/claims seam for trusted embedded callers.
Preserve the authenticated actor, instance, resource, request correlation, and
authorization outcome in operational evidence.

## Consequences

- Sentinel Home can propagate service or user identity without sharing its user
  database with Streaming.
- Global bearer tokens are not the long-term inter-product trust mechanism.
- Camera credentials remain private to Streaming and are never delegated to a
  domain product.
- Exact claim format, key rotation, and transport trust require an implementation
  work package and threat review.
