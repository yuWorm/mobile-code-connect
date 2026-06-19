# Server Auth Generated Device Login Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let server-agent login omit a permanent `device_id`, have Control Server generate the final device id during a short-lived server-auth session, and provide authenticated browser/device-code approval pages.

**Architecture:** Keep `device_id` as a non-secret stable identifier, but make it optional at the server-auth start boundary. Control normalizes missing ids into generated `srv_dev_*` values inside `ServerAuthSession`, exposes authenticated read-only session detail APIs for Vue approval pages, and preserves existing approve/exchange/poll semantics. SDK and `agentd` omit `device_id` by default while old explicit ids continue to work.

**Tech Stack:** Rust (`axum`, `serde`, workspace crates), Vue 3 + TypeScript, Vite/Vitest, existing Control API client and SDK tests.

---

## Chunk 1: Backend Protocol And Control State

### Task 1: Make server-auth start requests support generated device ids

**Files:**
- Modify: `crates/control-client/src/lib.rs`
- Modify: `crates/control/src/state.rs`
- Modify: `crates/control/tests/control_plane.rs`

- [x] **Step 1: Write failing backend tests for omitted and explicit device ids**

Add coverage to `crates/control/tests/control_plane.rs`:

```rust
#[tokio::test]
async fn browser_server_auth_generates_device_id_when_omitted() {
    let state = ControlState::new("dev-secret", "relay.example.com:4443", "punch.example.com:3478");
    let app = routes(state);
    let user_auth = register_user(app.clone(), "generated-browser-owner@example.com").await;

    let start = request_json(
        app.clone(),
        Method::POST,
        "/server-auth/browser/start",
        None,
        &serde_json::json!({
            "device_name": "Generated Browser Server",
            "server_public_key": "server-public-key-generated-browser"
        }),
    )
    .await;
    assert_eq!(start.status(), StatusCode::OK);
    let start: BrowserServerAuthStartResponse = json(start).await;

    let approval = get(
        app.clone(),
        &format!("/server-auth/browser/approve?session_id={}", start.session_id),
        &user_auth.access_token,
    )
    .await;
    assert_eq!(approval.status(), StatusCode::OK);
    let approval: BrowserServerAuthApprovalResponse = json(approval).await;

    let exchange = request_json(
        app,
        Method::POST,
        "/server-auth/browser/exchange",
        None,
        &BrowserServerAuthExchangeRequest {
            session_id: start.session_id,
            server_auth_code: approval.server_auth_code,
            server_public_key: "server-public-key-generated-browser".to_string(),
        },
    )
    .await;
    assert_eq!(exchange.status(), StatusCode::OK);
    let credential: ServerCredentialResponse = json(exchange).await;
    assert!(credential.device_id.as_str().starts_with("srv_dev_"));
}
```

Also extend existing explicit-id browser/device-code tests to assert `pc_001`-style ids are still preserved.

- [x] **Step 2: Run backend test and verify it fails**

Run: `cargo test -p mobilecode_connect_control --test control_plane browser_server_auth_generates_device_id_when_omitted`

Expected: fail to deserialize/start because `device_id` is required.

- [x] **Step 3: Update request type and generation helper**

In `crates/control-client/src/lib.rs`, change:

```rust
pub struct StartServerAuthRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_id: Option<DeviceId>,
    pub device_name: String,
    pub server_public_key: String,
}
```

Update all local test fixtures in that file to use `Some(DeviceId::new(...))` when preserving explicit ids.

In `crates/control/src/state.rs`, add:

```rust
fn new_server_device_id() -> DeviceId {
    DeviceId::new(format!("srv_dev_{}", uuid::Uuid::new_v4().simple()))
}
```

In `start_browser_server_auth` and `start_device_server_auth`, validate `device_name` and `server_public_key`, and normalize:

```rust
let device_id = request.device_id.unwrap_or_else(new_server_device_id);
if device_id.as_str().trim().is_empty() { return Err(ControlPlaneError::InvalidInput); }
```

Store `device_id` in `ServerAuthSession`.

- [x] **Step 4: Run backend tests for generated and explicit ids**

Run: `cargo test -p mobilecode_connect_control --test control_plane server_auth`

