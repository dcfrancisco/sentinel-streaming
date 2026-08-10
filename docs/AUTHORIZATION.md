# Authorization

The current roles are intentionally small:

| Role | Authorities |
| --- | --- |
| Viewer | `VIEW_STREAM`, `VIEW_SOURCE` |
| Operator | Viewer plus `CONTROL_PTZ`, `VIEW_DIAGNOSTICS` |
| Administrator | All authorities, including `MANAGE_SOURCE`, `RUN_ONBOARDING`, and `ADMINISTER_SYSTEM` |

Authorization is enforced in API middleware and again at the PTZ service
boundary. UI hiding is not a security control. PTZ failures return
`PTZ_AUTHORIZATION_DENIED` and cannot issue an ONVIF request.
