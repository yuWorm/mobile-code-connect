# Usage Period Reset Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let admins reset a user's current usage period so Relay traffic quota enforcement can continue without changing the user's plan.

**Architecture:** Store a per-user `UserUsagePeriod` in the Control snapshot. Resetting a user records a new period start timestamp and removes existing Relay usage reports for sessions initiated by that user. Usage summaries and quota checks then reflect the current period. Automatic calendar billing periods are intentionally out of scope.

**Tech Stack:** Rust, Axum routes, serde snapshot persistence, plain HTML Control Admin.

---

## Task 1: Red Test

**Files:**
- Modify: `crates/control/tests/control_plane.rs`

- [x] Add a failing test where a user exhausts traffic quota.
- [x] Verify a non-admin reset is forbidden.
- [x] Verify admin reset returns a period object, clears usage summary actual bytes, and allows a new session.

## Task 2: DTOs And Persistence

**Files:**
- Modify: `crates/control-client/src/lib.rs`
- Modify: `crates/control/src/store.rs`

- [x] Add `UserUsagePeriod`.
- [x] Add `current_period_started_epoch_sec` to `UserUsageSummary`.
- [x] Add `user_usage_periods` to the Control snapshot with serde default.
- [x] Add `HttpControlClient::reset_user_usage_period`.

## Task 3: State And Routes

**Files:**
- Modify: `crates/control/src/state.rs`
- Modify: `crates/control/src/routes.rs`

- [x] Add `ControlState::reset_user_usage_period`.
- [x] Remove current usage reports for sessions initiated by the reset user.
- [x] Add admin-only `POST /usage/users/{user_id}/reset`.
- [x] Record an audit log entry for reset.

## Task 4: UI And Docs

**Files:**
- Modify: `docs/control-admin.html`
- Modify: `crates/mobile-core/tests/smoke_script.rs`
- Modify: `README.md`

- [x] Add usage reset controls to Control Admin.
- [x] Show usage period start in the usage table.
- [x] Document the reset endpoint and manual period behavior.

## Task 5: Verify

**Commands:**
- [x] `cargo test -p quic_tunnel_control --test control_plane admin_resets_user_usage_period_and_allows_new_sessions`
- [x] `cargo test -p quic_tunnel_control --test control_plane`
- [x] `cargo test -p quic_tunnel_control --test sqlite_store`
- [x] `cargo test -p quic_tunnel_mobile_core --test smoke_script`
- [x] `cargo test --workspace --no-run`
- [x] `cargo fmt --check`
