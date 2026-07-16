# ADR 0004: Optional Vision Observation

- Status: Accepted
- Date: 2026-07-16

## Context

Scene understanding is useful, but an unavailable provider, missing key, network
failure, or provider error must not prevent the streaming runtime from starting
or serving video.

## Decision

Vision runs as an optional scheduled consumer of the `FrameBuffer`. Providers
are abstracted behind a common interface. The OpenAI provider is enabled only
when `OPENAI_API_KEY` is available; failures are logged, metered, and isolated
from camera and pipeline execution.

Vision produces factual scene observations. It does not implement alerts,
security policy, notifications, face recognition, or product decisions.

## Consequences

- Video remains available without AI credentials or connectivity.
- Provider implementations can be replaced without changing capture flow.
- Frames may leave the device when a remote provider is enabled, so deployment
  documentation must call out privacy and cost implications.
- Provider retry, timeout, circuit-breaker, and cost controls are future work.
