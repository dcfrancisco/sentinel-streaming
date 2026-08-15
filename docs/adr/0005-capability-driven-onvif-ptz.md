# ADR 0005: Capability-Driven ONVIF and PTZ Control

- Status: Accepted
- Date: 2026-08-11

## Context

Sentinel already performs ONVIF WS-Discovery and returns device identity and
stream-profile hints. The current implementation does not query ONVIF device,
media, or PTZ services, retain normalized capabilities, or issue PTZ commands.
PTZ is consequential device control and cannot be safely inferred from the
presence of ONVIF alone.

## Decision

Extend the existing camera/source model with normalized, capability-driven ONVIF
information. The adapter must retain only capabilities confirmed by the device
and selected media/profile services, including individual PTZ operations where
reported. Do not create an independent PTZ subsystem.

The API and administration console expose PTZ controls only when the relevant
capability is present. Unsupported operations are rejected before device
invocation. PTZ mutations require operation-level authorization beyond bearer
authentication and emit operational evidence through the existing events and
structured logging mechanisms. Evidence includes initiator, camera ID,
operation, requested movement or preset, timestamp, outcome, and correlation ID
when available.

Testing is split into protocol/unit tests, emulator integration tests, and
physical-camera acceptance. Emulator results must not be represented as physical
camera validation.

## Consequences

- Cameras without PTZ remain fully supported as video sources.
- Admin and domain clients receive stable provider-neutral capability data.
- Vendor-specific ONVIF details remain behind the source/device adapter.
- Authorization and audit evidence become prerequisites for PTZ mutations.
- Physical camera certification remains a separate operational activity.
