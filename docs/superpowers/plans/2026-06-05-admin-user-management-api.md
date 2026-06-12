# Admin User Management API Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the first admin-only user management API for listing, inspecting, enabling, and disabling Control users.

**Architecture:** Extend the shared Control API model with user summary/detail/status request types. Store user profile metadata and enabled state on `UserAccount`, while preserving old snapshot compatibility through serde defaults. Routes stay small and admin-only; disabled users cannot log in or continue using existing bearer tokens.

**Tech Stack:** Rust, Axum, existing Control state store, SQLite JSON snapshot persistence, plain HTML admin page.

---

## Task 1: User Management API Contract

**Files:**
- Modify: `crates/control-client/src/lib.rs`
- Modify: `crates/control-client/Cargo.toml`
- Test: `crates/control/tests/control_plane.rs`

- [x] Add red test for admin-only `GET /users`.
- [x] Add red test for `GET /users/{user_id}` detail.
- [x] Add red test for `POST /users/{user_id}/status`.
- [x] Add `UserSummary`, `UserDetail`, and `UpdateUserStatusRequest`.
- [x] Add `HttpControlClient` methods for user management.

## Task 2: Store User Profile And Enabled State

**Files:**
- Modify: `crates/control/src/store.rs`
- Modify: `crates/control/src/state.rs`

- [x] Add `email`, `display_name`, and `enabled` to `UserAccount` with serde defaults.
- [x] Persist email/display name on normal registration.
- [x] Persist email/display name on admin bootstrap.
- [x] Reject disabled users during login.
- [x] Reject disabled users during bearer-token authentication.

## Task 3: Admin-Only Routes

**Files:**
- Modify: `crates/control/src/routes.rs`
- Test: `crates/control/tests/control_plane.rs`

- [x] Add `GET /users`.
- [x] Add `GET /users/{user_id}`.
- [x] Add `POST /users/{user_id}/status`.
- [x] Return `403` for normal user tokens.
- [x] Return `404` for missing users through the existing `UserNotFound` mapping.

## Task 4: Simple Admin Page And Docs

**Files:**
- Modify: `docs/control-admin.html`
- Modify: `README.md`
- Modify: `crates/mobile-core/tests/smoke_script.rs`

- [x] Add a simple Users panel to Control Admin.
- [x] Support load users, load detail, enable, and disable.
- [x] Sync selected user detail into the existing plan editor.
- [x] Document user management endpoints as admin-only.
- [x] Update static page contract tests.

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
