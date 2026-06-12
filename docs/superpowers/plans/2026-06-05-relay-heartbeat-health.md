# Relay Heartbeat Health Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Relay pool health reflect recent relayd heartbeats so Control stops assigning stale Relays.

**Architecture:** Add `last_seen_epoch_sec` to `RelayNode` and let Control compute effective health from the stored manual health flag plus a 90-second heartbeat timeout. `relayd` keeps startup self-registration and adds a periodic heartbeat using the existing admin-only Relay update API. The simple admin page shows last-seen time.

**Tech Stack:** Rust, Axum, existing `HttpControlClient`, plain HTML, shell dev-stack.

---

## Task 1: Control Health Aging

**Files:**
- Modify: `crates/control-client/src/lib.rs`
- Modify: `crates/control/src/state.rs`
- Test: `crates/control/tests/control_plane.rs`

- [x] Add `RelayNode.last_seen_epoch_sec` with serde default for old snapshots.
- [x] Stamp Relay registration and update with Control health time.
- [x] Add test clock helpers for deterministic heartbeat aging tests.
- [x] Mark Relays effectively unhealthy when last seen is older than 90 seconds.
- [x] Ensure stale Relays are skipped by session creation.

## Task 2: Relayd Heartbeat

**Files:**
- Modify: `apps/relayd/src/main.rs`

- [x] Add `--heartbeat-interval-sec`.
- [x] Keep default heartbeat interval at 30 seconds.
- [x] After Control registration, spawn a heartbeat task.
- [x] Heartbeat updates Relay address, admin address, capacity, and `healthy=true`.
- [x] Keep heartbeat failures non-fatal and retry on the next tick.

## Task 3: Dev Stack, Admin Page, Docs

**Files:**
- Modify: `scripts/dev-stack.sh`
- Modify: `docs/control-admin.html`
- Modify: `README.md`
- Modify: `crates/mobile-core/tests/smoke_script.rs`

- [x] Expose `QUIC_TEST_RELAY_HEARTBEAT_INTERVAL_SEC`.
- [x] Pass heartbeat interval to relayd when Relay self-registration is enabled.
- [x] Show Relay last-seen time in the simple Control Admin page.
- [x] Document heartbeat interval and 90-second Control stale timeout.
- [x] Update script/page contract tests.

## Task 4: Verify

**Commands:**
- [x] `cargo fmt`
- [x] `cargo test -p relayd`
- [x] `cargo test -p quic_tunnel_control --test control_plane --test sqlite_store`
- [x] `cargo test -p quic_tunnel_mobile_core --test smoke_script`
- [x] `cargo test --workspace --no-run`
- [x] `cargo fmt --check`

**Sandbox note:** `relayd_registers_itself_with_control` remains ignored because it binds a local TCP listener and should be run on the host or CI.
