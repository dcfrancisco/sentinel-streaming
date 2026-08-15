# Authentication

The standalone implementation uses an explicit `security.mode` setting. Tokens
map to principals and roles in memory at process startup; they are never
returned by the API or written to events.

For human development and local validation, use:

```yaml
security:
  mode: OPEN_LOCAL_TEST
```

This mode requires a loopback bind, opens the Admin/API surface without login,
and is not production-safe. The Admin UI labels the active mode.

For authenticated local or standalone operation, use:

```yaml
security:
  mode: LOCAL_ADMIN_AUTH
```

`SENTINEL_VIEWER_TOKEN`, `SENTINEL_OPERATOR_TOKEN`, and
`SENTINEL_ADMIN_TOKEN` configure the Viewer, Operator, and Administrator
roles. `SENTINEL_API_TOKEN` remains an Operator-compatible legacy token.
`SENTINEL_BOOTSTRAP_TOKEN` is a temporary Administrator token for first-run
setup. There is no shipped default credential or user database in this WP.

Send `Authorization: Bearer <token>` to protected API routes in
`LOCAL_ADMIN_AUTH`. The CLI can use
its existing keyring-backed profile token. Larger deployments can replace the
authenticator with an OIDC/SSO-backed implementation without changing source,
ONVIF, PTZ, or media domain contracts.
