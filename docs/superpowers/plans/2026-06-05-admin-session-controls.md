# Admin Session Controls Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the zero-build Control Admin page clearer and safer around login state, token roles, and admin-only actions.

**Architecture:** Keep the existing single HTML page and server API unchanged. Decode Control token payloads in the browser for display only, add explicit user/admin token controls, and prevent admin-only UI actions from calling the API when no admin token is present.

**Tech Stack:** Plain HTML/CSS/JavaScript, existing mobile-core static contract test.

---

## Task 1: Red Test

**Files:**
- Modify: `crates/mobile-core/tests/smoke_script.rs`

- [x] Add static checks for current identity display.
- [x] Add static checks for separate user/admin logout controls.
- [x] Add static checks for user-token-to-admin promotion.
- [x] Add static checks for admin action token guard.

## Task 2: Admin Page UI

**Files:**
- Modify: `docs/control-admin.html`

- [x] Add current identity/status fields.
- [x] Add clear user token and clear admin token buttons.
- [x] Add button to copy a signed admin user token into the admin token field.
- [x] Improve header status to show decoded roles.

## Task 3: Admin Action Guard

**Files:**
- Modify: `docs/control-admin.html`

- [x] Add `requireToken(kind)` helper.
- [x] Use it before admin-only API calls in `run`.
- [x] Keep user-scoped actions unchanged.

## Task 4: Verify

**Commands:**
- [x] `cargo fmt`
- [x] `cargo test -p quic_tunnel_mobile_core --test smoke_script`
- [x] `cargo test --workspace --no-run`
- [x] `cargo fmt --check`
