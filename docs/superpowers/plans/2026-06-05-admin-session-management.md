# Admin Session Management Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a simple admin-only Control session management surface for listing and closing sessions.

**Architecture:** Reuse existing `AgentSessionAssignment` state. Add a read-only admin summary DTO that joins session assignments with user/device/service metadata, expose it through `GET /sessions`, and reuse the existing admin-authorized `POST /sessions/{session_id}/close` for closing.

**Tech Stack:** Rust, Axum, existing Control state store, plain HTML Control Admin.

---

## Task 1: Red Test

**Files:**
- Modify: `crates/control/tests/control_plane.rs`

- [x] Add failing test for admin-only `GET /sessions`.
- [x] Verify ordinary users receive `403`.
- [x] Verify summaries include session id, status, user, device, service, client, relay address, and expiry.
- [x] Verify admin can close a listed session.

## Task 2: Shared DTO

**Files:**
- Modify: `crates/control-client/src/lib.rs`

- [x] Add `AdminSessionSummary`.
- [x] Add `HttpControlClient::admin_sessions`.

## Task 3: State And Route

**Files:**
- Modify: `crates/control/src/state.rs`
- Modify: `crates/control/src/routes.rs`

- [x] Add `ControlState::admin_session_summaries`.
- [x] Join assignments to users/devices/services.
- [x] Add admin-only `GET /sessions`.

## Task 4: Admin Page And Contract

**Files:**
- Modify: `docs/control-admin.html`
- Modify: `crates/mobile-core/tests/smoke_script.rs`
- Modify: `README.md`

- [x] Add Sessions panel.
- [x] Add Load and Close controls.
- [x] Add static contract checks.
- [x] Document admin session endpoint.

## Task 5: Verify

**Commands:**
- [x] `cargo fmt`
- [x] `cargo test -p quic_tunnel_control --test control_plane`
- [x] `cargo test -p quic_tunnel_mobile_core --test smoke_script`
- [x] `cargo test --workspace --no-run`
- [x] `cargo fmt --check`
