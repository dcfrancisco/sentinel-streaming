# ADR 0016: Low-Latency Streaming and Buffering

- Status: Proposed
- Date: 2026-11-11
- Related: ADR 0001, ADR 0008, ADR 0013

## Context

Sentinel currently has a bounded `FrameBuffer` with a default capacity of 300
frames. It is an in-memory recent-frame ring: writes append the newest frame
and evict the oldest at capacity; consumers such as preview, MJPEG, and Vision
read retained or latest frames. At 30 FPS, 300 frames represent about ten
seconds of history. This is useful for consumers and bounds frame count, but it
is not a network jitter buffer, a playout queue, or a guarantee of smooth
playback.

The pipeline is ordered and stages run synchronously for each captured frame.
The preview stage JPEG-encodes each frame. The MJPEG path also JPEG-encodes
frames for each connected viewer. The Admin Console camera-card preview uses
repeated still-frame requests, currently spaced about 1.2 seconds apart; it is
a thumbnail/inspection view, not continuous video playback. Camera detail
playback uses the normalized media gateway when a supported WebRTC or HLS
stream is available. These paths have different latency and CPU costs.

Current status and metrics expose frame-buffer capacity, utilization,
evictions, pipeline/source counters, and aggregate MJPEG output, but do not
provide an end-to-end capture-to-display latency distribution or per-stage
processing timings. Therefore, “slow streaming” cannot be attributed to buffer
capacity alone without measuring the full path.

The buffer is bounded by frame count, not by bytes. Frames retain raw RGB
payloads. For example, 300 frames at 1280x720 RGB are approximately 791 MiB
of pixel data alone (300 x 1280 x 720 x 3), before allocator and other runtime
overhead. This is an upper-bound illustration for a full buffer at that
resolution, not a measured memory reading. The camera worker also hands frames
to the pipeline through a capacity-two channel using a blocking send. Since
pipeline stages run synchronously, a slow downstream stage can backpressure
the camera worker instead of allowing an unbounded queue to grow.

Source disconnections are reported separately from buffer occupancy. A failed
frame read marks a source disconnected and enters its reconnect path; having a
buffer does not keep a failed camera or transport connected. Repeated card
preview requests are also not continuous video: they poll one current JPEG
roughly every 1.2 seconds.

The operator-provided runtime log reports `pipeline.vision: false` and shows
only buffer/preview pipeline stages, yet OpenAI observations continue to fail
with “no output_text” every few seconds while analyzing up to five frames.
This confirms unnecessary Vision work in that observed run, but does not show
a camera disconnect or prove that Vision caused the perceived stream
slowdown. Source review found that startup defaults combined both Vision
switches, but runtime `VisionControl::apply` did not enforce `pipeline.vision`;
an administrator update could therefore override the pipeline-level disable
switch. The runtime gate has now been added, with a regression test that has
not yet been run.

## Decision

Keep the current ring buffer bounded and latest-frame-oriented for live
consumers. Do not increase its capacity as a generic response to sluggish
playback: retaining more history can increase memory use and the age of
available frames, but does not make a slow encoder, transport, or browser
render faster.

Treat capture buffering, low-latency live delivery, temporal Vision sampling,
and any future lossless recording queue as separate concerns. Live preview and
playback should prefer fresh frames and discard stale queued frames under
overload. Recording, if implemented, must define its own bounded
backpressure/drop policy and must not block camera capture or unrelated
consumers. All buffers and processing state should be source-scoped as
specified by ADR 0008.

Vision may run only when both `pipeline.vision` and `vision.enabled` are true.
The same effective gate must apply to startup, runtime API mutations, and
scheduler construction; disabling either setting must stop/cancel work and
prevent future provider requests.

## Recommended implementation sequence

1. **Enforce the Vision gate.** Implemented: runtime updates reject enabling
   Vision when `pipeline.vision` is false, complementing the startup check.
   The regression test is pending execution. This prevents needless network
   requests and image processing when Vision is administratively disabled.