Expected: pass.

- [x] **Step 5: Mark Task 1 complete in this plan**

Update this task's checkboxes to `[x]`.

### Task 2: Add authenticated server-auth session detail APIs

**Files:**
- Modify: `crates/control-client/src/lib.rs`
- Modify: `crates/control/src/state.rs`
- Modify: `crates/control/src/routes.rs`
- Modify: `crates/control/tests/control_plane.rs`

- [x] **Step 1: Write failing tests for detail endpoints**

Add tests covering:

- `GET /server-auth/browser/session?session_id=...` without token returns `401`.
- With user token returns `session_id`, `mode`, `status`, `device_id`, `device_name`, `server_public_key_fingerprint`, and `expires_epoch_sec`.
- `GET /server-auth/device/session?user_code=...` behaves the same for device-code sessions.

Use expected fingerprint `sha256:<hex-or-base64url>` according to the implementation helper chosen in Step 3.

- [x] **Step 2: Run detail endpoint tests and verify they fail**

Run: `cargo test -p mobilecode_connect_control --test control_plane server_auth_session_detail`

Expected: fail because routes/types do not exist.

- [x] **Step 3: Add detail response types and state methods**

In `crates/control-client/src/lib.rs`, add:

```rust
pub struct ServerAuthSessionDetail {
    pub session_id: String,
    pub mode: ServerAuthMode,
    pub status: ServerAuthStatus,
    pub device_id: DeviceId,
    pub device_name: String,
    pub server_public_key_fingerprint: String,
    pub expires_epoch_sec: u64,
}
```

Ensure `ServerAuthMode` serializes to the existing snake_case values.

In `crates/control/src/state.rs`, add methods:

```rust
pub fn browser_server_auth_session_detail(&self, session_id: &str) -> Result<ServerAuthSessionDetail, ControlPlaneError>
pub fn device_server_auth_session_detail(&self, user_code: &str) -> Result<ServerAuthSessionDetail, ControlPlaneError>
```

Both should reject empty input, find the matching session, mark expired sessions as expired when needed, and return detail. Use a helper:

```rust
fn server_public_key_fingerprint(value: &str) -> String {
    format!("sha256:{}", secret_hash(value))
}
```

- [x] **Step 4: Add routes and client methods**

In `crates/control/src/routes.rs`, add routes before mutating approval routes:

```rust
.route("/server-auth/browser/session", get(browser_server_auth_session_detail))
.route("/server-auth/device/session", get(device_server_auth_session_detail))
```

Handlers must use `logged_in_user_id_from_headers` or `logged_in_human_claims_from_headers`.

In `crates/control-client/src/lib.rs`, add `browser_server_auth_session_detail` and `device_server_auth_session_detail`.

- [x] **Step 5: Run detail tests**

Run: `cargo test -p mobilecode_connect_control --test control_plane server_auth_session_detail`

Expected: pass.

- [x] **Step 6: Mark Task 2 complete in this plan**

Update this task's checkboxes to `[x]`.

### Task 3: Route browser navigations to the SPA without breaking JSON APIs

**Files:**
- Modify: `crates/control/src/routes.rs`
- Modify: `crates/control/tests/control_plane.rs`

- [x] **Step 1: Write failing routing tests**

Add tests showing:

- `GET /server-auth/browser/approve?session_id=...` with `Accept: text/html` and no `Authorization` returns HTML/SP⁠A fallback.
- `GET /server-auth/device?user_code=...` with `Accept: text/html` and no `Authorization` returns HTML/SP⁠A fallback.
- Existing JSON approval with `Authorization` still returns JSON.

- [x] **Step 2: Run routing tests and verify they fail**

Run: `cargo test -p mobilecode_connect_control --test control_plane server_auth_browser_navigation_returns_spa`

Expected: fail because current handlers return `401`.

- [x] **Step 3: Implement HTML navigation detection**

In `crates/control/src/routes.rs`, add:

```rust
fn accepts_html(headers: &HeaderMap) -> bool {
    headers
        .get(axum::http::header::ACCEPT)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.contains("text/html"))
        .unwrap_or(false)
}
```

Change browser/device approval handlers to return `axum::response::Response`.

