# Mobile UniFFI SDK Optimization Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Improve the first UniFFI mobile SDK surface so Swift/Kotlin callers get cleaner lifecycle APIs and a repeatable binding generation command.

**Architecture:** Keep the existing `FfiMobileTunnel` object and add small compatibility-preserving helpers around it. Add one repository script that builds `quic_tunnel_mobile_core` and runs `uniffi-bindgen` for Swift/Kotlin using the verified metadata flags.

**Tech Stack:** Rust 2021, Tokio, UniFFI 0.31.1, shell script, existing mobile-core tests.

---

## Chunk 1: FFI API Polish

### Task 1: Add failing FFI API tests

**Files:**
- Modify: `crates/mobile-core/src/ffi.rs`

- [x] Add tests for `FfiMobileTunnelConfig::new(...)` defaults, `FfiP2pOrRelayConfig::with_defaults(...)`, `FfiMobileTunnel::start_with_control_relay(...)`, `is_closed()`, and `close_all_services()`.
- [x] Run `cargo test -p quic_tunnel_mobile_core ffi::tests` and confirm the tests fail because the new helpers do not exist.

### Task 2: Implement helper API

**Files:**
- Modify: `crates/mobile-core/src/ffi.rs`

- [x] Add UniFFI helpers for config defaults.
- [x] Add relay-only control constructor using `TunnelClient::start_with_control(...)`.
- [x] Add `is_closed()` and `close_all_services()` methods.
- [x] Keep existing constructors and methods intact.
- [x] Run `cargo test -p quic_tunnel_mobile_core ffi::tests`.

## Chunk 2: Binding Generation Script

### Task 3: Add script and docs

**Files:**
- Create: `scripts/gen-mobile-bindings.sh`
- Modify: `README.md`

- [x] Add a script that accepts `--language swift|kotlin|all`, `--library <path>`, and `--out-dir <dir>`.
- [x] Default to building `quic_tunnel_mobile_core --release` and generating both languages into `target/uniffi`.
- [x] Use `--metadata-no-deps --crate quic_tunnel_mobile_core`.
- [x] Update README to use the script while preserving direct command context.

### Task 4: Verify

**Files:**
- Existing project files only.

- [x] Run `cargo fmt --check`.
- [x] Run `cargo test -p quic_tunnel_mobile_core`.
- [x] Run `scripts/gen-mobile-bindings.sh --language all`.
- [x] Run `cargo test -p mobile-cli`.
- [x] Git commit is skipped because this workspace is not visible as a Git repository to the tool environment.
