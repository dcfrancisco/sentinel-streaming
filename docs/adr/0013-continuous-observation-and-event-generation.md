# ADR 0013: Continuous Observation and Factual Event Generation

- Status: Proposed
- Date: 2026-09-19

## Context

Streaming has source-scoped frame principles, bounded buffering, optional scheduled vision, and an in-memory event bus. The current scheduler is not a durable continuous-observation service: it has no per-source checkpoint, durable delivery cursor, bounded inference queue with explicit overload semantics, or restart evidence. Home analysis is operator-triggered.

## Decision

Continuous observation is a source-scoped Streaming runtime concern. Each active source schedules frame selection under cadence and resource budgets, submits bounded inference work, and publishes factual observation events through the versioned integration envelope. Streaming does not create Home incidents, send notifications, or authorize actions.

The runtime defines cadence, frame selection, queue/backpressure, timeout/retry/cost budgets, source recovery, idempotency, provider/model provenance, captured/observed timestamps, evidence references, no-provider behavior, and replay/delivery semantics. A latest-analysis endpoint is a convenience view, not the system of record.

## Consequences

Streaming owns observation scheduling and factual event publication. Home consumes facts through a versioned contract and interprets them. A bounded local queue is acceptable initially, but reliable continuous operation requires durable checkpoint/delivery evidence.

## Verification gate

Real or deterministic stream input must produce observations, survive provider/source failure and recovery, preserve source identity, and show evidence after restart. A manual Analyze click or unit test alone does not satisfy this ADR.
