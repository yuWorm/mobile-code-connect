# GitHub OAuth Server Login Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement GitHub OAuth login and first-version server login for `agentd` using both browser and device-code UX paths.

**Architecture:** Control owns all GitHub OAuth exchange, account linking, server auth sessions, and server credential issuance. `agentd` never receives GitHub tokens; it receives a Control-issued Agent credential that is accepted only on agent-owned routes. Tests use a fake OAuth provider client so the implementation is deterministic and does not require network access.

**Tech Stack:** Rust, Axum, serde snapshot persistence, existing HMAC Control tokens, Clap, Tokio, existing Control client patterns.

---

## File Structure

- Modify `crates/auth/src/token.rs`: add `ControlRole::Agent` and version/device credential fields to Control token claims.
- Modify `crates/control/src/token_issuer.rs`: add Agent credential token issuance.
- Modify `crates/control-client/src/lib.rs`: add DTOs and client methods for OAuth starts, server auth, server credentials, and admin/user listing where needed.
- Modify `crates/control/src/store.rs`: add persisted OAuth identities, OAuth login sessions, server auth sessions, and server credentials with serde defaults.
- Modify `crates/control/src/state.rs`: implement OAuth account linking, server auth sessions, server credential issuance/status checks, and Agent credential validation.
- Modify `crates/control/src/routes.rs`: add OAuth/server auth routes and update agent route authorization.
- Modify `apps/control-server/src/main.rs`: add GitHub OAuth and public URL config.
- Modify `apps/agentd/src/main.rs`: add `login`, `login --device-code`, credential-file parsing/writing, and credential-file based Control run.
- Modify `README.md` and `docs/production-readiness.md`: document OAuth setup and server login.
- Modify `crates/mobile-core/tests/smoke_script.rs`: add static contract checks for the new docs and CLI strings.

## Chunk 1: Token And DTO Contracts

### Task 1: Add Agent Role To Control Tokens

**Files:**
- Modify: `crates/auth/src/token.rs`
- Modify: `crates/control/src/token_issuer.rs`
- Test: `crates/auth/tests/token_roundtrip.rs`

- [x] **Step 1: Write failing token roundtrip test**

Add a test that signs and verifies a Control token with role `Agent`, a
`credential_id`, and `server_credential_version`.

- [x] **Step 2: Run the test and verify it fails**

Run: `cargo test -p quic_tunnel_auth agent_control_token_preserves_credential_metadata`

Expected: FAIL because `ControlRole::Agent` and credential metadata do not exist.

- [x] **Step 3: Implement the token fields**

Add `Agent` to `ControlRole`, and add backward-compatible optional fields to
`ControlTokenClaims`:

```rust
#[serde(default)]
pub credential_id: Option<String>,
#[serde(default)]
pub server_credential_version: Option<u64>,
```

- [x] **Step 4: Add issuer helper**

Add `TokenIssuer::issue_agent_control_token(user_id, credential_id, version, exp)`.

- [x] **Step 5: Verify**

Run: `cargo test -p quic_tunnel_auth`
Run: `cargo test -p quic_tunnel_control --test control_plane --no-run`

### Task 2: Add Control Client DTOs

**Files:**
- Modify: `crates/control-client/src/lib.rs`

- [x] **Step 1: Add DTO tests for serde shape**

Add tests for serializing/deserializing:

- `OAuthProvider`
- `OAuthIdentity`
- `ServerAuthMode`
- `ServerAuthStatus`
- `StartServerAuthRequest`
- `BrowserServerAuthStartResponse`
- `DeviceServerAuthStartResponse`
- `PollServerAuthRequest`
- `ServerCredentialResponse`
- `ServerCredentialSummary`
- `UpdateServerCredentialStatusRequest`

- [x] **Step 2: Run tests and verify failure**

Run: `cargo test -p quic_tunnel_control_client oauth_server_auth_dtos_roundtrip`

Expected: FAIL because DTOs do not exist.

- [x] **Step 3: Implement DTOs**

Follow existing serde patterns in `crates/control-client/src/lib.rs`; use
snake_case enums and keep fields owned `String` values except protocol IDs.

- [x] **Step 4: Verify**

Run: `cargo test -p quic_tunnel_control_client`

## Chunk 2: Store And OAuth Account Linking

### Task 3: Persist OAuth Identities And Sessions

**Files:**
- Modify: `crates/control/src/store.rs`
- Modify: `crates/control/src/state.rs`
- Test: `crates/control/tests/sqlite_store.rs`

- [x] **Step 1: Write failing persistence test**

Create a SQLite state, upsert a GitHub OAuth identity linked to a user, reload
state, and assert the identity survives.

- [x] **Step 2: Run and verify failure**

Run: `cargo test -p quic_tunnel_control --test sqlite_store oauth_identity_persists_across_restart`

Expected: FAIL because store fields and state methods do not exist.

- [x] **Step 3: Add store fields**

Add serde-defaulted fields:

```rust
pub(crate) oauth_identities: HashMap<String, OAuthIdentity>,
pub(crate) oauth_login_sessions: HashMap<String, OAuthLoginSession>,
pub(crate) server_auth_sessions: HashMap<String, ServerAuthSession>,
pub(crate) server_credentials: HashMap<String, ServerCredential>,
```

