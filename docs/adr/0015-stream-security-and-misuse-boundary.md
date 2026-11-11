# ADR 0015: Stream Security and Misuse Boundary

- Status: Proposed
- Date: 2026-09-19

## Decision

All source, stream, artifact, playback, diagnostics, and consequential control operations cross an explicit authenticated-actor and resource-authorization boundary. Audit records contain actor, instance, source/artifact, request, correlation, authorization outcome, and result without secrets.

The threat model covers unauthorized stream access, credential theft, replay, stream substitution, camera spoofing, metadata leakage, unauthorized tracking, event/memory poisoning, reasoning manipulation, tool abuse, and privilege escalation. Controls include short-lived playback grants, source-scoped authorization, redaction, replay-resistant context, provenance validation, rate/cost limits, and fail-closed action boundaries.

No platform API grants a model direct control over cameras, PTZ, evidence, policy, or external actions. Future physical actuators use the same authority boundary.

## Consequences

Standalone tokens remain an adapter, not the long-term inter-product identity model. Home may assert identity, but Streaming owns camera secrets and protocol details.

## Verification gate

Negative authorization, replay/substitution, redaction, resource-isolation, and audit inspection tests are required before security/privacy claims are elevated.
