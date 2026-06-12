# Rust QUIC Tunnel Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a Rust-first QUIC tunnel MVP where `agent`, `mobile-core`, and shared SDK crates are embeddable by other Rust programs, while `relay`, `control`, `punch`, and Admin API can run as standalone services.

**Architecture:** Core behavior lives in reusable crates with explicit service structs and async APIs. Binary apps only parse config, initialize tracing, compose crates, and run listeners. The first working milestone is Relay Tunnel without P2P; Control, Punch/P2P, Relay controls, and mobile wrappers build on that foundation.

**Tech Stack:** Rust workspace, tokio, quinn, rustls, axum, tower, serde, serde_json, serde_yaml, thiserror, anyhow, tracing, clap, uuid, jsonwebtoken or hmac/sha2, governor or internal token bucket.

---

## Scope Check

The PRD covers several independent subsystems. This plan keeps them in one master roadmap because their public contracts must be designed together, but implementation should proceed in chunks:

1. Workspace and shared crates.
2. Relay Tunnel MVP with hardcoded or fixture tokens.
3. Control Server and real token/session flow.
4. Punch Server and P2P path selection.
5. Relay controls and Admin API.
6. Mobile wrapper preparation.

Do not start P2P before Relay Tunnel is proven end to end.

---

## Architecture Decisions

### Library/Binary Boundary

Every long-running component must have an embeddable library API and a thin standalone binary:

```text
crates/agent        -> apps/agentd
crates/mobile-core  -> apps/mobile-cli
crates/relay        -> apps/relayd
crates/control      -> apps/control-server
crates/punch        -> apps/punch-server
```

The library exposes:

```rust
pub struct Config { /* component-specific */ }
pub struct Service { /* component runtime */ }

impl Service {
    pub async fn new(config: Config) -> Result<Self, Error>;
    pub async fn run(self, shutdown: impl Future<Output = ()> + Send) -> Result<(), Error>;
}
```

The binary only:

```text
1. Parses CLI args.
2. Loads config.
3. Initializes tracing.
4. Creates the service.
5. Calls run().
```

This keeps Agent and Mobile SDK usable inside other Rust programs and keeps server components deployable as separate daemons.

### Phase 1 Transport Boundary

P2P is not part of the first runnable milestone. The first end-to-end flow is:

```text
mobile-cli local listener
  -> relayd QUIC
  -> agentd QUIC
  -> local HTTP service
```

`mobile-core` should still expose a `PathSelector` abstraction from the beginning:

```rust
pub enum TunnelPath {
    Relay,
    P2p,
}
```

Phase 1 always resolves to `TunnelPath::Relay`; later P2P can fill the same interface.

### Service Composition

Use dependency traits for embeddable use:

```rust
#[async_trait::async_trait]
pub trait ControlClient: Send + Sync {
    async fn create_session(&self, request: CreateSessionRequest) -> Result<CreateSessionResponse, ControlError>;
}
```

Standalone binaries use HTTP clients. Tests can use in-memory fake clients.

---

## File Map

### Workspace

- Create: `Cargo.toml`  
  Workspace members and shared dependency versions.
- Create: `.gitignore`  
  Rust build output and local config exclusions.
- Create: `rust-toolchain.toml`  
  Pin stable Rust channel.
- Create: `README.md`  
  Local development and MVP run commands.

### Shared Crates

- Create: `crates/protocol/Cargo.toml`
- Create: `crates/protocol/src/lib.rs`
- Create: `crates/protocol/src/ids.rs`  
  Newtype IDs: `SessionId`, `DeviceId`, `ClientId`, `ServiceId`, `StreamId`.
- Create: `crates/protocol/src/model.rs`  
  Shared models: device, service, session, candidate, relay limits, stats.
- Create: `crates/protocol/src/frame.rs`  
  Control frames and data stream header encode/decode.
- Create: `crates/protocol/src/error.rs`  
  Wire-level error codes.
- Create: `crates/protocol/tests/frame_roundtrip.rs`

