# ADR 0001: Single Pipeline and Frame Buffer Boundary

- Status: Accepted
- Date: 2026-07-16

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