If there is no `Authorization` header and `accepts_html(headers)` is true, return `web_index_response()`.
Otherwise preserve the existing JSON approval behavior.

- [x] **Step 4: Run routing tests**

Run: `cargo test -p mobilecode_connect_control --test control_plane server_auth_browser_navigation_returns_spa`

Expected: pass.

- [x] **Step 5: Mark Task 3 complete in this plan**

Update this task's checkboxes to `[x]`.

---

## Chunk 2: SDK And agentd Generated-Device Login

### Task 4: Make SDK server login input accept generated devices

**Files:**
- Modify: `crates/sdk/src/server_auth.rs`
- Modify: `crates/sdk/src/facade.rs`
- Modify: `crates/sdk/tests/server_auth_sdk.rs`
- Modify: `crates/sdk/tests/facade_sdk.rs`
- Modify: `crates/sdk/tests/live_workflow.rs`
- Modify: `crates/sdk/examples/sdk_live_workflow.rs`
- Modify: `crates/sdk/examples/sdk_mock_workflow.rs`

- [ ] **Step 1: Write failing SDK tests**

In `crates/sdk/tests/server_auth_sdk.rs`, add/adjust tests proving:

- `ServerLoginInput::generated_device(...)` sends `StartServerAuthRequest { device_id: None, ... }`.
- `ServerLoginInput::existing_device(...)` sends `Some(DeviceId::new("pc_001"))`.
- persisted credential uses the response `device_id`.

- [ ] **Step 2: Run SDK tests and verify they fail**

Run: `cargo test -p mobilecode_connect_sdk --test server_auth_sdk generated_device`

Expected: fail because helpers/optional request do not exist.

- [ ] **Step 3: Implement SDK input helpers**

Change `ServerLoginInput`:

```rust
pub struct ServerLoginInput {
    pub device_id: Option<DeviceId>,
    pub device_name: String,
    pub server_public_key: String,
}
```

Add:

```rust
pub fn generated_device(device_name: impl Into<String>, server_public_key: impl Into<String>) -> Self
pub fn existing_device(device_id: DeviceId, device_name: impl Into<String>, server_public_key: impl Into<String>) -> Self
```

Update `start_request()` to copy the optional id.

- [ ] **Step 4: Update SDK call sites**

Use `ServerLoginInput::existing_device(...)` anywhere tests/examples intentionally expect a fixed `pc_001`.

Use `ServerLoginInput::generated_device(...)` in new generated-device tests.

- [ ] **Step 5: Run SDK tests**

Run:

```bash
cargo test -p mobilecode_connect_sdk --test server_auth_sdk
cargo test -p mobilecode_connect_sdk --test facade_sdk
cargo test -p mobilecode_connect_sdk --test live_workflow
```

Expected: pass.

- [ ] **Step 6: Mark Task 4 complete in this plan**

Update this task's checkboxes to `[x]`.

### Task 5: Make `agentd login --device` optional while keeping browser default

**Files:**
- Modify: `apps/agentd/src/main.rs`

- [ ] **Step 1: Write failing CLI tests**

Update `apps/agentd/src/main.rs` tests:

- `login_args_accept_browser_and_device_code_modes` should accept login without `--device`.
- default login remains browser mode (`device_code == false`).
- `--device-code` works without `--device`.
- explicit `--device pc_002` still parses as `Some(DeviceId::new("pc_002"))`.

- [ ] **Step 2: Run agentd tests and verify they fail**

Run: `cargo test -p agentd login_args_accept_browser_and_device_code_modes`

Expected: fail because `device_id` is required/defaulted.

- [ ] **Step 3: Update CLI args and login input construction**

Change:

```rust
device_id: Option<DeviceId>
```

with no default value for login args only. Keep run args unchanged.

Build `server_public_key` fallback with either explicit id or a fresh UUID-based suffix if no id exists. Build `ServerLoginInput` using:

```rust
match args.device_id {
    Some(device_id) => ServerLoginInput::existing_device(device_id, args.device_name.clone(), server_public_key),
    None => ServerLoginInput::generated_device(args.device_name.clone(), server_public_key),
}
```

- [ ] **Step 4: Run agentd tests**

