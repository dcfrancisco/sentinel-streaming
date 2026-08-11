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
 physical-camera certification, or SS-WP-010 and later work.

## SS-WP-010 — Media Telemetry and Stream Supervision

Implemented scope:

- normalized `MediaTelemetry` and explicit media-delivery state model;
- structured MediaMTX path/health inspection without exposing MediaMTX response shapes;
- bounded periodic `MediaSupervisor` with one in-flight check per source;
- startup timeout, stale/no-activity detection, bitrate delta, reconnect accounting, and recovery observation;
- separate source health and media health in source APIs/admin;
- authenticated telemetry API and product-language admin status;
- normalized media operational events and configurable thresholds.

Explicitly not included: physical-camera certification, recording, audio or AI
intelligence, automatic gateway re-registration orchestration, per-viewer
MediaMTX authorization, PostgreSQL, pgvector, or SS-WP-011 and later work.

## SS-WP-011 — Physical Camera Interoperability and Certification Harness

Implemented scope:

- structured physical-device matrix schema and result states;
- repeatable certification procedure covering onboarding, ONVIF, RTSP,
  browser playback, PTZ, telemetry, failures, recovery, security, and soak;
- human-readable report template and sanitized evidence guidance;
- non-destructive normalized API capture harness;
- explicit `PHYSICAL_DEVICE` separation from emulator and local MediaMTX evidence.

## SS-WP-012 — Recording, Snapshots, and Evidence Capture Foundation

Implemented scope:

- bounded on-demand JPEG snapshots from the latest decoded frame;
- bounded FFmpeg RTSP short clips with configurable default/max duration;
- pluggable `MediaArtifactStore` and local filesystem backend;
- normalized artifact metadata, SHA-256 checksum, provenance, correlation ID,
  and retention hooks;
- separate capture/view/delete authorities and operational events;
- protected artifact metadata/content/delete APIs and minimal Admin capture UI;
- bounded concurrent captures and credential/path redaction.

Explicitly not included: continuous NVR recording, database blobs, automated
retention deletion, audio intelligence, AI interpretation, physical-camera
certification, or SS-WP-013 and later work packages.

No physical camera was available for this work package, so no certification
result is claimed. Physical PTZ and failure actions remain operator-confirmed.

Explicitly not included: new camera features, AI/audio intelligence, recording,
enterprise SSO, per-viewer MediaMTX authorization, PostgreSQL, pgvector, or
SS-WP-012 and later work.

## SS-WP-013 — Audio Transport and Media Capability Support

Implemented scope:

- normalized ONVIF audio encoder/profile metadata;
- MediaMTX audio-track detection and provider-neutral audio telemetry;
- explicit audio delivery states distinct from video delivery health;
- muted, user-controlled Admin browser audio playback when an audio track is
  available;
- optional audio-preserving bounded clips and audio artifact metadata;
- `VIEW_AUDIO` authority included in existing role boundaries;
- deterministic unit, protocol, emulator, and media-contract coverage;
- physical certification schema extended for RTSP, WebRTC, HLS, and clip audio.

Explicitly not included: speech recognition, sound-event AI, transcription,
talk-back/two-way audio, continuous NVR, physical-camera certification,
packaging, diagnostics bundles, soak testing, PostgreSQL, pgvector, or
SS-WP-014 and later.
