# Persistent Admin Users Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make admin access a persisted Control user role instead of only an ephemeral bootstrap token.

**Architecture:** Store a `ControlRole` on `UserAccount` with serde default `User` for old snapshots. Normal registration creates user-role accounts. Control startup can bootstrap or update a persistent admin user through CLI/env, and `/auth/login` returns a signed Control token with the account's persisted role.

**Tech Stack:** Rust, existing HMAC Control tokens, SQLite JSON snapshot persistence, Clap/env config, plain HTML admin page.

---

## Task 1: Persist User Roles

**Files:**
- Modify: `crates/control/src/store.rs`
- Modify: `crates/control/src/state.rs`
- Test: `crates/control/tests/sqlite_store.rs`

- [x] Add red tests for admin role persistence and normal user role tokens.
- [x] Add `UserAccount.role` with serde default `ControlRole::User`.
- [x] Issue login tokens using the persisted account role.
- [x] Keep `/auth/register` creating `ControlRole::User` accounts.

## Task 2: Bootstrap Admin User

**Files:**
- Modify: `crates/control/src/state.rs`
- Test: `crates/control/tests/sqlite_store.rs`

- [x] Add `ControlState::bootstrap_admin_user`.
- [x] Create an admin account if the email does not exist.
- [x] Upgrade/update an existing account to admin and reset its password.
- [x] Persist the admin account through SQLite snapshot reload.

## Task 3: CLI, Dev Stack, Admin Page

**Files:**
- Modify: `apps/control-server/src/main.rs`
- Modify: `apps/control-server/Cargo.toml`
- Modify: `scripts/dev-stack.sh`
- Modify: `docs/control-admin.html`
- Modify: `README.md`
- Modify: `crates/mobile-core/tests/smoke_script.rs`

- [x] Add `--bootstrap-admin-email` / `QUIC_TUNNEL_ADMIN_EMAIL`.
- [x] Add `--bootstrap-admin-password` / `QUIC_TUNNEL_ADMIN_PASSWORD`.
- [x] Bootstrap admin before serving or printing tokens.
- [x] Add dev-stack envs for persistent admin startup.
- [x] Let the simple admin page auto-copy admin login tokens into the admin token box.
- [x] Document persistent admin login flow.

## Task 4: Verify

**Commands:**
- [x] `cargo fmt`
- [x] `bash -n scripts/dev-stack.sh`
- [x] `cargo test -p quic_tunnel_control --test sqlite_store`
- [x] `cargo test -p control-server`
- [x] `cargo test -p quic_tunnel_mobile_core --test smoke_script`
- [x] `cargo test -p quic_tunnel_control --test control_plane`
- [x] `cargo test --workspace --no-run`
- [x] `cargo fmt --check`
