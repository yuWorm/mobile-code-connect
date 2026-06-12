# Relay Bootstrap Install Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build one-command Relay provisioning where Control creates a short-lived single-use bootstrap, a Linux target host exchanges it, and `relayd` automatically registers and heartbeats back to Control.

**Architecture:** Add Relay bootstrap request/response models to `control-client`, persist bootstrap records in Control state, expose admin create and unauthenticated exchange routes, and extend `relayd` to bootstrap into its existing registration path. Add CLI and script support so admins can copy an install command without exposing admin credentials or long-lived Relay control tokens.

**Tech Stack:** Rust, Axum, serde, existing HMAC token issuer, existing Control store persistence, clap, shell script dry-run tests.

---

## Files

- Modify `crates/control-client/src/lib.rs`: add bootstrap request/response structs and client methods.
- Modify `crates/control/src/store.rs`: persist `RelayBootstrapRecord`.
- Modify `crates/control/src/state.rs`: create/exchange bootstrap state machine, audit-safe validation, token hashing.
- Modify `crates/control/src/routes.rs`: add `POST /relay-bootstraps` and `POST /relay-bootstraps/{bootstrap_id}/exchange`.
- Modify `apps/relayd/src/main.rs`: add bootstrap CLI args and exchange before normal control registration.
- Modify `apps/mobile-cli/src/main.rs`: add `admin create-relay-bootstrap`.
- Modify `crates/sdk/src/admin.rs`: re-export and wrap admin bootstrap creation.
- Add `scripts/install-relayd.sh`: Linux/systemd installer with dry-run mode.
- Modify `crates/control/tests/control_plane.rs`: route/auth behavior tests.
- Modify `crates/control/tests/sqlite_store.rs`: persistence round-trip test.
- Modify `apps/relayd/src/main.rs` tests: bootstrap args and exchange config mapping.
- Modify `apps/mobile-cli/src/main.rs` tests: admin CLI parsing/request.
- Modify `crates/mobile-core/tests/smoke_script.rs`: production script/docs know about installer.
- Modify `scripts/production-check.sh`: shell syntax check for installer.
- Modify `docs/production-readiness.md`: add relay bootstrap production gate notes.

## Chunk 1: Control-Client Models

### Task 1: Add Relay Bootstrap DTOs

**Files:**
- Modify: `crates/control-client/src/lib.rs`

- [ ] **Step 1: Write failing compile test by referencing new DTOs**

Add a unit test near existing control-client tests or compile-facing code that constructs:

```rust
let request = CreateRelayBootstrapRequest {
    relay_id: "relay_auto".to_string(),
    control_url: "https://control.example.com".to_string(),
    relay_addr: "relay.example.com:4443".to_string(),
    admin_addr: "127.0.0.1:9090".to_string(),
    capacity_streams: 128,
    heartbeat_interval_sec: 30,
    ttl_sec: 900,
};
```

Expected response structs:

```rust
RelayBootstrapResponse {
    bootstrap_id,
    relay_id,
    control_url,
    expires_epoch_sec,
    install_command,
    no_service_install_command,
    bootstrap_token,
}
RelayBootstrapExchangeRequest { bootstrap_token }
RelayBootstrapExchangeResponse {
    control_url,
    control_token,
    relay_id,
    token_secret,
    relay_addr,
    admin_addr,
    capacity_streams,
    heartbeat_interval_sec,
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p quic_tunnel_control_client relay_bootstrap --lib`

Expected: compile failure because types do not exist.

- [ ] **Step 3: Implement DTOs and client methods**

Add `Serialize`, `Deserialize`, `Clone`, `Debug`, `PartialEq`, `Eq` structs.

Add methods:

```rust
pub async fn create_relay_bootstrap(
    &self,
    request: CreateRelayBootstrapRequest,
) -> Result<RelayBootstrapResponse, ControlClientError>

pub async fn exchange_relay_bootstrap(
    &self,
    bootstrap_id: &str,
    request: RelayBootstrapExchangeRequest,
) -> Result<RelayBootstrapExchangeResponse, ControlClientError>
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p quic_tunnel_control_client relay_bootstrap --lib`