- Create: `crates/auth/Cargo.toml`
- Create: `crates/auth/src/lib.rs`
- Create: `crates/auth/src/token.rs`  
  Token claims and signing/verification.
- Create: `crates/auth/tests/token_roundtrip.rs`

- Create: `crates/tunnel/Cargo.toml`
- Create: `crates/tunnel/src/lib.rs`
- Create: `crates/tunnel/src/quic.rs`  
  Quinn endpoint/client/server helpers.
- Create: `crates/tunnel/src/stream.rs`  
  Data stream open/read header helpers.
- Create: `crates/tunnel/src/copy.rs`  
  Bidirectional copy with byte accounting.
- Create: `crates/tunnel/src/stats.rs`  
  Atomic traffic counters.
- Create: `crates/tunnel/tests/stream_header.rs`

### Embeddable Agent

- Create: `crates/agent/Cargo.toml`
- Create: `crates/agent/src/lib.rs`
- Create: `crates/agent/src/config.rs`
- Create: `crates/agent/src/service_registry.rs`
- Create: `crates/agent/src/runtime.rs`  
  Embeddable `Agent` service.
- Create: `crates/agent/src/relay_client.rs`
- Create: `crates/agent/src/stream_handler.rs`
- Create: `crates/agent/tests/service_registry.rs`
- Create: `apps/agentd/Cargo.toml`
- Create: `apps/agentd/src/main.rs`

### Embeddable Mobile SDK Core

- Create: `crates/mobile-core/Cargo.toml`
- Create: `crates/mobile-core/src/lib.rs`
- Create: `crates/mobile-core/src/config.rs`
- Create: `crates/mobile-core/src/client.rs`
- Create: `crates/mobile-core/src/forward.rs`
- Create: `crates/mobile-core/src/path.rs`
- Create: `crates/mobile-core/src/status.rs`
- Create: `crates/mobile-core/src/events.rs`
- Create: `crates/mobile-core/src/ffi.rs`
- Create: `crates/mobile-core/tests/local_forward_config.rs`
- Create: `apps/mobile-cli/Cargo.toml`
- Create: `apps/mobile-cli/src/main.rs`

### Standalone Relay Service

- Create: `crates/relay/Cargo.toml`
- Create: `crates/relay/src/lib.rs`
- Create: `crates/relay/src/config.rs`
- Create: `crates/relay/src/runtime.rs`
- Create: `crates/relay/src/session.rs`
- Create: `crates/relay/src/bind.rs`
- Create: `crates/relay/src/forward.rs`
- Create: `crates/relay/src/limiter.rs`
- Create: `crates/relay/src/admin.rs`
- Create: `crates/relay/tests/session_binding.rs`
- Create: `apps/relayd/Cargo.toml`
- Create: `apps/relayd/src/main.rs`

### Standalone Control Service

- Create: `crates/control/Cargo.toml`
- Create: `crates/control/src/lib.rs`
- Create: `crates/control/src/config.rs`
- Create: `crates/control/src/state.rs`
- Create: `crates/control/src/routes.rs`
- Create: `crates/control/src/session.rs`
- Create: `crates/control/src/token_issuer.rs`
- Create: `crates/control/tests/session_flow.rs`
- Create: `apps/control-server/Cargo.toml`
- Create: `apps/control-server/src/main.rs`

### Standalone Punch Service

- Create: `crates/punch/Cargo.toml`
- Create: `crates/punch/src/lib.rs`
- Create: `crates/punch/src/config.rs`
- Create: `crates/punch/src/server.rs`
- Create: `crates/punch/src/candidate.rs`
- Create: `crates/punch/src/probe.rs`
- Create: `crates/punch/tests/candidate_store.rs`
- Create: `apps/punch-server/Cargo.toml`
- Create: `apps/punch-server/src/main.rs`

### End-to-End Tests

- Create: `tests/e2e_relay_tunnel.rs`
- Create: `tests/e2e_control_relay.rs`
- Create: `tests/support/mod.rs`
- Create: `tests/support/http_echo.rs`
- Create: `tests/support/process.rs`

---

## Chunk 1: Workspace And Shared Contracts

