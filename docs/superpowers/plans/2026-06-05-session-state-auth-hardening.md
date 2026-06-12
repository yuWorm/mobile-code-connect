# Session State Auth Hardening Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Require proper Control authorization for agent session list/claim/bound/close state changes.

**Architecture:** Keep existing session state storage. Add route-level bearer checks for agent session endpoints, then ask `ControlState` whether the caller can operate on the target device/session. User tokens are limited to their own controlled devices; admin tokens can inspect or mutate any session.

**Tech Stack:** Rust, Axum, existing Control token roles and state store.

---

## Task 1: Red Test

**Files:**
- Modify: `crates/control/tests/control_plane.rs`

- [x] Add failing test for anonymous session claim/close rejection under strict auth.
- [x] Add failing test for cross-user session list/claim/close rejection.
- [x] Verify owner user and admin still can operate.

## Task 2: State Helpers And Routes

**Files:**
- Modify: `crates/control/src/state.rs`
- Modify: `crates/control/src/routes.rs`

- [x] Add owner-aware agent session listing.
- [x] Add owner/admin authorization before claim, bound, and close.
- [x] Preserve existing dev anonymous behavior when strict auth is disabled and no bearer exists.

## Task 3: Verify

**Commands:**
- [x] `cargo fmt`
- [x] `cargo test -p quic_tunnel_control --test control_plane`
- [x] `cargo test -p quic_tunnel_control --test sqlite_store`
- [x] `cargo test --workspace --no-run`
- [x] `cargo fmt --check`
