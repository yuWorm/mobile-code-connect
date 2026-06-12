# Relay Health Reporting Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add relayd self-check health reporting and Control-side relay health snapshots.

**Architecture:** Keep the existing `/relays/{relay_id}` update route backward compatible and add `/relays/{relay_id}/health` for relayd health reports. Extend `RelayNode` with the latest snapshot. Relayd computes the report locally from listener state and session metrics, and relay admin exposes a small health endpoint for future active probes.

**Tech Stack:** Rust, Axum, Serde, Tokio, existing control-client DTOs, existing relay session store.

---

## Chunk 1: Control DTOs And State

### Task 1: Add relay health API types

**Files:**
- Modify: `crates/control-client/src/lib.rs`
- Modify: `web/src/lib/control/types.ts`

- [ ] Add `RelayHealthStatus`, `RelayHealthReport`, `RelayHealthSnapshot`, and `ReportRelayHealthRequest`.
- [ ] Add `HttpControlClient::report_relay_health`.
- [ ] Add snapshot fields to `RelayNode` while preserving `healthy`.

### Task 2: Add failing Control tests

**Files:**
- Modify: `crates/control/tests/control_plane.rs`

- [ ] Add a test that posts `/relays/{relay_id}/health` and verifies the relay list returns the status, reason, uptime, version, active sessions, active streams, and byte totals.
- [ ] Add a test that advances the relay health clock beyond timeout and verifies the returned relay is unhealthy/stale.

### Task 3: Implement Control health application

**Files:**
- Modify: `crates/control/src/state.rs`

- [ ] Initialize default health snapshot during relay registration.
- [ ] Apply health reports during relay health endpoint calls.
- [ ] Convert stale relays to effective `unhealthy` health status on reads.
- [ ] Keep existing `healthy` behavior backward compatible for callers without health reports.

## Chunk 2: Relayd Health Reports

### Task 4: Add failing relayd report tests

**Files:**
- Modify: `apps/relayd/src/main.rs`

- [ ] Add a unit test for health report construction from relay sessions.
- [ ] Verify data-plane failure reports `unhealthy` and a clear reason.

### Task 5: Implement relayd health report generation

**Files:**
- Modify: `apps/relayd/src/main.rs`

- [ ] Track `started_at`.
- [ ] Pass data-plane/admin listener flags into the heartbeat loop.
- [ ] Build `RelayHealthReport` from session snapshots on every heartbeat.
- [ ] Send the report in `UpdateRelayRequest`.

## Chunk 3: Relay Admin Health Endpoint

### Task 6: Add failing relay admin test

**Files:**
- Modify: `crates/relay/tests/admin.rs`

- [ ] Add a test for `GET /admin/health`.
- [ ] Verify session/stream/byte metrics are included.

### Task 7: Implement admin health endpoint

**Files:**
- Modify: `crates/relay/src/session.rs`
- Modify: `crates/relay/src/admin.rs`

- [ ] Add a `RelaySessionMetrics` summary helper to the session store.
- [ ] Add `RelayAdminHealth`.
- [ ] Route `GET /admin/health` to return metrics.

## Chunk 4: Docs And Verification

### Task 8: Update docs

**Files:**
- Modify: `docs/production-readiness.md`

- [ ] Document the relay health reporting behavior and remaining external probe considerations.

### Task 9: Verify

Run:
- `cargo fmt --check`
- `cargo test -p quic_tunnel_control relay_health`
- `cargo test -p relayd`
- `cargo test -p quic_tunnel_relay admin_health`
- `cargo test -p quic_tunnel_control_client`
- `cargo test -p quic_tunnel_sdk --test admin_sdk`
- `cargo test -p mobile-cli`
