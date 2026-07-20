# Camera Certification Process

Use this process whenever a new physical camera model is evaluated. The goal
is to produce repeatable evidence and clearly separate verified behavior from
assumptions based on a brand or protocol label.

## 1. Record the device

Record exact model, hardware revision, firmware, region, purchase channel,
power/network mode, and whether a subscription is required. Create a dedicated
least-privilege test account.

## 2. Establish discovery behavior

Test OS enumeration for USB, Streaming discovery for ONVIF, and manual setup for
RTSP/MJPEG. Record whether discovery works across reboot and whether multicast
is required.

## 3. Establish media behavior

Test connection before registration. Verify authentication, one-frame capture,
continuous MJPEG, resolution, FPS, latency, codec, and substream behavior.

## 4. Exercise failures

Test wrong credentials, unavailable stream, camera power loss, network
interruption, source stop/start, and recovery. Record the time to detect the
failure, reconnect attempts, time to recover, and whether a process restart was
needed.

## 5. Run stability validation

Run at least 30 minutes for an initial qualification with one MJPEG viewer.
For release candidates, run the source in the documented 24-hour/72-hour
endurance profile. Record memory, CPU, FPS, frame age, reconnects, and errors.

## 6. Publish the result

Add a row to the compatibility matrix with one of:

- **Verified** — all required tests passed on the recorded environment.
- **Verified with limitations** — usable, but a known model/network limitation exists.
- **Discovery only** — identity is found, but media setup is not yet automatic.
- **Not compatible** — no supported local media path or a reproducible blocking failure.

Attach sanitized JSON, screenshots, logs, metrics, and the exact test date. A
new firmware version requires revalidation when it changes streaming,
authentication, or discovery behavior.

## 7. Acceptance rule for new providers

Future providers must expose the same lifecycle and frame contract as existing
sources. They must pass the common checklist, provide useful failure states,
avoid leaking credentials, work through the manager-owned pipeline, and support
health/recovery evidence. No provider is complete merely because it can open a
stream once.
