# Relay Control-Owned Management Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Control the only default Relay management plane, so Relay bootstrap/install never exposes a separate Relay admin HTTP surface unless an operator explicitly enables local debug mode.

**Architecture:** Keep existing `admin_addr` and `admin_bound` fields for wire/storage compatibility, but stop using them in the default provisioning path. `relayd` will only start its HTTP admin routes when a debug-only listen flag is provided, and it will not advertise that address to Control by default. Control and the Admin UI will manage Relay state through Control APIs fed by Relay registration, heartbeat, and usage reports.

**Tech Stack:** Rust, Axum, clap, serde, bash installer, Vue 3, shadcn-vue components, Bun tests, Cargo tests.

---

## File Map

- Modify `apps/relayd/src/main.rs`: add explicit debug admin CLI, stop inferring/advertising admin address, keep deprecated `--admin-listen` compatibility if practical, update unit tests.
- Modify `scripts/install-relayd.sh`: remove default admin listener from generated commands/env, add opt-in `--debug-admin-listen`, update dry-run/manual/systemd command generation.
- Modify `crates/control-client/src/lib.rs`: make relay admin fields defaultable/legacy where needed, keep structs compatible.
- Modify `crates/control/src/state.rs`: ignore bootstrap/admin-address input for new Control-managed relays, store/report empty `admin_addr` by default, keep old state readable.
- Modify `crates/control/tests/control_plane.rs`: prove bootstrap/exchange/register/heartbeat no longer propagate admin address by default.
- Modify `crates/mobile-core/tests/smoke_script.rs`: prove installer dry-run does not start Relay admin by default and documents debug-only opt-in.
- Modify `web/src/lib/control/types.ts`: mark admin fields as optional/legacy-compatible where frontend no longer depends on them.
- Modify `web/src/views/admin/AdminRelaysView.vue`: remove Relay admin address input/display from bootstrap and normal Relay forms; send empty legacy field if API still requires it.
- Modify `web/src/views/__tests__/admin-infra-filters.test.ts`: update source assertions for new Admin Relay UX.
- Modify `web/src/lib/control/__tests__/api.test.ts`: update request/response fixture expectations for empty/legacy admin fields.
- Modify `web/src/lib/i18n/messages.ts`: remove or stop using copy that suggests a separate Relay admin endpoint.
- Modify `README.md` and `docs/production-readiness.md`: describe Control-owned Relay management and debug-only local Relay admin.

## Scope Decisions

- Do not delete `crates/relay/src/admin.rs` in this round. It stays as an explicit local debug tool.
- Do not remove `admin_addr`/`admin_bound` from public DTOs in this round. Removing them is a later API-versioning cleanup.
- Do not build a Control-to-Relay command channel in this round. Existing heartbeat and usage reporting are enough for current Admin UI state.
- Do not make Control probe Relay admin HTTP. That would reintroduce the unsafe management plane.

## Chunk 1: Relayd Debug Admin Boundary

### Task 1: Add tests for opt-in debug admin semantics

**Files:**
- Modify: `apps/relayd/src/main.rs`

- [ ] **Step 1: Write failing tests**

Update/add unit tests under `#[cfg(test)] mod tests`:

```rust
#[test]
fn relayd_args_do_not_advertise_admin_addr_from_bound_debug_listener() {
    let cli = Cli::try_parse_from([
        "relayd",
        "--bind",
        "127.0.0.1:0",
        "--token-secret",
        "dev-secret",
        "--control-url",
        "http://127.0.0.1:4242",
        "--control-token",
        "relay-token",
        "--relay-id",
        "relay_auto",
    ])
    .unwrap();

    let registration = cli
        .control_registration("127.0.0.1:4443".parse().unwrap())
        .unwrap()
        .unwrap();

    assert_eq!(registration.admin_addr, "");
}

#[test]
fn relayd_debug_admin_listen_is_explicit_local_debug_only() {
    let cli = Cli::try_parse_from([
        "relayd",
        "--bind",
        "127.0.0.1:4443",
        "--token-secret",
        "dev-secret",
        "--debug-admin-listen",
        "127.0.0.1:9090",
    ])
    .unwrap();

    assert_eq!(cli.debug_admin_listen.unwrap().to_string(), "127.0.0.1:9090");
}

#[test]
fn bootstrap_registration_ignores_legacy_admin_addr_from_control_response() {
    let response = quic_tunnel_control_client::RelayBootstrapExchangeResponse {
        control_url: "https://control.example.com".to_string(),
        control_token: "relay-control-token".to_string(),
        relay_id: "relay_bootstrap".to_string(),
        token_secret: "relay-data-plane-secret".to_string(),
        relay_addr: "relay.example.com:4443".to_string(),
        admin_addr: "legacy-admin.example.com:9090".to_string(),
        capacity_streams: 64,
        heartbeat_interval_sec: 15,
    };

    let registration = RelayControlRegistration::from_bootstrap(response);
    assert_eq!(registration.admin_addr, "");
}
```

