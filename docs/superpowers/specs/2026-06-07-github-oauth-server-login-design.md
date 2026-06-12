# GitHub OAuth And Server Login Design

## Goal

Add a first production-oriented OAuth path for the Control plane:

- users can sign in or be created through GitHub OAuth;
- `agentd` can bind a controlled server to a user through either browser login
  or device-code login;
- both server login UX paths produce the same Control-issued server credential;
- GitHub tokens and OAuth secrets stay inside Control.

## Non-Goals

- Multiple OAuth providers.
- Payment, subscription, or billing-provider integration.
- Final user-facing dashboard UI.
- Controller-device self-service sharing UX.
- Refresh-token rotation for server credentials. The first version can issue a
  revocable long-lived Control token and store credential metadata.
- Public deployment manifests.

## Current Context

Control already has password-based `/auth/register` and `/auth/login`, role-aware
Control tokens, persistent users, admin management APIs, controlled devices,
device access grants, Relay credentials, and SQLite snapshot persistence.
`agentd` currently accepts `--agent-token` and uses it to register a controlled
device and poll Control. The local dev stack still uses placeholder tokens in
non-strict mode.

The OAuth/server-login work should keep these surfaces intact while adding a
new credential path.

## Design Choice

Use Control-mediated OAuth:

```text
GitHub OAuth only happens in Control.
agentd never receives a GitHub access token.
agentd receives only a Control-issued server credential.
```

This keeps the GitHub client secret, GitHub access token exchange, account
linking, auditing, plan limits, and credential revocation centralized.

## External OAuth Behavior

The implementation follows GitHub OAuth App behavior:

- Web application flow for browser login.
- Device flow for CLI/headless login.
- GitHub user identity comes from the authenticated user profile plus verified
  primary email lookup.

Reference docs:

- GitHub OAuth Apps authorization:
  `https://docs.github.com/apps/oauth-apps/building-oauth-apps/authorizing-oauth-apps`
- GitHub REST API emails:
  `https://docs.github.com/rest/users/emails`

## Data Model

### OAuthIdentity

Stored in the Control snapshot:

```rust
pub struct OAuthIdentity {
    pub provider: OAuthProvider,        // github
    pub provider_user_id: String,       // GitHub numeric id as string
    pub user_id: UserId,
    pub email: String,
    pub login: String,
    pub avatar_url: String,
    pub created_epoch_sec: u64,
    pub updated_epoch_sec: u64,
}
```

Indexes in the snapshot:

- `oauth_identities: HashMap<String, OAuthIdentity>`
- key format: `"{provider}:{provider_user_id}"`

### ServerAuthSession

Stored in the Control snapshot with a TTL:

```rust
pub struct ServerAuthSession {
    pub session_id: String,
    pub mode: ServerAuthMode,           // browser | device_code
    pub status: ServerAuthStatus,       // pending | approved | denied | expired | consumed
    pub device_name: String,
    pub server_public_key: String,
    pub user_code_hash: Option<String>,
    pub device_code_hash: Option<String>,
    pub auth_code_hash: Option<String>,
    pub approved_user_id: Option<UserId>,
    pub poll_interval_sec: u64,
    pub expires_epoch_sec: u64,
    pub created_epoch_sec: u64,
    pub updated_epoch_sec: u64,
}
```

Hash all user/device/auth codes before persistence. Raw codes are returned once.

### ServerCredential

Stored in the Control snapshot:

```rust
pub struct ServerCredential {
    pub credential_id: String,
    pub user_id: UserId,
    pub device_id: DeviceId,
    pub device_name: String,
    pub server_public_key: String,
    pub enabled: bool,
    pub token_version: u64,
    pub created_epoch_sec: u64,
    pub last_used_epoch_sec: Option<u64>,
}
```

The first version can use a signed Control token with role `Agent`, a
`credential_id`, and `server_credential_version`. Revocation disables the
credential or increments `token_version`.

## Token Model

Add one new Control role:

```rust
pub enum ControlRole {
    User,
    Admin,
    Relay,
    Agent,
}
```

Agent tokens are allowed to:

- register/update their own controlled device;
- register/update services for their own device;
- upload their own P2P certificate;
- poll/claim/bind sessions for their own device.

Agent tokens are not allowed to:

- call admin APIs;
- list user devices as a controller;
- access another server credential;
- manage Relay credentials.

The auth check should validate:

- token role is `Agent`;
- credential exists;
- credential is enabled;
- token version matches;
- token device id matches the target device route.

## User OAuth Flow

### Start

```http
GET /auth/oauth/github/start?redirect_uri=<optional>
```

Control generates `state`, PKCE verifier/challenge, stores a short-lived OAuth
login session, and redirects to GitHub.

### Callback

```http
GET /auth/oauth/github/callback?code=...&state=...
```

Control validates state, exchanges code with GitHub, fetches the GitHub user and
verified primary email, then:

- finds an existing `OAuthIdentity`, or
- links to an existing user by verified email, or
- creates a new user account with no password login.

Control returns the existing `AuthResponse` shape so current clients can keep
using `access_token`.

## Server Browser Login Flow

### Start

```http
POST /server-auth/browser/start
```

Request:

```json
{
  "device_id": "pc_001",
  "device_name": "Office PC",
  "server_public_key": "base64url..."
}
```

Response:

```json
{
  "session_id": "srv_auth_...",
  "auth_url": "https://control.example.com/server-auth/browser/approve?session_id=...",
  "expires_in": 600
}
```

