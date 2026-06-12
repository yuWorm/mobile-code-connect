# Control-Owned Relay Live Ops Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move Relay Admin live operations into the Control management plane without making browsers or operators connect directly to Relay Admin HTTP.

**Architecture:** Relay reports session snapshots to Control during the existing heartbeat. Control stores the latest per-relay snapshots and exposes admin-only read APIs plus a relay-scoped command queue. Admin UI calls only Control APIs; relayd polls Control for pending commands, executes them against its local `RelaySessionStore`, and reports command results back to Control.

**Tech Stack:** Rust, Axum, serde, in-memory/SQLite persisted Control state, relayd heartbeat loop, Vue 3, shadcn-vue components, Bun tests, Cargo tests.

---

## File Map

- Modify `crates/control-client/src/lib.rs`: add shared Relay Live Ops DTOs and HTTP client methods.
- Modify `crates/control/src/store.rs`: persist relay session snapshots and relay command records.
- Modify `crates/control/src/state.rs`: add relay session snapshot storage, admin command creation, relay command leasing, and result reporting.
- Modify `crates/control/src/routes.rs`: add Control-owned Relay Live Ops routes.
- Modify `crates/control/tests/control_plane.rs`: cover admin read/command routes and relay-only command polling/result routes.
- Modify `apps/relayd/src/main.rs`: include session snapshots in heartbeat, poll pending commands, execute disconnect locally, and report results.
- Modify `web/src/lib/control/types.ts`: expose Relay Live Ops DTOs to the frontend.
- Modify `web/src/lib/control/api.ts`: add Relay Live Ops API methods.
- Modify `web/src/views/admin/AdminRelaysView.vue`: show relay sessions in the Relay detail dialog and trigger disconnect through Control.
- Modify `web/src/lib/i18n/messages.ts`: add Chinese/English strings for relay sessions and command status.
- Modify `web/src/views/__tests__/admin-infra-filters.test.ts`: assert the Relay view uses Control-owned sessions/commands, not Relay Admin URLs.
- Modify `web/src/lib/control/__tests__/api.test.ts`: assert new API paths and payloads.
- Modify `README.md` and `docs/production-readiness.md`: clarify Control-owned Relay Live Ops and debug-only Relay Admin.

## Scope Decisions

- Do not make Control proxy arbitrary Relay Admin HTTP requests.
- Do not expose `admin_addr` or require `--debug-admin-listen` for the production UI.
- Keep `docs/relay-admin.html` as local-only debug UI.
- Keep command processing pull-based: relayd polls Control with its relay token; Control does not need direct network reachability to relay nodes.
- Implement one command type in this round: `disconnect_session`.

## Chunk 1: Shared DTOs and Control State

### Task 1: Add Relay Live Ops DTOs and state tests

**Files:**
- Modify: `crates/control-client/src/lib.rs`
- Modify: `crates/control/src/store.rs`
- Modify: `crates/control/src/state.rs`
- Modify: `crates/control/tests/control_plane.rs`

- [ ] **Step 1: Write failing Control state/API tests**

Add tests that prove:

- `ReportRelayHealthRequest` can include `sessions: Vec<RelaySessionSnapshot>`.
- `GET /relays/{relay_id}/sessions` returns the latest snapshots to admins.
- A user token cannot read relay session snapshots.
- `POST /relays/{relay_id}/sessions/{session_id}/disconnect` creates a pending command, returns command status, and writes an audit log.
- A relay token can fetch its own pending commands but not another relay's commands.
- A relay token can report command success/failure and the command transitions out of pending.

Run:

```bash
cargo test -p quic_tunnel_control --test control_plane relay_live_ops
```

Expected: FAIL because DTOs/routes/state do not exist yet.

- [ ] **Step 2: Add shared DTOs**

Add to `crates/control-client/src/lib.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelaySessionSnapshot {
    pub session_id: SessionId,
    pub state: String,
    pub mobile_bound: bool,
    pub agent_bound: bool,
    pub limits: RelayLimits,
    pub stats: TrafficStats,
    #[serde(default)]
    pub last_seen_epoch_sec: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelayCommand {
    pub command_id: String,
    pub relay_id: String,
    pub kind: RelayCommandKind,
    pub session_id: Option<SessionId>,
    pub status: RelayCommandStatus,
    pub requested_epoch_sec: u64,
    pub updated_epoch_sec: u64,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelayCommandKind {
    DisconnectSession,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelayCommandStatus {
    Pending,
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportRelayCommandResultRequest {
    pub status: RelayCommandStatus,
    #[serde(default)]
    pub message: String,
}
```