Run: `cargo test -p agentd login_args_accept_browser_and_device_code_modes`

Expected: pass.

- [ ] **Step 5: Mark Task 5 complete in this plan**

Update this task's checkboxes to `[x]`.

---

## Chunk 3: Web Approval Pages

### Task 6: Add TypeScript API/types and route-guard coverage

**Files:**
- Modify: `web/src/lib/control/types.ts`
- Modify: `web/src/lib/control/api.ts`
- Modify: `web/src/lib/control/__tests__/api.test.ts`
- Modify: `web/src/router/index.ts`
- Modify: `web/src/router/__tests__/guards.test.ts`
- Modify: `web/src/views/LoginView.vue`
- Modify: `web/src/lib/control/__tests__/oauth.test.ts`
- Modify: `web/src/views/__tests__/oauth-login.test.ts`

- [ ] **Step 1: Write failing frontend API/guard tests**

Add tests for:

- `StartServerAuthRequest.device_id` optional at type/source level.
- `controlApi.browserServerAuthSessionDetail(sessionId)` calls `/server-auth/browser/session?session_id=...`.
- `controlApi.deviceServerAuthSessionDetail(userCode)` calls `/server-auth/device/session?user_code=...`.
- anonymous approval routes redirect to `/login?redirect=<fullPath>`.
- LoginView passes `route.query.redirect` into `githubOAuthCallbackUrl`.

- [ ] **Step 2: Run frontend targeted tests and verify they fail**

Run: `pnpm --dir web test --run src/lib/control/__tests__/api.test.ts src/router/__tests__/guards.test.ts src/lib/control/__tests__/oauth.test.ts src/views/__tests__/oauth-login.test.ts`

Expected: fail on missing methods/routes/redirect preservation.

- [ ] **Step 3: Implement frontend API/types/routes**

Add `ServerAuthSessionDetail` type and API methods.

Add protected routes:

```ts
{
  path: '/server-auth/browser/approve',
  name: 'server-auth-browser-approve',
  component: () => import('@/views/ServerAuthBrowserApproveView.vue'),
  meta: { requiresAuth: true, title: 'Server Login Approval' },
}
{
  path: '/server-auth/device',
  name: 'server-auth-device',
  component: () => import('@/views/ServerAuthDeviceApproveView.vue'),
  meta: { requiresAuth: true, title: 'Device Code Approval' },
}
```

Update `LoginView.startGithubOAuth()`:

```ts
const redirect = typeof route.query.redirect === 'string' ? route.query.redirect : undefined
const redirectUri = githubOAuthCallbackUrl(window.location.href, redirect)
```

- [ ] **Step 4: Run frontend API/guard tests**

Run: `pnpm --dir web test --run src/lib/control/__tests__/api.test.ts src/router/__tests__/guards.test.ts src/lib/control/__tests__/oauth.test.ts src/views/__tests__/oauth-login.test.ts`

Expected: pass.

- [ ] **Step 5: Mark Task 6 complete in this plan**

Update this task's checkboxes to `[x]`.

### Task 7: Build browser and device-code approval views

**Files:**
- Create: `web/src/views/ServerAuthBrowserApproveView.vue`
- Create: `web/src/views/ServerAuthDeviceApproveView.vue`
- Create: `web/src/views/__tests__/server-auth-approval.test.ts`
- Modify: `web/src/lib/i18n/messages.ts`

- [ ] **Step 1: Write failing approval-view tests**

Create tests that source-check for:

- browser view calls `browserServerAuthSessionDetail` and `approveBrowserServerAuth`;
- browser view renders/copies `server_auth_code` after approval;
- device view supports user-code input and calls `deviceServerAuthSessionDetail`;
- device view calls `approveDeviceServerAuth` and `denyDeviceServerAuth`;
- both views display generated `device_id` and `server_public_key_fingerprint`;
- all visible strings use i18n keys.

- [ ] **Step 2: Run approval-view tests and verify they fail**

Run: `pnpm --dir web test --run src/views/__tests__/server-auth-approval.test.ts`

Expected: fail because views do not exist.

- [ ] **Step 3: Implement approval views**

Use existing UI primitives: `Card`, `Button`, `Input`, `Label`, `Badge`, `InfoRow`, loading/error states, and lucide icons.

