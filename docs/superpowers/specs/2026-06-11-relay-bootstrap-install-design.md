# Relay Bootstrap Install Design

## Goal

Allow an administrator to create a Relay node in Control, copy one install command, run it on a target Linux host, and have `relayd` install, register, heartbeat, and report usage back to Control without exposing an admin token or long-lived Relay control token in the copied command.

## Current Context

The project already has most of the runtime pieces:

- `relayd` can register itself with Control when started with `--control-url`, `--control-token`, `--relay-id`, advertised addresses, capacity, and heartbeat interval.
- Control already has Relay nodes, Relay credentials, Relay control tokens, credential rotation, heartbeat health, and usage reporting.
- Relay control tokens use `ControlRole::Relay`, are scoped to one `relay_id`, and are invalidated by credential `enabled=false` or token version rotation.
- Admin APIs can create Relay credentials and register/update Relay nodes, but there is no one-command installation/bootstrap flow.

The missing product flow is not the Relay tunnel itself; it is secure provisioning.

## Design

Control will add a Relay bootstrap object. Admin creates this object and gets an installation command. The command includes only `control_url`, `bootstrap_id`, and a one-time bootstrap secret. The target host exchanges those values for the actual Relay runtime configuration, then starts `relayd` with the existing Control registration path.

The bootstrap exchange is separate from normal Relay heartbeat. Its token is short-lived and single-use. The exchange response contains the Relay-scoped control token and runtime arguments needed by `relayd`; after exchange, Control marks the bootstrap as consumed.

## Data Model

Add a `RelayBootstrapRecord` to Control state:

- `bootstrap_id: String`
- `control_url: String`
- `relay_id: String`
- `relay_addr: String`
- `admin_addr: String`
- `capacity_streams: u32`
- `heartbeat_interval_sec: u64`
- `token_secret: String`
- `token_hash: String`
- `created_epoch_sec: u64`
- `expires_epoch_sec: u64`
- `consumed_epoch_sec: Option<u64>`
- `created_by: String`

The plaintext bootstrap token is returned only once from the create API. Control persists only `token_hash`.

The `token_secret` is the Relay data-plane token secret used by `relayd` to validate per-session Relay tokens. This is sensitive, so it is only returned through the one-time exchange. Existing deployments can still pass `--token-secret` manually; bootstrap adds a safer provisioning path.

## API

Admin endpoint:

`POST /relay-bootstraps`

Request:

```json
{
  "relay_id": "relay_us_west_001",
  "control_url": "https://control.example.com",
  "relay_addr": "relay.example.com:4443",
  "admin_addr": "127.0.0.1:9090",
  "capacity_streams": 128,
  "heartbeat_interval_sec": 30,
  "ttl_sec": 900
}
```

Response:

```json
{
  "bootstrap_id": "rb_...",
  "relay_id": "relay_us_west_001",
  "control_url": "https://control.example.com",
  "expires_epoch_sec": 1781097600,
  "install_command": "curl -fsSL ... | sudo sh -s -- --control-url ... --bootstrap-id rb_... --bootstrap-token ...",
  "no_service_install_command": "curl -fsSL ... | sudo sh -s -- --control-url ... --bootstrap-id rb_... --bootstrap-token ... --no-service",
  "bootstrap_token": "shown-once"
}
```

Bootstrap exchange endpoint:

`POST /relay-bootstraps/{bootstrap_id}/exchange`

Request:

```json
{
  "bootstrap_token": "shown-once"
}
```

Response:

```json
{
  "control_url": "https://control.example.com",
  "control_token": "relay-scoped-control-token",
  "relay_id": "relay_us_west_001",
  "token_secret": "relay-data-plane-secret",
  "relay_addr": "relay.example.com:4443",
  "admin_addr": "127.0.0.1:9090",
  "capacity_streams": 128,
  "heartbeat_interval_sec": 30
}
```

The exchange endpoint does not accept admin credentials. It authenticates only by bootstrap id plus bootstrap token, verifies expiry and single-use state, creates or enables the Relay credential for `relay_id`, issues the Relay-scoped control token, consumes the bootstrap, and returns config.

## CLI And Install Flow

`mobile-cli admin create-relay-bootstrap` will call the admin API and print JSON containing the install command and bootstrap metadata. The CLI passes its `--control` value as `control_url`, so Control does not need to infer a public URL from request headers or reverse proxy state.

`relayd` will accept:

- `--bootstrap-control-url`
- `--bootstrap-id`
- `--bootstrap-token`

When these are present, `relayd` exchanges the bootstrap token, then uses the returned config exactly as if the operator had supplied the current explicit Control registration arguments.

For Linux hosts, add `scripts/install-relayd.sh`. The script will:

- accept `--control-url`, `--bootstrap-id`, `--bootstrap-token`
- download or use a local `relayd` binary path in development mode
- write `/etc/quic-tunnel/relayd.env`
- install a `quic-tunnel-relayd.service` systemd unit
- start and enable the service
- support `--no-service` for testing: exchange the bootstrap, write the env file,
  skip systemd, and print a manual `relayd` command

The first implementation will support a dry-run mode for tests and local development. Real binary distribution URLs can be configured later without changing the bootstrap API.

## Security

- Admin tokens are never placed in install commands.
- Relay control token is not visible until the target host exchanges the one-time bootstrap token.
- Bootstrap tokens are single-use and short-lived.
- Control stores only a hash of the bootstrap token.
- Relay credential rotation or disablement invalidates already-issued Relay control tokens.
- The exchange response is intentionally scoped to one Relay id.
- Audit logs record bootstrap creation and exchange consumption without logging plaintext secrets.

## Error Handling

Bootstrap creation rejects empty Relay ids, empty Relay addresses, zero capacity, zero heartbeat interval, and zero TTL.

Bootstrap exchange returns unauthorized or not found for missing, expired, consumed, or invalid-token bootstraps. It must not reveal whether a specific bootstrap id exists when the token is wrong.

`relayd` startup fails fast if bootstrap exchange fails, rather than starting an unregistered Relay.

## Testing

Add focused tests for:

- Control state creates a bootstrap and stores only token hash.
- Bootstrap exchange succeeds once and consumes the record.
- Expired and consumed bootstraps cannot be exchanged.
- Relay credential is created/enabled and Relay control token works after exchange.
- Routes enforce admin-only bootstrap creation and no-admin bootstrap exchange.
- Control client serializes create/exchange requests.
- `relayd` CLI bootstrap arguments build registration config.
- `mobile-cli admin create-relay-bootstrap` parses arguments and produces the expected request.
- `scripts/install-relayd.sh --dry-run` renders the expected systemd/env install actions.
- `scripts/install-relayd.sh --dry-run --no-service` skips systemd actions and prints the manual start command.

Socket-binding end-to-end tests remain optional because this environment often sandboxes local listener creation.

## Out Of Scope

- Remote SSH execution from Control.
- macOS LaunchDaemon or Windows service installation.
- Package repository hosting and binary auto-update.
- Multi-use bootstrap tokens.
- UI polish for Relay bootstrap creation beyond API/CLI support.
