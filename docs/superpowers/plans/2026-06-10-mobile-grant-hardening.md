# Mobile Grant Hardening Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Complete the next hardening round for agent-issued mobile grants: native pairing FFI, agent grant administration, P2P certificate binding, replay resistance, and a full grant browser proxy integration test.

**Architecture:** Keep Control as a secret-free broker. Server-agent owns invite/grant secrets and revocation state in its local grant file. Mobile derives grant secrets locally and uses P2P-first/Relay-fallback connectors; optional P2P certificate fingerprints bind grants to the intended server-agent identity.

**Tech Stack:** Rust, Tokio, Axum test servers, UniFFI, Clap, serde JSON, HMAC-SHA256, SHA-256.

---

## Chunk 1: Protocol And Grant State Hardening

### Task 1: Certificate Fingerprints And Grant Credential Field

**Files:**
- Modify: `crates/protocol/src/mobile_grant.rs`
- Modify tests: `crates/protocol/src/mobile_grant.rs`
- Modify downstream constructors in `crates/mobile-core`, `crates/sdk`, tests, and CLI JSON handling as required.

- [ ] Add failing protocol tests:
  - `mobile_grant_certificate_fingerprint_is_stable`
  - `mobile_grant_credential_deserializes_without_fingerprint`
- [ ] Run: `cargo test -p quic_tunnel_protocol mobile_grant_certificate_fingerprint_is_stable mobile_grant_credential_deserializes_without_fingerprint`
- [ ] Implement `mobile_grant_certificate_fingerprint(cert_der: &[u8]) -> String`.
- [ ] Add `#[serde(default)] pub agent_p2p_cert_fingerprint: Option<String>` to `MobileGrantCredential`.
- [ ] Update all credential constructors with `agent_p2p_cert_fingerprint`.
- [ ] Run: `cargo test -p quic_tunnel_protocol`.

### Task 2: Agent Grant Summaries, Revoke Invite, Replay State

**Files:**
- Modify: `crates/agent/src/mobile_grant.rs`
- Modify tests: `crates/agent/tests/mobile_grant.rs`

- [ ] Add failing tests:
  - `mobile_grant_manager_lists_and_revokes_invites_and_grants`
  - `mobile_grant_manager_rejects_reused_pairing_nonce_for_different_payload`
  - `mobile_grant_manager_reuses_grant_for_identical_pairing_retry`
  - `mobile_grant_manager_rejects_reused_session_nonce_for_different_payload`
- [ ] Run focused agent tests and confirm failure.
- [ ] Add `MobileInviteSummary`, `MobileGrantSummary`, `PairingApprovalRecord`, session nonce tracking to persisted state.
- [ ] Implement list/revoke helpers and idempotent pairing/session replay checks.
- [ ] Run: `cargo test -p quic_tunnel_agent --test mobile_grant`.

## Chunk 2: Agent CLI Administration And P2P Fingerprint Binding

### Task 3: Agentd Mobile Invite/Grant Commands

**Files:**
- Modify: `apps/agentd/src/main.rs`
- Modify tests: `apps/agentd/src/main.rs`
- Update docs: `README.md`

- [ ] Add failing parser/state tests for:
  - `mobile-invite create`
  - `mobile-invite list`
  - `mobile-invite revoke`
  - `mobile-grant list`
  - `mobile-grant revoke`
- [ ] Run focused `agentd` tests and confirm failure.
- [ ] Implement Clap subcommands using the same local `--mobile-grants-file`.
- [ ] Output JSON for create/list commands.
- [ ] Make `agentd run --p2p-identity-dir ... --mobile-invite-service ...` include the P2P fingerprint in printed invite.
- [ ] Run: `cargo test -p agentd`.

### Task 4: Mobile Connector Verifies Grant P2P Fingerprint

**Files:**
- Modify: `crates/mobile-core/src/forward.rs`
- Modify tests: `crates/mobile-core/tests/control_p2p_agent.rs` or focused connector test.

- [ ] Add failing test where grant has a fingerprint but Control returns a different `agent_p2p_cert_der`.
- [ ] Run focused mobile-core test and confirm failure.
- [ ] Verify approved grant sessions before opening stream; reject missing/mismatched P2P certificate when fingerprint is present.
- [ ] Run: `cargo test -p quic_tunnel_mobile_core control_p2p_agent`.

## Chunk 3: Native Pairing FFI

### Task 5: UniFFI Pairing Records And Functions

**Files:**
- Modify: `crates/mobile-core/src/ffi.rs`
- Modify tests: `crates/mobile-core/src/ffi.rs`
- Regenerate: `target/uniffi/...` via `scripts/gen-mobile-bindings.sh --language all`

- [ ] Add failing FFI tests for converting invite payloads, starting pairing, polling approved grant, and carrying fingerprint into `FfiMobileGrantCredential`.
- [ ] Run focused mobile-core FFI tests and confirm failure.
- [ ] Implement `FfiMobileInvitePayload`, pairing session/result/status records, `start_mobile_grant_pairing`, and `poll_mobile_grant_pairing_once`.
- [ ] Run: `cargo test -p quic_tunnel_mobile_core ffi::tests::mobile_grant_pairing`.
- [ ] Run: `scripts/gen-mobile-bindings.sh --language all`.
- [ ] Confirm generated Swift/Kotlin expose the new pairing functions.

## Chunk 4: End-To-End Grant Browser Proxy Test And Docs

### Task 6: Full Integration Test

**Files:**
- Modify: `crates/mobile-core/tests/control_p2p_agent.rs` or create `crates/mobile-core/tests/mobile_grant_browser_proxy.rs`
- Update: `README.md`

- [ ] Add failing e2e test for invite -> pair -> grant -> mobile tunnel -> browser proxy -> agent local HTTP service.
- [ ] Run focused test and confirm failure.
- [ ] Implement any missing glue revealed by the test.
- [ ] Update README with native pairing and agent grant admin examples.
- [ ] Run full verification:
  - `cargo fmt --check`
  - `cargo test -p quic_tunnel_protocol -p quic_tunnel_control -p quic_tunnel_agent -p quic_tunnel_mobile_core -p quic_tunnel_sdk -p agentd -p mobile-cli`
  - `scripts/package-mobile-ios.sh --dry-run ...`
  - `scripts/package-mobile-android.sh --dry-run ...`

