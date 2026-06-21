# SDK 使用指南

本文档说明 MobileCode Connect 的 SDK 接入方式，重点覆盖移动端和受控 server/agent 端。仓库里有两层 SDK：

- `mobilecode_connect_sdk`：Rust 应用优先使用的高层 SDK，封装 Control API、移动 tunnel、server auth 和 server 注册流程。
- `mobilecode_connect_mobile_core::ffi`：面向 iOS/Android 的 UniFFI 接口，生成 Swift/Kotlin bindings 后由平台 wrapper 使用。

## 组件关系

典型部署包含四类组件：

- Control Server：用户登录、设备/服务注册、session 分配、server credential、mobile grant pairing。
- Relay/Punch：为 tunnel 提供 Relay 转发和 P2P 打洞辅助。
- Agent/Server：运行在受控设备上，注册可访问服务，轮询并绑定 session。
- Mobile/Controller：运行在移动端或控制端，发现设备服务，创建 session，并通过 tunnel 或 browser proxy 访问服务。

Rust 侧推荐从 `MobileCodeConnectSdk` 开始。它统一配置 Control URL、用户 token store、server credential store 和 HTTP client options，然后分出 `auth()`、`controller()`、`admin()`、`server_auth()`、`server()` 等子 SDK。

## Rust SDK 初始化

```rust
use std::time::Duration;
use mobilecode_connect_sdk::{HttpControlClientOptions, MobileCodeConnectSdk};

let sdk = MobileCodeConnectSdk::builder()
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
```

存储选择：

- `memory_token_store()` / `MemoryTokenStore`：适合测试和短生命周期工具。
- `token_file(...)` / `FileTokenStore`：适合需要跨进程重启保留用户 Control token 的应用。
- `memory_server_credential_store()` / `MemoryServerCredentialStore`：适合测试。
- `server_credential_file(...)` / `FileServerCredentialStore`：适合 agent/server 持久保存 Control 颁发的 server credential。

`FileTokenStore` 和 `FileServerCredentialStore` 在 Unix 上会把文件权限设置为 `0600`。

## 用户和 Controller 流程

用户登录或注册后，SDK 会保存 Control access token。后续 controller、admin 和用户 token 模式的 mobile tunnel 会从同一个 token store 取 token。

```rust
use mobilecode_connect_sdk::{LoginInput, RegisterControllerInput};
use mobilecode_connect_protocol::ClientId;

let token = sdk.login(LoginInput {
    email: "member@example.com".to_string(),
    password: "password-123".to_string(),
}).await?;

let controller = sdk.ensure_controller(RegisterControllerInput {
    client_id: ClientId::new("phone_001"),
    name: "Phone".to_string(),
}).await?;
```

常用方法：

- `sdk.ensure_login(...)`：已有 token 时复用，否则登录。
- `sdk.ensure_login_fresh(..., now_epoch_sec)`：只复用本地未过期 token。
- `sdk.list_devices()`：列出当前用户可见的受控设备。
- `sdk.list_device_services(&device_id)`：列出设备暴露的服务。
- `sdk.create_session(...)`：为一个 controller、device、service 创建访问 session。

## 移动端：Rust 调用方式

Rust 移动端或 CLI 可以直接用 `MobileTunnelSdk`。

### 用户 token 模式

```rust
use mobilecode_connect_sdk::{
    MobileTunnelConfig, MobileTunnelSdk, OpenServiceInput, P2pOrRelayTunnelConfig,
};
use mobilecode_connect_protocol::{ClientId, DeviceId, ServiceId};

let tunnel = MobileTunnelSdk::start_with_control_p2p_or_relay(
    MobileTunnelConfig {
        control_server_url: "http://127.0.0.1:8080".to_string(),
        client_id: ClientId::new("phone_001"),
        control_client_options: Default::default(),
    },
    sdk.token_store(),
    p2p_or_relay_config,
).await?;

let forward = tunnel.open_service(OpenServiceInput {
    device_id: DeviceId::new("pc_001"),
    service_id: ServiceId::new("svc_web"),
    local_port: 0,
}).await?;

println!("local forward: 127.0.0.1:{}", forward.local_port());
tunnel.close_service(forward.handle_id()).await?;
```

