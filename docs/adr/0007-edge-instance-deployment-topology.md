# ADR 0007: Edge Instance Deployment Topology

- Status: Accepted
- Date: 2026-08-15

## Context

Sentinel Streaming is both a standalone product and an embeddable platform
service. Sentinel Home and other domain products need a stable integration
boundary, but adding household tenancy, cloud control-plane behavior, or domain
policy to Streaming would erase that boundary.

## Decision

The initial commercial topology is one Sentinel Streaming edge instance per
home or managed site. The instance has a stable `StreamingInstanceId`. A domain
product such as Sentinel Home owns household/site tenancy and maps its records
to immutable Streaming camera identities through optional external references.

Supported deployment profiles are:

- `standalone`: Streaming owns its service installation, local bootstrap,
  administration console, and configured local storage;
- `embedded-edge`: an external product or supervisor operates Streaming through
  its versioned API, while Streaming continues to own camera protocols, source
  lifecycle, media delivery, and its setup/operations console.

The console may be management-network-only or disabled by embedded deployment
policy. Its product ownership does not move to the embedding application.
Centralized multi-tenant Streaming is deferred and requires a separate ADR.

## Consequences

- Sentinel Home owns households, users, arming state, alarms, incidents, and
  notifications.
- Streaming does not contain Home domain types or use Home database keys as its
  canonical identities.
- Independent deployment, restart, upgrade, and rollback remain possible.
- Cross-product behavior must use versioned APIs, events, playback grants, and
  artifact references rather than a shared mutable database.