### Task 1: Initialize Rust workspace

**Files:**
- Create: `Cargo.toml`
- Create: `.gitignore`
- Create: `rust-toolchain.toml`
- Create: `README.md`

- [ ] **Step 1: Create workspace manifest**

Use a workspace with resolver 2 and path members for all crates/apps.

```toml
[workspace]
resolver = "2"
members = [
  "crates/protocol",
  "crates/auth",
  "crates/tunnel",
  "crates/agent",
  "crates/mobile-core",
  "crates/relay",
  "crates/control",
  "crates/punch",
  "apps/agentd",
  "apps/mobile-cli",
  "apps/relayd",
  "apps/control-server",
  "apps/punch-server",
]

[workspace.package]
edition = "2021"
version = "0.1.0"
license = "UNLICENSED"

[workspace.dependencies]
anyhow = "1"
async-trait = "0.1"
axum = "0.8"
bytes = "1"
clap = { version = "4", features = ["derive", "env"] }
quinn = "0.11"
rcgen = "0.13"
rustls = "0.23"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
serde_yaml = "0.9"
thiserror = "2"
tokio = { version = "1", features = ["full"] }
tower = "0.5"
tower-http = "0.6"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "fmt"] }
uuid = { version = "1", features = ["v4", "serde"] }
```

- [ ] **Step 2: Add workspace metadata files**

`.gitignore` should include:

```gitignore
/target
/.env
*.local.yaml
*.local.toml
```

`rust-toolchain.toml`:

```toml
[toolchain]
channel = "stable"
```

- [ ] **Step 3: Add README bootstrap notes**

Record the intended first commands without claiming they pass before member crates exist:

````markdown
# QUIC Tunnel

Rust workspace for embeddable Agent/Mobile SDK crates and standalone Relay,
Control, Punch, and Admin API services.

Initial verification after crate skeletons exist:

```bash
cargo metadata --format-version 1
cargo check --workspace
```
````

- [ ] **Step 4: Commit**

If the directory is a Git repo:

```bash
git add Cargo.toml .gitignore rust-toolchain.toml README.md
git commit -m "chore: initialize rust workspace"
```

If it is not a Git repo, record the changed files in the task summary.

### Task 2: Create empty crate/app skeletons

**Files:**
- Create every `Cargo.toml` and `src/lib.rs` or `src/main.rs` listed in the File Map.

- [ ] **Step 1: Create protocol crate manifest**

```toml
[package]
name = "quic_tunnel_protocol"
edition.workspace = true
version.workspace = true
license.workspace = true

[dependencies]
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
uuid.workspace = true
```

- [ ] **Step 2: Create app manifests**

Each app package depends on its matching library crate plus `anyhow`, `clap`, `tokio`, `tracing`, and `tracing-subscriber`.

- [ ] **Step 3: Add placeholder exports**

Each library `src/lib.rs` should compile with a minimal module export or placeholder type.

- [ ] **Step 4: Verify workspace compiles**

Run:

```bash
cargo check --workspace
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates apps Cargo.toml
git commit -m "chore: add workspace crate skeletons"
```

Skip commit if not in a Git repo.

### Task 3: Implement protocol models and frame encoding

**Files:**
- Create: `crates/protocol/src/ids.rs`
- Create: `crates/protocol/src/model.rs`
- Create: `crates/protocol/src/frame.rs`
- Create: `crates/protocol/src/error.rs`
- Modify: `crates/protocol/src/lib.rs`
- Test: `crates/protocol/tests/frame_roundtrip.rs`

- [ ] **Step 1: Write failing frame roundtrip tests**

Test:

```rust
use quic_tunnel_protocol::{DataStreamHeader, ServiceId, SessionId, StreamId};

#[test]
fn data_stream_header_roundtrips_with_length_prefix() {
    let header = DataStreamHeader {
        stream_id: StreamId::new("stream_001"),
        session_id: SessionId::new("sess_001"),
        service_id: ServiceId::new("svc_web_3000"),
    };

    let bytes = header.encode_with_len_prefix().unwrap();
    let decoded = DataStreamHeader::decode_with_len_prefix(&bytes).unwrap();

    assert_eq!(decoded, header);
}
```

