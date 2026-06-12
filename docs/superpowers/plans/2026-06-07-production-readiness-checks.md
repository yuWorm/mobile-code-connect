# Production Readiness Checks Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a repeatable release-readiness checklist and one-command validation script for the current MVP.

**Architecture:** Keep the release gate as a shell script that runs deterministic local checks by default and exposes opt-in flags for heavier workspace and E2E validation. Keep deployment and product gaps in a Markdown checklist so production hardening work can be tracked without changing runtime behavior.

**Tech Stack:** Bash, Cargo tests, existing smoke-script contract tests, Markdown docs.

---

### Task 1: Smoke Contract

**Files:**
- Modify: `crates/mobile-core/tests/smoke_script.rs`
- Create: `scripts/production-check.sh`
- Create: `docs/production-readiness.md`

- [x] **Step 1: Write the failing test**

Add a smoke contract that requires the production check script, checklist doc,
script executability, default Cargo checks, optional full/E2E gates, and
production security terms.

- [x] **Step 2: Run the test to verify it fails**

Run: `cargo test -p quic_tunnel_mobile_core --test smoke_script production_check_documents_release_readiness`

Expected: FAIL because `scripts/production-check.sh` does not exist yet.

### Task 2: Production Gate Script And Checklist

**Files:**
- Create: `scripts/production-check.sh`
- Create: `docs/production-readiness.md`
- Modify: `README.md`

- [x] **Step 1: Add `scripts/production-check.sh`**

Implement a Bash script that runs syntax checks, formatting, smoke contracts,
`mobile-cli` tests, and workspace compile checks by default. Add opt-in
environment flags for strict runtime environment checks, full workspace tests,
and the socket-binding E2E smoke script.

- [x] **Step 2: Add `docs/production-readiness.md`**

Document release gates, security, secrets, TLS, persistence, operations,
observability, billing/quota, backup, rollback, and known non-production gaps.

- [x] **Step 3: Link the checklist from `README.md`**

Add a short production readiness section with the one-command check.

### Task 3: Verify

**Files:**
- Test: `scripts/production-check.sh`
- Test: `crates/mobile-core/tests/smoke_script.rs`

- [x] **Step 1: Run targeted smoke contract**

Run: `cargo test -p quic_tunnel_mobile_core --test smoke_script production_check_documents_release_readiness`

- [x] **Step 2: Run script syntax check**

Run: `bash -n scripts/production-check.sh`

- [x] **Step 3: Run the production check script**

Run: `./scripts/production-check.sh`

- [x] **Step 4: Run full smoke contract and workspace compile gate**

Run: `cargo test -p quic_tunnel_mobile_core --test smoke_script`
Run: `cargo test --workspace --no-run`
