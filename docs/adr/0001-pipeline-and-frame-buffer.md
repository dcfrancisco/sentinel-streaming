# ADR 0001: Single Pipeline and Frame Buffer Boundary

- Status: Superseded by ADR 0008
- Date: 2026-07-16

This decision preserved the correct ordering, bounded-memory, and source-adapter
isolation principles for the original single-active-source runtime. ADR 0008
retains those principles while making the processing path and canonical frame
buffer source-scoped for concurrent multi-camera operation.

## Context

Future preview, recording, Vision, snapshots, and event generation all need
access to captured frames. Direct access from each feature would couple every
feature to every camera implementation and make buffering or source changes
unsafe.

## Decision

Every source emits frames through `FrameProvider` into one processing pipeline.
The bounded `FrameBuffer` is the canonical repository for recent frames. Higher-
level consumers read from the buffer and never access cameras directly.

## Consequences

- New consumers are pipeline stages or buffer consumers.
- Memory usage is bounded by buffer capacity.
- Consumers can be tested without a physical camera.
- Preview, Vision, and MJPEG share the same frame history.
- A slow consumer must not own or block the source implementation.