Browser view:

- read `session_id` from route query;
- load detail on mount;
- approve on button click;
- show one-time auth code and copy button.

Device view:

- read optional `user_code` from route query;
- if absent, show input form;
- after detail loads, show request summary;
- approve/deny with existing APIs.

- [ ] **Step 4: Add i18n messages**

Add Chinese and English keys under `serverAuthApproval.*` for titles, descriptions, labels, buttons, and error text.

- [ ] **Step 5: Run approval-view tests**

Run: `pnpm --dir web test --run src/views/__tests__/server-auth-approval.test.ts src/views/__tests__/all-pages-i18n.test.ts`

Expected: pass.

- [ ] **Step 6: Mark Task 7 complete in this plan**

Update this task's checkboxes to `[x]`.

### Task 8: Update user credentials page to omit manual device id

**Files:**
- Modify: `web/src/views/user/UserCredentialsView.vue`
- Modify: `web/src/views/user/__tests__/user-credentials.test.ts`
- Modify: `web/src/lib/i18n/messages.ts`

- [ ] **Step 1: Write failing credentials-page tests**

Update tests to require:

- start device-code auth form no longer requires `authForm.device_id`;
- request sent to `startDeviceServerAuth` omits `device_id`;
- generated credential device id is displayed after polling returns a credential.

- [ ] **Step 2: Run credentials-page tests and verify they fail**

Run: `pnpm --dir web test --run src/views/user/__tests__/user-credentials.test.ts`

Expected: fail because device id is currently required.

- [ ] **Step 3: Update credentials page**

Remove the device id input from the start-auth form. Change `hasDeviceAuthForm` to require only `device_name` and `server_public_key`. Submit:

```ts
{
  device_name: authForm.device_name.trim(),
  server_public_key: authForm.server_public_key.trim(),
}
```

Keep filter device id field for the credentials table.

- [ ] **Step 4: Run credentials-page tests**

Run: `pnpm --dir web test --run src/views/user/__tests__/user-credentials.test.ts`

Expected: pass.

- [ ] **Step 5: Mark Task 8 complete in this plan**

Update this task's checkboxes to `[x]`.

---

## Chunk 4: Integration Verification

### Task 9: Run workspace verification and fix regressions

**Files:**
- Modify as needed based on compiler/test failures.
- Update this plan as each verification step completes.

- [ ] **Step 1: Format Rust code**

Run: `cargo fmt`

Expected: no formatting diff remains.

- [ ] **Step 2: Run Rust test suite for touched crates**

Run: `cargo test -p mobilecode_connect_control_client -p mobilecode_connect_control -p mobilecode_connect_sdk -p agentd`

Expected: pass.

- [ ] **Step 3: Run web test suite**

Run: `pnpm --dir web test --run`

Expected: pass.

- [ ] **Step 4: Build web**

Run: `pnpm --dir web build`

Expected: pass.

- [ ] **Step 5: Build Rust workspace targets touched by the feature**

Run: `cargo build -p mobilecode_connect_control -p mobilecode_connect_sdk -p agentd`

Expected: pass.

- [ ] **Step 6: Inspect git diff**

Run: `git diff --stat` and `git diff --check`

Expected: changes are scoped to this feature and no whitespace errors.

- [ ] **Step 7: Mark Task 9 complete in this plan**

Update this task's checkboxes to `[x]`.

### Task 10: Final commit and completion audit

**Files:**
- Modify: this plan file to mark all completed tasks.

- [ ] **Step 1: Confirm all plan checkboxes are complete**

Run: `rg -n "^- \\[ \\]" docs/superpowers/plans/2026-06-20-server-auth-generated-device-login.md`

Expected: no output.

- [ ] **Step 2: Commit implementation**

Run:

```bash
git add .
git commit -m "feat: support generated server auth device login"
```

Expected: commit succeeds.

- [ ] **Step 3: Final status**

Run: `git status --short`

Expected: clean worktree or only intentionally ignored local files.

- [ ] **Step 4: Mark Task 10 complete in this plan**

Update this task's checkboxes to `[x]` before the final implementation commit if the plan file is included in that commit.