- [ ] **Step 2: Run test and verify it fails**

Run:

```bash
cargo test -p quic_tunnel_protocol --test frame_roundtrip
```

Expected: FAIL because protocol types are missing.

- [ ] **Step 3: Implement ID newtypes**

Provide transparent wrappers with `Clone`, `Debug`, `Eq`, `Hash`, `Serialize`, `Deserialize`.

- [ ] **Step 4: Implement data stream header**

Implement `u32_be header_len + JSON header` encode/decode. Reject headers larger than 64 KiB.

- [ ] **Step 5: Implement control frames and errors**

Add enum variants for `Hello`, `Auth`, `Ping`, `Pong`, `RelayBind`, `SessionReady`, `SessionClosed`, `Error`, `TrafficReport`.

- [ ] **Step 6: Run tests**

Run:

```bash
cargo test -p quic_tunnel_protocol
cargo check --workspace
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/protocol
git commit -m "feat: add shared protocol models"
```

### Task 4: Implement token claims

**Files:**
- Create: `crates/auth/src/token.rs`
- Modify: `crates/auth/src/lib.rs`
- Test: `crates/auth/tests/token_roundtrip.rs`

- [ ] **Step 1: Write failing token test**

Test signing and verifying `RelayTokenClaims` with session limits.

- [ ] **Step 2: Implement claims structs**

Use protocol ID types for `session_id`, `client_id`, `device_id`, and `service_id`.

- [ ] **Step 3: Implement MVP signer**

Use HMAC-SHA256 or `jsonwebtoken` HS256. Keep key injection explicit through `TokenKey`.

- [ ] **Step 4: Verify expired token rejection**

Add a test with an `exp` in the past.

- [ ] **Step 5: Run**

```bash
cargo test -p quic_tunnel_auth
cargo check --workspace
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/auth
git commit -m "feat: add relay token claims"
```

---

## Chunk 2: Relay Tunnel MVP

### Task 5: Implement tunnel stream helpers

**Files:**
- Create: `crates/tunnel/src/stream.rs`
- Create: `crates/tunnel/src/copy.rs`
- Create: `crates/tunnel/src/stats.rs`
- Modify: `crates/tunnel/src/lib.rs`
- Test: `crates/tunnel/tests/stream_header.rs`

- [ ] **Step 1: Write failing stream header test**

Test that a header can be written to an async byte stream and read back before payload bytes.

- [ ] **Step 2: Implement stream header read/write**

Expose:

```rust
pub async fn write_data_header<W>(writer: &mut W, header: &DataStreamHeader) -> Result<(), TunnelError>
where
    W: tokio::io::AsyncWrite + Unpin;

pub async fn read_data_header<R>(reader: &mut R) -> Result<DataStreamHeader, TunnelError>
where
    R: tokio::io::AsyncRead + Unpin;
```

- [ ] **Step 3: Implement byte accounting copy**

Wrap `tokio::io::copy_bidirectional` and increment `TrafficStats`.

- [ ] **Step 4: Run**

```bash
cargo test -p quic_tunnel_tunnel
cargo check --workspace
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/tunnel
git commit -m "feat: add tunnel stream helpers"
```

### Task 6: Implement Agent embeddable service registry

**Files:**
- Create: `crates/agent/src/config.rs`
- Create: `crates/agent/src/service_registry.rs`
- Modify: `crates/agent/src/lib.rs`
- Test: `crates/agent/tests/service_registry.rs`

- [ ] **Step 1: Write failing service lookup tests**

Verify lookup by `ServiceId`, duplicate service rejection, and invalid target rejection.

- [ ] **Step 2: Implement config structs**

Expose:

```rust
pub struct AgentConfig {
    pub device_id: DeviceId,
    pub control_server: String,
    pub auth_token: String,
    pub services: Vec<ServiceConfig>,
}
```

- [ ] **Step 3: Implement `ServiceRegistry`**

Make it cloneable and read-only after construction.

- [ ] **Step 4: Run**

