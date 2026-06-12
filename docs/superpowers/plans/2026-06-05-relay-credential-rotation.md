# Relay Credential Rotation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add persistent Relay credential records so Relay registration tokens can be disabled and rotated.

**Architecture:** Keep Relay node state (`/relays`) separate from Relay credential state (`/relay-credentials`). Control tokens for `ControlRole::Relay` now carry an optional `relay_token_version`; missing credential records still allow version `1` tokens for compatibility, while an existing credential record becomes authoritative for enabled state and token version. Rotating a credential increments the version and invalidates older Relay control tokens.

**Tech Stack:** Rust, Axum, existing HMAC Control token signer, SQLite JSON snapshot persistence, plain HTML admin page.

---

## Task 1: Red Tests

**Files:**
- Modify: `crates/control/tests/control_plane.rs`
- Modify: `crates/control/tests/sqlite_store.rs`

- [x] Add a route-level red test for admin-only Relay credential management.
- [x] Verify normal user tokens cannot list or create Relay credentials.
- [x] Verify admin can create and fetch a Relay credential.
- [x] Verify token version starts at `1`.
- [x] Verify rotating a credential increments the version and rejects the old Relay token.
- [x] Verify a new Relay token works after rotation.
- [x] Verify disabling a credential rejects token issuance and existing Relay tokens.
- [x] Verify missing credential rotation returns `404`.
- [x] Add SQLite restart coverage for credential enabled state and token version.

## Task 2: Shared Types And Token Claims

**Files:**
- Modify: `crates/auth/src/token.rs`
- Modify: `crates/auth/tests/token_roundtrip.rs`
- Modify: `crates/control-client/src/lib.rs`
- Modify: `crates/control/src/token_issuer.rs`

- [x] Add optional `relay_token_version` to `ControlTokenClaims` with serde default.
- [x] Preserve admin/user Control token roundtrip behavior.
- [x] Add `TokenIssuer::issue_relay_control_token`.
- [x] Add `RelayCredential`.
- [x] Add `CreateRelayCredentialRequest`.
- [x] Add `UpdateRelayCredentialStatusRequest`.
- [x] Add HTTP client methods for credential list/create/get/status/rotate.

## Task 3: Store And State

**Files:**
- Modify: `crates/control/src/store.rs`
- Modify: `crates/control/src/state.rs`

- [x] Add `relay_credentials` to the snapshot store with serde default.
- [x] Add `relay_credentials`.
- [x] Add `relay_credential`.
- [x] Add `create_relay_credential`.
- [x] Add `update_relay_credential_status`.
- [x] Add `rotate_relay_credential`.
- [x] Make `issue_relay_token` use stored token version when a credential exists.
- [x] Reject token issuance for disabled credentials.
- [x] Make Relay token validation enforce stored enabled state and token version.
- [x] Keep version `1` tokens valid when no credential record exists yet.

## Task 4: Routes, Admin Page, Docs

**Files:**
- Modify: `crates/control/src/routes.rs`
- Modify: `docs/control-admin.html`
- Modify: `crates/mobile-core/tests/smoke_script.rs`
- Modify: `README.md`

- [x] Add `GET /relay-credentials`.
- [x] Add `POST /relay-credentials`.
- [x] Add `GET /relay-credentials/{relay_id}`.
- [x] Add `POST /relay-credentials/{relay_id}/status`.
- [x] Add `POST /relay-credentials/{relay_id}/rotate`.
- [x] Require admin Control tokens for every credential route.
- [x] Add Relay credential controls to Control Admin.
- [x] Update static page contract tests.
- [x] Document credential APIs and version semantics.

## Task 5: Verify

**Commands:**
- [x] `cargo fmt`
- [x] `cargo test -p quic_tunnel_auth --test token_roundtrip`
- [x] `cargo test -p quic_tunnel_control --test control_plane`
- [x] `cargo test -p quic_tunnel_control --test sqlite_store`
- [x] `cargo test -p quic_tunnel_mobile_core --test smoke_script`
- [x] `cargo test -p control-server`
- [x] `cargo test -p relayd`
- [x] `cargo test --workspace --no-run`
- [x] `cargo fmt --check`