`local_port = 0` 表示让系统分配临时端口。

### Mobile grant 模式

Mobile grant 模式不要求移动端持有用户登录 token。流程如下：

1. Agent/server 生成 invite JSON，里面包含 `control_url`、`device_id`、允许访问的 `services`、`invite_secret` 和过期信息。
2. 移动端用 invite、`client_id`、请求的 services 和随机 nonce 发起 pairing。
3. Agent 或用户批准 pairing 后，移动端轮询得到 `MobileGrantCredential`。
4. 移动端把 grant 存入安全存储，然后用 `start_with_mobile_grant` 启动 tunnel。

```rust
use mobilecode_connect_sdk::{
    MobileGrantPairingInput, MobileTunnelConfig, MobileTunnelSdk, SdkMobileGrantStore,
};
use mobilecode_connect_protocol::{ClientId, ServiceId};

let pairing = MobileTunnelSdk::start_mobile_grant_pairing(
    MobileGrantPairingInput {
        invite,
        client_id: ClientId::new("phone_001"),
        requested_services: vec![ServiceId::new("svc_web")],
        nonce: "random-nonce".to_string(),
    },
    Default::default(),
).await?;

let grant_store = SdkMobileGrantStore::file("state/mobile-grant.json");
let grant = loop {
    if let Some(grant) = MobileTunnelSdk::complete_mobile_grant_pairing_once(
        pairing.clone(),
        grant_store.clone(),
        Default::default(),
    ).await? {
        break grant;
    }
    tokio::time::sleep(std::time::Duration::from_millis(pairing.poll_interval_ms)).await;
};

let tunnel = MobileTunnelSdk::start_with_mobile_grant(
    MobileTunnelConfig {
        control_server_url: grant.control_url.clone(),
        client_id: grant.client_id.clone(),
        control_client_options: Default::default(),
    },
    grant,
    p2p_or_relay_config,
).await?;
```

`start_with_mobile_grant` 会校验 grant 的 `control_url` 和 `client_id` 必须与 tunnel config 一致，且 grant 必须包含 `grant_id`、`grant_secret` 和允许访问的 services。

## 移动端：iOS/Android 原生接入

原生移动端使用 `mobilecode_connect_mobile_core` 生成的 UniFFI bindings。`mobile/ios` 和 `mobile/android` 目录下已经放了平台 package skeleton 和薄 wrapper。

生成 bindings 和平台包：

```bash
cargo install uniffi --version 0.31.1 --features cli --locked
scripts/gen-mobile-bindings.sh --language all
scripts/package-mobile-ios.sh
scripts/package-mobile-android.sh
```

iOS 包布局：

- `mobile/ios/Package.swift`
- `Sources/MobileCodeConnectMobileSdk/Generated/mobilecode_connect_mobile_core.swift`
- `Artifacts/mobilecode_connect_mobile_coreFFI.xcframework`

Android 包布局：

- `src/main/java/uniffi/mobilecode_connect_mobile_core/...`
- `src/main/jniLibs/<abi>/libmobilecode_connect_mobile_core.so`
- `MobileCodeConnectBrowserProxyController.kt`
- `MobileCodeConnectMobileGrantPairingController.kt`
- `MobileCodeConnectMobileGrantSecureStore.kt`

### UniFFI tunnel API

主要 UniFFI 类型和函数：

- `mobileTunnelConfig(userToken, controlServerUrl, clientId)`
- `p2pOrRelayConfigWithDefaults(relayServerCertDer)`
- `FfiMobileTunnel.startInMemory(config)`
- `FfiMobileTunnel.startWithControlP2pOrRelay(config, p2pOrRelay)`
- `FfiMobileTunnel.startWithMobileGrant(config, grant, p2pOrRelay)`
- `FfiMobileTunnel.openService(FfiOpenServiceRequest)`
- `FfiMobileTunnel.startBrowserProxy()`
- `FfiMobileTunnel.shutdown()`