2. **Triage observed slowness with existing signals.** Check
   `/api/v1/sources` for source state, last frame, disconnects, decode failures,
   and reconnect counts; `/api/v1/status` for buffer length/capacity/
   utilization/evictions; and `/metrics` for source frame/drop counters,
   process memory/CPU, MJPEG viewers, output FPS/bytes, and errors. A rising
   disconnect count points to capture/transport recovery, while a running
   source with low displayed FPS points to delivery, encoding, or client
   playback. These signals narrow the investigation but do not reveal
   end-to-end latency.
3. **Measure before tuning.** Add capture timestamps and report frame age at
   each delivery boundary, capture/output FPS, dropped frames, bounded-channel
   backpressure, JPEG encode duration, and p50/p95/p99 end-to-end latency. Keep
   metrics source-scoped and distinguish unmeasured values from zero.
4. **Keep capture independent of slow consumers.** Replace blocking live-frame
   handoff with a bounded latest-wins policy where dropping stale live frames
   is acceptable. Count drops and capture stalls explicitly. Do not apply this
   loss policy to any future recording path.
5. **Make the admin preview continuous.** Keep card previews lightweight, but
   use an authenticated continuous playback transport for camera detail (and,
   where appropriate, cards) rather than polling still JPEGs. Prefer the
   normalized WebRTC path for low-latency playback; label HLS fallback as a
   higher-latency option. Do not bypass authorization to make an image element
   work.
6. **Avoid stale-frame queues.** Use a small bounded latest-wins handoff for
   live encoders and viewers. If a consumer cannot keep up, skip obsolete
   frames and count them instead of allowing delay to grow.
7. **Control encode cost.** Measure before changing resolution or FPS. Reuse an
   encoded frame across compatible MJPEG viewers where safe, or use a
   hardware/streaming transport when available. Bound viewer count and
   per-source CPU/memory consumption.
8. **Bound retention by memory and purpose.** Track estimated retained bytes
   (or enforce a per-source byte budget) in addition to frame count. Expose a
   frame/time budget only after accounting for resolution, frame rate, Vision
   sample needs, and a per-source memory limit. Do not simply increase the
   capacity to address disconnects or slow playback, and do not conflate
   temporal-analysis history with live-playback buffering.
9. **Benchmark representative paths.** Record baseline and target latency for
   built-in and RTSP sources, one and multiple viewers, and gateway playback.
   Verify that slow Vision/provider requests do not delay frame delivery.

## Consequences

- Buffer presence is documented accurately: the current buffer provides
  bounded recent-frame retention, not playback smoothing.
- The default capacity of 300 raw frames can have substantial memory cost at
  HD resolutions; frame-count bounds alone are not a useful cross-resolution
  memory budget.
- A source disconnect is a capture/transport lifecycle failure, not something
  repaired by retaining more frames. Diagnosis must correlate source failure
  and reconnect metrics with buffer, process, and output metrics.
- The observed Vision failures occurred despite the logged pipeline Vision
  disable setting. This requires enforcing the effective configuration gate
  on runtime mutations; it is not evidence that a camera disconnected.
- Increasing capacity is not the first-line fix for perceived latency.
- The Admin Console's periodically refreshed card preview may look slow even
  while the camera pipeline is capturing faster; the detail player is the
  continuous playback surface where supported.
- Low-latency changes require observability and measurements before selecting
  queue sizes, resolution, FPS, or transport defaults.
- Simultaneous per-camera performance and resource isolation remain blocked on
  implementation of the source-scoped runtime in ADR 0008.

## Verification gate

Before accepting a low-latency implementation, report measured capture-to-
display p50/p95/p99 latency, source FPS, output FPS, dropped frames, encoder
cost, memory use, and viewer count for documented test conditions. Include
slow-consumer and Vision-provider failure cases, and verify that live paths
discard stale frames without blocking capture. Do not claim a latency
improvement based only on a larger frame buffer or a successful build.
