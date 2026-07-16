# ADR 0003: API-Backed CLI Administration

- Status: Accepted
- Date: 2026-07-16

## Context

The service needs CLI, automation, and future product clients without creating
separate business logic or state transitions for each interface.

## Decision

The CLI calls the public REST administration API for runtime operations. The
service owns source lifecycle, status, metrics, and shutdown behavior. CLI
authentication and profiles provide credentials for those requests.

## Consequences

- CLI and REST behavior share one source of truth.
- API compatibility becomes an operational contract.
- The CLI can operate against local or remote profiles.
- Commands require clear endpoint errors and should not silently mutate local
  service state.