- [x] **Step 4: Add state helpers**

Add normalized key helpers and state methods for upserting/finding OAuth
identities.

- [x] **Step 5: Verify**

Run: `cargo test -p quic_tunnel_control --test sqlite_store`

### Task 4: Implement Fakeable GitHub OAuth Login

**Files:**
- Create: `crates/control/src/oauth.rs`
- Modify: `crates/control/src/lib.rs`
- Modify: `crates/control/src/state.rs`
- Modify: `crates/control/src/routes.rs`
- Test: `crates/control/tests/control_plane.rs`

- [x] **Step 1: Write failing route tests**

Cover:

- `/auth/oauth/github/start` returns redirect when config exists.
- callback with fake GitHub identity creates a user and returns/logs in.
- second callback for same provider id returns the same user.
- unverified email is forbidden.

- [x] **Step 2: Run and verify failure**

Run: `cargo test -p quic_tunnel_control --test control_plane github_oauth`

Expected: FAIL because routes/config/provider do not exist.

- [x] **Step 3: Add OAuth module**

Define:

```rust
pub trait OAuthProviderClient {
    async fn exchange_code(&self, code: &str, verifier: &str) -> Result<OAuthAccessToken, OAuthError>;
    async fn user_profile(&self, access_token: &str) -> Result<OAuthUserProfile, OAuthError>;
    async fn primary_verified_email(&self, access_token: &str) -> Result<String, OAuthError>;
}
```

Use a fake implementation in tests. Do not call GitHub in route tests.

- [x] **Step 4: Add state login method**

Add `ControlState::login_or_create_oauth_user(provider_profile, verified_email)`.

- [x] **Step 5: Add routes**

Add `/auth/oauth/github/start` and `/auth/oauth/github/callback`.

- [x] **Step 6: Verify**

Run: `cargo test -p quic_tunnel_control --test control_plane github_oauth`
Run: `cargo test -p quic_tunnel_control --test sqlite_store`

## Chunk 3: Server Auth Sessions And Credentials

### Task 5: Browser Server Auth

**Files:**
- Modify: `crates/control/src/state.rs`
- Modify: `crates/control/src/routes.rs`
- Modify: `crates/control-client/src/lib.rs`
- Test: `crates/control/tests/control_plane.rs`

- [x] **Step 1: Write failing browser flow test**

Simulate:

1. start browser auth with device id, name, public key;
2. approve as logged-in GitHub user;
3. exchange one-time auth code;
4. receive Agent server credential;
5. replay exchange is rejected.

- [x] **Step 2: Run and verify failure**

Run: `cargo test -p quic_tunnel_control --test control_plane browser_server_auth_issues_agent_credential_once`

Expected: FAIL because server auth routes do not exist.

- [x] **Step 3: Implement session state**

Add `ControlState::start_browser_server_auth`, `approve_server_auth_session`,
and `exchange_browser_server_auth`.

- [x] **Step 4: Implement routes and client methods**

Routes:

- `POST /server-auth/browser/start`
- `GET /server-auth/browser/approve`
- `POST /server-auth/browser/exchange`

- [x] **Step 5: Verify**

Run: `cargo test -p quic_tunnel_control --test control_plane browser_server_auth`

### Task 6: Device-Code Server Auth

**Files:**
- Modify: `crates/control/src/state.rs`
- Modify: `crates/control/src/routes.rs`
- Modify: `crates/control-client/src/lib.rs`
- Test: `crates/control/tests/control_plane.rs`

- [x] **Step 1: Write failing device-code flow test**

Cover pending poll, approval, successful poll, slow_down when polling too fast,
expired_token, and access_denied.

- [x] **Step 2: Run and verify failure**

Run: `cargo test -p quic_tunnel_control --test control_plane device_code_server_auth`

Expected: FAIL because device-code routes do not exist.

- [x] **Step 3: Implement device-code session methods**

Add `start_device_server_auth`, `approve_device_server_auth`, and
`poll_device_server_auth`. Hash raw codes before persistence.

- [x] **Step 4: Implement routes and client methods**

Routes:

- `POST /server-auth/device/start`
- `GET /server-auth/device`
- `POST /server-auth/device/poll`

- [x] **Step 5: Verify**

Run: `cargo test -p quic_tunnel_control --test control_plane device_code_server_auth`

### Task 7: Server Credential Revocation And Listing

**Files:**
- Modify: `crates/control/src/state.rs`
- Modify: `crates/control/src/routes.rs`
- Modify: `crates/control-client/src/lib.rs`
- Test: `crates/control/tests/control_plane.rs`

- [x] **Step 1: Write failing credential management test**

Verify a user can list their server credentials, disable one, and that disabled
credentials cannot authenticate agent routes.

- [x] **Step 2: Run and verify failure**

Run: `cargo test -p quic_tunnel_control --test control_plane server_credentials_can_be_disabled`

- [x] **Step 3: Implement state and routes**

Routes:

- `GET /server-credentials`
- `POST /server-credentials/{credential_id}/status`

