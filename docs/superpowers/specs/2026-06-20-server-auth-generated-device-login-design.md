# Server Auth Generated Device Login Design

## Goal

Make server-agent login work like a short-lived approval flow:

- the server-agent does not require the operator to choose or type a permanent
  `device_id` before login;
- Control Server generates the eventual `device_id` during the pending
  server-auth session;
- unauthenticated users who open a server-auth approval URL see the normal login
  options first, then return to the approval page;
- browser login stays the default `agentd login` mode;
- device-code login remains available for SSH/headless server scenarios through
  `agentd login --device-code`.

## Non-Goals

- Removing the existing browser login flow.
- Making `device_id` a credential or secret.
- Adding direct GitHub OAuth to `agentd`.
- Replacing server credentials or agent Control tokens.
- Adding a fully automatic browser callback from Control to `agentd`. The first
  version can keep the current one-time browser auth-code exchange.

## Current Context

Control already has:

- password login and GitHub OAuth login for users;
- server-auth browser and device-code APIs;
- pending server-auth sessions with short TTL;
- one-time browser auth codes and high-entropy device codes stored as hashes;
- server credentials that issue `role=Agent` Control tokens;
- a Vue login page with password and GitHub OAuth options;
- a user credentials page that can manually start device-code server auth.

The current gap is that `StartServerAuthRequest` requires `device_id`. This
makes server-side login awkward because the operator has to invent the stable
server identity before the Control-approved login completes.

## Design Choice

Keep `device_id` as a stable, non-secret identifier, but let Control generate it
when a server-auth session starts.

Security does not depend on secrecy of `device_id`. The security boundary is:

- logged-in user approval;
- short-lived server-auth session;
- hashed one-time codes;
- high-entropy `device_code`;
- one-time consumption;
- public-key binding;
- signed agent credential token with `credential_id`, `user_id`, token version,
  and `role=Agent`.

## Backend Protocol

### StartServerAuthRequest

Make `device_id` optional in Rust and TypeScript:

```rust
pub struct StartServerAuthRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_id: Option<DeviceId>,
    pub device_name: String,
    pub server_public_key: String,
}
```

Old clients that still send `device_id` continue to work. New clients omit it.

### Device ID Generation

When `device_id` is absent, Control generates one before inserting the
`ServerAuthSession`:

```text
srv_dev_<random>
```

The random suffix should be generated with the existing UUID v4 source or an
equivalent CSPRNG-backed generator. The generated id is stored only in the
pending server-auth session until the session is completed. If the session
expires, is denied, or is never exchanged/polled successfully, the generated id
does not become an active server credential.

If a caller provides `device_id`, Control keeps using it for compatibility.

### ServerAuthSession

`ServerAuthSession.device_id` remains required internally. Session creation
normalizes it as:

```text
request.device_id.unwrap_or_else(new_server_device_id)
```

The TTL remains `SERVER_AUTH_SESSION_TTL_SEC`.

### Approval Details

The Vue approval pages need to show what is being approved before they call the
mutating approval API. Add read-only, authenticated detail endpoints:

```http
GET /server-auth/browser/session?session_id=...
GET /server-auth/device/session?user_code=...
```

Both return a shared shape:

```json
{
  "session_id": "srv_auth_...",
  "mode": "browser",
  "status": "pending",
  "device_id": "srv_dev_...",
  "device_name": "Build Server",
  "server_public_key_fingerprint": "sha256:...",
  "expires_epoch_sec": 1760000000
}
```

The detail endpoints require a user/admin token. They do not approve anything.

The fingerprint is a display value derived from `server_public_key`. It helps
the user recognize the request, but it is not used as the only auth check.

### Browser Approval Route

Keep the existing API approval behavior for programmatic clients:

```http
GET /server-auth/browser/approve?session_id=...
Authorization: Bearer <user token>
```

For browser navigations without an Authorization header and with an HTML accept
header, return the SPA index instead of JSON. The Vue route then handles login
redirect and calls the detail/approval APIs with the stored user token.

### Device-Code Approval Route

Keep the existing API approval behavior:

```http
GET /server-auth/device?user_code=...
Authorization: Bearer <user token>
```

For browser navigations, return the SPA index. The Vue page supports both:

- `/server-auth/device?user_code=ABCD-EFGH`
- `/server-auth/device` with a user-code input box.

The page calls the detail endpoint first, then lets the user approve or deny.

### Exchange And Poll

No major protocol change:

- browser exchange still checks session id, one-time auth code hash, status, and
  `server_public_key`;
- device-code poll still checks high-entropy `device_code` and
  `server_public_key`;
- completion creates `ServerCredential` with the session's generated
  `device_id`;
