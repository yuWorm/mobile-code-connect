# Control Usage Summary Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Expose an admin-only Control usage summary showing each user's sessions and granted Relay quota.

**Architecture:** Derive summaries from existing Control state instead of adding a new usage table. Agent session assignments already persist per session, and their signed Relay tokens contain the quota granted at session creation. This summary is Control-side quota/session observability, not actual Relay traffic byte accounting.

**Tech Stack:** Rust, Axum, existing Control state store, signed Relay token verification, plain HTML admin page.

---

## Task 1: Red Test

**Files:**
- Modify: `crates/control/tests/control_plane.rs`

- [x] Add red test for `GET /usage/users`.
- [x] Verify normal user tokens receive `403`.
- [x] Create a user with a custom plan quota.
- [x] Register a controlled device and service.
- [x] Create a Control session.
- [x] Verify usage summary reports user id, email, plan id, controller/device counts.
- [x] Verify usage summary reports session status counts.
- [x] Verify usage summary reports current per-session quota and total granted quota.

## Task 2: Shared Type

**Files:**
- Modify: `crates/control-client/src/lib.rs`

- [x] Add `UserUsageSummary`.
- [x] Add `HttpControlClient::user_usage_summaries`.

## Task 3: State And Route

**Files:**
- Modify: `crates/control/src/state.rs`
- Modify: `crates/control/src/routes.rs`

- [x] Add `ControlState::user_usage_summaries`.
- [x] Aggregate sessions from existing `agent_sessions`.
- [x] Map sessions back to users through controlled devices.
- [x] Verify Relay tokens to sum granted traffic quota.
- [x] Count pending/claimed/bound/closed/expired sessions.
- [x] Add admin-only `GET /usage/users`.

## Task 4: Admin Page And Docs

**Files:**
- Modify: `docs/control-admin.html`
- Modify: `crates/mobile-core/tests/smoke_script.rs`
- Modify: `README.md`

- [x] Add Usage panel to Control Admin.
- [x] Add static page contract checks for the usage UI and endpoint.
- [x] Document `GET /usage/users`.
- [x] Clarify this is granted quota/session summary, not actual Relay byte accounting.

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