- [ ] **Step 2: Run tests and verify failure**

Run:

```bash
cargo test -p relayd relayd_args_do_not_advertise_admin_addr_from_bound_debug_listener
cargo test -p relayd relayd_debug_admin_listen_is_explicit_local_debug_only
cargo test -p relayd bootstrap_registration_ignores_legacy_admin_addr_from_control_response
```

Expected: FAIL or compile error because `debug_admin_listen` and the new behavior do not exist yet.

- [ ] **Step 3: Implement minimal relayd changes**

Change `Cli`:

```rust
#[arg(long)]
debug_admin_listen: Option<SocketAddr>,
#[arg(long, hide = true)]
admin_listen: Option<SocketAddr>,
```

Add helper:

```rust
fn debug_admin_listen(&self) -> Option<SocketAddr> {
    self.debug_admin_listen.or(self.admin_listen)
}
```

Change `control_registration` to no longer accept `admin_addr: Option<SocketAddr>` and no longer infer `admin_addr` from a bound listener. Only use an explicitly configured `--advertise-admin-addr` if keeping that compatibility is required; otherwise return an empty `admin_addr`.

Change `RelayControlRegistration::from_bootstrap` to set `admin_addr: String::new()`.

Change main admin binding to:

```rust
let admin_listener = if let Some(admin_listen) = cli.debug_admin_listen() {
    Some(tokio::net::TcpListener::bind(admin_listen).await?)
} else {
    None
};
```

- [ ] **Step 4: Run relayd tests**

Run:

```bash
cargo test -p relayd relayd_args_
cargo test -p relayd bootstrap_registration_ignores_legacy_admin_addr_from_control_response
cargo test -p relayd health_report_
```

Expected: PASS.

## Chunk 2: Installer Defaults

### Task 2: Prove installer does not start Relay admin by default

**Files:**
- Modify: `crates/mobile-core/tests/smoke_script.rs`
- Modify: `scripts/install-relayd.sh`

- [ ] **Step 1: Write failing installer tests**

Extend `relay_installer_no_service_dry_run_skips_systemd_and_prints_manual_start`:

```rust
assert!(!stdout.contains("--admin-listen"));
assert!(!stdout.contains("--debug-admin-listen"));
assert!(!stdout.contains("QUIC_TUNNEL_RELAY_ADMIN_LISTEN"));
assert!(!stdout.contains("QUIC_TUNNEL_RELAY_ADVERTISE_ADMIN_ADDR"));
```

Add a second dry-run test:

```rust
#[test]
fn relay_installer_debug_admin_is_explicit_opt_in() {
    let output = Command::new("bash")
        .arg(&relay_installer_path)
        .arg("--dry-run")
        .arg("--no-service")
        .arg("--control-url")
        .arg("127.0.0.1:4242")
        .arg("--bootstrap-id")
        .arg("rb_001")
        .arg("--bootstrap-token")
        .arg("shown-once")
        .arg("--relayd-url")
        .arg("127.0.0.1:4242/relayd")
        .arg("--debug-admin-listen")
        .arg("127.0.0.1:9090")
        .output()
        .expect("install-relayd dry-run should execute");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "{stdout}");
    assert!(stdout.contains("--debug-admin-listen"));
    assert!(stdout.contains("127.0.0.1:9090"));
}
```

- [ ] **Step 2: Run tests and verify failure**

Run:

```bash
cargo test -p quic_tunnel_mobile_core --test smoke_script relay_installer_
```

Expected: FAIL because dry-run currently prints `--admin-listen`.

- [ ] **Step 3: Implement installer changes**

In `scripts/install-relayd.sh`:

