# Relay Control Registration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let `relayd` optionally register itself into the Control Relay pool on startup.

**Architecture:** Keep Relay runtime unchanged and add startup registration in `apps/relayd`. The binary accepts Control URL, admin token, relay id, advertised addresses, and capacity. Dev-stack keeps the old startup path by default and adds an opt-in mode that starts Control before Relay so registration can succeed.

**Tech Stack:** Rust, Clap, existing `HttpControlClient`, shell dev-stack.

---

## Task 1: Relayd Registration Arguments

**Files:**
- Modify: `apps/relayd/src/main.rs`
- Modify: `apps/relayd/Cargo.toml`

- [x] Add a failing test for `--control-url`, `--control-token`, `--relay-id`, and `--capacity-streams`.
- [x] Add optional CLI fields for Control registration.
- [x] Add `RelayControlRegistration` request assembly.
- [x] Validate token, relay id, and positive capacity when Control registration is enabled.
- [x] Verify `cargo test -p relayd` passes for non-network tests.

## Task 2: Register With Control

**Files:**
- Modify: `apps/relayd/src/main.rs`

- [x] Add startup call using `HttpControlClient::register_relay`.
- [x] Use actual bound Relay/Admin addresses unless advertised addresses are provided.
- [x] Add a local TCP integration test for real Control registration.
- [x] Mark the local TCP integration test ignored in sandboxed environments.

## Task 3: Dev Stack And Docs

**Files:**
- Modify: `scripts/dev-stack.sh`
- Modify: `README.md`
- Modify: `crates/mobile-core/tests/smoke_script.rs`

- [x] Add `QUIC_TEST_RELAY_CONTROL_REGISTER=1` opt-in.
- [x] Start Control before Relay only when auto-registration is enabled.
- [x] Pass admin token and advertised Relay addresses to `relayd`.
- [x] Document the opt-in dev-stack command and manual relayd flags.
- [x] Update script contract tests.

## Task 4: Verify

**Commands:**
- [x] `cargo fmt`
- [x] `cargo test -p relayd`
- [x] `cargo test -p quic_tunnel_mobile_core --test smoke_script`
- [x] `cargo test -p quic_tunnel_control --test control_plane`
- [x] `cargo test --workspace --no-run`
- [x] `cargo fmt --check`

**Sandbox note:** `cargo test -p quic_tunnel_control --test control_client_flow` and the ignored `relayd_registers_itself_with_control` test require local TCP listeners. They were blocked by the current sandbox policy and should be run on the host or CI.
