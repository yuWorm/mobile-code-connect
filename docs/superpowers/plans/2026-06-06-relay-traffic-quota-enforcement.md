# Relay Traffic Quota Enforcement Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Block new Control sessions after a user's reported actual Relay traffic reaches the current plan quota.

**Architecture:** Reuse Relay usage reports already stored in Control state. Session creation checks the initiating user's cumulative `actual_total_bytes` against `plan.relay_limits.traffic_quota_bytes` before auto-registering a controller or issuing session tokens. This is intentionally not a billing-period model yet.

**Tech Stack:** Rust, Axum routes, existing Control state store and usage reporting.

---

## Task 1: Red Test

**Files:**
- Modify: `crates/control/tests/control_plane.rs`

- [x] Add a failing test where a user with a 28-byte traffic quota can create the first session.
- [x] Report 28 bytes of actual Relay usage for that session.
- [x] Verify the next `POST /sessions` returns `402 Payment Required`.

## Task 2: Quota Check

**Files:**
- Modify: `crates/control/src/state.rs`
- Modify: `crates/control/src/routes.rs`

- [x] Add a helper that sums reported actual Relay total bytes for the session initiating user.
- [x] Add `ControlPlaneError::RelayTrafficQuotaExceeded`.
- [x] Check quota before controller auto-registration and token issuance.
- [x] Map quota exhaustion to `402 Payment Required`.

## Task 3: Docs

**Files:**
- Modify: `README.md`

- [x] Document the cumulative actual-usage enforcement behavior.
- [x] State that billing-period reset logic is not implemented yet.

## Task 4: Verify

**Commands:**
- [x] `cargo test -p quic_tunnel_control --test control_plane plan_relay_traffic_quota_blocks_new_sessions_after_actual_usage_exhausts_quota`
- [x] `cargo test -p quic_tunnel_control --test control_plane`
- [x] `cargo test -p quic_tunnel_control --test session_flow`
- [x] `cargo test -p quic_tunnel_control --test sqlite_store`
- [x] `cargo test -p quic_tunnel_mobile_core --test smoke_script`
- [x] `cargo test --workspace --no-run`
- [x] `cargo fmt --check`
