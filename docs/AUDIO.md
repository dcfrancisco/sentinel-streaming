# Audio transport

SS-WP-013 adds camera-to-client audio transport and capability reporting. It
does not implement speech recognition, sound-event intelligence, transcription,
talk-back, or two-way audio.

ONVIF profiles may report an audio encoder, sample rate, and channel count.
MediaMTX path telemetry may independently report an observed audio track and
codec. Sentinel preserves unknown values instead of inventing metadata.

Audio delivery states distinguish source capability, transport, and browser
readiness: `READY`, `UNSUPPORTED`, `UNKNOWN`, and `UNAVAILABLE`. The Admin
player requests audio when an audio track is reported, starts muted, and
provides an explicit unmute control. Video-only sources remain valid.

Bounded clips map an optional first audio track and copy it when the source and
container support stream copy. Artifact metadata records audio presence and
observed codec/sample-rate/channel values. Snapshots remain image-only.

Automated coverage uses deterministic protocol and emulator/media fixtures. No
physical-camera audio compatibility is claimed until a `PHYSICAL_DEVICE`
certification report records it.
