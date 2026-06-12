# QUIC Tunnel

Rust workspace for embeddable Agent/Mobile SDK crates and standalone Relay,
Control, Punch, and Admin API services.

Initial verification after crate skeletons exist:

```bash
cargo metadata --format-version 1
cargo check --workspace
```

## Control plane MVP

The Control API now has a first-stage control plane with optional SQLite
snapshot persistence:

```text
POST /auth/register
POST /auth/login
POST /auth/password
GET  /auth/oauth/github/start
GET  /auth/oauth/github/callback
GET  /oauth/identities
GET  /oauth/identities/{provider}/{provider_user_id}
DELETE /oauth/identities/{provider}/{provider_user_id}
POST /server-auth/browser/start
GET  /server-auth/browser/approve
POST /server-auth/browser/exchange
POST /server-auth/device/start
GET  /server-auth/device
POST /server-auth/device/poll
GET  /server-credentials
GET  /server-credentials/{credential_id}
POST /server-credentials/{credential_id}/status
POST /server-credentials/{credential_id}/rotate
GET  /dashboard
GET  /audit-logs
GET  /usage/users
POST /usage/users/{user_id}/reset
POST /usage/relay-sessions
GET  /sessions
POST /sessions
POST /sessions/{session_id}/close
GET  /controllers
POST /controllers/register
DELETE /controllers/{client_id}
GET  /users
POST /users
GET  /users/{user_id}
POST /users/{user_id}/status
POST /users/{user_id}/role
GET  /devices
GET  /devices/{device_id}
DELETE /devices/{device_id}
GET  /devices/{device_id}/access
POST /devices/{device_id}/access
DELETE /devices/{device_id}/access/{user_id}
GET  /plans/current
GET  /plans/catalog
GET  /plans/catalog/{plan_id}
POST /plans/catalog
GET  /plans/users/{user_id}
POST /plans/users/{user_id}
POST /plans/users/{user_id}/assign
GET  /relay-credentials
POST /relay-credentials
GET  /relay-credentials/{relay_id}
POST /relay-credentials/{relay_id}/status
POST /relay-credentials/{relay_id}/rotate
POST /relays/register
GET  /relays
GET  /relays/{relay_id}
POST /relays/{relay_id}
DELETE /relays/{relay_id}
```

Users receive a Control access token from register/login. Authenticated requests
use `Authorization: Bearer <token>`. For compatibility with the local smoke
stack, requests without a token still run as the seeded development user
`user_001`.

Rust applications should prefer the high-level `quic_tunnel_sdk` crate over
calling every HTTP endpoint directly. `AuthSdk` handles register/login/password
flows and token storage, `ControllerSdk` handles controller registration,
device/service discovery, and session creation, and `MobileTunnelSdk` wraps the
mobile tunnel open/close/status flow on top of `mobile-core`. `ServerAuthSdk`
handles controlled-server login, and `ServerSdk` uses the saved server
credential to register the server device/services/P2P certificate and manage
agent session lifecycle calls. `AdminSdk` wraps admin-token operations for
dashboard, users, plans, relay credentials, relay pool, usage, audit logs, and
session queries, plus server credentials, controlled devices, device access
grants, and controller inventory. Use `MemoryTokenStore` for tests or
short-lived tools and `FileTokenStore` for embedded apps that need to persist a
user Control token across process restarts.
For application-level wiring, `QuicTunnelSdk::builder().control_url(...).token_file(...).build()`
shares one token store across `auth()`, `controller()`, `admin()`, and mobile
tunnel helpers, so an embedded Rust app can log in once and reuse the same
credential for controller/device/session/admin flows. Add
`server_credential_file(...)` when the same app also owns a controlled server;
the facade then shares that credential across `server_auth()` and `server()`.
The facade also exposes direct workflow methods such as `login(...)`,
`register_controller(...)`, `create_session(...)`, and
`open_mobile_service_in_memory(...)` for apps that do not need to manage the
individual sub-SDK objects. Use `ensure_login(...)`, `ensure_register(...)`, and
`ensure_controller(...)` when an app wants to reuse saved local credentials
before making a Control request; use `ensure_login_fresh(...)`,
`ensure_register_fresh(...)`, or `current_valid_token(...)` when the app also
wants to reject locally expired user tokens before reuse. Use
`ensure_browser_server_login(...)` or `ensure_device_code_server_login(...)` to
reuse a saved controlled-server credential for the current Control URL, or
receive the pending login that must be approved by the user. `SdkError` exposes
`control_status_code()`, `is_unauthorized()`, `is_forbidden()`, and
`requires_reauthentication()` so embedded apps can route 401 re-login flows
separately from 403 permission failures. Control HTTP calls default to no
timeout and no retry; pass `HttpControlClientOptions` to the facade builder when
an embedded app wants bounded requests and retry on transport failures,
timeouts, 408, 429, or 5xx responses.

```rust
use std::time::Duration;
use quic_tunnel_sdk::HttpControlClientOptions;

let sdk = QuicTunnelSdk::builder()
    .control_url("http://127.0.0.1:8080")
    .token_file("state/user-token.json")
    .server_credential_file("state/server-credential.json")
    .control_client_options(
        HttpControlClientOptions::default()
            .with_request_timeout(Duration::from_secs(5))
            .with_max_retries(2)
            .with_retry_backoff(Duration::from_millis(100)),
    )
    .build()?;

let auth = sdk.auth()?;
let controller = sdk.controller()?;
let server_auth = sdk.server_auth()?;
let server = sdk.server()?;
let admin = sdk.admin()?;
```

