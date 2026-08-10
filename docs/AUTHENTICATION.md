# Authentication

The standalone implementation uses bearer tokens supplied through environment
variables. Tokens map to principals and roles in memory at process startup;
they are never returned by the API or written to events.

`SENTINEL_VIEWER_TOKEN`, `SENTINEL_OPERATOR_TOKEN`, and
`SENTINEL_ADMIN_TOKEN` configure the Viewer, Operator, and Administrator
roles. `SENTINEL_API_TOKEN` remains an Operator-compatible legacy token.
`SENTINEL_BOOTSTRAP_TOKEN` is a temporary Administrator token for first-run
setup. There is no shipped default credential or user database in this WP.

Send `Authorization: Bearer <token>` to protected API routes. With no token
configured, only use the intentionally open local-development mode; never
expose that mode remotely. The CLI can use
its existing keyring-backed profile token. Larger deployments can replace the
authenticator with an OIDC/SSO-backed implementation without changing source,
ONVIF, PTZ, or media domain contracts.
