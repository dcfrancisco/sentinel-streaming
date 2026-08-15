# ADR 0006: Sentinel Streaming-Owned Admin and Setup Console

- Status: Accepted
- Date: 2026-08-11

## Context

Sentinel Streaming is shared infrastructure, but a headless service alone would
make camera installation unnecessarily technical. Sentinel Home, Sentinel
Campus, and Sentinel Buildings are separate domain products and must not absorb
streaming setup or operations responsibilities.

## Decision

Sentinel Streaming will have its own web interface with two modes: SETUP and
OPERATIONS. The default Add Camera flow hides RTSP, ONVIF profile, codec,
MediaMTX, and browser-transport details behind discovery, capability
negotiation, automated verification, preview, friendly naming, and save.
Manual RTSP configuration remains an Advanced path.

The console consumes Sentinel APIs and normalized capability/playback models. It
does not expose domain-specific homeowner, campus, or building workflows, and
domain applications do not become dependencies of this repository.

In an embedded deployment the console remains owned by Sentinel Streaming. It
may be restricted to a management network, disabled by deployment policy, or
launched from a domain product, but its setup and operational behavior remains
implemented through the same public Sentinel Streaming API. Domain products do
not duplicate ONVIF, RTSP, profile-selection, or MediaGateway logic.

## Consequences

- Setup UX becomes a product requirement, not merely an API client.
- The API needs normalized onboarding verification and playback contracts.
- Technical diagnostics remain available without burdening ordinary operators.
- The console can test PTZ only when capability discovery authorizes it.
- Home, Campus, and Buildings remain independent consumers of the platform.
