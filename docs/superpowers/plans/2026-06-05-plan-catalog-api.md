# Plan Catalog API Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an admin-only plan catalog so Control can manage reusable plan templates and assign a template to a user without rewriting the full plan payload each time.

**Architecture:** Store plan templates in the existing Control snapshot store beside per-user plans. Keep the active user plan as a copied `Plan` so existing session token signing and direct user-plan overrides keep working. Expose catalog and assignment routes only to admin Control tokens, and keep the simple zero-build Control Admin page aligned with the new API.

**Tech Stack:** Rust, Axum, existing Control state store, SQLite JSON snapshot persistence, plain HTML admin page.

---

## Task 1: Plan Catalog API Contract

**Files:**
- Modify: `crates/control-client/src/lib.rs`
- Test: `crates/control/tests/control_plane.rs`

- [x] Add red test for admin-only `GET /plans/catalog`.
- [x] Add red test for `POST /plans/catalog`.
- [x] Add red test for `GET /plans/catalog/{plan_id}`.
- [x] Add red test for `POST /plans/users/{user_id}/assign`.
- [x] Add `UpdatePlanCatalogRequest` and `AssignUserPlanRequest`.
- [x] Add `HttpControlClient` methods for catalog and assignment routes.

## Task 2: Store Plan Templates

**Files:**
- Modify: `crates/control/src/store.rs`
- Modify: `crates/control/src/state.rs`
- Test: `crates/control/tests/sqlite_store.rs`

- [x] Add `plan_catalog` to the snapshot store with a serde default.
- [x] Seed the default `free` plan template.
- [x] Validate and upsert catalog templates.
- [x] Return `404` for missing catalog plans.
- [x] Copy a catalog template into a user's active plan during assignment.
- [x] Persist catalog templates and assigned user plans through SQLite restart.

## Task 3: Admin-Only Routes

**Files:**
- Modify: `crates/control/src/routes.rs`
- Test: `crates/control/tests/control_plane.rs`

- [x] Add `GET /plans/catalog`.
- [x] Add `POST /plans/catalog`.
- [x] Add `GET /plans/catalog/{plan_id}`.
- [x] Add `POST /plans/users/{user_id}/assign`.
- [x] Require admin Control tokens for every catalog and assignment route.
- [x] Map `PlanNotFound` to HTTP `404`.

## Task 4: Simple Admin Page And Docs

**Files:**
- Modify: `docs/control-admin.html`
- Modify: `README.md`
- Modify: `crates/mobile-core/tests/smoke_script.rs`

- [x] Add a simple Plan Catalog panel to Control Admin.
- [x] Support load catalog, load template, save template, and assign template.
- [x] Sync assigned templates into the existing per-user plan editor.
- [x] Document plan catalog and template assignment endpoints.
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
