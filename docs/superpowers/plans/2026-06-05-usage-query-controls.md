# Usage Query Controls Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add lightweight query controls to `GET /usage/users` so admins can page and sort usage summaries without changing the existing response shape.

**Architecture:** Keep `GET /usage/users` returning `Vec<UserUsageSummary>` for compatibility. Add optional query params parsed at the route boundary, then reuse the existing summary aggregation and apply sort/offset/limit in `ControlState`. The admin page builds a query string from simple form controls.

**Tech Stack:** Rust, Axum query extractor, existing Control client, plain HTML admin page.

---

## Task 1: Red Test

**Files:**
- Modify: `crates/control/tests/control_plane.rs`

- [x] Add a failing test for `GET /usage/users?sort=actual_total_bytes&limit=2&offset=1`.
- [x] Verify admin-only behavior remains unchanged.
- [x] Verify default no-query behavior still works.

## Task 2: Shared Query Type

**Files:**
- Modify: `crates/control-client/src/lib.rs`

- [x] Add `UserUsageListQuery`.
- [x] Add `HttpControlClient::user_usage_summaries_with_query`.

## Task 3: State And Route

**Files:**
- Modify: `crates/control/src/state.rs`
- Modify: `crates/control/src/routes.rs`

- [x] Add query-aware state method.
- [x] Sort by `email`, `actual_total_bytes`, `relay_quota_granted_bytes`, and `session_count`.
- [x] Apply `offset` and `limit`.
- [x] Keep existing no-query path compatible.

## Task 4: Admin Page And Docs

**Files:**
- Modify: `docs/control-admin.html`
- Modify: `crates/mobile-core/tests/smoke_script.rs`
- Modify: `README.md`

- [x] Add Usage sort/limit/offset controls.
- [x] Add static contract checks.
- [x] Document query params.

## Task 5: Verify

**Commands:**
- [x] `cargo fmt`
- [x] `cargo test -p quic_tunnel_control --test control_plane`
- [x] `cargo test -p quic_tunnel_mobile_core --test smoke_script`
- [x] `cargo test -p control-server`
- [x] `cargo test --workspace --no-run`
- [x] `cargo fmt --check`