- completion marks the session consumed and clears one-time auth material.

## Frontend Design

Add two protected Vue routes outside the admin/center shells:

```text
/server-auth/browser/approve
/server-auth/device
```

Both use `meta.requiresAuth = true`. The existing route guard sends anonymous
users to:

```text
/login?redirect=<full approval URL>
```

After password login, registration, or GitHub OAuth, the user returns to the
approval URL.

`LoginView` should pass the current `redirect` query into
`githubOAuthCallbackUrl(window.location.href, redirect)` so GitHub login
preserves the original server-auth approval route.

### Browser Approval Page

States:

- missing `session_id`: show an error and link back to `/center/credentials`;
- loading details;
- expired/not found/not ready;
- pending/approved request summary;
- approval success showing the one-time `server_auth_code`;
- approval failure with retry.

The page displays:

- device name;
- generated device id;
- server public key fingerprint;
- expiry;
- approve and cancel actions.

### Device-Code Approval Page

States:

- no `user_code` query: show a user-code input form;
- loading details after code entry;
- pending request summary;
- approved/denied result;
- expired/not found/not ready;
- approval failure with retry.

The page displays the same request details as browser approval. It also offers
approve and deny actions.

### Existing Credentials Page

The user credentials page can keep the manual "start device-code auth" panel,
but `device_id` becomes optional or removed from the form. It should submit only
`device_name` and `server_public_key` by default and show the generated
`device_id` after poll returns a credential.

## SDK And CLI

### SDK

Make `ServerLoginInput.device_id` optional:

```rust
pub struct ServerLoginInput {
    pub device_id: Option<DeviceId>,
    pub device_name: String,
    pub server_public_key: String,
}
```

For compatibility and ergonomics, add constructors/helpers rather than forcing
all call sites to set `Some(...)` manually:

```rust
ServerLoginInput::generated_device(device_name, server_public_key)
ServerLoginInput::existing_device(device_id, device_name, server_public_key)
```

The persisted `StoredServerCredential.device_id` remains required because the
credential response always includes the final id.

### agentd

Keep browser login as default:

```text
agentd login --control https://control.example.com --name "Build Server"
```

Device-code login remains explicit:

```text
agentd login --device-code --control https://control.example.com --name "Build Server"
```

The `--device` option becomes optional. If provided, it requests a specific
legacy/existing device id. If omitted, Control generates one.

In device-code mode, `agentd` prints:

```text
Open: <verification_uri>
Code: <user_code>
Complete URL: <verification_uri_complete>
```

Then it polls until approved, denied, expired, or slowed down according to the
existing SDK behavior.

## Security Notes

- `device_id` is not a secret and should not authorize anything by itself.
- Generated `device_id` values must be high entropy enough to avoid practical
  guessing and collisions.
- Server-auth session ids, device codes, user codes, and browser auth codes
  remain short-lived.
- Raw auth material must not be persisted except as hashes where the current
  implementation already does this.
- Browser approval codes are one-time use and are cleared after exchange.
- Device-code poll must keep matching `server_public_key` before issuing a
  credential.
- Detail endpoints require authentication to avoid exposing pending server
  request metadata to anonymous callers.
- Device-code detail/approval should use normalized user codes and should
  preserve existing slow-down behavior for polling. If brute-force protection is
  added later, it should rate-limit lookup/approval attempts per remote
  address/session.

## Tests

Backend tests:

- starting browser auth without `device_id` returns a session whose exchange
  credential has a generated `device_id`;
- starting device-code auth without `device_id` returns a credential with a
  generated `device_id` after approval and poll;
- old requests with explicit `device_id` still preserve that id;
- detail endpoints require auth and return generated id, fingerprint, status,
  and expiry;
- browser/device HTML navigation returns the SPA index while API approval still
  returns JSON;
- one-time browser exchange still rejects replay.

SDK/CLI tests:

- `ServerLoginInput` can omit `device_id`;
- persisted credential stores the generated response `device_id`;
- `agentd login` defaults to browser mode and accepts omitted `--device`;
- `agentd login --device-code` accepts omitted `--device`;
- explicit `--device` still works.

Frontend tests:

- route guard redirects anonymous approval routes to `/login?redirect=...`;
- LoginView preserves `redirect` through GitHub OAuth;
- browser approval page calls detail then approve and renders the one-time code;
- device approval page accepts a code, calls detail, and approve/deny APIs;
- credentials page no longer requires manual `device_id` for starting
  device-code auth.

## Rollout

The change is backwards compatible at the HTTP JSON layer because explicit
`device_id` requests still work. New clients can omit `device_id`. Existing
stored server credentials remain valid because they already contain a concrete
`device_id`.
