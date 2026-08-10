# Camera Lifecycle

The existing runtime source lifecycle remains authoritative:

```text
stopped -> starting -> running
                 \-> failed
running -> disconnected -> reconnecting -> running
```

SS-WP-003 adds a separate validation lifecycle field so validation does not
pretend to start the media decoder:

```text
unknown -> validating -> validated
                    \-> failed
```

Validation success sets `validation: validated` and `health: healthy`. A failed
validation sets `validation: failed` and `health: unhealthy`, increments
`consecutive_validation_failures`, and retains the normalized failure.

Validation does not automatically start an RTSP worker, change a stopped source
to running, or claim that decoded video is available.
