# ADR 0010: Integration Event Contract

- Status: Proposed
- Date: 2026-08-15

## Context

The current bounded in-memory event store and SSE feed are operationally useful
but do not yet form a durable inter-product contract. Sentinel Home must consume
facts without depending on internal strings, transport-specific behavior, or
Streaming policy decisions.

## Proposed decision

Separate event ownership into three families:

- operational events owned by Streaming, such as source disconnection, media
  stall, recovery, or configuration failure;
- observations owned by Streaming, such as a factual person, vehicle, or sound
  observation when an observation provider is enabled;
- domain events owned by Sentinel Home or another consumer, such as possible
  intrusion, alarm activation, or resident notification.

The transport-neutral integration envelope includes at least:

```text
eventId, eventType, schemaVersion, instanceId, sourceId, sequence,
occurredAt, observedAt, correlationId, causationId, payload
```

Event type and payload schemas are versioned independently from delivery
transport. SSE, WebSocket, webhook, broker, or a durable log may implement the
contract, but their delivery and replay guarantees must be explicit.

## Consequences

- Streaming publishes facts and never raises Home security incidents.
- Consumers can deduplicate, order, correlate, and evolve event handling.
- Current SSE remains a best-effort operational feed until a work package adds
  the required sequencing, schema, and delivery semantics.