- Replace `admin_listen="127.0.0.1:9090"` with `debug_admin_listen=""`.
- Replace `--admin-listen` help text with `--debug-admin-listen ADDR`.
- Optionally keep `--admin-listen` as a deprecated alias that sets `debug_admin_listen`, but do not show it in the generated admin UI command.
- Stop writing:
  - `QUIC_TUNNEL_RELAY_ADMIN_LISTEN`
  - `QUIC_TUNNEL_RELAY_ADVERTISE_ADMIN_ADDR`
- Update `print_manual_relayd_command` so it conditionally appends `--debug-admin-listen "$QUIC_TUNNEL_RELAY_DEBUG_ADMIN_LISTEN"` only when the env var exists and is non-empty.
- Update the systemd `ExecStart` shell to build argv with `set --` and conditionally append the debug admin flag.

- [ ] **Step 4: Run shell and smoke tests**

Run:

```bash
bash -n scripts/install-relayd.sh
cargo test -p quic_tunnel_mobile_core --test smoke_script relay_installer_
```

Expected: PASS.

## Chunk 3: Control State Compatibility

### Task 3: Make Control ignore relay admin address in new default flows

**Files:**
- Modify: `crates/control-client/src/lib.rs`
- Modify: `crates/control/src/state.rs`
- Modify: `crates/control/tests/control_plane.rs`

- [ ] **Step 1: Write failing Control tests**

Update `admin_creates_single_use_relay_bootstrap_for_relayd_install`:

```rust
admin_addr: "should-be-ignored.example.com:9090".to_string(),
```

Expected assertions:

```rust
assert_eq!(exchange.admin_addr, "");
assert!(!created.install_command.contains("9090"));
assert!(!created.install_command.contains("admin-listen"));
```

When registering from exchange:

```rust
admin_addr: exchange.admin_addr,
```

Expected relay:

```rust
assert_eq!(relay.admin_addr, "");
assert!(!relay.admin_bound);
```

Add/update health report test so a relay health request with a legacy `admin_addr` still results in stored `relay.admin_addr == ""`, unless the product deliberately keeps legacy display compatibility.

- [ ] **Step 2: Run tests and verify failure**

Run:

```bash
cargo test -p quic_tunnel_control admin_creates_single_use_relay_bootstrap_for_relayd_install
cargo test -p quic_tunnel_control relay_health
```

Expected: FAIL because Control currently propagates/stores admin address.

- [ ] **Step 3: Implement Control behavior**

In `crates/control-client/src/lib.rs`, add serde defaults for legacy admin fields:

```rust
#[serde(default)]
pub admin_addr: String,
```

Apply to:

- `CreateRelayBootstrapRequest`
- `RelayBootstrapExchangeResponse`
- `RegisterRelayRequest`
- `UpdateRelayRequest`
- `ReportRelayHealthRequest`
- `RelayNode`

In `ControlState::create_relay_bootstrap`, ignore `request.admin_addr`:

```rust
let admin_addr = String::new();
```

In `register_relay`, store:

```rust
admin_bound: false,
admin_addr: String::new(),
```

In `update_relay` and `report_relay_health`, stop writing `request.admin_addr` into `relay.admin_addr`; preserve `admin_bound` only from health if needed for debug visibility.

- [ ] **Step 4: Run Control tests**

Run:

```bash
cargo test -p quic_tunnel_control admin_creates_single_use_relay_bootstrap_for_relayd_install
cargo test -p quic_tunnel_control relay_health
cargo test -p quic_tunnel_control_client relay_bootstrap
```

Expected: PASS.

## Chunk 4: Admin Frontend UX

### Task 4: Remove Relay admin address from normal Admin workflows

**Files:**
- Modify: `web/src/lib/control/types.ts`
- Modify: `web/src/views/admin/AdminRelaysView.vue`
- Modify: `web/src/views/__tests__/admin-infra-filters.test.ts`
- Modify: `web/src/lib/control/__tests__/api.test.ts`
- Modify: `web/src/lib/i18n/messages.ts`

- [ ] **Step 1: Write failing source-level tests**

Update `web/src/views/__tests__/admin-infra-filters.test.ts` assertions:

```ts
expect(source).not.toContain('bootstrap-admin-addr')
expect(source).not.toContain("relayForm.admin_addr.trim() !== ''")
expect(source).not.toContain('exchangeResult.admin_addr')
```

Update API fixtures to expect `admin_addr: ''` when creating bootstrap/relay requests.

- [ ] **Step 2: Run frontend tests and verify failure**

Run:

```bash
bun test web/src/lib/control/__tests__/api.test.ts web/src/views/__tests__/admin-infra-filters.test.ts
```