```bash
cargo test -p quic_tunnel_agent
cargo check --workspace
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/agent
git commit -m "feat: add agent service registry"
```

### Task 7: Implement Agent stream handler

**Files:**
- Create: `crates/agent/src/stream_handler.rs`
- Modify: `crates/agent/src/lib.rs`
- Test: `crates/agent/tests/stream_handler.rs`

- [ ] **Step 1: Write failing stream handler test**

Start a local `TcpListener` echo server, create an in-memory duplex stream with an `OPEN_STREAM` header, and verify bytes are proxied to the target service.

- [ ] **Step 2: Implement handler**

Expose:

```rust
pub async fn handle_data_stream<S>(
    stream: S,
    registry: ServiceRegistry,
) -> Result<(), AgentError>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static;
```

- [ ] **Step 3: Add error mapping**

Return `SERVICE_NOT_FOUND` for missing services and `SERVICE_DIAL_FAILED` for failed TCP dials.

- [ ] **Step 4: Run**

```bash
cargo test -p quic_tunnel_agent
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/agent
git commit -m "feat: proxy agent streams to local services"
```

### Task 8: Implement Relay session binding

**Files:**
- Create: `crates/relay/src/session.rs`
- Create: `crates/relay/src/bind.rs`
- Modify: `crates/relay/src/lib.rs`
- Test: `crates/relay/tests/session_binding.rs`

- [ ] **Step 1: Write failing binding tests**

Cases:

```text
1. mobile bind alone leaves session waiting.
2. agent bind alone leaves session waiting.
3. mobile + agent for same session marks ready.
4. duplicate role bind is rejected.
5. mismatched token/session is rejected.
```

- [ ] **Step 2: Implement RelaySessionStore**

Use `Arc<RwLock<HashMap<SessionId, RelaySession>>>` for MVP.

- [ ] **Step 3: Implement token verification hook**

Accept a trait:

```rust
pub trait RelayTokenVerifier: Send + Sync {
    fn verify(&self, token: &str) -> Result<RelayTokenClaims, RelayError>;
}
```

- [ ] **Step 4: Run**

```bash
cargo test -p quic_tunnel_relay
cargo check --workspace
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/relay
git commit -m "feat: bind relay session peers"
```

### Task 9: Implement Relay QUIC runtime and forwarder

**Files:**
- Create: `crates/relay/src/runtime.rs`
- Create: `crates/relay/src/forward.rs`
- Modify: `crates/relay/src/lib.rs`
- Create: `apps/relayd/src/main.rs`
- Test: `crates/relay/tests/session_binding.rs`

- [ ] **Step 1: Add library service API**

Expose:

```rust
pub struct RelayService {
    // config, endpoint, sessions
}

impl RelayService {
    pub async fn new(config: RelayConfig) -> Result<Self, RelayError>;
    pub async fn run_until(self, shutdown: impl Future<Output = ()> + Send) -> Result<(), RelayError>;
}
```

- [ ] **Step 2: Implement binary wrapper**

`apps/relayd` parses config and starts `RelayService`.

- [ ] **Step 3: Implement stream forwarding**

When a mobile opens a data stream, read the data header, open a matching stream to the bound agent, write the same header, and bidirectionally copy bytes.

- [ ] **Step 4: Run**

```bash
cargo check -p relayd
cargo test -p quic_tunnel_relay
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/relay apps/relayd
git commit -m "feat: run relay quic service"
```

### Task 10: Implement Mobile core relay path and local forward

**Files:**
- Create: `crates/mobile-core/src/config.rs`
- Create: `crates/mobile-core/src/client.rs`
- Create: `crates/mobile-core/src/forward.rs`
- Create: `crates/mobile-core/src/path.rs`
- Create: `crates/mobile-core/src/status.rs`
- Create: `crates/mobile-core/src/events.rs`
- Modify: `crates/mobile-core/src/lib.rs`
- Create: `apps/mobile-cli/src/main.rs`
- Test: `crates/mobile-core/tests/local_forward_config.rs`

- [ ] **Step 1: Write failing config/status tests**