- [x] **Step 4: Add audit logs**

Record issue/status events.

- [x] **Step 5: Verify**

Run: `cargo test -p quic_tunnel_control --test control_plane server_credentials`

## Chunk 4: Agent Route Authorization

### Task 8: Accept Agent Credentials On Agent Routes

**Files:**
- Modify: `crates/control/src/routes.rs`
- Modify: `crates/control/src/state.rs`
- Test: `crates/control/tests/control_plane.rs`
- Test: `crates/mobile-core/tests/control_p2p_agent.rs`

- [x] **Step 1: Write failing auth-boundary test**

Verify:

- Agent credential can register/update only its own `device_id`.
- Agent credential can register services and P2P cert only for its device.
- User token cannot call agent routes in strict mode.
- Admin token can still operate where existing behavior requires admin override.

- [x] **Step 2: Run and verify failure**

Run: `cargo test -p quic_tunnel_control --test control_plane agent_credential_authorizes_only_own_device`

- [x] **Step 3: Add route helper**

Add `agent_or_admin_for_device_from_headers(state, headers, device_id)`.

- [x] **Step 4: Update route handlers**

Update agent register/services/p2p/session poll/claim/bind routes to use Agent
credential boundaries.

- [x] **Step 5: Verify**

Run: `cargo test -p quic_tunnel_control --test control_plane`
Run: `cargo test -p quic_tunnel_mobile_core --test control_p2p_agent --no-run`

## Chunk 5: Agentd Login UX

### Task 9: Add Agentd Subcommands And Credential File

**Files:**
- Modify: `apps/agentd/Cargo.toml`
- Modify: `apps/agentd/src/main.rs`
- Test: `apps/agentd/src/main.rs`

- [x] **Step 1: Write failing CLI parsing tests**

Cover:

- `agentd login --control ... --device ... --name ...`
- `agentd login --device-code --control ... --device ... --name ...`
- `agentd run --credential-file ...`
- existing direct Relay mode still parses.

- [x] **Step 2: Run and verify failure**

Run: `cargo test -p agentd login_args`

- [x] **Step 3: Convert CLI to subcommands**

Preserve existing no-subcommand behavior as `run` compatibility if needed, but
prefer explicit `run` and `login`.

- [x] **Step 4: Add credential-file read/write**

Write JSON with `0600` permissions on Unix.

- [x] **Step 5: Implement browser login flow**

Start browser auth, print/open `auth_url`, prompt for auth code, exchange, save
credential.

- [x] **Step 6: Implement device-code login flow**

Start device auth, print verification URI/user code, poll respecting interval,
save credential on success.

- [x] **Step 7: Verify**

Run: `cargo test -p agentd`

## Chunk 6: Config, Docs, And Dev Workflow

### Task 10: Control Server OAuth Config

**Files:**
- Modify: `apps/control-server/src/main.rs`
- Modify: `apps/control-server/Cargo.toml`
- Test: `apps/control-server/src/main.rs`

- [x] **Step 1: Write failing CLI/env parsing test**

Cover `--public-url`, `--github-client-id`, `--github-client-secret`, and
`--github-redirect-url`.

- [x] **Step 2: Run and verify failure**

Run: `cargo test -p control-server oauth_config`

- [x] **Step 3: Add config flags**

Add Clap/env args and pass config into `ControlState` or routes.

- [x] **Step 4: Verify**

Run: `cargo test -p control-server`

### Task 11: Docs And Smoke Contracts

**Files:**
- Modify: `README.md`
- Modify: `docs/production-readiness.md`
- Modify: `crates/mobile-core/tests/smoke_script.rs`
- Optional Modify: `scripts/dev-stack.sh`

- [x] **Step 1: Add failing static contract test**

Assert docs mention:

- GitHub OAuth setup env vars.
- `agentd login`.
- `agentd login --device-code`.
- `server credential`.
- Agent role boundaries.

- [x] **Step 2: Run and verify failure**

Run: `cargo test -p quic_tunnel_mobile_core --test smoke_script oauth_server_login_docs_are_current`

- [x] **Step 3: Update docs**

Document local fake/testing path and production GitHub OAuth setup.

- [x] **Step 4: Verify**

Run: `cargo test -p quic_tunnel_mobile_core --test smoke_script`

## Final Verification

- [x] `cargo fmt --check`
- [x] `cargo test -p quic_tunnel_auth`
- [x] `cargo test -p quic_tunnel_control_client`
- [x] `cargo test -p quic_tunnel_control --test control_plane`
- [x] `cargo test -p quic_tunnel_control --test sqlite_store`
- [x] `cargo test -p control-server`
- [x] `cargo test -p agentd`
- [x] `cargo test -p quic_tunnel_mobile_core --test smoke_script`
- [x] `cargo test --workspace --no-run`

## Notes

- Use `superpowers:test-driven-development` for each implementation task.
- Do not make network calls in tests; use a fake OAuth provider client.
- Keep existing dev-stack placeholder-token behavior until strict-auth paths are
  explicitly switched to credential-file based agent login.
- If a task gets too large, split it before coding rather than hiding behavior in
  one route/state change.
