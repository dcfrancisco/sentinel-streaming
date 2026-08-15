# Sentinel Streaming — Weekend RC Candidate

This is the short-term product gate for the tested weekend build. It takes
priority over roadmap, platform-integration, emulator, and multi-camera work.
It is an **untagged validation build**, not a release tag.

## Release gate

- [ ] Fresh macOS install completes successfully.
- [ ] LaunchAgent starts the tested build.
- [ ] `/admin` opens without a token in `OPEN_LOCAL_TEST` mode.
- [ ] Camera-free startup is valid and `/health/ready` is usable.
- [ ] One physical camera can be discovered or entered manually.
- [ ] ONVIF inspection succeeds when supported.
- [ ] RTSP validation succeeds.
- [ ] Live video works.
- [ ] Audio works when the camera and browser path support it.
- [ ] PTZ works when advertised by the camera.
- [ ] Snapshot and short clip capture work.
- [ ] Camera disconnect is visible in status and logs.
- [ ] Camera reconnect/recovery succeeds.
- [ ] Restarting the service restores the product.
- [ ] Credentials do not appear in the UI, logs, or support bundle.
- [ ] The exact tested package is archived.

## Candidate identity

Each package must record:

- Git commit hash;
- clean or dirty worktree state;
- binary SHA-256;
- archive SHA-256.

The package name should use `rc-candidate`. Do not create a Git tag until every
gate passes and the final package is rebuilt from the final clean commit.

## Tag decision gate

Tagging is allowed only after clean-install, service/restart, physical-camera,
A/V, PTZ where advertised, capture, disconnect/recovery, credential-safety,
automated-suite, and final-clean-rebuild checks all pass. A failed check means
fix the blocker and rebuild the untagged candidate.

## Weekend operating rule

Fix only blockers found while running this flow. Defer SS-WP-015A, SS-WP-015B,
new ADR implementation, emulator expansion, remote access, and opportunistic
refactors until the RC gate is complete.

If a capability is unstable, mark it unsupported for the RC rather than
blocking fixed-camera live view and recovery.