用户 token 模式传入真实 `userToken`。Mobile grant 模式不使用用户 token，但当前 `FfiMobileTunnelConfig` 仍有 `user_token` 字段；调用 `startWithMobileGrant` 时可传空字符串，并依赖 grant 完成授权。

### UniFFI mobile grant pairing

低层函数：

```text
let options = mobileGrantPairingOptionsWithDefaults()
let pairing = try startMobileGrantPairing(
  invite: invite,
  clientId: "phone_001",
  requestedServices: ["svc_web"],
  nonce: "<random nonce>",
  options: options
)
let result = try pollMobileGrantPairingOnce(pairing: pairing, options: options)
```

当 `result.status` 为 `Approved` 时，`result.grant` 是 `FfiMobileGrantCredential`。平台 wrapper 已经提供安全存储：

- iOS：`MobileCodeConnectMobileGrantSecureStore`，使用 Keychain。
- Android：`MobileCodeConnectMobileGrantSecureStore`，使用 Android Keystore AES-GCM 加密后写入 app-private preferences。

### WebView browser proxy

嵌入式浏览器推荐使用 browser proxy，而不是每个服务单独手动 open forward。调用方式：

1. 用 `FfiMobileTunnel.startWithControlP2pOrRelay(...)` 或 `startWithMobileGrant(...)` 启动 tunnel。
2. 创建 `MobileCodeConnectBrowserProxyController(tunnel)`。
3. 调 `startBrowserProxy()` 得到本地 HTTP CONNECT proxy 的 host/port。
4. 把 WebView proxy 设置为该 host/port。
5. 用 `deviceServiceUrl(deviceId, serviceId, pathAndQuery)` 生成设备服务 URL。

iOS wrapper 需要 iOS 17+，通过 `WKWebViewConfiguration.websiteDataStore.proxyConfigurations` 设置 proxy：

```swift
let controller = MobileCodeConnectBrowserProxyController(tunnel: tunnel)
let proxy = try controller.startBrowserProxy()
let configuration = WKWebViewConfiguration()
try controller.applyProxy(to: configuration, proxy: proxy)
let url = try controller.deviceServiceUrl(deviceId: "pc_001", serviceId: "svc_web", pathAndQuery: "/")
```

Android wrapper 使用 AndroidX WebKit `ProxyController.setProxyOverride`：

```kotlin
val controller = MobileCodeConnectBrowserProxyController(tunnel)
val proxy = controller.startBrowserProxy()
controller.applyProxy(executor)
val url = controller.deviceServiceUrl("pc_001", "svc_web", "/")
```

默认 browser proxy 配置：

- 绑定 `127.0.0.1:0`
- synthetic domain suffix 为 `.mobilecode-connect.local`
- 最大连接数 `256`
- request head timeout `10s`
- direct connect timeout `10s`
- tunnel open timeout `15s`
- idle timeout `120s`
- direct fallback policy 为 `LocalNetworkAndDomain`

设备服务 URL 形如：

```text
http://s-svc-5fweb.d-pc-5f001.mobilecode-connect.local/
```

这些 synthetic host 只代表设备服务。Control API、server/agent API 仍应继续使用正常的 Control URL。普通公网或局域网页面在默认策略下走 direct-network fallback，不进入 tunnel。

## Server/Agent 端接入

受控 server/agent 端的 SDK 使用分两步：先拿 server credential，再用 credential 注册设备服务并处理 session。

### 1. 获取 server credential

`ServerAuthSdk` 支持 browser login 和 device-code login。新接入推荐不要在
server/agent 启动认证时手动分配长期 `device_id`，而是使用
`ServerLoginInput::generated_device(...)`，让 Control Server 在短时
server-auth session 内生成最终 `srv_dev_*` 设备 ID。显式固定 ID 的旧接入仍然可用，
适合迁移既有设备或需要保持历史 ID 的场景。