Validate `TunnelConfig`, `OpenServiceRequest`, and default path selection returns `Relay`.

- [ ] **Step 2: Implement embeddable client API**

Expose:

```rust
pub struct TunnelClient { /* internal state */ }

impl TunnelClient {
    pub async fn start(config: TunnelConfig) -> Result<Self, TunnelError>;
    pub async fn open_service(&self, request: OpenServiceRequest) -> Result<LocalForwardHandle, TunnelError>;
    pub async fn close_service(&self, handle_id: String) -> Result<(), TunnelError>;
    pub fn status(&self) -> TunnelStatus;
}
```

- [ ] **Step 3: Implement local listener**

`open_service` binds `127.0.0.1:{local_port}` and spawns accept loop. Each TCP connection opens a relay QUIC data stream.

- [ ] **Step 4: Implement mobile-cli**

Commands:

```text
mobile-cli open-service --relay https://127.0.0.1:4433 --session sess_001 --token dev-token --service svc_web_3000 --local 18080
```

- [ ] **Step 5: Run**

```bash
cargo test -p quic_tunnel_mobile_core
cargo check -p mobile-cli
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/mobile-core apps/mobile-cli
git commit -m "feat: add mobile relay local forward"
```

### Task 11: Add Relay Tunnel end-to-end test

**Files:**
- Create: `tests/e2e_relay_tunnel.rs`
- Create: `tests/support/mod.rs`
- Create: `tests/support/http_echo.rs`
- Create: `tests/support/process.rs`

- [ ] **Step 1: Write failing E2E test**

Test should:

```text
1. Start local HTTP echo service on random port.
2. Start RelayService on random UDP port.
3. Start Agent configured with service_id -> echo port.
4. Start TunnelClient with local random port.
5. HTTP GET local forwarded port.
6. Assert response came from echo service.
```

- [ ] **Step 2: Run and verify failure**

```bash
cargo test --test e2e_relay_tunnel -- --nocapture
```

Expected: FAIL until all runtime pieces are wired.

- [ ] **Step 3: Wire Agent relay client runtime**

Implement missing Agent `RelayClient` connection handling.

- [ ] **Step 4: Make E2E pass**

```bash
cargo test --test e2e_relay_tunnel -- --nocapture
```

Expected: PASS.

- [ ] **Step 5: Run workspace tests**

```bash
cargo test --workspace
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add tests crates/agent crates/relay crates/mobile-core
git commit -m "test: cover relay tunnel end to end"
```

---

## Chunk 3: Control Server And Real Session Flow

### Task 12: Implement Control state and routes

**Files:**
- Create: `crates/control/src/state.rs`
- Create: `crates/control/src/routes.rs`
- Create: `crates/control/src/session.rs`
- Create: `crates/control/src/token_issuer.rs`
- Create: `apps/control-server/src/main.rs`
- Test: `crates/control/tests/session_flow.rs`

- [ ] **Step 1: Write failing route tests**

Use `tower::ServiceExt` to call axum routes in-memory.

Cases:

```text
1. agent registers device.
2. agent registers services.
3. mobile lists devices.
4. mobile creates session.
5. response includes access_token, relay_token, relay_addr, punch_addr.
```

- [ ] **Step 2: Implement `ControlState`**

MVP state is in-memory maps protected by `Arc<RwLock<_>>`.

- [ ] **Step 3: Implement route functions**

Return `Router<ControlState>` and apply `.with_state(state)` in the app/binary layer.

- [ ] **Step 4: Implement token issuer**

Use `quic_tunnel_auth` for relay token claims.

- [ ] **Step 5: Run**

```bash
cargo test -p quic_tunnel_control
cargo check -p control-server
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/control apps/control-server
git commit -m "feat: add control server session flow"
```

### Task 13: Wire Agent and Mobile to Control

**Files:**
- Modify: `crates/agent/src/runtime.rs`
- Modify: `crates/mobile-core/src/client.rs`
- Modify: `crates/mobile-core/src/path.rs`
- Test: `tests/e2e_control_relay.rs`

