# Agent-Issued Mobile Grants Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a QR/copy pairing path where server-agent issues long-lived revocable mobile grants, while Control only coordinates pending requests and creates sessions after agent approval.

**Architecture:** Add shared DTOs and HMAC proof helpers, then add Control pending pairing/session state and endpoints. Agent SDK/runtime owns invites and grants, mobile SDK stores the grant and uses it to create P2P-or-Relay sessions that carry browser proxy traffic.

**Tech Stack:** Rust workspace, Axum Control routes, `quic_tunnel_control_client`, `quic_tunnel_agent`, `quic_tunnel_mobile_core`, `quic_tunnel_sdk`, existing `hmac`/`sha2`/`base64`/`uuid`, Tokio tests.

---

### Task 1: Shared Grant DTOs And Proof Helpers

**Files:**
- Create: `crates/protocol/src/mobile_grant.rs`
- Modify: `crates/protocol/src/lib.rs`
- Modify: `crates/protocol/Cargo.toml`
- Test: `crates/protocol/src/mobile_grant.rs`

- [ ] **Step 1: Write failing tests**

Add tests for canonical HMAC proof and scope checks:

```rust
#[test]
fn pairing_proof_is_stable_and_secret_dependent() {
    let request = MobilePairingRequest::new_for_test();
    let proof = request.sign("secret-a");
    assert!(request.verify("secret-a", &proof));
    assert!(!request.verify("secret-b", &proof));
}

#[test]
fn grant_allows_only_scoped_services_and_matching_version() {
    let grant = MobileGrantCredential::new_for_test(vec![ServiceId::new("web")]);
    assert!(grant.allows(&ServiceId::new("web"), 1));
    assert!(!grant.allows(&ServiceId::new("ssh"), 1));
    assert!(!grant.allows(&ServiceId::new("web"), 2));
}
```

- [ ] **Step 2: Run red test**

Run: `cargo test -p quic_tunnel_protocol mobile_grant`

Expected: compile failure because the module and types do not exist.

- [ ] **Step 3: Implement DTOs and helpers**

Add serializable types for invite payloads, pairing requests, pairing responses, grant credentials, grant-session start/poll requests, and canonical proof helpers using `HMAC-SHA256`.

- [ ] **Step 4: Run green test**

Run: `cargo test -p quic_tunnel_protocol mobile_grant`

Expected: pass.

### Task 2: Control Pending Pairing And Grant Session APIs

**Files:**
- Modify: `crates/control-client/src/lib.rs`
- Modify: `crates/control/src/store.rs`
- Modify: `crates/control/src/state.rs`
- Modify: `crates/control/src/routes.rs`
- Test: `crates/control/tests/control_plane.rs`

- [ ] **Step 1: Write failing route tests**

Add tests that:

```rust
// mobile submits pairing without Authorization
POST /agent-grants/pairing/start -> 200 pending_pairing_id
// agent token lists and approves pending pairing
GET /agent/devices/{device_id}/pairing-requests -> includes request
POST /agent/pairing/{pending_pairing_id}/approve -> 200 approved grant metadata
// mobile polls result
GET /agent-grants/pairing/{pending_pairing_id} -> approved
```

Add grant-session tests:

```rust
POST /agent-grants/sessions/start -> 200 pending_session_id
GET /agent/devices/{device_id}/grant-session-requests -> includes request
POST /agent/grant-sessions/{pending_session_id}/approve -> 200 CreateSessionResponse
GET /agent-grants/sessions/{pending_session_id} -> approved CreateSessionResponse
```

- [ ] **Step 2: Run red tests**

Run: `cargo test -p quic_tunnel_control agent_grant`

Expected: 404/compile failures for missing routes/types.

- [ ] **Step 3: Implement in-memory Control state**

Store pending pairing and grant-session requests with status, expiry, device id, client id, service id, proof, and optional approved result. Do not store invite or grant secrets.

- [ ] **Step 4: Implement routes and client methods**

Add unauthenticated mobile start/poll endpoints and authenticated agent list/approve/deny endpoints. Reuse existing agent token authorization for device ownership.

- [ ] **Step 5: Run green tests**

Run: `cargo test -p quic_tunnel_control agent_grant`

Expected: pass.

### Task 3: Agent Invite And Grant Manager

**Files:**
- Create: `crates/agent/src/mobile_grant.rs`
- Modify: `crates/agent/src/lib.rs`
- Test: `crates/agent/tests/mobile_grant.rs`

- [ ] **Step 1: Write failing tests**

Tests cover:

```rust
let manager = MobileGrantManager::default();
let invite = manager.create_invite(scope, now, ttl);
let request = MobilePairingRequest::from_invite(&invite, "mobile_001");
let grant = manager.approve_pairing(request, now).unwrap();
assert!(manager.verify_session(&grant_session_request, now).is_ok());
manager.revoke_grant(&grant.grant_id).unwrap();
assert!(manager.verify_session(&grant_session_request, now).is_err());
```