Expected: PASS.

## Chunk 2: Control State And Routes

### Task 2: Add Bootstrap State Machine

**Files:**
- Modify: `crates/control/src/store.rs`
- Modify: `crates/control/src/state.rs`
- Test: `crates/control/tests/control_plane.rs`
- Test: `crates/control/tests/sqlite_store.rs`

- [ ] **Step 1: Write failing state tests**

Add tests proving:

- `create_relay_bootstrap` returns a plaintext token once and stores only hash.
- `exchange_relay_bootstrap` succeeds once.
- second exchange fails.
- expired exchange fails.
- exchange creates/enables relay credential and issues a Relay-scoped token.

Use deterministic `now` if the state already supports test clock helpers; otherwise keep focused tests around expiry values that are clearly future/past.

- [ ] **Step 2: Run tests to verify failure**

Run: `cargo test -p quic_tunnel_control relay_bootstrap`

Expected: compile failure/missing methods.

- [ ] **Step 3: Implement state records and methods**

Add store field:

```rust
#[serde(default)]
pub(crate) relay_bootstraps: HashMap<String, RelayBootstrapRecord>,
```

Add `RelayBootstrapRecord` in `store.rs`.

Add state methods:

```rust
pub fn create_relay_bootstrap(
    &self,
    actor: &ControlTokenClaims,
    request: CreateRelayBootstrapRequest,
) -> Result<RelayBootstrapResponse, ControlPlaneError>

pub fn exchange_relay_bootstrap(
    &self,
    bootstrap_id: &str,
    request: RelayBootstrapExchangeRequest,
) -> Result<RelayBootstrapExchangeResponse, ControlPlaneError>
```

Hash token with existing HMAC/SHA tooling or `sha2` already in workspace. Generate ids/secrets with `uuid`.

Validation:

- reject empty `relay_id`, `control_url`, `relay_addr`
- reject `capacity_streams == 0`
- reject `heartbeat_interval_sec == 0`
- reject `ttl_sec == 0`
- cap TTL to a conservative maximum, e.g. 24 hours

Exchange behavior:

- fail if missing, expired, consumed, or hash mismatch
- create relay credential if absent, otherwise enable existing credential
- issue relay control token using existing `issue_relay_token`
- mark consumed before returning config

- [ ] **Step 4: Add route tests**

Add tests:

- ordinary user cannot create bootstrap
- admin can create bootstrap
- exchange does not require admin bearer token
- invalid token cannot exchange

- [ ] **Step 5: Run tests to verify pass**

Run: `cargo test -p quic_tunnel_control relay_bootstrap`

Expected: PASS.

## Chunk 3: relayd Bootstrap

### Task 3: Exchange Bootstrap In relayd

**Files:**
- Modify: `apps/relayd/src/main.rs`

- [ ] **Step 1: Write failing CLI tests**

Add tests:

- `relayd --bootstrap-control-url ... --bootstrap-id ... --bootstrap-token ...` parses.
- bootstrap args conflict with explicit `--control-url/--control-token/--relay-id` or produce a clear error when incomplete.
- exchange response maps into `RelayControlRegistration` and `RelayConfig`.

- [ ] **Step 2: Run tests to verify failure**

Run: `cargo test -p relayd bootstrap`

Expected: compile/parser failure.

- [ ] **Step 3: Implement bootstrap args and exchange**

Add CLI fields:

```rust
bootstrap_control_url: Option<String>,
bootstrap_id: Option<String>,
bootstrap_token: Option<String>,
```

Before starting `RelayService`, resolve bootstrap config if present:

- call `HttpControlClient::new(bootstrap_control_url)`
- call `exchange_relay_bootstrap`
- use returned `token_secret` for `RelayConfig`
- use returned `control_url/control_token/relay_id/...` for registration

Keep existing explicit mode untouched.

- [ ] **Step 4: Run tests to verify pass**

Run: `cargo test -p relayd bootstrap`

Expected: PASS.

## Chunk 4: Admin SDK And CLI