- [ ] **Step 1: Write failing control-relay E2E test**

The test should no longer pass hardcoded session/token. It should create state through Control API.

- [ ] **Step 2: Implement HTTP control client**

Create small clients inside `agent` and `mobile-core`, or introduce `crates/control-client` if duplication grows.

- [ ] **Step 3: Register Agent from config**

Agent startup posts device and service list to Control.

- [ ] **Step 4: Mobile creates session**

`open_service` calls Control before connecting Relay.

- [ ] **Step 5: Run**

```bash
cargo test --test e2e_control_relay -- --nocapture
cargo test --workspace
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/agent crates/mobile-core tests
git commit -m "feat: use control server for relay sessions"
```

---

## Chunk 4: Relay Controls And Admin API

### Task 14: Implement traffic stats and limits

**Files:**
- Create: `crates/relay/src/limiter.rs`
- Modify: `crates/relay/src/session.rs`
- Modify: `crates/relay/src/forward.rs`
- Test: `crates/relay/tests/limits.rs`

- [ ] **Step 1: Write failing limit tests**

Cases:

```text
1. active streams over max_streams returns MAX_STREAMS_EXCEEDED.
2. traffic over quota returns TRAFFIC_QUOTA_EXCEEDED.
3. duration over max duration closes session.
4. stats count uplink and downlink bytes separately.
```

- [ ] **Step 2: Implement counters**

Use atomic counters or session lock with clear direction accounting:

```text
uplink = mobile -> agent
downlink = agent -> mobile
```

- [ ] **Step 3: Implement token bucket**

Use `governor` or a small internal token bucket. Keep API independent from the implementation.

- [ ] **Step 4: Run**

```bash
cargo test -p quic_tunnel_relay
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/relay
git commit -m "feat: enforce relay session limits"
```

### Task 15: Implement Admin API

**Files:**
- Create: `crates/relay/src/admin.rs`
- Modify: `crates/relay/src/runtime.rs`
- Test: `crates/relay/tests/admin_api.rs`
- Modify: `docs/relay-admin.html` if response field casing changes.

- [ ] **Step 1: Write failing Admin API tests**

Use axum in-memory tests:

```text
GET /sessions/{session_id}/stats
POST /sessions/{session_id}/disconnect
```

- [ ] **Step 2: Implement `admin_router`**

Return `Router<RelayAdminState>`. Apply `.with_state()` in `RelayService`.

- [ ] **Step 3: Align response shape with admin page**

Existing `docs/relay-admin.html` reads `UplinkBytes`, `DownlinkBytes`, `TotalBytes`, `ActiveStreams`, and `Duration`. Either keep this shape or update the page and tests together.

- [ ] **Step 4: Run**

```bash
cargo test -p quic_tunnel_relay
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/relay docs/relay-admin.html
git commit -m "feat: add relay admin api"
```

---

## Chunk 5: Punch And P2P

### Task 16: Implement Punch Server candidate discovery

**Files:**
- Create: `crates/punch/src/server.rs`
- Create: `crates/punch/src/candidate.rs`
- Create: `crates/punch/src/probe.rs`
- Create: `apps/punch-server/src/main.rs`
- Test: `crates/punch/tests/candidate_store.rs`

- [ ] **Step 1: Write failing candidate tests**

Validate that UDP HELLO records peer public address and session metadata.

- [ ] **Step 2: Implement UDP packet model**

Reuse protocol IDs and add punch message structs.

- [ ] **Step 3: Implement candidate store**

Store by `session_id + peer_role`.

- [ ] **Step 4: Run**

```bash
cargo test -p quic_tunnel_punch
cargo check -p punch-server
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/punch apps/punch-server
git commit -m "feat: add punch candidate discovery"
```

### Task 17: Add P2P path selector

**Files:**
- Modify: `crates/mobile-core/src/path.rs`
- Modify: `crates/agent/src/runtime.rs`
- Modify: `crates/tunnel/src/quic.rs`
- Test: `tests/e2e_p2p_local.rs`

- [ ] **Step 1: Write failing local P2P test**

