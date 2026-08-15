# ADR 0008: Per-Source Runtime Isolation

- Status: Accepted
- Date: 2026-08-15
- Supersedes: ADR 0001

## Context

The current API can register multiple sources, but decoded processing uses one
active `FrameProvider`, one frame buffer, one preview, and one Vision state.
Starting another decoded source switches the active worker. That implementation
cannot fulfill a concurrent multi-camera platform contract.

ADR 0001 correctly required ordered frame processing, bounded memory, and no
direct camera access by consumers, but scoped the canonical path globally.

## Decision

Each active source has exactly one ordered processing path and one bounded
canonical frame buffer for that source. Mutable frame-processing state is never
shared implicitly between sources.

A source-scoped runtime owns or addresses:

- frame provider and worker lifecycle;
- processing pipeline and bounded frame buffer;
- preview and diagnostic MJPEG output;
- optional Vision scheduling and latest observation;
- snapshot/clip capture context;
- health, recovery, and resource accounting.

Shared services such as the API, event publisher, configuration repository, and
`MediaGateway` may multiplex sources only through explicit `CameraId`-scoped
contracts. A route containing a camera/source identifier must never return
global frame state belonging to another source.

## Consequences

- Concurrent camera count and resource budgets become explicit product limits.
- Failure or backpressure in one source must not stop unrelated sources.
- Existing pipeline stages remain reusable but execute in a source-scoped
  runtime context.
- The current single-active-source implementation remains a declared limitation
  until SS-WP-015B implements this decision.
- Multi-camera soak claims require the source-scoped runtime and cannot be based
  on repeatedly switching one active pipeline.
