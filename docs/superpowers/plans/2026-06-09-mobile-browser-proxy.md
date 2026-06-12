# Mobile Browser Proxy Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add one SDK-managed browser proxy endpoint so mobile WebViews can reach dynamic device services without per-service forwarded ports.

**Architecture:** Add `crates/mobile-core/src/browser_proxy.rs` with a TCP proxy listener that maps synthetic hosts to existing `StreamConnector` streams. Expose it from `TunnelClient` and then from the UniFFI `FfiMobileTunnel` object as `start_browser_proxy(...)`.

**Tech Stack:** Rust 2021, Tokio TCP, existing `StreamConnector`, UniFFI records/objects.

---

## Chunk 1: Browser Proxy Core

### Task 1: Failing core tests

**Files:**
- Create: `crates/mobile-core/tests/browser_proxy.rs`
- Create: `crates/mobile-core/src/browser_proxy.rs`
- Modify: `crates/mobile-core/src/lib.rs`

- [x] Test host parsing for `<service>.<device>.qtunnel.local`.
- [x] Test HTTP absolute-form request forwarding rewrites to origin-form.
- [x] Test `CONNECT` returns `200 Connection Established` and tunnels bytes.
- [x] Test shutdown releases the proxy port.
- [x] Run `cargo test -p quic_tunnel_mobile_core --test browser_proxy` and confirm failures.

### Task 2: Implement core proxy

**Files:**
- Modify: `crates/mobile-core/src/browser_proxy.rs`
- Modify: `crates/mobile-core/src/lib.rs`

- [x] Add `BrowserProxy`, `BrowserProxyConfig`, `BrowserProxyHandle`, and host parser.
- [x] Accept TCP connections and parse HTTP request head.
- [x] Open `StreamConnector` with parsed device/service.
- [x] Rewrite absolute-form HTTP request line to origin-form.
- [x] Implement CONNECT passthrough.
- [x] Run `cargo test -p quic_tunnel_mobile_core --test browser_proxy`.

## Chunk 2: Client And UniFFI Surface

### Task 3: Failing FFI tests

**Files:**
- Modify: `crates/mobile-core/src/client.rs`
- Modify: `crates/mobile-core/src/ffi.rs`

- [x] Add tests for `FfiMobileTunnel::start_browser_proxy(...)`, returned host/port, status, close, and tunnel shutdown closing the proxy.
- [x] Run `cargo test -p quic_tunnel_mobile_core ffi::tests` and confirm failures.

### Task 4: Implement FFI surface

**Files:**
- Modify: `crates/mobile-core/src/client.rs`
- Modify: `crates/mobile-core/src/ffi.rs`

- [x] Add `TunnelClient::start_browser_proxy(...)`.
- [x] Add `FfiBrowserProxy` UniFFI object.
- [x] Add proxy tracking to `FfiMobileTunnel` so `shutdown()` closes active proxy handles.
- [x] Run focused FFI tests.

## Chunk 3: Docs And Verification

### Task 5: README and verification

**Files:**
- Modify: `README.md`

- [x] Document WebView proxy mode and `*.qtunnel.local` naming.
- [x] Run `cargo fmt --check`.
- [x] Run `cargo test -p quic_tunnel_mobile_core`.
- [x] Run `scripts/gen-mobile-bindings.sh --language all`.
- [x] Run `cargo test -p mobile-cli`.
- [x] Git commit is skipped because this workspace is not visible as a Git repository to the tool environment.
