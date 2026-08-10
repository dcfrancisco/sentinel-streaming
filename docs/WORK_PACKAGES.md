# Work Package Status

## SS-WP-003 — RTSP Validation and Stream Health

Implemented scope:

- bounded Rust/Tokio RTSP `OPTIONS` and `DESCRIBE` validation;
- normalized validation failure categories;
- validation and health fields on the existing source model;
- explicit source validation API;
- minimal Sentinel Streaming admin validation page;
- configurable validation timeout;
- deterministic tests and credential-safe output.

Explicitly not included:

- full media decoding changes;
- audio;
- AI;
- ONVIF;
- PTZ;
- MediaMTX;
- WebRTC/HLS;
- persistence;
- authentication or authorization changes;
- complex recovery orchestration.

The next work package must extend this health model rather than replace it.

## SS-WP-004 — Health Monitoring and Bounded Recovery

Implemented scope:

- periodic RTSP source health checks;
- bounded concurrent validation;
- one in-flight recovery loop per source;
- progressive, capped retry delay and maximum attempts;
- failure-category-aware retry policy with no automatic retry for
  authentication/configuration failures;
- recovery state/history on the source API and admin page;
- shutdown-aware monitor runtime;
- deterministic unit/protocol/integration-style tests using injected RTSP
  validation backends.

Explicitly not included: decoded media supervision, ONVIF, PTZ, MediaMTX,
WebRTC/HLS, AI, audio, persistence, or authentication/authorization changes.

## SS-WP-005 — ONVIF Discovery and Capability Normalization

Implemented scope:

- WS-Discovery over UDP with bounded discovery timeouts;
- ONVIF device information, service capability, media profile, and RTSP URI
  inspection;
- normalized video, audio, snapshot, events, and capability-driven PTZ data;
- normalized capability retention on the existing source model;
- handoff of a selected ONVIF RTSP URI to the existing RTSP validator;
- ONVIF discovery, inspection, and capability API routes;
- minimal admin discovery/capability display;
- deterministic emulator/protocol tests, explicitly not physical-camera tests.

Explicitly not included: PTZ movement, presets/control, MediaMTX, WebRTC/HLS,
decoded video, AI, audio intelligence, persistence, or domain applications.

## SS-WP-006 — Capability-Driven PTZ Control

Implemented scope:

- continuous, relative, and absolute ONVIF movement;
- stop, zoom, preset listing, and go-to-preset operations;
- normalized capability and axis/operation gating before network I/O;
- explicit PTZ authority boundary requiring the authenticated operator path;
- normalized operational PTZ events with actor, source, operation, request,
  outcome, and correlation ID;
- deterministic emulator SOAP-operation verification and admin controls.

Explicitly not included: MediaMTX, WebRTC/HLS, decoded video, AI/audio
intelligence, persistence, PostgreSQL, pgvector, or physical-camera validation.

## SS-WP-007 — MediaGateway, MediaMTX, and Browser Playback

Implemented scope:

- `MediaGateway` boundary with `MediaMtxAdapter` implementation;
- registration/removal of validated RTSP sources;
- normalized WebRTC-preferred and HLS-fallback playback contracts;
- separate media-delivery health and normalized gateway failures;
- configurable external MediaMTX API and playback bases;
- graceful gateway shutdown/reconciliation;
- admin browser live-view integration and deterministic gateway tests.

Explicitly not included: onboarding wizard, decoded Sentinel video processing,
AI/audio intelligence, recording, persistence, PostgreSQL, pgvector, or
physical-camera certification.

## SS-WP-007A — Local Media Playback Verification

Verification-only slice following SS-WP-007:

- project-local FFmpeg moving RTSP test source;
- Docker-free local MediaMTX runbook;
- real MediaMtxAdapter registration and playback verification;
- WebRTC/HLS endpoint and `/admin` visual acceptance guidance;
- MediaMTX stop/restart and separate media/source-health checks;
- explicit `LOCAL_MEDIAMTX_INTEGRATION` classification.

Explicitly not included: the full virtual camera lab, ONVIF emulator
executable, virtual PTZ scene generation, onboarding, AI, audio intelligence,
persistence, or physical-camera certification.

SS-WP-007A evidence is classified as `LOCAL_MEDIAMTX_INTEGRATION`. It used
MediaMTX v1.20.0, a deterministic FFmpeg moving RTSP source, real adapter
registration, WHEP/HLS endpoint checks, `/admin` browser playback, gateway
failure, restart, and explicit re-registration. It is not physical-device
certification.

The commercial roadmap and CR-1 through CR-10 gates are maintained in
`docs/roadmap.md` and `docs/COMMERCIAL_READINESS.md`.

## SS-WP-008 — Zero-Friction Camera Onboarding

Implemented scope:

- in-memory discovery session API;
- discovery, selection, credential, ONVIF inspection, and capability steps;
- automatic selection of the best available RTSP media profile;
- reuse of the existing RTSP validator and MediaGateway registration;
- readiness checks for ONVIF, profile, RTSP, media delivery, browser preview,
  PTZ, and health monitoring;
- friendly name/location capture;
- product-language failure messages with optional technical details;
- minimal Admin Add Camera flow.

Explicitly not included: persistence, automatic browser verification, full
onboarding wizard polish, physical-camera certification, or SS-WP-010 and later
work packages.

## SS-WP-009 — Authentication, Roles, Secrets, and Playback Security

Implemented scope:

- bearer-token principals with Viewer, Operator, and Administrator roles;
- server-side authority checks for source, onboarding, diagnostics, PTZ, and playback operations;
- preserved `PtzAuthority` boundary requiring `CONTROL_PTZ` before ONVIF I/O;
- explicit first-run bootstrap token contract with no shipped default credential;
- credential-safe config/source/debug/event representations;
- authenticated admin login shell and product-language auth failures;
- authentication and authorization security events through the existing event store;
- documented TLS reverse-proxy deployment boundary and playback limitation.

Explicitly not included: enterprise SSO, durable local users/sessions, token
rotation, CSRF-protected cookie sessions, per-viewer MediaMTX authorization,
media telemetry, physical-camera certification, or SS-WP-010 and later work.
