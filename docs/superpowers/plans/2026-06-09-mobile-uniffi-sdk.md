# Mobile UniFFI SDK Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a UniFFI-compatible Rust facade for the existing mobile tunnel so iOS and Android can call the local forwarding SDK.

**Architecture:** Keep `apps/mobile-cli` as a test/debug binary. Add the foreign-language contract to `crates/mobile-core/src/ffi.rs`, backed by `TunnelClient` and an owned Tokio runtime. Use primitive DTOs at the FFI boundary and convert to internal Rust types inside the facade.

**Tech Stack:** Rust 2021, Tokio, UniFFI proc macros, existing `quic_tunnel_mobile_core`.

---

## Chunk 1: FFI Contract

### Task 1: Add failing lifecycle tests

**Files:**
- Modify: `crates/mobile-core/src/ffi.rs`

- [x] Add tests for FFI config conversion, in-memory tunnel lifecycle, status DTOs, handle DTOs, shutdown, and closed-tunnel errors.
- [x] Run `cargo test -p quic_tunnel_mobile_core ffi::tests` and confirm failure.

### Task 2: Implement the UniFFI facade

**Files:**
- Modify: `Cargo.toml`
- Modify: `crates/mobile-core/Cargo.toml`
- Modify: `crates/mobile-core/src/lib.rs`
- Modify: `crates/mobile-core/src/ffi.rs`

- [x] Add `uniffi` as a workspace dependency and set mobile-core crate types for Rust, iOS, and Android consumers.
- [x] Add `uniffi::setup_scaffolding!()` to `lib.rs`.
- [x] Implement UniFFI records, enums, flat error type, and `FfiMobileTunnel`.
- [x] Run the focused FFI tests and fix failures.

### Task 3: Verify existing consumers still compile

**Files:**
- Existing workspace files only.

- [x] Run `cargo test -p quic_tunnel_mobile_core`.
- [x] Run `cargo test -p mobile-cli`.
- [x] Document any environment or dependency blocker.

## Chunk 2: SDK Notes

### Task 4: Document binding generation commands

**Files:**
- Modify: `README.md`

- [x] Add short Mobile SDK notes for building the Rust library and generating Swift/Kotlin bindings with `uniffi-bindgen`.
- [x] Keep packaging as future work.
