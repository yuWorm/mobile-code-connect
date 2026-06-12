# Admin User Role Management Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add admin-only user creation and role management so the Control Admin page can manage user lifecycle without relying on public registration.

**Architecture:** Keep public `/auth/register` unchanged for self-service user registration. Add admin-only `/users` creation and `/users/{user_id}/role` update routes that return `UserSummary`, persist through the existing snapshot store, and only allow human account roles (`user` and `admin`). Relay credentials stay out of this user-role path.

**Tech Stack:** Rust, Axum, existing Control state store, SQLite JSON snapshot persistence, plain HTML admin page.

---

## Task 1: User Creation And Role API Contract

**Files:**
- Modify: `crates/control-client/src/lib.rs`
- Test: `crates/control/tests/control_plane.rs`

- [x] Add red test for normal user token rejection on `POST /users`.
- [x] Add red test for admin-created user with `admin` role.
- [x] Add red test for duplicate email returning `409`.
- [x] Add red test for created admin login returning an admin token.
- [x] Add red test for `POST /users/{user_id}/role`.
- [x] Add red test rejecting `relay` as a human account role.
- [x] Add `CreateUserRequest` and `UpdateUserRoleRequest`.
- [x] Add `HttpControlClient` methods for creating users and updating roles.

## Task 2: Control State Implementation

**Files:**
- Modify: `crates/control/src/state.rs`
- Test: `crates/control/tests/sqlite_store.rs`

- [x] Add `ControlState::create_user`.
- [x] Reuse email/password validation and normalized email storage.
- [x] Initialize created users with the default `free` plan.
- [x] Add `ControlState::update_user_role`.
- [x] Reject `ControlRole::Relay` for human user accounts.
- [x] Persist admin-created users and later role updates through SQLite restart.

## Task 3: Admin-Only Routes

**Files:**
- Modify: `crates/control/src/routes.rs`
- Test: `crates/control/tests/control_plane.rs`

- [x] Add `POST /users`.
- [x] Add `POST /users/{user_id}/role`.
- [x] Require admin Control tokens for both routes.
- [x] Map duplicate email to HTTP `409`.
- [x] Map missing users to HTTP `404`.
- [x] Map invalid human account roles to HTTP `400`.

## Task 4: Simple Admin Page And Docs

**Files:**
- Modify: `docs/control-admin.html`
- Modify: `README.md`
- Modify: `crates/mobile-core/tests/smoke_script.rs`

- [x] Add user creation fields to the Users panel.
- [x] Add role select and role update button.
- [x] Sync loaded user detail into the role/status fields.
- [x] Document admin user creation and role update payloads.
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
