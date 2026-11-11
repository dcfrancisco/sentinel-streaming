# Streaming reliability and Admin Console inventory

This inventory records the starting point of the current work, not a claim of
production readiness or successful physical-camera/OpenAI verification.

| Area | Assessment | Evidence and gap |
|---|---|---|
| Admin shell and onboarding | Partial | `src/admin.rs` serves one embedded console with discovery, ONVIF inspection, RTSP playback and capability-gated PTZ. Dashboard, navigation, manual source management and operational views are incomplete. |
| Camera lifecycle | Partial | `src/sources.rs` owns registration, start, stop, removal, health and reconnect; configured and registered state is not durable API-managed configuration. The decoded pipeline has one active source. Physical device behavior is not verified here. |
| Stream management | Partial | MJPEG, frame, playback and media health APIs exist; the console does not present a complete stream inventory or all operational metrics. Some metrics such as latency are not measured. |
| Vision activation | Broken at baseline | The placeholder pipeline stage uses `pipeline.vision`, while `main.rs` independently starts the real scheduler using `vision.enabled`, which defaults to true. |
| OpenAI Responses API | Broken at baseline | The raw HTTP response parser expects top-level `output_text` instead of traversing `output[].content[]`; model selection and bounded retries are missing. |
| Vision administration | Missing | No authenticated configuration mutation, runtime enable/disable, provider status or observation history UI exists. |
| Events and health | Partial | The bounded event store, source health, recovery status and Prometheus metrics exist; the console lacks filtering and an operational overview. |
| Configuration and diagnostics | Partial | Configuration loads and validates at startup, and support snapshots attempt redaction. `/api/v1/config` is read-only; there is no safe persisted mutation workflow or restart indicator. |
| Authentication | Partial | Bearer tokens have viewer/operator/administrator roles, and the development bypass is limited to loopback by config validation. The browser stores bearer tokens in session storage; no server session or cookie CSRF protection exists. API authorization must be verified for every mutation. |
| CORS and browser protection | Partial | The router uses permissive CORS and the frame handler explicitly allows all origins. Production browser restrictions and headers need review without silently breaking existing integrations. |
| Tests | Partial | Tests cover mock scheduler output, source lifecycle and other integration paths, but baseline lacks OpenAI HTTP mocks and full admin/security acceptance coverage. |
| Documentation | Partial | `docs/admin-console.md` describes an intended broader console rather than distinguishing delivered features from targets. |

The raw startup configuration was logged using `Debug`; configured source URIs
have a debug redactor, but full configuration logging unnecessarily exposes
operational details. Credential and repository-history review is not complete.
Do not treat this inventory as a security audit or as evidence that no secrets
were ever committed. Rotate credentials if exposure is confirmed; do not copy
secrets into issues, logs, tests, or documentation.

## Implementation review

- The vision scheduler now uses both startup switches and a serialized,
  runtime-only administrator override. Disabling waits for worker cancellation;
  startup settings return after restart. Camera processing remains separate.
- The provider uses raw Responses API output items, validates structured
  observations, handles refusal and incomplete responses, and reports
  sanitized errors. It limits attempts, enforces request timeouts, uses
  exponential jittered backoff and honors `Retry-After`, including across
  runtime configuration changes. HTTP 400, 401, 403, 404 and 422 stop the
  worker until an administrator changes settings or the process restarts.
- Vision status reports analysis and HTTP counters, rate limits, latency and
  aggregate input/output tokens when both usage fields are returned. The status
  does not include credentials. Request counters count analyses separately
  from HTTP attempts.
- The existing console adds dashboard information, source registration and
  lifecycle controls, periodically refreshed frame preview, vision controls,
  filtered recent events, health and support bundle download.
- A synthetic runtime smoke test exposed a shutdown panic caused by polling
  the completed server task twice. Shutdown now joins each task once and a
  child-process regression test verifies a clean stop through the API.
- Source start, stop, restart and removal are serialized. Starting an already
  running source preserves its status and does not create another worker;
  stopping an inactive source does not tear down the active source of the same
  type. Reconnect attempts recheck the requested state under the same lifecycle
  lock so a stopped or switched source cannot restart after its backoff.
  Concurrent start, source-switch and reconnect-stop regressions use synthetic
  frames.
- Source deletion and source-test authorization are enforced server-side.
  Cross-origin browser access is no longer enabled by default; unsafe requests
  with an Origin must be same-origin. Local-test host validation limits DNS
  rebinding. Authenticated administration is now the default; the loopback-only
  local-test authentication bypass requires explicit selection.
  Startup no longer logs the entire configuration, RTSP query strings are
  redacted in configuration/support URLs, and onboarding responses no longer
  return raw provider errors. Browser tokens are not attached to
  cross-origin playback requests. The console sends anti-framing, no-sniff,
  and no-referrer headers; it does not yet have a restrictive content security
  policy or a server-managed browser session.
- A narrow, value-free scan of tracked files and reachable history for
  `sk-`-shaped OpenAI keys found no matches. This is not a comprehensive
  credential scan or evidence that no secrets were exposed elsewhere.

## Remaining gaps

- Camera editing and durable camera configuration are not implemented. Manual
  RTSP registration does not provide a credential-reference editor. Discovery
  and PTZ continue to rely on existing provider behavior.
- Vision configuration is not persisted; timeout and retry controls are fixed,
  not configurable in the console. There is no separate AI connectivity probe,
  revision/conflict mechanism, durable observation store, or per-camera vision
  state. Vision events have no source ID because buffered frames lack source
  provenance; a recent-frame selection may span a source switch. The decoded
  runtime remains single-active-source.
- Some stream/viewer/latency metrics, event severity, complete log filtering,
  system configuration mutation, configuration validation UI and restart
  orchestration are unavailable. The dashboard identifies unmeasured values.
- Existing diagnostics may contain sensitive operational information beyond
  known URL/secret fields; review support bundles before sharing. A dedicated
  security review, real-browser testing and physical camera testing are still
  needed. No live OpenAI request or physical-camera test was performed.

## Verification

- `cargo fmt --check`: passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo test`: passed (75 tests), including mocked OpenAI responses, API
  authorization, vision cancellation and synthetic process shutdown.
- `cargo build --release`: passed.
- Embedded JavaScript syntax check: passed; no browser test was performed.
- A local synthetic release smoke test returned ready, served the admin page and
  synthetic JPEG frames, rejected vision enable without a key, and kept
  serving frames after the rejection. Its initial shutdown exposed the panic
  described above; the fix passed the child-process regression test.
- A final authenticated synthetic release smoke test checked admin-page
  delivery, unauthorized and authorized requests, JPEG frames, cross-origin
  mutation rejection, missing-key rejection, disabled vision, continued
  frame delivery and clean shutdown. Browser automation was unavailable.

No physical camera, browser interaction or live OpenAI verification was
performed. These results do not establish production readiness.