`quic_tunnel_mobile_core::ffi` exposes the first UniFFI-ready mobile SDK
surface for iOS and Android. For native callers that need a direct socket-like
endpoint, the app creates `FfiMobileTunnel`, opens a `127.0.0.1:<port>` forward
for one device/service, reads status, and closes the forward. Passing
`local_port = 0` asks the OS to allocate an ephemeral local port, and the
returned `FfiForwardHandle` contains the actual port.

Native apps can pair without logging into Control by importing the JSON invite
as `FfiMobileInvitePayload`. The Swift/Kotlin wrappers expose
`QuicTunnelMobileGrantPairingController`, which generates a random nonce and
wraps the low-level UniFFI start/poll calls. Native callers can also call those
UniFFI functions directly:

```text
let options = mobileGrantPairingOptionsWithDefaults()
let pairing = try startMobileGrantPairing(
  invite: invite,
  clientId: "mobile_001",
  requestedServices: ["svc_web_3000"],
  nonce: "<random nonce>",
  options: options
)
let result = try pollMobileGrantPairingOnce(pairing: pairing, options: options)
```

When `result.status` is approved, `result.grant` contains a
`FfiMobileGrantCredential` with the locally derived grant secret and optional
agent P2P certificate fingerprint. Store that credential in platform secure
storage, then pass it to `FfiMobileTunnel.startWithMobileGrant(...)`. The
Swift/Kotlin wrappers include `QuicTunnelMobileGrantSecureStore`: iOS stores the
credential JSON in Keychain, and Android encrypts it with an Android Keystore
AES-GCM key before writing app-private preferences. The shared UniFFI helpers
`mobileGrantCredentialToJson(...)` and `mobileGrantCredentialFromJson(...)`
provide the stable credential encoding used by those stores.

For embedded browsers, prefer `FfiMobileTunnel.startBrowserProxy()`. It starts
one SDK-managed local HTTP proxy and returns `FfiBrowserProxy.host()` and
`FfiBrowserProxy.port()`. Configure the app's WebView proxy to that endpoint,
then navigate to typed device-service routes:

```text
browserProxyRouteHttpUrl(browserProxyDeviceServiceRoute(device_id, service_id), "/")
```

For example:

```text
http://s-svc-5fweb-5f3000.d-pc-5f001.qtunnel.local/
```

`browserProxyDeviceServiceRoute(...)` returns a descriptor whose kind is
`DeviceService` and whose host is DNS-safe and reversible. The lower-level
`browserProxyHostForService(...)` helper remains available, and the legacy
`<service_id>.<device_id>.qtunnel.local` shape is still accepted for
compatibility. Treat these synthetic URLs as device-service URLs only. Control
server and server-agent APIs should keep using their normal control URL; ordinary
public or LAN website URLs should keep their normal host and will use the proxy's
direct-network fallback instead of the tunnel.

For native routing decisions, use `browserProxyClassifyUrlWithDefaults(url,
controlServerUrl)` before applying special handling in app code. It returns
`DeviceService` with the decoded `device_id + service_id` for synthetic hosts,
`ControlServer` for the configured control origin, and `DirectNetwork` for
ordinary websites. This keeps mobile WebView interception code explicit: device
service URLs enter the tunnel, control/server SDK calls stay on the configured
control URL, and unrelated browser traffic stays direct.

The browser proxy maps synthetic hosts back to `device_id + service_id`, then
opens a stream through the same mobile tunnel connector as `open_service`.
Production mobile clients should start the tunnel with the P2P-or-Relay entry
point, so device-service HTTP proxy traffic is carried over P2P when available
and falls back to Relay when P2P fails or misses the fallback delay. The proxy
rewrites HTTP proxy absolute-form requests to origin-form, preserves any request
body bytes already read with the header, and supports HTTPS `CONNECT` as raw TCP
passthrough. Plain HTTP proxy requests are handled as one request per proxy
connection with `Connection: close`; request bodies with `Content-Length` are
forwarded exactly, and chunked request bodies are forwarded with their original
chunk framing. Ambiguous `Transfer-Encoding: chunked` plus `Content-Length`
requests are rejected before opening a tunnel stream. The proxy uses bounded
buffered reads for HTTP heads and
request bodies, and keeps HTTPS `CONNECT` on the raw tunnel copy path.
`startBrowserProxy()` uses `127.0.0.1:0`, `.qtunnel.local`, and a 256-connection
limit by default. It also applies mobile-safe timeouts: 10 seconds to receive an
HTTP request head, 10 seconds to connect direct-network targets, 15 seconds to
open a tunnel stream, and 120 seconds of idle time for raw `CONNECT` tunnels.
The proxy only binds loopback addresses such as `127.0.0.1`, `::1`, or
`localhost`; it will not bind `0.0.0.0` because this endpoint is for the local
embedded browser, not for other devices on the network. Use
`browserProxyConfigWithDefaults()` with `startBrowserProxyWithConfig(...)` and
`browserProxyHostWithSuffix(...)` when the app needs a fixed bind port, a
different synthetic domain suffix, a lower connection limit, or different
timeouts. It does not MITM TLS and it is not a VPN or system-wide proxy.
By default, requests for non-qtunnel hosts are routed directly from the mobile
SDK proxy to the normal network target only when the host is a domain name or a
loopback/private/link-local IP literal. That lets a WebView global proxy load
normal websites and local-network resources without opening direct dialing to
all public IP literals. This default is `LocalNetworkAndDomain`. Apps that need
a wider or tighter embedded-browser boundary can set `direct_fallback_policy` /
`directFallbackPolicy` to `AllowAll` or `Disabled`.
`FfiBrowserProxy.stats()` exposes cumulative counters for accepted and active
connections, tunnel/direct connections, forbidden direct fallbacks, timeouts, and
request errors. It also reports tunnel/direct byte counts in both directions and
raw `CONNECT` closures caused by idle timeout. The Swift/Kotlin wrapper
`stats()` helpers return the same snapshot so mobile apps can surface proxy
diagnostics without parsing logs.
`FfiMobileTunnel.status().transport` reports the underlay path counters shared by
`open_service` and the browser proxy: P2P attempts/connections/failures, Relay
fallbacks/connections/failures, and the last successful underlay path. Before the
first stream succeeds, a P2P-or-Relay tunnel reports `path = P2p` to reflect the
preferred path, not a Relay-only connection.

