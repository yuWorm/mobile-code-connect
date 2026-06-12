# Admin List Pagination Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace management list responses with a unified paginated envelope.

**Architecture:** Introduce `Page<T>` and one broad `AdminListQuery` DTO. Management list routes return `Page<T>` with `items`, `total`, `limit`, and `offset`. Filtering/sorting is applied in route/state helpers before paging. Runtime agent/mobile polling endpoints keep their current shapes.

**Tech Stack:** Rust, Axum query extractor, existing Control state store, plain HTML Control Admin.

---

## Task 1: Red Test

**Files:**
- Modify: `crates/control/tests/control_plane.rs`

- [x] Add failing test for `GET /users?q=...&role=...&limit=...&offset=...`.
- [x] Verify response envelope includes `items`, `total`, `limit`, and `offset`.

## Task 2: Shared DTO

**Files:**
- Modify: `crates/control-client/src/lib.rs`

- [x] Add `Page<T>`.
- [x] Add `AdminListQuery`.
- [x] Update management list client methods to return `Page<T>`.

## Task 3: Routes And State

**Files:**
- Modify: `crates/control/src/state.rs`
- Modify: `crates/control/src/routes.rs`

- [x] Convert management list routes to `Page<T>`.
- [x] Add filtering, sorting, and paging for users, sessions, usage, audit logs, relays, relay credentials, controllers, devices, and plan catalog.

## Task 4: Tests And Admin Page

**Files:**
- Modify: `crates/control/tests/control_plane.rs`
- Modify: `crates/control/tests/control_client_flow.rs`
- Modify: `docs/control-admin.html`
- Modify: `crates/mobile-core/tests/smoke_script.rs`
- Modify: `README.md`

- [x] Update tests to read `page.items`.
- [x] Update Control Admin render calls to read `page.items`.
- [x] Add shared query controls where already present or needed.
- [x] Document the new envelope and query params.

## Task 5: Verify

**Commands:**
- [x] `cargo fmt`
- [x] `cargo test -p quic_tunnel_control --test control_plane`
- [x] `cargo test -p quic_tunnel_control --test control_client_flow --no-run`
- [x] `cargo test -p quic_tunnel_control --test sqlite_store`
- [x] `cargo test -p quic_tunnel_mobile_core --test smoke_script`
- [x] `cargo test --workspace --no-run`
- [x] `cargo fmt --check`

Note: running `cargo test -p quic_tunnel_control --test control_client_flow`
without `--no-run` requires binding a local 127.0.0.1 listener. The sandbox
returned `PermissionDenied (Operation not permitted)`, and escalation was
rejected by policy.