```rust
use mobilecode_connect_sdk::{
    ServerAuthSdk, FileServerCredentialStore, ServerLoginInput,
};

let server_auth = ServerAuthSdk::with_http_client(
    "https://control.example.com",
    FileServerCredentialStore::new("agentd-credential.json"),
)?;

let login_input = ServerLoginInput::generated_device(
    "Office PC",
    "agent-public-key",
);

let pending = server_auth.start_device_code_login(login_input).await?;

println!("Open: {}", pending.verification_uri);
println!("Code: {}", pending.user_code);
println!("Complete URL: {}", pending.verification_uri_complete);

let credential = server_auth
    .complete_device_code_login(pending, std::time::Duration::from_secs(1))
    .await?;

println!("Control generated device id: {}", credential.device_id);
```

用户打开 `verification_uri_complete` 后，如果尚未登录 Control Web，会先进入登录页；
登录成功后会自动回到 device-code 审批页。审批页会展示 Control 生成的
`device_id`、设备名和 server public key 指纹，用户确认后 server 端轮询得到并保存
`StoredServerCredential`。

Browser login 流程是：

```rust
let pending = server_auth
    .start_browser_login(ServerLoginInput::generated_device(
        "Office PC",
        "agent-public-key",
    ))
    .await?;
println!("Open: {}", pending.auth_url);
let credential = server_auth.complete_browser_login(pending, server_auth_code).await?;
```

用户打开 `auth_url` 后，如果尚未登录也会先走登录页；审批完成后页面显示一次性
`server_auth_code`，server/agent 把该 code 传给 `complete_browser_login(...)` 完成交换。
保存后的 credential 包含 `control_server`、`credential_id`、Control 最终分配的
`device_id`、`device_name`、`server_token` 和 `token_type`。

如果需要保留旧设备 ID，用 `existing_device(...)`：

```rust
use mobilecode_connect_protocol::DeviceId;

let login_input = ServerLoginInput::existing_device(
    DeviceId::new("pc_001"),
    "Office PC",
    "agent-public-key",
);
```

### 2. 注册 server 设备和服务

拿到 credential 后，`ServerSdk` 会从同一个 credential store 读取 `server_token`，并作为 bearer token 调 Control API。

```rust
use mobilecode_connect_sdk::ServerRegistrationInput;
use mobilecode_connect_protocol::{
    Device, DeviceStatus, Service, ServiceId, ServiceProtocol, UserId,
};

let server = sdk.server()?;
let credential = server_auth
    .load_credential()
    .await?
    .expect("server credential must exist before registration");
let device_id = credential.device_id.clone();

server.register_server(ServerRegistrationInput {
    device: Device {
        device_id: device_id.clone(),
        user_id: UserId::new("user_001"),
        name: "Office PC".to_string(),
        status: DeviceStatus::Online,
        agent_version: "0.1.0".to_string(),
    },
    services: vec![Service {
        service_id: ServiceId::new("svc_web"),
        device_id,
        name: "Web".to_string(),
        protocol: ServiceProtocol::Tcp,
        target_host: "127.0.0.1".to_string(),
        target_port: 3000,
    }],
    p2p_certificate_der: None,
}).await?;
```

常用 server 方法：

- `register_device(device)`
- `register_services(services)`
- `register_p2p_certificate(certificate_der)`
- `list_sessions()`
- `claim_session(&session_id)`
- `mark_session_bound(&session_id)`
- `close_session(&session_id)`

`register_server(...)` 是组合方法，会按顺序注册 device、可选 P2P certificate、services。

## agentd 命令行用法

`apps/agentd` 是受控 server 端的参考实现。

登录并保存 server credential：

```bash
agentd login \
  --control https://control.example.com \
  --name "Office PC" \
  --credential-file agentd-credential.json
```

默认是 browser login：命令会输出审批 URL，用户在默认浏览器或任意浏览器打开后登录并审批，
再把页面展示的一次性 auth code 粘回终端。无头/SSH 环境使用 device-code 模式：

```bash
agentd login \
  --device-code \
  --control https://control.example.com \
  --name "Office PC" \
  --credential-file agentd-credential.json
```

以上两种模式默认都不传 `--device`，Control Server 会生成最终 `srv_dev_*`
设备 ID 并写入 `agentd-credential.json`。如果要复用已有设备 ID，可显式传入：

