# Mobile Grant Hardening Design

## Goal

Make the agent-issued mobile grant flow product-ready enough for native iOS/Android SDK usage: native pairing, agent-side grant administration, P2P identity binding, replay resistance, and one full end-to-end test path.

## Scope

This round covers the core Rust/UniFFI/CLI implementation. It does not implement platform Keychain/Keystore adapters yet; Swift/Kotlin callers will receive the grant credential and can store it with platform secure storage.

## Design

### Native Mobile Pairing API

`mobile-core` will expose UniFFI records and functions for the pairing lifecycle:

- `FfiMobileInvitePayload`
- `FfiMobileGrantPairingSession`
- `FfiMobileGrantPairingPollResult`
- `FfiMobileGrantPairingStatus`
- `start_mobile_grant_pairing(...)`
- `poll_mobile_grant_pairing_once(...)`

The native API mirrors the Rust SDK behavior: the mobile app scans/copies an invite, signs a pairing request locally with the invite secret, submits it to Control, polls the pending result, derives the grant secret locally, and receives `FfiMobileGrantCredential`. The FFI helper will not persist credentials; native apps decide where to store them.

### Agent Grant Administration

`MobileGrantManager` already persists local grant state. It will gain read and mutation helpers:

- list invite summaries
- list grant summaries
- revoke invite
- revoke grant

`agentd` will expose local administration commands backed by the same `--mobile-grants-file` state file. The running agent can continue approving requests from that file, while one-shot commands can create/list/revoke records.

### P2P Certificate Fingerprint Binding

Invites can include `agent_p2p_cert_fingerprint`. This will be populated when `agentd` has a P2P identity. The fingerprint is SHA-256 over DER certificate bytes, encoded with URL-safe base64 without padding.

`MobileGrantCredential` will carry the optional fingerprint forward. When a grant-based P2P-or-Relay session is approved, mobile-core will verify that Control returned an agent P2P certificate whose fingerprint matches the grant. If the fingerprint is present and missing/mismatched in the session, the mobile connector rejects the session before opening P2P.

Relay fallback remains available after P2P attempts fail, but fingerprint binding protects the P2P certificate identity when P2P is used.

### Replay Resistance And Idempotency

`MobileGrantManager` will track pairing approval keys and used session nonces in the local store.

Pairing approval is idempotent for the exact same request: if the same pending request is processed again because the agent failed to report approval to Control, the manager returns the same grant. If a nonce is reused with a different request payload, it is rejected.

Grant session verification rejects reused nonces for new requests. The exact same grant-session request is idempotent enough for agent retry handling: repeated verification of the same request returns success and does not create a local contradiction. Control still owns pending request IDs and session creation.

### End-To-End Test

Add an integration test that exercises:

1. agent creates invite with P2P fingerprint,
2. mobile pairs and stores a grant without user token,
3. mobile starts grant-based P2P-or-Relay tunnel,
4. browser proxy navigates to a typed device-service URL,
5. bytes reach an agent-local HTTP service through the tunnel,
6. transport stats prove P2P was attempted, and fallback behavior remains covered by existing relay tests.

## Error Handling

FFI returns existing `FfiMobileError` variants with clear messages. Agent CLI commands fail fast on missing grant files, unknown grant IDs, invalid invite scope, or missing P2P identity when a fingerprint is explicitly required.

## Testing

Use TDD for each behavior:

- protocol fingerprint helper and credential serde compatibility,
- agent grant list/revoke/replay/idempotency,
- agentd CLI parsing and state mutation,
- mobile-core FFI pairing helpers,
- P2P fingerprint mismatch rejection,
- full grant + browser proxy integration.

