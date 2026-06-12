# Relay Usage Reporting Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let Relay report actual per-session traffic stats to Control so the Control Admin usage view can show both granted quota and real bytes.

**Architecture:** Relay keeps local session stats as it does today and periodically posts a snapshot to Control using its existing Control relay token. Control stores the latest stats per session id, persists them in the existing snapshot store, and folds them into `GET /usage/users` by mapping session ids back through existing agent session assignments.

**Tech Stack:** Rust, Axum, existing Control client, existing Relay session store/admin session models, SQLite JSON snapshot persistence, plain HTML admin page.

---

## Task 1: Control API Red Test

**Files:**
- Modify: `crates/control/tests/control_plane.rs`

- [x] Add a failing test for `POST /usage/relay-sessions`.
- [x] Verify ordinary user tokens receive `403`.
- [x] Verify a relay token can report only its own `relay_id`.
- [x] Create a user session, report uplink/downlink/total bytes, then verify `GET /usage/users` includes actual traffic bytes.

## Task 2: Shared DTOs

**Files:**
- Modify: `crates/control-client/src/lib.rs`

- [x] Add `RelaySessionUsageReport`.
- [x] Add `ReportRelaySessionUsageRequest`.
- [x] Extend `UserUsageSummary` with actual uplink/downlink/total bytes.
- [x] Add `HttpControlClient::report_relay_session_usage`.

## Task 3: Control State And Route

**Files:**
- Modify: `crates/control/src/store.rs`
- Modify: `crates/control/src/state.rs`
- Modify: `crates/control/src/routes.rs`

- [x] Persist latest reported Relay usage by session id.
- [x] Add `ControlState::report_relay_session_usage`.
- [x] Authorize the route with admin or matching relay token.
- [x] Aggregate stored actual bytes into `user_usage_summaries`.

## Task 4: relayd Reporter

**Files:**
- Modify: `crates/relay/src/admin.rs`
- Modify: `apps/relayd/src/main.rs`

- [x] Expose relay session snapshots from `RelayService`/admin state path without HTTP.
- [x] Post usage snapshots during the existing heartbeat loop.
- [x] Add unit coverage for CLI/reporting helper behavior.

## Task 5: Admin Page And Docs

**Files:**
- Modify: `docs/control-admin.html`
- Modify: `crates/mobile-core/tests/smoke_script.rs`
- Modify: `README.md`

- [x] Show actual uplink/downlink/total columns in Usage.
- [x] Add static page contract checks.
- [x] Document `POST /usage/relay-sessions` and the Control usage semantics.

## Task 6: Verify

**Commands:**
- [x] `cargo fmt`
- [x] `cargo test -p quic_tunnel_control --test control_plane`
- [x] `cargo test -p quic_tunnel_control --test sqlite_store`
- [x] `cargo test -p quic_tunnel_mobile_core --test smoke_script`
- [x] `cargo test -p control-server`
- [x] `cargo test -p relayd`
- [x] `cargo test --workspace --no-run`
- [x] `cargo fmt --check`
