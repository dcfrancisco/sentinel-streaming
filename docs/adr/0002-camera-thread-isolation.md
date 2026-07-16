# ADR 0002: Isolate Non-Send Camera Backends

- Status: Accepted
- Date: 2026-07-16

## Context

The Nokhwa native macOS backend contains a capture backend that is not `Send`.
The async runtime and Axum application state must remain movable across Tokio
tasks and threads.

## Decision

Construct and use the built-in camera on a dedicated named OS thread. Transfer
decoded frames to the async runtime through a bounded Tokio channel. The source
manager owns the worker and its stop signal.

## Consequences

- Camera backend thread-safety does not leak into the rest of the service.
- Capture failure can close the worker channel and trigger manager recovery.
- The bounded channel applies backpressure and prevents unbounded frame queues.
- Future adapters may use a different execution model while preserving the
  `FrameProvider` boundary.
