# Device Access Grants Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a first-version authorization layer that lets admins grant users access to controlled devices they do not own.

**Architecture:** Store explicit `DeviceId -> UserId` grants in Control state and keep owner-only authority separate from controller access. Mobile/controller APIs can use owner-or-grant checks, while Agent-side claim/bind polling remains owner/admin only. Sessions record the initiating user so usage, Relay tokens, and close authorization stay attached to the controller user.

**Tech Stack:** Rust, Axum routes, serde snapshot persistence, plain HTML Control Admin.

---

## Task 1: Red Test

**Files:**
- Modify: `crates/control/tests/control_plane.rs`

- [x] Add failing test for admin granting device access.
- [x] Cover pre-grant denial, post-grant device/service/session access, Agent-side denial, close authorization, and revoke.

## Task 2: DTOs And Store

**Files:**
- Modify: `crates/control-client/src/lib.rs`
- Modify: `crates/control/src/store.rs`

- [x] Add `DeviceAccessGrant`.
- [x] Add `GrantDeviceAccessRequest`.
- [x] Add `device_access_grants` snapshot field with serde default.
- [x] Add `user_id` to `AgentSessionAssignment` with backward-compatible serde default.

## Task 3: State And Routes

**Files:**
- Modify: `crates/control/src/state.rs`
- Modify: `crates/control/src/routes.rs`

- [x] Add grant/list/revoke state methods.
- [x] Add owner-or-grant access checks for device listing, device detail, services, and session creation.
- [x] Keep Agent-side session polling/claim/bind owner/admin only.
- [x] Allow session initiator, device owner, or admin to close a session.
- [x] Add admin routes under `/devices/{device_id}/access`.
- [x] Record audit logs for grant and revoke.

## Task 4: UI And Docs

**Files:**
- Modify: `docs/control-admin.html`
- Modify: `crates/mobile-core/tests/smoke_script.rs`
- Modify: `README.md`

- [x] Add Control Admin fields/buttons/table for device access grants.
- [x] Add static smoke coverage for the new UI controls.
- [x] Document the device access API and authorization boundary.

## Task 5: Verify

**Commands:**
- [x] `cargo test -p quic_tunnel_control --test control_plane`
- [x] `cargo test -p quic_tunnel_control --test sqlite_store`
- [x] `cargo test -p quic_tunnel_mobile_core --test smoke_script`
- [x] `cargo test -p quic_tunnel_control --test session_flow`
- [x] `cargo test -p quic_tunnel_control --test control_client_flow --no-run`
- [x] `cargo test --workspace --no-run`
- [x] `cargo fmt --check`

Note: `control_client_flow` runtime tests bind local listeners, which this sandbox has previously rejected. The `--no-run` pass verifies the updated client code compiles.
