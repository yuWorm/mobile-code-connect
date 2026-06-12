# Control Admin Console Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a usable Control Admin page for local and early production operations.

**Architecture:** Serve a static, zero-build HTML admin console from `control-server` through the existing Control router. The page calls the current Control API directly, stores user/admin tokens in browser local storage, and keeps global management actions behind an admin token. Dev scripts expose the page URL and a matching admin-token bootstrap command.

**Tech Stack:** Rust, Axum, existing Control API, plain HTML/CSS/JavaScript, dev-stack shell script.

---

## Task 1: Serve Control Admin Page

**Files:**
- Modify: `crates/control/src/routes.rs`
- Test: `crates/control/tests/control_plane.rs`

- [x] Add a route test for `GET /admin`.
- [x] Verify the test fails with `404`.
- [x] Add `/admin` and `/admin/` routes that return embedded HTML.
- [x] Verify the route test passes.

## Task 2: Build Zero-Dependency Admin Console

**Files:**
- Create: `docs/control-admin.html`
- Test: `crates/mobile-core/tests/smoke_script.rs`

- [x] Add login/register controls using `/auth/login` and `/auth/register`.
- [x] Add user token storage for user-scoped APIs.
- [x] Add admin token storage for global management APIs.
- [x] Add current/user plan loading and admin plan update controls.
- [x] Add controller list/register/remove controls.
- [x] Add controlled-device list/service/remove controls.
- [x] Add Relay pool list/register/update/delete controls.
- [x] Add a static page contract test for current API paths and DOM ids.

## Task 3: Expose Local Test Workflow

**Files:**
- Modify: `scripts/dev-stack.sh`
- Modify: `README.md`
- Test: `crates/mobile-core/tests/smoke_script.rs`

- [x] Add `./scripts/dev-stack.sh admin-token`.
- [x] Print the Control Admin URL in `status` and `run-all`.
- [x] Document the admin page URL and token bootstrap command.
- [x] Keep strict auth disabled in smoke/dev-stack scripts.
- [x] Verify the dev-stack script contract test passes.

## Task 4: Verify

**Commands:**
- [x] `cargo fmt`
- [x] `cargo test -p quic_tunnel_control --test control_plane`
- [x] `cargo test -p quic_tunnel_mobile_core --test smoke_script`
- [x] `cargo test --workspace --no-run`
- [x] `cargo fmt --check`
