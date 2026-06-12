# Relay Registration Token Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let Relay self-registration and heartbeat use a dedicated `relay` Control token instead of reusing an admin token.

**Architecture:** Keep admin Relay pool management intact, but allow `ControlRole::Relay` tokens to call only `POST /relays/register` and `POST /relays/{relay_id}` for the Relay id carried in the token subject. Relay tokens are printed by `control-server --print-relay-token <relay_id>` and consumed by the existing `relayd --control-token` path. User-scoped and admin management APIs reject Relay tokens.

**Tech Stack:** Rust, Axum, existing Control token signer, control-server CLI, relayd CLI, dev-stack Bash script.

---

## Task 1: Relay Role Permission Contract

**Files:**
- Modify: `crates/control/tests/control_plane.rs`

- [x] Add red test for `ControlState::issue_relay_token`.
- [x] Verify a relay token can `POST /relays/register` for its own Relay id.
- [x] Verify a relay token can heartbeat with `POST /relays/{relay_id}` for its own Relay id.
- [x] Verify a relay token cannot register or update another Relay id.
- [x] Verify a relay token cannot access `/users`, `/relays`, or `/controllers`.
- [x] Verify an admin token can still inspect the registered Relay.

## Task 2: Control Token And Route Enforcement

**Files:**
- Modify: `crates/control/src/state.rs`
- Modify: `crates/control/src/routes.rs`

- [x] Add `ControlState::issue_relay_token`.
- [x] Sign relay registration tokens with `ControlRole::Relay` and subject equal to the Relay id.
- [x] Reject empty Relay ids during token issuance.
- [x] Add route helper that accepts admin tokens or same-id relay tokens.
- [x] Use that helper for `POST /relays/register` and `POST /relays/{relay_id}`.
- [x] Reject `ControlRole::Relay` from user-scoped helpers.

## Task 3: CLI And Dev Stack

**Files:**
- Modify: `apps/control-server/src/main.rs`
- Modify: `apps/relayd/src/main.rs`
- Modify: `scripts/dev-stack.sh`
- Modify: `crates/mobile-core/tests/smoke_script.rs`

- [x] Add red CLI parse test for `--print-relay-token`.
- [x] Add `control-server --print-relay-token <relay_id>`.
- [x] Reject simultaneous admin-token and relay-token print modes.
- [x] Keep `relayd --control-token` unchanged as the token input.
- [x] Update ignored relayd integration test to register with a relay token.
- [x] Add `./scripts/dev-stack.sh relay-token`.
- [x] Make dev-stack Relay self-registration use relay tokens.
- [x] Update static script tests.

## Task 4: Docs

**Files:**
- Modify: `README.md`

- [x] Document `--print-relay-token`.
- [x] Document `./scripts/dev-stack.sh relay-token`.
- [x] Document that Relay register/heartbeat accepts admin or matching relay token.
- [x] Update manual relayd self-registration example to use `$RELAY_TOKEN`.

## Task 5: Verify

**Commands:**
- [x] `cargo fmt`
- [x] `cargo test -p quic_tunnel_control --test control_plane`
- [x] `cargo test -p control-server`
- [x] `cargo test -p relayd`
- [x] `cargo test -p quic_tunnel_mobile_core --test smoke_script`
- [x] `cargo test --workspace --no-run`
- [x] `cargo fmt --check`
- [x] `cargo test -p quic_tunnel_control --test sqlite_store`