- [ ] **Step 2: Run red test**

Run: `cargo test -p quic_tunnel_agent --test mobile_grant`

Expected: compile failure for missing manager.

- [ ] **Step 3: Implement in-memory manager**

Implement invite generation, pairing approval, grant storage, revocation, and grant-session proof verification. Keep file-backed storage out of this task.

- [ ] **Step 4: Run green test**

Run: `cargo test -p quic_tunnel_agent --test mobile_grant`

Expected: pass.

### Task 4: Agent Runtime Claim Enforcement

**Files:**
- Modify: `crates/control-client/src/lib.rs`
- Modify: `crates/agent/src/runtime.rs`
- Test: `crates/agent/tests/control_registration.rs`

- [ ] **Step 1: Write failing test**

Create a claimed grant session assignment with grant metadata and a revoked local grant; `AgentControlRuntime` must refuse to bind it.

- [ ] **Step 2: Run red test**

Run: `cargo test -p quic_tunnel_agent grant_session`

Expected: failure because assignment metadata is not enforced.

- [ ] **Step 3: Add grant metadata to `AgentSessionAssignment` and enforce it**

When metadata is present, agent runtime checks local grant store before starting P2P/Relay. Normal user-token sessions without grant metadata continue to work.

- [ ] **Step 4: Run green test**

Run: `cargo test -p quic_tunnel_agent grant_session`

Expected: pass.

### Task 5: Mobile SDK Grant Flow

**Files:**
- Modify: `crates/mobile-core/src/client.rs`
- Modify: `crates/mobile-core/src/ffi.rs`
- Modify: `crates/sdk/src/mobile.rs`
- Modify: `crates/sdk/src/lib.rs`
- Test: `crates/sdk/tests/mobile_tunnel_sdk.rs`

- [ ] **Step 1: Write failing SDK test**

Test that SDK imports an invite, completes pairing through a fake control API, stores a grant, and starts `start_with_control_p2p_or_relay_with_grant` without a user token.

- [ ] **Step 2: Run red test**

Run: `cargo test -p quic_tunnel_sdk mobile_grant`

Expected: compile failure for missing SDK APIs.

- [ ] **Step 3: Implement SDK APIs**

Add typed mobile grant config/store helpers and a grant-based `ControlP2pOrRelayStreamConnector` path that creates sessions through the new Control grant-session endpoints.

- [ ] **Step 4: Run green test**

Run: `cargo test -p quic_tunnel_sdk mobile_grant`

Expected: pass.

### Task 6: CLI And Documentation Smoke

**Files:**
- Modify: `apps/agentd/src/main.rs`
- Modify: `apps/mobile-cli/src/main.rs`
- Modify: `README.md`
- Test: `apps/mobile-cli/src/main.rs`
- Test: `crates/mobile-core/tests/smoke_script.rs`

- [ ] **Step 1: Write failing CLI tests**

Add parser tests for:

```text
agentd invite --service svc_web_3000
mobile-cli pair --invite <payload>
mobile-cli open-service --grant-file <path>
```

- [ ] **Step 2: Run red tests**

Run: `cargo test -p mobile-cli grant`

Expected: parser tests fail until commands exist.

- [ ] **Step 3: Implement minimal CLI and docs**

Agent prints invite payload; mobile imports invite, stores grant, and starts browser proxy using grant mode.

- [ ] **Step 4: Full verification**

Run:

```bash
cargo fmt --check
cargo test -p quic_tunnel_protocol
cargo test -p quic_tunnel_control
cargo test -p quic_tunnel_agent
cargo test -p quic_tunnel_mobile_core
cargo test -p quic_tunnel_sdk
cargo test -p mobile-cli
scripts/gen-mobile-bindings.sh --language all
scripts/package-mobile-ios.sh --dry-run --ios-min-version 17.0 --targets aarch64-apple-ios,aarch64-apple-ios-sim,x86_64-apple-ios --xcframework-output target/mobile-package-dry-run/ios/quic_tunnel_mobile_coreFFI.xcframework --staging-dir target/mobile-package-dry-run/ios
scripts/package-mobile-android.sh --dry-run --ndk-home /opt/android-ndk --ndk-host-tag linux-x86_64 --android-api 24 --gradle-task assembleRelease --aar-output-dir target/mobile-package-dry-run/android/aar --targets aarch64-linux-android,x86_64-linux-android --staging-dir target/mobile-package-dry-run/android
```

Expected: all pass. `scripts/gen-mobile-bindings.sh` may warn about missing `ktlint`; that warning is acceptable if generation succeeds.
