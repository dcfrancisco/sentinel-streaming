# Commercial readiness

Sentinel Streaming is being developed as a commercial standalone and embeddable
video streaming platform. These gates track product evidence separately from
feature implementation. A passing local emulator or MediaMTX check is not
physical-camera certification.

| Gate | Requirement | Current status |
|---|---|---|
| CR-1 Installation | Fresh supported machine reaches a working installation with minimal infrastructure. | Partial; macOS bundle, idempotent user-local LaunchAgent installer, config validation, secure bootstrap, and readiness procedure exist; clean-machine timing evidence remains. |
| CR-2 Camera onboarding | Non-technical operator discovers and adds a supported camera without constructing RTSP URLs. | Partial; SS-WP-008 flow and the physical onboarding checklist exist, but no physical device report is recorded yet. |
| CR-3 Live playback | Reliable WebRTC live view with HLS/LL-HLS fallback where appropriate. | Partial; local MediaMTX verification, muted/user-controlled audio playback, and media telemetry exist; physical browser/device certification remains. |
| CR-4 Recovery | Camera, network, and media-gateway failures recover predictably without retry storms. | Partial; bounded source/media supervision and physical failure procedure exist, no physical-device recovery report is recorded yet. |
| CR-5 Security | Credentials protected and consequential controls require authority. | Partial; bearer roles, server-side PTZ authority, redaction, audit events, and TLS deployment guidance are implemented; direct MediaMTX per-viewer authorization and durable identity/secret rotation remain. |
| CR-6 Interoperability | Multiple real camera vendors/models/firmware versions are certified. | Framework ready; audio evidence fields are defined, but no physical-device results are recorded, so not started. |
| CR-7 Stability | 24-hour, 72-hour, and 7-day soak tests are available and tracked. | Framework and report fields ready; physical long-duration evidence pending. |
| CR-8 Observability | Operators can distinguish camera, RTSP, ONVIF, MediaMTX, browser, credential, configuration, network, and recovery failures. | Partial; normalized source/media/audio evidence and physical failure capture procedure exist; device reports and browser diagnostics remain. |
| CR-9 Upgradeability | Supported configuration survives upgrades and migrations. | Not started; persistence/migration strategy remains. |
| CR-10 API stability | External clients can rely on documented versioned service contracts. | Partial; `/api/v1` exists, compatibility policy and release discipline remain. |

## Embeddable platform gate

The word `embeddable` is not a production claim yet. The current runtime can
register multiple sources but has one active decoded processing path and global
frame/preview/Vision state. Sentinel Home and other products must not take a
production dependency until SS-WP-015A and SS-WP-015B provide contract-tested
identity, source-scoped processing, authorization, events, artifact references,
playback delegation, and compatibility behavior.

The initial commercial topology is one Streaming edge instance per home or
managed site. Domain products own tenancy and policy; Streaming owns camera and
media infrastructure. See [Platform integration boundary](platform-integration.md).

SS-WP-012 adds operator-requested snapshot and bounded-clip evidence capture.
It improves CR-3 and CR-8 by making media outputs inspectable, but does not
complete continuous recording, retention policy, backup/restore, or a durable
evidence-management product.

## Evidence classes

- `UNIT` — isolated domain or adapter behavior;
- `PROTOCOL` — deterministic protocol exchange;
- `IN_PROCESS_EMULATOR` — in-process camera/emulator fixtures;
- `STANDALONE_EMULATOR` — project-local executable/tooling;
- `LOCAL_MEDIAMTX_INTEGRATION` — real local MediaMTX plus a deterministic RTSP source;
- `PHYSICAL_DEVICE` — real camera/model/firmware verification.

Evidence must retain its class. Emulator and local MediaMTX results must not be
reported as physical-device compatibility.

## Product boundaries

Sentinel Streaming owns camera/device integration, RTSP/ONVIF/PTZ infrastructure,
media delivery, stream health and recovery, video intelligence, operational APIs,
and its own admin/setup/operations console. Sentinel Home, Campus, and Buildings
remain separate domain applications and are not merged into this product.

Persistence remains pluggable. SQLite is optional for standalone deployments
when durable local state is needed; PostgreSQL and pgvector require separate
justification and are not current dependencies.

## SS-WP-009 evidence

Authentication and role checks cover source reads, source management,
onboarding, diagnostics, PTZ, and playback API operations. Deterministic unit
tests cover role authority and PTZ denial/allowance; credential serialization
and debug redaction are covered by existing source/media/ONVIF tests. This is
not a complete commercial security certification: TLS termination, durable
user/session management, token rotation, CSRF/session-cookie hardening, and
per-viewer MediaMTX authorization remain open product work.
