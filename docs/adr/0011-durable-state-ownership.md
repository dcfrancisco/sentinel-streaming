# ADR 0011: Durable State Ownership

- Status: Proposed
- Date: 2026-08-15

## Context

Standalone operation needs safe local durability, while embedded products need
independent lifecycle and storage choices. Sharing database tables would couple
deployments and make ownership, migration, and rollback unsafe.

## Proposed decision

Sentinel Streaming owns persistence contracts for:

- its stable instance identity;
- immutable camera identity and camera/source configuration;
- credential references and protocol configuration;
- operational state needed for restart and reconciliation;
- Streaming-owned event/observation delivery state;
- media artifact metadata and configured artifact storage.

Domain products own their tenants, users, rooms, policies, incidents,
notifications, and mappings to Streaming identifiers. Products exchange stable
identifiers and references; they do not share mutable database tables.

Persistence remains adapter-based. A standalone local backend may use files or
SQLite when explicitly implemented. Embedded deployments may select another
backend without changing public contracts. PostgreSQL is not made mandatory by
this decision.

## Consequences

- Configuration migration and rollback belong to the Streaming release
  lifecycle.
- External references are mappings, not ownership transfers.
- Retention, quota, and deletion rules need explicit treatment for events and
  media artifacts.
- The present in-memory/runtime configuration behavior remains a limitation
  until a dedicated persistence work package is completed.