The mobile app owns applying the returned proxy endpoint to its embedded browser
and clearing it when the browser/tunnel closes. The thin platform wrappers are
checked in as source templates:

- `mobile/ios/Sources/QuicTunnelMobileSdk/QuicTunnelBrowserProxyController.swift`
- `mobile/ios/Sources/QuicTunnelMobileSdk/QuicTunnelMobileGrantPairingController.swift`
- `mobile/ios/Sources/QuicTunnelMobileSdk/QuicTunnelMobileGrantSecureStore.swift`
- `mobile/android/src/main/java/dev/quictunnel/mobile/QuicTunnelBrowserProxyController.kt`
- `mobile/android/src/main/java/dev/quictunnel/mobile/QuicTunnelMobileGrantPairingController.kt`
- `mobile/android/src/main/java/dev/quictunnel/mobile/QuicTunnelMobileGrantSecureStore.kt`

On iOS 17+, the wrapper creates a `Network.ProxyConfiguration` for the local
HTTP CONNECT proxy and assigns it to
`WKWebViewConfiguration.websiteDataStore.proxyConfigurations` before constructing
the `WKWebView`. On Android, the wrapper uses AndroidX WebKit
`ProxyController.setProxyOverride` with a single `ProxyConfig` rule pointing at
`FfiBrowserProxy.host()` and `FfiBrowserProxy.port()`. Both wrappers expose
`makeBrowserProxyConfig(...)`, `deviceServiceUrl(...)`, `classify(...)`,
`closeBrowserProxy()`, and `shutdown()` so app code does not need to manually
build synthetic hosts, hand-edit UniFFI config records, or remember the
proxy/tunnel close order. Older WebView fallbacks are intentionally out of scope.
Agent service targets are still guarded on the receiver side: the agent derives
allowed LAN CIDRs from its local interfaces and rejects direct IP targets
outside those receiver-side networks before dialing the service. DNS names are
resolved by the agent and only resolution results inside the receiver-side CIDRs
are dialed; direct public IP targets such as `8.8.8.8` are rejected on the agent
side.

Generate bindings from the built mobile-core library:

```bash
cargo install uniffi --version 0.31.1 --features cli --locked
scripts/gen-mobile-bindings.sh --language all
```

Use the platform-specific built library path for Linux/Android (`.so`) or
iOS/macOS (`.a`/`.dylib`) as appropriate. Pass `--library <path>` to the script
when generating bindings from a cross-compiled library instead of the host
release build.

The platform package skeletons are checked in:

- iOS SwiftPM: `mobile/ios/Package.swift`
- Android Gradle library: `mobile/android/settings.gradle.kts` and
  `mobile/android/build.gradle.kts`

For an iOS release, place the generated
`Artifacts/quic_tunnel_mobile_coreFFI.xcframework` under `mobile/ios/`; the
Swift package exports `QuicTunnelMobileSdk`, includes the generated UniFFI Swift
source under `Sources/QuicTunnelMobileSdk/Generated`, and depends on the
`quic_tunnel_mobile_coreFFI` binary target. For an Android release, copy
generated UniFFI Kotlin bindings into `mobile/android/src/main/uniffi/kotlin`
and native `.so` libraries into `mobile/android/src/main/jniLibs/<abi>/`, then
assemble the AAR from `mobile/android`.

The packaging scripts perform those staging steps:

```bash
scripts/package-mobile-ios.sh
scripts/package-mobile-android.sh
```

Use `--skip-xcodebuild` or `--skip-gradle` to stage package inputs without
creating the final XCFramework/AAR. iOS packaging defaults to
`--ios-min-version 17.0`, matching the checked-in WebView proxy wrapper, and can
build `aarch64-apple-ios`, `aarch64-apple-ios-sim`, and `x86_64-apple-ios`.
When multiple simulator targets are requested, the script uses `lipo` to merge
them into one simulator static library before calling
`xcodebuild -create-xcframework`, so the final XCFramework has one device slice
and one simulator slice. Pass `--xcframework-output <path>` when CI should write
the XCFramework outside `mobile/ios/Artifacts`.

Android packaging runs `assembleRelease` when a Gradle command or wrapper is
available; pass `--gradle-task assembleDebug` or another task name when staging a
non-release AAR. Native libraries are stripped with NDK `llvm-strip` by default;
use `--no-strip` for debug symbols. Built AARs are copied into
`target/mobile-package/android/aar` by default, or into `--aar-output-dir <dir>`
when CI needs a stable artifact directory. Both packaging scripts write
`mobile-package-manifest.json` under the staging directory with SHA-256 and byte
size entries for staged artifacts. Use `--dry-run` to print the target build,
binding generation, copy, strip, manifest, and packaging steps without requiring
the cross-compilation toolchains. Android packaging auto-configures NDK clang
linkers from `ANDROID_NDK_HOME`/`ANDROID_NDK_ROOT`; pass `--ndk-home`,
`--ndk-host-tag`, or `--android-api` when the default detection is not enough.