Extend `ReportRelayHealthRequest`:

```rust
#[serde(default)]
pub sessions: Vec<RelaySessionSnapshot>,
```

- [ ] **Step 3: Add store records**

Add to `ControlStore`:

```rust
#[serde(default)]
pub(crate) relay_session_snapshots: HashMap<String, HashMap<SessionId, RelaySessionSnapshot>>,
#[serde(default)]
pub(crate) relay_commands: HashMap<String, RelayCommand>,
```

- [ ] **Step 4: Add state methods**

Add:

- `relay_sessions(relay_id) -> Result<Vec<RelaySessionSnapshot>, ControlPlaneError>`
- `request_relay_session_disconnect(actor, relay_id, session_id) -> Result<RelayCommand, ControlPlaneError>`
- `pending_relay_commands(relay_id) -> Result<Vec<RelayCommand>, ControlPlaneError>`
- `report_relay_command_result(relay_id, command_id, request) -> Result<RelayCommand, ControlPlaneError>`

When storing health, replace that relay's snapshot map from `request.sessions`, setting `last_seen_epoch_sec` to the current heartbeat time when the snapshot has zero.

- [ ] **Step 5: Run focused tests**

Run:

```bash
cargo test -p quic_tunnel_control --test control_plane relay_live_ops
```

Expected: PASS.

## Chunk 2: Control Routes and Client Methods

### Task 2: Expose Control-owned Relay Live Ops APIs

**Files:**
- Modify: `crates/control/src/routes.rs`
- Modify: `crates/control-client/src/lib.rs`
- Modify: `crates/control/tests/control_plane.rs`

- [ ] **Step 1: Write failing route/client path tests**

Add or extend tests for:

- `GET /relays/{relay_id}/sessions`
- `POST /relays/{relay_id}/sessions/{session_id}/disconnect`
- `GET /relays/{relay_id}/commands`
- `POST /relays/{relay_id}/commands/{command_id}/result`

Run:

```bash
cargo test -p quic_tunnel_control --test control_plane relay_live_ops
cargo test -p quic_tunnel_control_client relay_live_ops
```

Expected: FAIL because routes/client helpers do not exist yet.

- [ ] **Step 2: Add routes**

Add Axum routes:

```rust
.route("/relays/{relay_id}/sessions", get(list_relay_sessions))
.route(
    "/relays/{relay_id}/sessions/{session_id}/disconnect",
    post(disconnect_relay_session),
)
.route("/relays/{relay_id}/commands", get(list_relay_commands))
.route(
    "/relays/{relay_id}/commands/{command_id}/result",
    post(report_relay_command_result),
)
```

Authorization:

- sessions/disconnect are admin-only.
- commands/result use `relay_writer_from_headers`, so only the matching relay token or admin can exercise them.

- [ ] **Step 3: Add client methods**

Add to `HttpControlClient`:

- `relay_sessions(relay_id) -> Result<Page<RelaySessionSnapshot>, ControlClientError>`
- `relay_sessions_with_query(relay_id, query)`
- `request_relay_session_disconnect(relay_id, session_id) -> Result<RelayCommand, ControlClientError>`
- `pending_relay_commands(relay_id) -> Result<Vec<RelayCommand>, ControlClientError>`
- `report_relay_command_result(relay_id, command_id, request) -> Result<RelayCommand, ControlClientError>`

- [ ] **Step 4: Run focused tests**

Run:

```bash
cargo test -p quic_tunnel_control --test control_plane relay_live_ops
cargo test -p quic_tunnel_control_client relay_live_ops
```

Expected: PASS.

## Chunk 3: relayd Heartbeat and Command Execution

### Task 3: Report session snapshots and execute queued commands

**Files:**
- Modify: `apps/relayd/src/main.rs`
- Modify: `crates/relay/src/admin.rs` only if a reusable conversion helper is needed.

- [ ] **Step 1: Write failing relayd unit tests**

Add tests that prove:

- `session_snapshot_from_relay_session` matches Relay Admin session semantics.
- `health_request_from_sessions` includes session snapshots.
- `execute_relay_command` closes a local relay session for `disconnect_session`.
- Unknown or missing local sessions are reported as failed without panicking.