```bash
agentd login \
  --control https://control.example.com \
  --device pc_001 \
  --name "Office PC" \
  --credential-file agentd-credential.json
```

运行 agent 并注册本地服务：

```bash
agentd run \
  --control https://control.example.com \
  --credential-file agentd-credential.json \
  --relay-cert relay.der \
  --service svc_web=127.0.0.1:3000
```

生成 mobile invite：

```bash
agentd mobile-invite create \
  --mobile-grants-file mobile-grants.json \
  --control https://control.example.com \
  --device pc_001 \
  --service svc_web
```

运行 agent 时也可以直接用 `--mobile-invite-service` 输出一次性 invite。移动端拿到 invite 后执行 pairing，pairing 被批准后得到 mobile grant。

## mobile-cli 调试用法

`apps/mobile-cli` 是移动端 SDK 的命令行测试工具。

用 invite 换 grant：

```bash
mobile-cli pair \
  --invite-file invite.json \
  --grant-file mobile-grant.json \
  --client phone_001 \
  --service svc_web
```

用用户 token 打开服务：

```bash
mobile-cli open-service \
  --control https://control.example.com \
  --token "$CONTROL_USER_TOKEN" \
  --client phone_001 \
  --device pc_001 \
  --service svc_web \
  --local 0 \
  --relay-cert relay.der
```

用 mobile grant 打开服务：

```bash
mobile-cli open-service \
  --control https://control.example.com \
  --grant-file mobile-grant.json \
  --client phone_001 \
  --device pc_001 \
  --service svc_web \
  --local 0 \
  --relay-cert relay.der
```

## 端到端顺序

用户 token 模式：

1. 用户通过 `AuthSdk` 登录或注册。
2. 移动端通过 `ControllerSdk` 注册 controller。
3. Agent/server 通过 `ServerAuthSdk` 登录并保存 server credential。
4. Agent/server 通过 `ServerSdk` 注册 device/services/P2P certificate。
5. 移动端列出 devices/services。
6. 移动端创建 session。
7. 移动端启动 `MobileTunnelSdk` 或 `FfiMobileTunnel`。
8. 移动端 `open_service` 或启动 browser proxy 访问服务。

Mobile grant 模式：

1. Agent/server 创建 mobile invite。
2. 移动端导入 invite 并发起 pairing。
3. Agent 或用户批准 pairing。
4. 移动端轮询得到 grant，并存入平台安全存储。
5. 移动端用 grant 启动 P2P-or-Relay tunnel。
6. 移动端 `open_service` 或 browser proxy 访问被授权的服务。

## 错误处理建议

- `SdkError::is_unauthorized()` / `requires_reauthentication()`：用户 token 缺失、过期或 Control 返回 401，应触发重新登录。
- `SdkError::is_forbidden()`：当前用户或 credential 没有权限，应提示权限问题，不应自动重试登录。
- `SdkError::NotAuthenticated`：本地 store 没有 token 或 server credential。
- Mobile grant pairing 返回 denied/expired 时，移动端应丢弃当前 pairing session，要求重新导入 invite 或重新发起授权。
- `start_with_mobile_grant` 的 config 和 grant 不匹配时，应检查 grant 是否来自同一个 Control URL 和同一个 `client_id`。

## 代码入口

- Rust SDK facade：`crates/sdk/src/facade.rs`
- 移动端 Rust SDK：`crates/sdk/src/mobile.rs`
- UniFFI 移动端接口：`crates/mobile-core/src/ffi.rs`
- Server auth SDK：`crates/sdk/src/server_auth.rs`
- Server SDK：`crates/sdk/src/server.rs`
- SDK mock/live examples：`crates/sdk/examples/`
- iOS wrapper：`mobile/ios/Sources/MobileCodeConnectMobileSdk/`
- Android wrapper：`mobile/android/src/main/java/dev/mobilecode/connect/mobile/`
- Agent 参考实现：`apps/agentd/src/main.rs`
- Mobile CLI 参考实现：`apps/mobile-cli/src/main.rs`