SDK examples can be compiled and run directly from the workspace:

```bash
cargo run -p quic_tunnel_sdk --example sdk_mock_workflow

QUIC_TUNNEL_SDK_LIVE_RUN=1 \
QUIC_TUNNEL_CONTROL_URL=http://127.0.0.1:8080 \
QUIC_TUNNEL_SDK_EMAIL=member@example.com \
QUIC_TUNNEL_SDK_PASSWORD=password-123 \
cargo run -p quic_tunnel_sdk --example sdk_live_workflow
```

`sdk_mock_workflow` runs the complete user/controller/server/session sequence
with in-process fake APIs. `sdk_live_workflow` uses file-backed user and server
credentials under `target/sdk-live-workflow` by default, starts device-code
server login when no server credential exists, and then registers a server
device/service before creating a controller session.

Control tokens carry a role. Register/login returns a normal `user` token for
user-scoped APIs. Global management APIs require an `admin` token:

```text
GET      /audit-logs
GET      /dashboard
GET      /usage/users
POST     /usage/users/{user_id}/reset
GET      /sessions
GET      /users
POST     /users
GET      /users/{user_id}
POST     /users/{user_id}/status
POST     /users/{user_id}/role
GET/POST /plans/catalog
GET      /plans/catalog/{plan_id}
GET/POST /plans/users/{user_id}
POST     /plans/users/{user_id}/assign
GET/POST /devices/{device_id}/access
DELETE   /devices/{device_id}/access/{user_id}
GET/POST /relay-credentials
GET      /relay-credentials/{relay_id}
POST     /relay-credentials/{relay_id}/status
POST     /relay-credentials/{relay_id}/rotate
GET      /relays
GET/DELETE /relays/{relay_id}
```

Relay registration, heartbeat updates, and usage reports accept either an
`admin` token or a dedicated `relay` token whose subject matches the Relay id:

```text
POST /relays/register
POST /relays/{relay_id}
POST /usage/relay-sessions
```

GitHub OAuth can be enabled on `control-server` with:

```bash
QUIC_TUNNEL_PUBLIC_URL=https://control.example.com
QUIC_TUNNEL_GITHUB_CLIENT_ID=<github-client-id>
QUIC_TUNNEL_GITHUB_CLIENT_SECRET=<github-client-secret>
QUIC_TUNNEL_GITHUB_REDIRECT_URL=https://control.example.com/auth/oauth/github/callback
```

The GitHub OAuth HTTP client uses the system curl binary at runtime, so
production Control hosts must have `curl` installed and available on `PATH`.

The Control API exposes `GET /auth/oauth/github/start` and
`GET /auth/oauth/github/callback` for user login/account creation. Users can
inspect their linked OAuth identities through `GET /oauth/identities` and
`GET /oauth/identities/{provider}/{provider_user_id}`; admins can query all
identities with shared list parameters such as `user_id`, `q`, `limit`,
`offset`, and `sort`. Owners and admins can unlink an OAuth identity with
`DELETE /oauth/identities/{provider}/{provider_user_id}` when the account still
has another login method; the API rejects deleting the last available login
method. Users can set a first password on OAuth-only accounts or change an
existing password through `POST /auth/password`; accounts with an existing
password must provide the current password. Server login uses Control-issued
server credential tokens instead of GitHub tokens:

```bash
agentd login --control https://control.example.com --device pc_001 --name "Office PC"
agentd login --device-code --control https://control.example.com --device pc_001 --name "Office PC"
agentd run --control https://control.example.com --credential-file agentd-credential.json --relay-cert relay.der --service svc_web=127.0.0.1:3000
```

Browser server login uses `/server-auth/browser/start`, user approval, and
`/server-auth/browser/exchange`. Headless login uses `/server-auth/device/start`
and `/server-auth/device/poll`. Rust applications should use
`quic_tunnel_sdk::ServerAuthSdk` with `FileServerCredentialStore` or a custom
`ServerCredentialStore`; `agentd login` uses the same SDK flow. After login,
`quic_tunnel_sdk::ServerSdk` can reuse that credential store for controlled
server registration and session lifecycle calls. The resulting
`ControlRole::Agent` credential is scoped to one controlled server device. An
Agent credential can register and update only its own device/services/P2P
certificate; users manage their server
credentials through `GET /server-credentials` and
`GET /server-credentials/{credential_id}`. Owners and admins can disable,
re-enable, and rotate credentials with
`POST /server-credentials/{credential_id}/status` and
`POST /server-credentials/{credential_id}/rotate`; rotation increments the
server credential version and invalidates older Agent tokens. Admins can use
the shared list query parameters such as `user_id`, `device_id`, `enabled`,
`q`, `limit`, `offset`, and `sort`. Successful Agent API authentication updates
the credential `last_used_epoch_sec` field for stale-credential review.

Management list endpoints now return a paginated envelope:

```json
{
  "items": [],
  "total": 0,
  "limit": 0,
  "offset": 0
}
```