Run:

```bash
cargo test -p relayd relay_live_ops
```

Expected: FAIL because helpers do not exist yet.

- [ ] **Step 2: Extend heartbeat**

Replace the health/usage double reporting block with:

1. build snapshots from `session_store.list()`;
2. send health with aggregate metrics plus snapshots;
3. report usage as today;
4. fetch pending commands;
5. execute commands locally;
6. report command results.

Keep command execution best-effort; one failed command must not stop future heartbeats.

- [ ] **Step 3: Run relayd tests**

Run:

```bash
cargo test -p relayd relay_live_ops
cargo test -p relayd
```

Expected: PASS except ignored network tests remain ignored.

## Chunk 4: Vue Control Admin Integration

### Task 4: Add Relay session table and disconnect action in Control Admin

**Files:**
- Modify: `web/src/lib/control/types.ts`
- Modify: `web/src/lib/control/api.ts`
- Modify: `web/src/lib/control/__tests__/api.test.ts`
- Modify: `web/src/views/admin/AdminRelaysView.vue`
- Modify: `web/src/lib/i18n/messages.ts`
- Modify: `web/src/views/__tests__/admin-infra-filters.test.ts`

- [ ] **Step 1: Write failing frontend tests**

Add source-level assertions that:

- `ControlApi.relaySessions()` calls `/relays/{relay_id}/sessions`.
- `ControlApi.disconnectRelaySession()` calls `/relays/{relay_id}/sessions/{session_id}/disconnect`.
- `AdminRelaysView` loads relay sessions through Control API when the Relay detail dialog opens.
- The detail dialog renders session state, bound peers, active streams, bytes, and a disconnect action.
- The view does not reference `relay-admin.html`, `/admin/sessions`, `admin_addr`, or a Relay Admin base URL.

Run:

```bash
bun test web/src/lib/control/__tests__/api.test.ts web/src/views/__tests__/admin-infra-filters.test.ts
```

Expected: FAIL.

- [ ] **Step 2: Add frontend types/API**

Add `RelaySessionSnapshot`, `RelayCommand`, `RelayCommandStatus`, and `RelayCommandKind` to `types.ts`. Add methods to `api.ts` mirroring the Rust client methods.

- [ ] **Step 3: Update AdminRelaysView**

In the detail dialog:

- create `relaySessions = useAsyncData(...)` or explicit `ref` state keyed by `selectedRelay.relay_id`;
- load sessions when detail opens;
- show a compact session table;
- add `ConfirmAction` for disconnect;
- after command creation, refresh sessions and relays.

- [ ] **Step 4: Add i18n strings**

Add Chinese and English keys for relay sessions and disconnect command feedback.

- [ ] **Step 5: Run frontend tests and build**

Run:

```bash
bun test web/src/lib/control/__tests__/api.test.ts web/src/views/__tests__/admin-infra-filters.test.ts
bun run build
```

Expected: PASS. Existing third-party Rolldown pure annotation warnings are acceptable only if exit code is 0.

## Chunk 5: Docs and Final Verification

### Task 5: Document the production flow and verify the changed surface

**Files:**
- Modify: `README.md`
- Modify: `docs/production-readiness.md`
- Modify: `crates/mobile-core/tests/smoke_script.rs` if docs assertions need to include the new flow.

- [ ] **Step 1: Write or update docs assertions**

Update smoke tests to assert docs mention:

- Control-owned Relay Live Ops.
- Relay Admin remains debug-only.
- Admin UI must use Control APIs for live sessions/commands.

- [ ] **Step 2: Update docs**

Document:

- relay session snapshots are heartbeat-reported;
- disconnect is command-queued and executed by relayd;
- no production browser connects to Relay Admin HTTP.

- [ ] **Step 3: Final verification**

Run:

```bash
cargo fmt --check
cargo test -p quic_tunnel_control --test control_plane
cargo test -p quic_tunnel_control_client
cargo test -p relayd
cargo test -p quic_tunnel_mobile_core --test smoke_script
bun test web/src/lib/control/__tests__/api.test.ts web/src/views/__tests__/admin-infra-filters.test.ts
bun run build
```

Expected: PASS. If sandbox blocks tests that bind local sockets, report the exact blocked command and run the narrower non-network checks that still prove this feature.