`agentd login` opens `auth_url` in a browser or prints it if browser opening is
not available.

### Approval

User signs into Control through GitHub OAuth, sees the requested server name,
and approves binding. Control marks the session approved and produces a raw
one-time `server_auth_code` for the browser page to display.

### Exchange

```http
POST /server-auth/browser/exchange
```

Request:

```json
{
  "session_id": "srv_auth_...",
  "server_auth_code": "raw-code",
  "server_public_key": "base64url..."
}
```

Response:

```json
{
  "credential_id": "srv_cred_...",
  "device_id": "pc_001",
  "server_token": "control-token",
  "token_type": "bearer"
}
```

After exchange, the auth session becomes `consumed`.

## Server Device-Code Login Flow

### Start

```http
POST /server-auth/device/start
```

Request is the same server metadata as browser start.

Response:

```json
{
  "device_code": "raw-device-code",
  "user_code": "ABCD-EFGH",
  "verification_uri": "https://control.example.com/server-auth/device",
  "verification_uri_complete": "https://control.example.com/server-auth/device?user_code=ABCD-EFGH",
  "expires_in": 600,
  "interval": 5
}
```

`agentd login --device-code` prints the URL and code. A QR code can be added
later; the first version may print only the complete URI.

### User Approval

The user opens the verification URI, signs in through GitHub OAuth, enters or
confirms the user code, reviews server metadata, and approves or denies.

### Poll

```http
POST /server-auth/device/poll
```

Request:

```json
{
  "device_code": "raw-device-code",
  "server_public_key": "base64url..."
}
```

Responses:

- `authorization_pending`: still waiting.
- `slow_down`: caller polled too quickly; increase interval.
- `expired_token`: session expired.
- `access_denied`: user denied.
- success: same `ServerCredentialResponse` as browser exchange.

## Agentd UX

Add subcommands:

```bash
agentd login --control https://control.example.com --device pc_001 --name "Office PC"
agentd login --device-code --control https://control.example.com --device pc_001 --name "Office PC"
agentd run --control https://control.example.com --credential-file ~/.quic-test/agentd.json ...
```

For compatibility, existing direct run mode can keep `--agent-token`, but the
recommended strict-auth path should be credential-file based.

Credential file:

```json
{
  "control_url": "https://control.example.com",
  "credential_id": "srv_cred_...",
  "device_id": "pc_001",
  "server_token": "control-token"
}
```

On Unix, write the file with `0600`.

## API And Route Ownership

New user-facing routes:

- `GET /auth/oauth/github/start`
- `GET /auth/oauth/github/callback`
- `GET /server-auth/browser/approve`
- `GET /server-auth/device`

New JSON routes:

- `POST /server-auth/browser/start`
- `POST /server-auth/browser/exchange`
- `POST /server-auth/device/start`
- `POST /server-auth/device/poll`
- `GET /server-credentials`
- `POST /server-credentials/{credential_id}/status`

Existing agent routes should switch from generic user auth to agent-or-admin
authorization where appropriate:

- `/agent/register`
- `/agent/services`
- `/agent/devices/{device_id}/p2p-cert`
- `/agent/devices/{device_id}/sessions`
- `/agent/sessions/{session_id}/claim`
- `/agent/sessions/{session_id}/bound`

## Configuration

`control-server` should accept:

- `--public-url` / `QUIC_TUNNEL_PUBLIC_URL`
- `--github-client-id` / `QUIC_TUNNEL_GITHUB_CLIENT_ID`
- `--github-client-secret` / `QUIC_TUNNEL_GITHUB_CLIENT_SECRET`
- `--github-redirect-url` / `QUIC_TUNNEL_GITHUB_REDIRECT_URL`

For tests, Control should depend on an `OAuthProviderClient` trait so GitHub HTTP
calls can be mocked without network access.

## Error Handling

- Missing OAuth config: `503 Service Unavailable`.
- Invalid OAuth state: `401 Unauthorized`.
- GitHub email unavailable or unverified: `403 Forbidden`.
- Duplicate provider identity conflict: `409 Conflict`.
- Expired server auth session: `400 Bad Request` with `expired_token`.
- Polling too fast: `400 Bad Request` with `slow_down`.
- Denied approval: `400 Bad Request` with `access_denied`.
- Disabled server credential: `401 Unauthorized`.

## Audit Events

Record audit logs for:

- GitHub OAuth user create.
- GitHub OAuth login.
- Server auth session start.
- Server auth approval or denial.
- Server credential issue.
- Server credential status update.
- Agent credential usage failure due to revoked/disabled credential.

## Test Strategy

Use TDD for every task:

- route tests in `crates/control/tests/control_plane.rs`;
- persistence tests in `crates/control/tests/sqlite_store.rs`;
- client method tests in `crates/control-client/src/lib.rs`;
- `agentd` CLI parsing and credential-file tests in `apps/agentd/src/main.rs`;
- static contract tests in `crates/mobile-core/tests/smoke_script.rs`.

Network calls to GitHub must be behind a trait and faked in tests.

## Rollout

1. Add DTOs, store types, and role model.
2. Add GitHub OAuth account login with fake provider tests.
3. Add server auth sessions and device-code/browser exchange.
4. Add server credential issuance and revocation.
5. Update agent route authorization.
6. Add `agentd login` and credential-file based run path.
7. Document dev and production setup.
