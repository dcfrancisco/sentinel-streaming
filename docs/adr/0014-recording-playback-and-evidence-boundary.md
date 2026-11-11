# ADR 0014: Bounded Recording, Playback, and Evidence Boundary

- Status: Proposed
- Date: 2026-09-19

## Decision

Streaming owns source media capture, bounded rolling/pre-event data used for approved clips, artifact bytes/metadata, storage adapters, retention execution, and secure playback grants. Home owns the domain link between an observation/incident/investigation and an artifact reference.

The first Home product target is bounded event-linked evidence, not a full commercial NVR. Continuous recording requires a separate capacity, retention, and privacy decision. Artifact metadata preserves source identity, capture and observed intervals, checksum, mechanism, storage reference, retention class, deletion/protection state, and correlation/causation links.

Playback uses short-lived source-scoped grants and never exposes camera credentials, RTSP URLs, or MediaMTX administration details to Home.

## Consequences

Home can build evidence-aware incidents without owning media protocols. Streaming can replace storage/gateway adapters without changing Home semantics. Current bounded artifacts are not proof of a complete Home playback product until integration and restart/retention verification exist.

## Verification gate

Event → bounded artifact → durable metadata → secure playback → expiry/delete must be exercised with unauthorized access, restart, checksum, retention, and source-isolation tests.