Expected: FAIL because UI still contains admin address fields.

- [ ] **Step 3: Implement UI cleanup**

In `AdminRelaysView.vue`:

- Remove `bootstrapForm.admin_addr`.
- Remove bootstrap admin address input.
- Send `admin_addr: ''` in `createRelayBootstrap` payload while API requires it.
- Remove `relayForm.admin_addr` from validation and form UI.
- Send `admin_addr: ''` from relay create/update payload while API requires it.
- Remove admin address from relay list secondary text.
- Remove `adminBound` and `adminAddr` rows from relay detail.
- Remove `exchangeResult.admin_addr` row from bootstrap exchange result.

In `web/src/lib/control/types.ts`, keep `admin_addr?: string` only where API compatibility needs it. Prefer `admin_addr: string` if changing optionality causes too much churn; the UI must not depend on it.

In `messages.ts`, keep old keys if other views still reference them; otherwise remove unused relay-admin copy.

- [ ] **Step 4: Run frontend tests/build**

Run:

```bash
bun test web/src/lib/control/__tests__/api.test.ts web/src/views/__tests__/admin-infra-filters.test.ts
bun run build
```

Expected: PASS. Known existing Rolldown annotation warnings are acceptable if build exits successfully.

## Chunk 5: Docs And Production Check

### Task 5: Update operator docs to match Control-owned management

**Files:**
- Modify: `README.md`
- Modify: `docs/production-readiness.md`
- Modify: `crates/mobile-core/tests/smoke_script.rs`

- [ ] **Step 1: Write failing doc assertions**

Update `production_check_documents_release_readiness` to require:

```rust
"Control-owned Relay management",
"--debug-admin-listen",
"debug-only local Relay admin",
```

and to reject old default guidance if the test style permits:

```rust
assert!(!checklist.contains("--admin-listen 127.0.0.1:9090"));
```

- [ ] **Step 2: Run doc tests and verify failure**

Run:

```bash
cargo test -p quic_tunnel_mobile_core --test smoke_script production_check_documents_release_readiness
```

Expected: FAIL until docs are updated.

- [ ] **Step 3: Update docs**

Document:

- Control `/admin` is the management plane.
- Relay install commands do not open a Relay admin HTTP endpoint.
- Relay health and usage flow from relayd to Control by heartbeat/reporting.
- `relayd --debug-admin-listen 127.0.0.1:9090` is local debugging only.
- Do not expose debug Relay admin publicly.

- [ ] **Step 4: Run doc tests**

Run:

```bash
cargo test -p quic_tunnel_mobile_core --test smoke_script production_check_documents_release_readiness
```

Expected: PASS.

## Final Verification

- [ ] **Step 1: Rust formatting**

Run:

```bash
cargo fmt --check
```

Expected: PASS.

- [ ] **Step 2: Focused Rust tests**

Run:

```bash
bash -n scripts/install-relayd.sh
cargo test -p relayd relayd_args_
cargo test -p relayd bootstrap_registration_ignores_legacy_admin_addr_from_control_response
cargo test -p quic_tunnel_control admin_creates_single_use_relay_bootstrap_for_relayd_install
cargo test -p quic_tunnel_control relay_health
cargo test -p quic_tunnel_mobile_core --test smoke_script relay_installer_
cargo test -p quic_tunnel_mobile_core --test smoke_script production_check_documents_release_readiness
```

Expected: PASS.

- [ ] **Step 3: Frontend tests and build**

Run:

```bash
bun test web/src/lib/control/__tests__/api.test.ts web/src/views/__tests__/admin-infra-filters.test.ts
bun run build
```

Expected: PASS.

- [ ] **Step 4: Broader regression tests**

Run:

```bash
cargo test -p quic_tunnel_control
cargo test -p quic_tunnel_control_client
cargo test -p quic_tunnel_mobile_core --test smoke_script
cargo test -p relayd
```

Expected: PASS. Ignored local TCP tests stay ignored unless explicitly run outside the sandbox.

## Rollback Notes

- If the UI changes cause too much type churn, keep `admin_addr: string` in TypeScript DTOs and send `''`; do not re-add visible Admin address fields.
- If deprecated `--admin-listen` compatibility complicates clap tests, keep it hidden and map it to debug mode. Do not make it appear in generated install commands.
- If old Control state contains `admin_addr`, keep deserialization compatibility but stop rendering it as a normal production field.