Run mobile and agent on localhost with UDP sockets and verify P2P path is selected before Relay.

- [ ] **Step 2: Implement probe/ack**

Mobile and Agent exchange signed probe messages through UDP.

- [ ] **Step 3: Reuse UDP socket for Quinn Endpoint**

Create Quinn endpoint from the same socket used for successful probe.

- [ ] **Step 4: Add race with Relay**

Start Relay after 300ms if P2P is not ready. First ready path wins.

- [ ] **Step 5: Run**

```bash
cargo test --test e2e_p2p_local -- --nocapture
cargo test --workspace
```

Expected: PASS locally.

- [ ] **Step 6: Commit**

```bash
git add crates/mobile-core crates/agent crates/tunnel tests
git commit -m "feat: add local p2p path selection"
```

---

## Chunk 6: Mobile Wrapper Preparation

### Task 18: Stabilize mobile-core embedding API

**Files:**
- Modify: `crates/mobile-core/src/lib.rs`
- Modify: `crates/mobile-core/src/ffi.rs`
- Test: `crates/mobile-core/tests/api_surface.rs`

- [ ] **Step 1: Write public API compile test**

Use an integration test that imports only public types and drives a fake `TunnelClient`.

- [ ] **Step 2: Ensure runtime ownership is explicit**

Offer two options:

```text
1. `TunnelClient::start` for Rust async programs that already own a tokio runtime.
2. `BlockingTunnelClient` or FFI facade for Swift/Kotlin that need runtime managed internally.
```

- [ ] **Step 3: Add handle-based lifecycle**

FFI-facing API should avoid exposing Rust futures directly.

- [ ] **Step 4: Run**

```bash
cargo test -p quic_tunnel_mobile_core
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/mobile-core
git commit -m "feat: stabilize mobile core embedding api"
```

### Task 19: Document integration modes

**Files:**
- Create: `docs/integration-rust.md`
- Create: `docs/integration-mobile.md`
- Modify: `README.md`

- [ ] **Step 1: Document Rust embedding**

Show how another Rust program uses:

```rust
let client = TunnelClient::start(config).await?;
let handle = client.open_service(request).await?;
```

- [ ] **Step 2: Document standalone service deployment**

List binaries:

```text
control-server
punch-server
relayd
agentd
mobile-cli
```

- [ ] **Step 3: Document mobile wrapper boundary**

State that Swift/Kotlin only wrap Rust mobile-core and do not implement tunnel logic.

- [ ] **Step 4: Commit**

```bash
git add docs README.md
git commit -m "docs: describe integration modes"
```

---

## Final Verification

Before claiming the MVP is complete, run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test --test e2e_relay_tunnel -- --nocapture
cargo test --test e2e_control_relay -- --nocapture
```

After P2P work is implemented, also run:

```bash
cargo test --test e2e_p2p_local -- --nocapture
```

Manual MVP smoke test:

```bash
# terminal 1
cargo run -p control-server -- --config examples/control.local.yaml

# terminal 2
cargo run -p relayd -- --config examples/relay.local.yaml

# terminal 3
cargo run -p agentd -- --config examples/agent.local.yaml

# terminal 4
cargo run -p mobile-cli -- open-service \
  --control http://127.0.0.1:4242 \
  --device pc_001 \
  --service svc_web_3000 \
  --local 18080

# terminal 5
curl http://127.0.0.1:18080
```

Expected:

```text
curl returns the remote local service response.
Relay stats show non-zero uplink/downlink bytes.
Admin disconnect closes the active session.
```

---

## Implementation Notes

1. Keep crate APIs stable before writing binaries.
2. Do not put business logic in `apps/*/main.rs`.
3. Do not introduce Redis until in-memory MVP passes.
4. Do not start mobile Swift/Kotlin wrappers before `mobile-cli` proves the SDK core.
5. Do not start P2P before Relay Tunnel E2E passes.
6. Keep all wire models in `quic_tunnel_protocol`.
7. Keep all token claims in `quic_tunnel_auth`.
8. Prefer tests with in-memory services before process-spawning E2E tests.