The envelope is used by `GET /users`, `GET /sessions`, `GET /usage/users`,
`GET /audit-logs`, `GET /controllers`, `GET /devices`, `GET /plans/catalog`,
`GET /devices/{device_id}/access`, `GET /relay-credentials`, and `GET /relays`.
These endpoints accept common `q`, `sort`, `limit`, and `offset` query
parameters; some lists also accept typed filters such as `role`, `enabled`,
`status`, `user_id`, `device_id`, `healthy`, `action`, and `target_type`.
Runtime polling endpoints used by agents/mobile clients, such as `GET /mobile/devices` and
`GET /agent/devices/{device_id}/sessions`, keep their array response shape.

The Rust `HttpControlClient` exposes matching `*_with_query(AdminListQuery)`
helpers for the paginated list endpoints: users, audit logs, usage summaries,
admin sessions, controllers, controlled devices, plan catalog, relay
credentials, relays, and device access grants. Query string values are
percent-encoded by the client.

Admins can fetch a compact Control dashboard snapshot with:

```text
GET /dashboard
```

The snapshot includes total/enabled/admin users, controlled devices and online
devices, controller count, session counts by status, Relay health totals,
current Relay-reported actual traffic totals, and the five most recent audit
entries. It is intended for the admin console overview and does not use the
paginated list envelope.

Admin management mutations are recorded in an audit log. Read it with:

```text
GET /audit-logs
```

Audit entries include the actor Control token subject, actor role, action,
target type/id, message, and creation epoch. Current audited actions cover user
creation/status/role changes, plan catalog and user plan changes, Relay
credential create/status/rotate, and admin-driven Relay pool changes.

Admins can inspect per-user Control-side usage summaries with:

```text
GET /usage/users
```

Optional query parameters:

```text
GET /usage/users?sort=actual_total_bytes&limit=50&offset=0
```

`sort` supports `email`, `actual_total_bytes`, `relay_quota_granted_bytes`, and
`session_count`. Numeric sorts return the largest values first. `q` can match
user id, email, or plan id. `limit` and `offset` page the rows inside the
standard `Page<UserUsageSummary>` response.

The summary is derived from Control session assignments, signed Relay token
limits, and Relay session usage reports. It reports session counts by status,
current plan, controller/device counts, current per-session quota, total Relay
quota bytes granted through created sessions, and actual uplink/downlink/total
bytes last reported by Relay. Without Relay reporting, actual byte fields stay
at `0`.

Control also enforces the current plan's `relay_limits.traffic_quota_bytes`
when a user creates a new session. Once Relay-reported actual total bytes for
that user reach the current quota, `POST /sessions` returns `402 Payment
Required` until the user's plan is changed or an admin resets the user's current
usage period:

```text
POST /usage/users/{user_id}/reset
```

The reset endpoint starts a new period for that user and clears existing Relay
usage reports for the user's sessions, so new reports count toward the new
period. Automatic calendar billing periods are not modeled yet.

Relays report their local runtime stats with:

```text
POST /usage/relay-sessions
```

The endpoint accepts either an `admin` token or a dedicated `relay` token whose
subject matches the request `relay_id`. `relayd` calls it automatically during
the existing Control heartbeat loop when started with `--control-url`,
`--control-token`, and `--relay-id`.

Admins can inspect all Control-created sessions with:

```text
GET /sessions
```

Each row includes the session id, user, controlled device, service, controller
client, status, Relay address, Punch address, and expiry. Session state
mutations require authorization: normal users can claim/bind/close only sessions
for their own controlled devices, while admins can close any session:

```text
POST /agent/sessions/{session_id}/claim
POST /agent/sessions/{session_id}/bound
POST /sessions/{session_id}/close
```

Print an admin token without starting the HTTP listener:

```bash
control-server \
  --token-secret dev-secret \
  --relay-addr 127.0.0.1:4443 \
  --punch-addr 127.0.0.1:3478 \
  --print-admin-token admin@example.com
```

Print a Relay registration token without starting the HTTP listener:

```bash
control-server \
  --token-secret dev-secret \
  --relay-addr 127.0.0.1:4443 \
  --punch-addr 127.0.0.1:3478 \
  --print-relay-token relay_local_001
```

Relay credentials can be managed and rotated by admins:

```json
{
  "relay_id": "relay_local_001",
  "enabled": true
}
```

sent to:

```text
POST /relay-credentials
```

Disable or enable a credential with:

```json
{
  "enabled": false
}
```

sent to:

```text
POST /relay-credentials/{relay_id}/status
```

Rotate a credential with:

```text
POST /relay-credentials/{relay_id}/rotate
```

Rotation increments the Relay credential `token_version`; old Relay control
tokens are rejected after rotation. If a Relay has no credential record yet,
version `1` tokens remain valid for compatibility. Once a credential record is
created, its `enabled` flag and `token_version` become authoritative.

Bootstrap a persistent admin user during Control startup:

```bash
control-server \
  --listen 127.0.0.1:4242 \
  --token-secret dev-secret \
  --relay-addr 127.0.0.1:4443 \
  --punch-addr 127.0.0.1:3478 \
  --state-db /tmp/quic-test-control.sqlite \
  --bootstrap-admin-email admin@example.com \
  --bootstrap-admin-password admin-password-123
```

The same can be configured with `QUIC_TUNNEL_ADMIN_EMAIL` and
`QUIC_TUNNEL_ADMIN_PASSWORD`. The admin user is stored in Control state and can
log in through `/auth/login`; its returned Control token carries the `admin`
role.

Admins can create users without signing a token for them:

```json
{
  "email": "member@example.com",
  "password": "password-123",
  "display_name": "Member",
  "role": "user",
  "enabled": true
}
```

sent to:

```text
POST /users
```

Use `role: "admin"` to create an administrator. User account management only
accepts `user` and `admin`; Relay credentials are managed separately from human
users. Update an existing user's role with:

```json
{
  "role": "admin"
}
```

sent to:

```text
POST /users/{user_id}/role
```

Enable strict auth when anonymous development access should be rejected:

```bash
control-server \
  --listen 127.0.0.1:4242 \
  --token-secret dev-secret \
  --relay-addr 127.0.0.1:4443 \
  --punch-addr 127.0.0.1:3478 \
  --strict-auth
```

`QUIC_TUNNEL_STRICT_AUTH=true` can also be used. The local smoke scripts keep
strict auth disabled so their placeholder-token flow continues to work.

The Control server also serves a zero-build admin page:

```text
http://127.0.0.1:4242/admin
```

The page can register/login a user, manage controller devices, inspect
controlled devices, view a dashboard summary, list/disable users, inspect
sessions/audit/usage with pagination controls, update user plans with an admin
token, manage plan templates, assign a template to a user, and manage the Relay
pool. The Users table supports row selection; selecting a user fills the user,
plan, usage-reset, and device-access user id inputs and renders that user's
controller/controlled-device detail. The Plan Catalog table supports selecting a
template into both plan editors before saving or assigning it. The Relay Pool
and Relay Credential tables also support row selection, so update/delete,
credential status, and rotation actions can reuse the selected Relay id.
Controlled device rows support selection as well: selecting a device fills the
device id, renders device detail, loads services, and loads access grants when
an admin token is available. Access grant rows can be selected to refill the
device/user pair for revoke. The Operations panel records recent success/failure
context, and admin management mutations refresh the Dashboard and Audit Logs
panels after they complete.

Agent and Mobile control-mode clients pass signed Control tokens through their
existing token flags:

```text
agentd --agent-token <control_access_token>
mobile-cli open-service --token <control_access_token>
```

`mobile-cli` also exposes simple admin list queries that return JSON and reuse
the Rust `HttpControlClient` query helpers:

```bash
mobile-cli admin users \
  --control http://127.0.0.1:4242 \
  --token <admin_token> \
  --q alice \
  --role admin \
  --limit 50

mobile-cli admin usage \
  --control http://127.0.0.1:4242 \
  --token <admin_token> \
  --sort actual_total_bytes \
  --limit 20

mobile-cli admin device-access \
  --control http://127.0.0.1:4242 \
  --token <admin_token> \
  --device server_001
```

Available admin list subcommands are `users`, `audit`, `usage`, `sessions`,
`controllers`, `devices`, `plan-catalog`, `relay-credentials`, `relays`, and
`device-access`.

The same `admin` namespace also exposes common management mutations:

```bash
mobile-cli admin create-user \
  --control http://127.0.0.1:4242 \
  --token <admin_token> \
  --email member@example.com \
  --password password-123 \
  --name Member \
  --role user

mobile-cli admin assign-plan \
  --control http://127.0.0.1:4242 \
  --token <admin_token> \
  --user-id user_abc \
  --plan-id team

mobile-cli admin grant-device-access \
  --control http://127.0.0.1:4242 \
  --token <admin_token> \
  --device server_001 \
  --user-id user_abc
```

Mutation subcommands are `create-user`, `set-user-status`, `assign-plan`,
`register-relay`, `update-relay`, `grant-device-access`, and
`revoke-device-access`. Commands that return a resource print JSON; revoke
prints `{"ok":true}` on success.

The local scripts still pass placeholder tokens; those are ignored by the HTTP
control client and keep using the seeded development user.

Run Control with persistent local state by passing a SQLite file:

```bash
control-server \
  --listen 127.0.0.1:4242 \
  --token-secret dev-secret \
  --relay-addr 127.0.0.1:4443 \
  --punch-addr 127.0.0.1:3478 \
  --state-db /tmp/quic-test-control.sqlite
```

The current model separates:

```text
Controlled device: the Agent/server being accessed
Controller device: the phone/laptop/client that opens a local forward
Plan: controller-device count and Relay limits
Relay pool: registered Relay nodes selected by Control when a session is created
```

When Mobile creates a session, Control now selects a Relay node from the pool
and signs the session Relay token with the current plan limits. New session
creation is also blocked when the user's reported actual Relay traffic has
reached the current plan traffic quota.

Controller devices are user-scoped. Listing returns the current user's
controller devices; deleting a controller frees one slot under the user's
current plan limit.

Controlled devices are also user-scoped. Deleting a controlled device removes
its registered services, P2P certificate, and pending agent session assignments
from Control state.

Admins can grant a user access to another user's controlled device:

```json
{
  "user_id": "user_abc"
}
```

sent to:

```text
POST /devices/{device_id}/access
```

List and revoke access grants with:

```text
GET    /devices/{device_id}/access
DELETE /devices/{device_id}/access/{user_id}
```

An access grant lets the target user see the controlled device through
`GET /mobile/devices`, inspect its services, and create sessions to those
services. It does not grant Agent-side authority: only the controlled device
owner or an admin can poll/claim/bind `/agent/devices/{device_id}/sessions`.
The user who creates a session is recorded as that session's user for Relay
tokens, usage summaries, and close authorization.

`GET /plans/current` returns the current user's plan. Admin-only plan catalog
routes store reusable plan templates:

```text
GET  /plans/catalog
GET  /plans/catalog/{plan_id}
POST /plans/catalog
```

`POST /plans/catalog` wraps the full `Plan`, including controller-device count
and Relay limits:

```json
{
  "plan": {
    "plan_id": "team",
    "name": "Team",
    "max_controller_devices": 4,
    "relay_limits": {
      "max_bps": 8192,
      "max_streams": 12,
      "max_duration_sec": 3600,
      "traffic_quota_bytes": 2097152
    }
  }
}
```

Assign a template to a user with:

```json
{
  "plan_id": "team"
}
```

sent to:

```text
POST /plans/users/{user_id}/assign
```

The assignment copies the current template into that user's active plan. Direct
admin-only updates through `GET/POST /plans/users/{user_id}` remain available
for inspecting or overriding a single user's full plan.

Relay pool updates are admin-only and relay-scoped. Updating a Relay changes
its advertised data-plane address, capacity, and health; unhealthy Relays are
skipped when Control creates a session. `admin_addr` is a legacy compatibility
field and should be sent as an empty string in Control-owned Relay management:

```json
{
  "relay_addr": "relay-new.example.com:4443",
  "admin_addr": "",
  "capacity_streams": 32,
  "healthy": false
}
```

## Local smoke stack

Run the full CLI stack smoke test:

```bash
./scripts/e2e-smoke.sh
```

Run the default production-readiness gate before a release candidate:

```bash
./scripts/production-check.sh
```

The full checklist is in `docs/production-readiness.md`.

Run the same stack as background services, then stop it when done:

```bash
./scripts/dev-stack.sh start-all
./scripts/dev-stack.sh status
./scripts/dev-stack.sh stop
```

For manual browser/curl testing, keep the stack in the foreground and stop it
with Ctrl-C:

```bash
QUIC_TEST_PATH=fallback ./scripts/dev-stack.sh run-all
```

The background stack starts Echo, Relay, Punch, Control, Agent, and Mobile in
order. It verifies both raw TCP forwarding (`hello` -> `world`) and HTTP
forwarding. The forwarded test service is available at:

```text
http://127.0.0.1:18080
```

The Control Admin page is available at:

```text
http://127.0.0.1:4242/admin
```

Print a matching admin token for the local stack:

```bash
./scripts/dev-stack.sh admin-token
```

With Control running, the dev stack can also call the admin CLI directly. These
helpers generate the local admin token automatically, unless
`QUIC_TEST_ADMIN_TOKEN` is already set:

```bash
./scripts/dev-stack.sh admin-users --limit 20
./scripts/dev-stack.sh admin-usage --sort actual_total_bytes --limit 20
./scripts/dev-stack.sh admin-devices --limit 20
./scripts/dev-stack.sh admin-relays --healthy true
./scripts/dev-stack.sh admin-create-user \
  --email user@example.com \
  --password user-password-123 \
  --name "Test User"
./scripts/dev-stack.sh admin-grant-device-access \
  --device pc_001 \
  --user-id user_123
./scripts/dev-stack.sh admin-device-access --device pc_001
./scripts/dev-stack.sh admin-revoke-device-access \
  --device pc_001 \
  --user-id user_123
```

Print a matching Relay registration token for the local Relay id:

```bash
./scripts/dev-stack.sh relay-token
```

Or start Control with a persistent admin login for the simple Control Admin
page:

```bash
QUIC_TEST_CONTROL_ADMIN_EMAIL=admin@example.com \
QUIC_TEST_CONTROL_ADMIN_PASSWORD=admin-password-123 \
./scripts/dev-stack.sh start-control
```

Control state is persisted by default under the dev stack state directory:

```text
${TMPDIR:-/tmp}/quic-test-dev-stack/control-state.sqlite
```

Control-owned Relay management uses the Control Admin page as the normal
operator surface:

```text
http://127.0.0.1:4242/admin
```

Control-owned Relay Live Ops keeps Relay session operations on the same trust
boundary. `relayd` includes session snapshots in its Control heartbeat; Control
caches those snapshots for the Admin Relay detail view. When an operator
disconnects a Relay session, Control queues a relay-scoped command and `relayd`
polls, executes it against its local session store, and reports the result.
Browsers and operators do not call Relay Admin HTTP in the production flow.

`relayd` keeps its own admin HTTP routes disabled by default. For one-off local
debugging only, start it with `--debug-admin-listen 127.0.0.1:9090` and keep the
listener bound to loopback.

To force Relay fallback and make Relay session stats visible in Control Admin:

```bash
QUIC_TEST_PATH=fallback ./scripts/dev-stack.sh start-all
```

To exercise Relay self-registration into the Control Relay pool:

```bash
QUIC_TEST_RELAY_CONTROL_REGISTER=1 ./scripts/dev-stack.sh start-all
```

When Relay self-registration is enabled, the dev stack prints a dedicated
Relay control token with `./scripts/dev-stack.sh relay-token` and passes it to
`relayd`. `relayd` also sends a Control heartbeat and Relay session usage
report every 30 seconds by default. Override it with:

```bash
QUIC_TEST_RELAY_CONTROL_REGISTER=1 \
QUIC_TEST_RELAY_HEARTBEAT_INTERVAL_SEC=10 \
./scripts/dev-stack.sh start-all
```

## Relay Tunnel MVP

Start Relay and Control:

```bash
cargo run -p relayd -- \
  --bind 127.0.0.1:4443 \
  --token-secret dev-secret \
  --cert-out relay.der
```

If you need the local debug-only Relay Admin API for troubleshooting, start
`relayd` with `--debug-admin-listen 127.0.0.1:9090` and query it from the same
host:

