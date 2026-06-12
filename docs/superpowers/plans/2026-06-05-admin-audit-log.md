# Admin Audit Log Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Record key Control admin management mutations in a persistent audit log and expose it to admins.

**Architecture:** Store audit entries in the existing Control snapshot store. Routes record audit entries after successful admin mutations because the route layer has the authenticated Control token claims. Relay self-registration/heartbeat through Relay tokens is intentionally not logged as admin audit noise; admin-driven Relay pool changes are logged.

**Tech Stack:** Rust, Axum, existing Control state store, SQLite JSON snapshot persistence, plain HTML admin page.

---

## Task 1: Red Tests

**Files:**
- Modify: `crates/control/tests/control_plane.rs`
- Modify: `crates/control/tests/sqlite_store.rs`

- [x] Add route-level red test for `GET /audit-logs`.
- [x] Verify normal users receive `403`.
- [x] Verify audit entries record actor subject and role.
- [x] Verify audit entries record action, target type/id, message, and creation epoch.
- [x] Verify user creation and status update are logged.
- [x] Verify user plan update is logged.
- [x] Verify Relay credential create/rotate are logged.
- [x] Verify admin Relay registration is logged.
- [x] Verify audit logs persist through SQLite restart.

## Task 2: Shared Type And Store

**Files:**
- Modify: `crates/control-client/src/lib.rs`
- Modify: `crates/control/src/store.rs`

- [x] Add `AuditLogEntry`.
- [x] Add `HttpControlClient::audit_logs`.
- [x] Add `audit_logs` to `ControlStore` with serde default.

## Task 3: State And Routes

**Files:**
- Modify: `crates/control/src/state.rs`
- Modify: `crates/control/src/routes.rs`

- [x] Add `ControlState::audit_logs` returning newest entries first.
- [x] Add `ControlState::record_audit_log`.
- [x] Add `GET /audit-logs`.
- [x] Add admin claims helper for mutating routes.
- [x] Record user create/status/role actions.
- [x] Record plan catalog, direct user plan, and assigned user plan actions.
- [x] Record Relay credential create/status/rotate actions.
- [x] Record admin-driven Relay register/update/delete actions.
- [x] Avoid logging Relay-token heartbeat/update as admin audit entries.

## Task 4: Admin Page And Docs

**Files:**
- Modify: `docs/control-admin.html`
- Modify: `crates/mobile-core/tests/smoke_script.rs`
- Modify: `README.md`

- [x] Add Audit Logs panel to Control Admin.
- [x] Add static page contract checks for audit UI and endpoint.
- [x] Document `GET /audit-logs`.
- [x] Document audited action categories.

## Task 5: Verify

**Commands:**
- [x] `cargo fmt`
- [x] `cargo test -p quic_tunnel_control --test control_plane`
- [x] `cargo test -p quic_tunnel_control --test sqlite_store`
- [x] `cargo test -p quic_tunnel_mobile_core --test smoke_script`
- [x] `cargo test -p control-server`
- [x] `cargo test -p relayd`
- [x] `cargo test --workspace --no-run`
- [x] `cargo fmt --check`
