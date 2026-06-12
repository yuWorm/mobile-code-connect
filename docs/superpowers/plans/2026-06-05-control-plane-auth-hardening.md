# Control Plane Auth Hardening Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the first production-oriented Control auth boundary: role-aware Control tokens, strict no-anonymous mode, and admin-only protection for global management APIs.

**Architecture:** Extend Control tokens with a `role` claim while keeping user tokens usable for normal user-scoped flows. `ControlState` owns auth policy through a `strict_auth` flag, preserving the local smoke stack's legacy anonymous `user_001` compatibility unless strict mode is enabled. Routes use small helper functions to require either an authenticated user or an admin role, avoiding a large route rewrite.

**Tech Stack:** Rust, Axum, existing HMAC token format, existing in-memory + SQLite snapshot Control state.

---

## Chunk 1: Token Roles And Strict Auth

### Task 1: Add Role Claims

**Files:**
- Modify: `crates/auth/src/token.rs`
- Modify: `crates/control/src/token_issuer.rs`
- Test: `crates/auth/tests/token_roundtrip.rs`

- [x] Add `ControlRole { User, Admin, Relay }` with serde `snake_case`.
- [x] Add `role: ControlRole` to `ControlTokenClaims`.
- [x] Change `TokenIssuer::issue_control_token` to accept a role.
- [x] Update register/login token issuance to use `ControlRole::User`.
- [x] Add/adjust token roundtrip tests for role preservation.

### Task 2: Add Strict Auth Mode

**Files:**
- Modify: `crates/control/src/state.rs`
- Modify: `apps/control-server/src/main.rs`
- Test: `crates/control/tests/control_plane.rs`

- [x] Add `strict_auth: bool` to `ControlState`.
- [x] Keep `ControlState::new` and `new_sqlite` in legacy-compatible mode by default.
- [x] Add `ControlState::with_strict_auth(bool) -> Self` builder.
- [x] Add `--strict-auth` / `QUIC_TUNNEL_STRICT_AUTH` to `control-server`.
- [x] In strict mode, missing `Authorization` returns unauthorized instead of default `user_001`.

## Chunk 2: Admin Authorization

### Task 3: Admin Token Issuance For Tests And Bootstrap

**Files:**
- Modify: `crates/control/src/state.rs`
- Modify: `apps/control-server/src/main.rs`
- Test: `apps/control-server/src/main.rs`

- [x] Add `ControlState::issue_admin_token(subject)`.
- [x] Add optional `--print-admin-token <subject>` command behavior to print an admin token and exit.
- [x] Keep server startup unchanged unless the flag is present.

### Task 4: Protect Global Management APIs

**Files:**
- Modify: `crates/control/src/routes.rs`
- Test: `crates/control/tests/control_plane.rs`

- [x] Add route helpers `user_id_from_headers` and `admin_from_headers`.
- [x] Leave user-scoped APIs as authenticated user:
  - `/controllers*`
  - `/devices*`
  - `/mobile/*`
  - `/agent/*`
  - `/sessions*`
- [x] Require admin for global management APIs:
  - `GET/POST /plans/users/{user_id}`
  - `POST /relays/register`
  - `GET /relays`
  - `GET/POST/DELETE /relays/{relay_id}`
- [x] Keep `/plans/current` user-scoped.
- [x] Add tests proving normal user tokens cannot update plans or relay pool, while admin tokens can.

## Chunk 3: Verification And Docs

### Task 5: Update Documentation And Scripts

**Files:**
- Modify: `README.md`
- Modify: `scripts/dev-stack.sh` if needed
- Modify: `scripts/e2e-smoke.sh` if needed

- [x] Document strict auth mode.
- [x] Document admin-token bootstrap command.
- [x] Keep smoke scripts on legacy-compatible mode unless explicitly enabled.

### Task 6: Verify

**Commands:**
- [x] `cargo fmt`
- [x] `cargo test -p quic_tunnel_auth`
- [x] `cargo test -p quic_tunnel_control --test control_plane --test sqlite_store`
- [x] `cargo test -p control-server`
- [x] `cargo test --workspace --no-run`
- [x] `cargo fmt --check`

**Expected:** Focused behavior tests pass. Full workspace compiles. Runtime listener tests may still need non-sandbox execution.