```bash
curl http://127.0.0.1:9090/admin/sessions
curl http://127.0.0.1:9090/admin/sessions/sess_001
curl -X POST http://127.0.0.1:9090/admin/sessions/sess_001/disconnect
```

```bash
cargo run -p punch-server -- \
  --bind 127.0.0.1:3478
```

```bash
cargo run -p control-server -- \
  --listen 127.0.0.1:4242 \
  --token-secret dev-secret \
  --relay-addr 127.0.0.1:4443 \
  --punch-addr 127.0.0.1:3478 \
  --state-db /tmp/quic-test-control.sqlite
```

Optionally start `relayd` after Control is running and let it register itself
into the Control Relay pool. First print a dedicated Relay registration token:

```bash
cargo run -p control-server -- \
  --token-secret dev-secret \
  --relay-addr 127.0.0.1:4443 \
  --punch-addr 127.0.0.1:3478 \
  --print-relay-token relay_local_001
```

Then pass it to `relayd`:

```bash
cargo run -p relayd -- \
  --bind 127.0.0.1:4443 \
  --token-secret dev-secret \
  --cert-out relay.der \
  --control-url http://127.0.0.1:4242 \
  --control-token "$RELAY_TOKEN" \
  --relay-id relay_local_001 \
  --advertise-addr 127.0.0.1:4443 \
  --capacity-streams 128 \
  --heartbeat-interval-sec 30
```

With `--control-url`, `--control-token`, and `--relay-id`, `relayd` sends both
Control heartbeat updates and Relay session usage reports. Control records each
Relay's `last_seen_epoch_sec`. Relays that have not been updated for more than
90 seconds are returned as unhealthy and skipped when Control creates new
sessions. Control `/admin` is the default management plane for Relay state.

Register an Agent with Control. The Agent polls Control for sessions and binds
Relay automatically when Mobile creates a session:

```bash
cargo run -p agentd -- \
  --control http://127.0.0.1:4242 \
  --device pc_001 \
  --relay-cert relay.der \
  --service svc_web_3000=127.0.0.1:3000 \
  --mobile-invite-service svc_web_3000 \
  --mobile-grants-file agentd-mobile-grants.json
```

When `--mobile-invite-service` is present, `agentd` prints a JSON mobile invite
and keeps the corresponding invite/grant manager in the server-agent local grant
file when `--mobile-grants-file` is provided. That file contains local pairing
secrets and grant revocation state and is written with private file permissions
on Unix platforms. Control only queues pairing/session requests; it does not
store invite or grant secrets. The agent verifies and approves requests locally.

The invite/grant file can also be managed without starting the long-running
agent process:

```bash
cargo run -p agentd -- mobile-invite create \
  --mobile-grants-file agentd-mobile-grants.json \
  --control http://127.0.0.1:4242 \
  --device pc_001 \
  --service svc_web_3000 \
  --p2p-identity-dir agent-p2p-identity

cargo run -p agentd -- mobile-invite list \
  --mobile-grants-file agentd-mobile-grants.json

cargo run -p agentd -- mobile-invite revoke \
  --mobile-grants-file agentd-mobile-grants.json \
  --invite-id inv_...

cargo run -p agentd -- mobile-grant list \
  --mobile-grants-file agentd-mobile-grants.json

cargo run -p agentd -- mobile-grant revoke \
  --mobile-grants-file agentd-mobile-grants.json \
  --grant-id gr_...
```

When the invite is created with a P2P identity, it includes the agent P2P
certificate fingerprint. The mobile grant carries that fingerprint forward and
the mobile connector rejects grant sessions whose Control-returned agent P2P
certificate does not match.

Pair a mobile client with that invite and store the resulting long-lived grant:

```bash
cargo run -p mobile-cli -- pair \
  --invite-file invite.json \
  --grant-file mobile-grant.json \
  --client mobile_001 \
  --service svc_web_3000
```

Open a local Mobile forward. In Control mode, `mobile-cli` uses
`quic_tunnel_sdk::MobileTunnelSdk`; the SDK asks Control for a session before it
opens the P2P-or-Relay stream:

```bash
cargo run -p mobile-cli -- open-service \
  --control http://127.0.0.1:4242 \
  --token user-token \
  --client mobile_001 \
  --device pc_001 \
  --service svc_web_3000 \
  --local 18080 \
  --relay-cert relay.der \
  --control-request-timeout-ms 5000 \
  --control-max-retries 2 \
  --control-retry-backoff-ms 100
```

The same forward can use the stored mobile grant instead of a Control user
token:

```bash
cargo run -p mobile-cli -- open-service \
  --control http://127.0.0.1:4242 \
  --grant-file mobile-grant.json \
  --client mobile_001 \
  --device pc_001 \
  --service svc_web_3000 \
  --local 18080 \
  --relay-cert relay.der
```

For direct Relay debugging, pass the session and relay token explicitly. This
path intentionally stays on the lower-level connector API:

```bash
cargo run -p agentd -- \
  --relay 127.0.0.1:4443 \
  --relay-cert relay.der \
  --session sess_001 \
  --relay-token "$RELAY_TOKEN" \
  --service svc_web_3000=127.0.0.1:3000
```

```bash
cargo run -p mobile-cli -- open-service \
  --control http://127.0.0.1:4242 \
  --token user-token \
  --client mobile_001 \
  --device pc_001 \
  --service svc_web_3000 \
  --local 18080 \
  --relay 127.0.0.1:4443 \
  --relay-cert relay.der \
  --session sess_001 \
  --relay-token "$RELAY_TOKEN"
```