### Task 4: Add Admin Bootstrap API To SDK And CLI

**Files:**
- Modify: `crates/sdk/src/admin.rs`
- Modify: `apps/mobile-cli/src/main.rs`

- [ ] **Step 1: Write failing CLI parsing test**

Add a test like:

```rust
mobile-cli admin create-relay-bootstrap \
  --control https://control.example.com \
  --token admin-token \
  --relay-id relay_auto \
  --relay-addr relay.example.com:4443 \
  --admin-addr 127.0.0.1:9090 \
  --capacity-streams 128 \
  --heartbeat-interval-sec 30 \
  --ttl-sec 900
```

Assert the request includes `control_url == --control`.

- [ ] **Step 2: Run tests to verify failure**

Run: `cargo test -p mobile-cli admin_create_relay_bootstrap`

Expected: parser failure.

- [ ] **Step 3: Implement SDK wrapper and CLI command**

SDK:

```rust
pub async fn create_relay_bootstrap(
    &self,
    request: CreateRelayBootstrapRequest,
) -> Result<RelayBootstrapResponse, SdkError>
```

CLI command calls `HttpControlClient::create_relay_bootstrap` and prints JSON.

- [ ] **Step 4: Run tests to verify pass**

Run: `cargo test -p mobile-cli admin_create_relay_bootstrap`

Expected: PASS.

## Chunk 5: Linux Installer Script

### Task 5: Add Dry-Run systemd Installer

**Files:**
- Add: `scripts/install-relayd.sh`
- Modify: `crates/mobile-core/tests/smoke_script.rs`
- Modify: `scripts/production-check.sh`
- Modify: `docs/production-readiness.md`

- [ ] **Step 1: Write failing smoke_script test**

Assert:

- `scripts/install-relayd.sh` exists.
- `bash -n scripts/install-relayd.sh` is covered by `scripts/production-check.sh`.
- docs mention relay bootstrap install command and dry-run.
- docs mention `--no-service` for manual startup tests.

- [ ] **Step 2: Run test to verify failure**

Run: `cargo test -p quic_tunnel_mobile_core --test smoke_script relay_bootstrap`

Expected: FAIL missing script/docs.

- [ ] **Step 3: Implement script**

Script supports:

- `--control-url`
- `--bootstrap-id`
- `--bootstrap-token`
- `--relayd-bin` for local/dev installs
- `--dry-run`
- `--no-service`
- `--install-dir`
- `--systemd-dir`
- `--env-file`

Dry-run prints the actions without writing `/etc`.

Real mode writes:

- env file with bootstrap args
- systemd unit running `relayd --bootstrap-control-url ... --bootstrap-id ... --bootstrap-token ...`
- `systemctl daemon-reload`
- `systemctl enable --now quic-tunnel-relayd`

- [ ] **Step 4: Add production check**

Add `bash -n scripts/install-relayd.sh` to `scripts/production-check.sh`.

- [ ] **Step 5: Run tests to verify pass**

Run:

```bash
cargo test -p quic_tunnel_mobile_core --test smoke_script relay_bootstrap
bash -n scripts/install-relayd.sh
```

Expected: PASS.

## Chunk 6: Final Verification

### Task 6: Run Focused And Production Gates

**Files:**
- No new files unless failures require fixes.

- [ ] **Step 1: Format**

Run: `cargo fmt --check`

Expected: PASS.

- [ ] **Step 2: Focused tests**

Run:

```bash
cargo test -p quic_tunnel_control relay_bootstrap
cargo test -p quic_tunnel_control_client relay_bootstrap --lib
cargo test -p relayd bootstrap
cargo test -p mobile-cli admin_create_relay_bootstrap
cargo test -p quic_tunnel_mobile_core --test smoke_script relay_bootstrap
```

Expected: PASS.

- [ ] **Step 3: Production check**

Run: `scripts/production-check.sh`

Expected: PASS, with real mobile package/device gates skipped unless env vars are set.

- [ ] **Step 4: Document remaining production gaps**

If real systemd install, public binary download, or real host online test is not run, state that clearly in the final response.
