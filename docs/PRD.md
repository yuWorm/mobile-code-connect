# QUIC P2P Tunnel MVP PRD（Rust-first 版）

## 1. 目标

本 PRD 用于验证一条最小可用链路：

```text
Mobile App
  ↓
Mobile SDK core
  ↓
P2P QUIC 或 Relay QUIC
  ↓
PC Agent
  ↓
Remote PC localhost service
```

MVP 必须证明：

```text
1. 手机 App 可以访问远程 PC 上的本地服务，例如 127.0.0.1:3000。
2. 手机侧可以监听本地端口，例如 127.0.0.1:18080。
3. 手机访问 127.0.0.1:18080 时，流量能转发到远程 PC 的 127.0.0.1:3000。
4. 优先尝试 P2P UDP 打洞。
5. P2P 不可用时自动回退 Relay。
6. Relay 可以做鉴权、连接数控制、流量统计、限速和强制断开。
7. MVP 阶段重点验证业务链路，不做完整 ICE、完整 TURN、系统 VPN 或复杂企业权限模型。
```

---

## 2. Rust 化边界

### 2.1 必须使用 Rust 实现

以下模块是核心网络链路，必须 Rust 化：

```text
1. PC Agent
2. Relay Server
3. Mobile SDK core
4. Tunnel 协议层
5. QUIC stream 转发层
6. P2P 打洞状态机
7. Relay 限速、限流、统计和会话控制
```

### 2.2 建议使用 Rust 实现

以下模块与核心协议、鉴权、session 状态强相关，也建议 Rust 化：

```text
1. Control Server
2. Punch Server
3. Admin API
4. CLI 测试工具
5. 共享数据模型和 token claims
```

原因：

```text
1. 可以共享 protocol crate，避免多语言模型漂移。
2. token、session、candidate、traffic stats 等结构可以统一序列化。
3. Control、Punch、Relay、Agent、Mobile 的状态机可以使用同一套错误码和 tracing 字段。
4. 端到端集成测试可以直接在 Rust workspace 内完成。
```

### 2.3 不强行使用 Rust 的部分

以下部分不是核心网络链路，不需要强行 Rust 化：

```text
1. iOS wrapper：Swift 调用 Rust FFI。
2. Android wrapper：Kotlin/Java 调用 Rust JNI 或 C ABI。
3. Relay Admin 前端：HTML/TypeScript/React 均可。
4. 基础设施：Dockerfile、Kubernetes、Prometheus、Grafana、Redis、Nginx/Caddy。
5. CI 脚本和本地开发脚本。
```

移动端的原则是：

```text
核心网络、协议、状态机、端口转发全部在 Rust mobile-core 中实现。
Swift/Kotlin 只做平台生命周期、权限、回调和对象封装。
```

---

## 3. 推荐技术栈

### 3.1 Rust 基础栈

```text
语言：Rust
异步运行时：tokio
QUIC：quinn
TLS：rustls
序列化：serde / serde_json
配置：serde_yaml 或 toml
日志与链路追踪：tracing / tracing-subscriber
错误处理：thiserror / anyhow
CLI：clap
测试：cargo test / tokio::test / integration tests
```

说明：

```text
1. quinn 支持基于自定义 UDP socket 构造 Endpoint，适合后续复用打洞 socket 建立 QUIC。
2. tokio 作为统一 runtime，便于 server、agent、CLI 和 mobile-core 共享异步代码。
3. protocol、tunnel、auth、traffic 等逻辑应沉淀为共享 crate。
```

### 3.2 Server 侧栈

```text
HTTP API：axum
中间件：tower / tower-http
限速：governor 或自研 token bucket
指标：metrics / metrics-exporter-prometheus
状态存储：内存优先，Redis 可选
管理 API：axum routes
```

### 3.3 Mobile 侧栈

```text
核心库：Rust mobile-core
本地端口监听：tokio::net::TcpListener
UDP 打洞：tokio::net::UdpSocket 或 std::net::UdpSocket + quinn runtime wrapper
QUIC：quinn
iOS：Swift wrapper + Rust staticlib/cdylib
Android：Kotlin wrapper + JNI/C ABI
```

MVP 阶段先提供 `mobile-cli` 跑通链路，再封装 iOS/Android。

---

## 4. MVP 功能范围

### 4.1 必须实现

```text
1. PC Agent 读取本地服务配置。
2. PC Agent 向 Control Server 注册设备。
3. PC Agent 向 Control Server 注册服务。
4. Mobile SDK 获取设备与服务列表。
5. Mobile SDK 请求访问 service_id。
6. Control Server 创建访问 session。
7. Mobile SDK 监听本地端口。
8. Relay 绑定 mobile 和 agent 两端连接。
9. QUIC Stream 双向转发 TCP 数据。
10. PC Agent 将 stream 转发到本机 localhost 服务。
11. Relay 回退链路可用。
12. 基础 UDP 打洞。
13. P2P 成功后使用 P2P QUIC。
14. P2P 失败后自动使用 Relay。
15. 基础 token 鉴权。
16. Relay 流量统计。
17. Relay 简单限速。
18. Relay 最大 stream 数控制。
19. Relay 强制断开 session。
20. 基础状态上报。
```

### 4.2 暂不实现

```text
1. 完整 ICE。
2. 标准 TURN 协议。
3. 系统级 VPN。
4. 多 Relay 节点智能调度。
5. 复杂 NAT 类型识别。
6. 自动服务扫描。
7. 复杂权限模型。
8. 企业空间隔离。
9. 端到端动态路径迁移。
10. 高级拥塞控制。
11. 多用户管理后台。
12. 计费系统。
```

---

## 5. 总体架构

```text
+-----------------------------+
|        Mobile App            |
|  WebView / HTTP Client       |
+--------------+--------------+
               |
               | 127.0.0.1:18080
               v
+-----------------------------+
|  Rust Mobile SDK core        |
|  Local Port Forward          |
|  UDP Punch                   |
|  QUIC Tunnel                 |
|  Relay Fallback              |
+--------------+--------------+
               |
               | Control API
               v
+-----------------------------+
|  Rust Control Server         |
|  Device / Service / Session  |
|  Token Issuer                |
+--------------+--------------+
               |
               | Candidate exchange
               v
+-----------------------------+
|  Rust Punch Server           |
|  Public Addr Discovery       |
|  Probe Coordination          |
+--------------+--------------+
               |
               | P2P QUIC preferred
               | Relay QUIC fallback
               v
+-----------------------------+
|  Rust Relay Server           |
|  Session Bind                |
|  Stream Forward              |
|  Rate Limit / Stats          |
+--------------+--------------+
               |
               v
+-----------------------------+
|  Rust PC Agent               |
|  Service Registry            |
|  QUIC Tunnel                 |
|  Local TCP Forward           |
+--------------+--------------+
               |
               | 127.0.0.1:3000
               v
+-----------------------------+
|  Remote Local Service        |
|  Web / API / Dev Server      |
+-----------------------------+
```

Relay 是可用性底座，P2P 是优化路径。

---

## 6. 组件职责

### 6.1 PC Agent

PC Agent 是运行在远程 PC 上的 Rust daemon。

职责：

```text
1. 读取 agent 配置。
2. 注册 device_id。
3. 注册 services。
4. 维护与 Control Server 的控制连接。
5. 接收访问请求。
6. 参与 UDP 打洞。
7. 连接 Relay。
8. 接收 P2P 或 Relay tunnel。
9. 根据 service_id 查找 localhost 目标地址。
10. 将 QUIC stream 与本地 TCP 连接双向复制。
11. 上报连接状态和流量。
```

### 6.2 Relay Server

Relay Server 是 Rust 服务端组件，负责兜底转发和商业化控制点。

职责：

```text
1. 接收 Mobile SDK QUIC 连接。
2. 接收 PC Agent QUIC 连接。
3. 校验 relay token。
4. 根据 session_id 绑定双方连接。
5. 按 stream 转发数据。
6. 统计上下行流量。
7. 控制最大 stream 数。
8. 控制最大连接时长。
9. 控制最大流量配额。
10. 按 session 限速。
11. 暴露 Admin API。
12. 支持强制断开 session。
```

### 6.3 Mobile SDK core

Mobile SDK core 是跨平台 Rust 核心库。

职责：

```text
1. 登录后请求设备与服务列表。
2. 请求创建访问 session。
3. 创建本地 TCP listener。
4. 参与 UDP 打洞。
5. 建立 P2P QUIC tunnel。
6. P2P 失败时连接 Relay。
7. 将本地 TCP 连接映射为 QUIC stream。
8. 暴露状态、错误和流量回调。
9. 为 iOS/Android wrapper 提供稳定 FFI。
```

### 6.4 Control Server

Control Server 建议 Rust 实现。

职责：

```text
1. 管理用户、设备、服务和 session。
2. 接收 PC Agent 注册。
3. 接收 Mobile SDK 服务访问请求。
4. 签发短期 access token。
5. 签发 relay token。
6. 协调候选地址交换。
7. 通知 agent 有新的访问 session。
8. 接收 Relay/Agent/Mobile 状态上报。
```

MVP 可先使用内存存储，后续再接 Redis 或数据库。

### 6.5 Punch Server

Punch Server 建议 Rust 实现。

职责：

```text
1. 接收 mobile 和 agent 的 UDP HELLO。
2. 记录服务端看到的公网 UDP 地址。
3. 生成 srflx candidate。
4. 辅助双方进行 probe/ack。
5. 向 Control Server 返回候选地址。
```

### 6.6 Admin Web

Admin Web 不要求 Rust。

职责：

```text
1. 调用 Rust Relay Admin API。
2. 查看 session stats。
3. 手动断开 session。
4. 辅助本地端口访问测试。
```

---

## 7. 核心业务模型

### 7.1 设备

```json
{
  "device_id": "pc_001",
  "user_id": "user_001",
  "name": "Office PC",
  "status": "online",
  "agent_version": "0.1.0"
}
```

### 7.2 服务

```json
{
  "service_id": "svc_web_3000",
  "device_id": "pc_001",
  "name": "Dev Web",
  "protocol": "tcp",
  "target_host": "127.0.0.1",
  "target_port": 3000
}
```

### 7.3 访问会话

```json
{
  "session_id": "sess_001",
  "user_id": "user_001",
  "client_id": "mobile_001",
  "device_id": "pc_001",
  "service_id": "svc_web_3000",
  "mode": "p2p_or_relay",
  "expire_at": 1760000000
}
```

### 7.4 候选地址

```json
{
  "candidate_type": "srflx",
  "addr": "203.0.113.1:42000",
  "priority": 100,
  "source": "punch_server"
}
```

候选类型：

```text
host：本地地址，例如 192.168.1.10:50000
srflx：Punch Server 看到的公网地址，例如 203.0.113.1:42000
relay：Relay 地址
```

### 7.5 Relay Token

Control Server 签发短期 Relay Token。

```json
{
  "session_id": "sess_001",
  "user_id": "user_001",
  "client_id": "mobile_001",
  "device_id": "pc_001",
  "service_id": "svc_web_3000",
  "max_bps": 2097152,
  "max_streams": 32,
  "max_duration_sec": 3600,
  "traffic_quota_bytes": 1073741824,
  "exp": 1760000000
}
```

---

## 8. 数据流设计

### 8.1 服务转发

远程 PC 服务：

```text
127.0.0.1:3000
```

手机侧访问：

```text
http://127.0.0.1:18080
```

转发链路：

```text
Mobile App
  ↓
127.0.0.1:18080
  ↓
Rust Mobile SDK core
  ↓
QUIC Stream
  ↓
Rust Relay 或 Rust P2P QUIC
  ↓
Rust PC Agent
  ↓
127.0.0.1:3000
```

### 8.2 本地端口转发

Mobile SDK core 启动本地监听：

```text
local_addr = 127.0.0.1:18080
remote_service_id = svc_web_3000
```

当 App 访问 `127.0.0.1:18080` 时：

```text
1. Mobile SDK 接收本地 TCP 连接。
2. Mobile SDK 在当前 tunnel 上打开一个 QUIC bidirectional stream。
3. Mobile SDK 发送 OPEN_STREAM header。
4. Mobile SDK 将 TCP 数据复制到 QUIC stream。
5. Mobile SDK 将 QUIC stream 数据复制回 TCP。
```

PC Agent 收到 `OPEN_STREAM` 后：

```text
1. 根据 service_id 查找目标服务。
2. 连接 127.0.0.1:3000。
3. 将 QUIC stream 数据复制到本地 TCP。
4. 将本地 TCP 响应复制回 QUIC stream。
```

---

## 9. Tunnel 协议设计

### 9.1 协议原则

MVP 使用简单协议，避免过早设计复杂 framing。

推荐模型：

```text
1. QUIC connection 承载一个 session。
2. 一个长期 control stream 传输 HELLO、AUTH、PING、ERROR、TRAFFIC_REPORT。
3. 每个本地 TCP 连接对应一个 QUIC bidirectional data stream。
4. data stream 的起始位置写入一个长度前缀 JSON header。
5. header 之后直接转发 raw TCP bytes。
```

这样 Relay 可以理解 stream 起始 header，完成鉴权、计数、限速和转发；同时避免对每个 TCP chunk 都包一层 DATA frame。

### 9.2 Control frame 类型

```text
HELLO
AUTH
PING
PONG
ERROR
TRAFFIC_REPORT
RELAY_BIND
SESSION_READY
SESSION_CLOSED
```

### 9.3 Data stream header

```json
{
  "type": "OPEN_STREAM",
  "stream_id": "stream_001",
  "session_id": "sess_001",
  "service_id": "svc_web_3000"
}
```

编码方式：

```text
u32_be header_len
header_len bytes JSON header
raw TCP bytes...
```

### 9.4 HELLO

Mobile 示例：

```json
{
  "type": "HELLO",
  "role": "mobile",
  "client_id": "mobile_001",
  "session_id": "sess_001",
  "protocol_version": "0.1.0"
}
```

Agent 示例：

```json
{
  "type": "HELLO",
  "role": "agent",
  "device_id": "pc_001",
  "session_id": "sess_001",
  "protocol_version": "0.1.0"
}
```

### 9.5 AUTH

```json
{
  "type": "AUTH",
  "session_id": "sess_001",
  "token": "short_lived_access_token"
}
```

### 9.6 ERROR

```json
{
  "type": "ERROR",
  "code": "SERVICE_NOT_FOUND",
  "message": "service_id not found"
}
```

错误码：

```text
AUTH_FAILED
SESSION_EXPIRED
SERVICE_NOT_FOUND
SERVICE_DIAL_FAILED
P2P_TIMEOUT
RELAY_REQUIRED
QUIC_HANDSHAKE_FAILED
STREAM_OPEN_FAILED
RATE_LIMITED
TRAFFIC_QUOTA_EXCEEDED
MAX_STREAMS_EXCEEDED
SESSION_CLOSED
```

---

## 10. Relay 设计

### 10.1 Relay 连接

Mobile SDK 和 PC Agent 都通过 QUIC 连接 Relay。

绑定消息：

```json
{
  "type": "RELAY_BIND",
  "role": "mobile",
  "session_id": "sess_001",
  "token": "relay_token"
}
```

```json
{
  "type": "RELAY_BIND",
  "role": "agent",
  "session_id": "sess_001",
  "token": "relay_token"
}
```

Relay 收到双方连接后：

```text
1. 校验 token。
2. 校验 session_id。
3. 绑定 mobile_conn 和 agent_conn。
4. 标记 session READY。
5. 开始按 stream 转发。
```

### 10.2 Relay session 内部模型

Rust 结构示意：

```rust
pub struct RelaySession {
    pub session_id: SessionId,
    pub mobile: Option<RelayPeer>,
    pub agent: Option<RelayPeer>,
    pub limits: RelayLimits,
    pub stats: TrafficStats,
    pub state: RelaySessionState,
}
```

### 10.3 转发模型

MVP 使用 stream 级转发：

```text
Mobile QUIC Stream ⇄ Relay ⇄ Agent QUIC Stream
```

优点：

```text
1. 可以按 stream 统计。
2. 可以按 stream 做并发限制。
3. Relay 可以识别 OPEN_STREAM header。
4. 方便实现强制断开和错误返回。
```

### 10.4 Relay 控制能力

MVP 必须支持：

```text
1. 单 session 最大带宽。
2. 单 session 最大 stream 数。
3. 单 session 最大连接时长。
4. 单 session 最大流量。
5. 手动断开 session。
```

后续扩展：

```text
1. 用户级带宽。
2. 设备级带宽。
3. 服务级带宽。
4. 组织级流量池。
5. IP 风控。
6. 多 Relay 节点调度。
```

### 10.5 Relay Admin API

MVP 提供：

```text
GET  /admin/sessions
GET  /admin/sessions/{session_id}
POST /admin/sessions/{session_id}/disconnect
```

Session 响应：

```json
{
  "session_id": "sess_001",
  "state": "ready",
  "mobile_bound": true,
  "agent_bound": true,
  "limits": {
    "max_bps": 2097152,
    "max_streams": 32,
    "max_duration_sec": 3600,
    "traffic_quota_bytes": 1073741824
  },
  "stats": {
    "session_id": "sess_001",
    "uplink_bytes": 123456,
    "downlink_bytes": 654321,
    "total_bytes": 777777,
    "duration_sec": 120,
    "active_streams": 4
  }
}
```

---

## 11. P2P 打洞设计

### 11.1 目标

MVP 只做简化 UDP hole punching，不做完整 ICE。

目标环境：

```text
1. 同一局域网。
2. 普通家用 NAT。
3. 一部分移动网络 NAT。
```

不承诺：

```text
1. 对称 NAT 必然成功。
2. 企业网络必然成功。
3. 双蜂窝网络必然成功。
```

### 11.2 打洞流程

```text
1. Mobile SDK 创建 UDP socket。
2. PC Agent 创建 UDP socket。
3. 双方分别向 Punch Server 发送 HELLO。
4. Punch Server 记录双方公网地址。
5. Control Server 交换双方候选地址。
6. Mobile SDK 和 PC Agent 同时向对方地址发送 probe。
7. 收到 probe 后回复 ack。
8. path 确认成功。
9. 在同一个 UDP socket 上建立 QUIC。
10. QUIC 成功后进入 P2P Tunnel。
```

### 11.3 Probe 包

```json
{
  "type": "PUNCH_PROBE",
  "session_id": "sess_001",
  "from": "mobile_001",
  "to": "pc_001",
  "nonce": "random_nonce",
  "timestamp": 1760000000,
  "hmac": "signature"
}
```

ACK：

```json
{
  "type": "PUNCH_ACK",
  "session_id": "sess_001",
  "from": "pc_001",
  "to": "mobile_001",
  "nonce": "random_nonce",
  "timestamp": 1760000001,
  "hmac": "signature"
}
```

### 11.4 UDP socket 复用要求

打洞 socket 和 QUIC socket 必须尽量复用同一个 UDP socket。

正确：

```text
UDP Probe 使用 socket A
QUIC Transport 也使用 socket A
```

避免：

```text
UDP Probe 使用 socket A
QUIC 重新创建 socket B
```

否则 NAT 映射可能不一致，导致打洞成功但 QUIC 失败。

### 11.5 Relay 竞速回退

不要等 P2P 完全失败后才启动 Relay。

推荐策略：

```text
0ms：开始 P2P
300ms：P2P 未 ready，则开始连接 Relay
最终谁先 READY 用谁
```

状态机：

```text
INIT
  ↓
GATHER_CANDIDATES
  ↓
TRY_P2P
  ├── P2P_READY
  └── RELAY_CONNECTING
        ↓
     RELAY_READY
```

---

## 12. Mobile SDK API 设计

### 12.1 Rust core API

```rust
pub struct TunnelConfig {
    pub user_token: String,
    pub control_server_url: String,
    pub client_id: String,
}

pub async fn start(config: TunnelConfig) -> Result<TunnelClient, TunnelError>;
```

打开远程服务：

```rust
impl TunnelClient {
    pub async fn open_service(
        &self,
        device_id: String,
        service_id: String,
        local_port: u16,
    ) -> Result<LocalForwardHandle, TunnelError>;

    pub async fn close_service(
        &self,
        handle_id: String,
    ) -> Result<(), TunnelError>;

    pub fn status(&self) -> TunnelStatus;
}
```

调用示例：

```text
open_service("pc_001", "svc_web_3000", 18080)
```

App 访问：

```text
http://127.0.0.1:18080
```

### 12.2 状态模型

```json
{
  "state": "connected",
  "path": "relay",
  "rtt_ms": 80,
  "uplink_bytes": 123456,
  "downlink_bytes": 654321
}
```

### 12.3 事件回调

```rust
pub trait TunnelEventListener: Send + Sync {
    fn on_state_changed(&self, state: TunnelState);
    fn on_error(&self, error: TunnelError);
    fn on_traffic_update(&self, stats: TrafficStats);
}
```

### 12.4 iOS/Android wrapper

平台 wrapper 不实现核心网络逻辑，只负责：

```text
1. 初始化 Rust runtime。
2. 管理 SDK 生命周期。
3. 转换 Swift/Kotlin 类型。
4. 派发状态回调。
5. 处理 App 权限和后台限制。
```

---

## 13. PC Agent API 设计

### 13.1 Agent 配置

```yaml
device_id: pc_001
control_server: https://control.example.com
auth_token: agent_token

services:
  - service_id: svc_web_3000
    name: Dev Web
    protocol: tcp
    target_host: 127.0.0.1
    target_port: 3000

  - service_id: svc_api_8080
    name: Backend API
    protocol: tcp
    target_host: 127.0.0.1
    target_port: 8080
```

### 13.2 Agent 启动流程

```text
1. 读取配置。
2. 连接 Control Server。
3. 注册 device_id。
4. 注册 services。
5. 保持控制连接。
6. 等待访问请求。
7. 收到访问请求后参与 P2P 或 Relay。
8. 建立 tunnel 后处理 stream。
```

### 13.3 Stream 处理

Rust 伪代码：

```rust
async fn handle_stream(
    stream: QuicDataStream,
    service_id: ServiceId,
    registry: ServiceRegistry,
) -> Result<(), AgentError> {
    let service = registry
        .get(&service_id)
        .ok_or(AgentError::ServiceNotFound)?;

    let target = tokio::net::TcpStream::connect(service.target_addr()).await?;
    copy_bidirectional(stream, target).await?;

    Ok(())
}
```

---

## 14. Control Server API 设计

MVP API：

```text
POST /agent/register
POST /agent/services
GET  /mobile/devices
GET  /mobile/devices/{device_id}/services
POST /sessions
GET  /sessions/{session_id}
POST /sessions/{session_id}/candidates
GET  /sessions/{session_id}/candidates
POST /sessions/{session_id}/close
```

创建 session：

```json
{
  "client_id": "mobile_001",
  "device_id": "pc_001",
  "service_id": "svc_web_3000"
}
```

响应：

```json
{
  "session_id": "sess_001",
  "access_token": "short_lived_access_token",
  "relay_token": "short_lived_relay_token",
  "relay_addr": "relay.example.com:4433",
  "punch_addr": "punch.example.com:3478",
  "expire_at": 1760000000
}
```

---

## 15. 推荐目录结构

使用 Cargo workspace：

```text
quic-tunnel/
  Cargo.toml
  crates/
    protocol/
      src/
        frame.rs
        model.rs
        error.rs
        token.rs
    tunnel/
      src/
        quic.rs
        stream.rs
        copy.rs
        stats.rs
    auth/
      src/
        jwt.rs
        hmac.rs
    control/
      src/
        api.rs
        session.rs
        registry.rs
    punch/
      src/
        server.rs
        candidate.rs
        probe.rs
    relay/
      src/
        server.rs
        session.rs
        bind.rs
        limiter.rs
        admin.rs
    agent/
      src/
        config.rs
        client.rs
        service.rs
        stream.rs
    mobile-core/
      src/
        client.rs
        forward.rs
        ffi.rs
        status.rs
  apps/
    control-server/
      src/main.rs
    punch-server/
      src/main.rs
    relayd/
      src/main.rs
    agentd/
      src/main.rs
    mobile-cli/
      src/main.rs
  mobile/
    ios/
    android/
  admin-web/
    relay-admin.html
  docs/
    PRD.md
```

---

## 16. MVP 实现顺序

### 阶段一：Relay Tunnel

先不做 P2P。

目标：

```text
Mobile CLI / Mobile SDK core
  ↓
Rust Relay
  ↓
Rust PC Agent
  ↓
127.0.0.1:3000
```

实现内容：

```text
1. protocol crate。
2. tunnel crate。
3. PC Agent 读取服务配置。
4. Relay 绑定 mobile 和 agent。
5. Mobile CLI 本地监听端口。
6. QUIC stream 转发。
7. 本地 TCP 转发。
8. Admin stats 和 disconnect。
```

验收标准：

```text
访问 http://127.0.0.1:18080 可以打开远程 PC 的 http://127.0.0.1:3000。
```

### 阶段二：Control Server

实现内容：

```text
1. Agent 注册设备。
2. Agent 注册服务。
3. Mobile 查询设备和服务。
4. Mobile 创建 session。
5. Control 签发 access token 和 relay token。
6. Relay 校验 token。
```

验收标准：

```text
不再依赖写死的 session_id、service_id 和 token。
```

### 阶段三：P2P 打洞

实现内容：

```text
1. Punch Server。
2. candidate 采集。
3. probe/ack。
4. 同 UDP socket 建立 QUIC。
5. P2P 与 Relay 竞速。
6. P2P 失败自动 Relay。
```

验收标准：

```text
同一局域网或普通 NAT 环境下 P2P 成功。
严格 NAT 环境下 Relay 成功。
```

### 阶段四：Relay 控制能力

实现内容：

```text
1. session 限速。
2. session 流量统计。
3. 最大 stream 数限制。
4. 最大连接时长限制。
5. 最大流量限制。
6. 手动断开 session。
```

验收标准：

```text
可以限制某个 session 的带宽。
可以统计某个 session 的流量。
可以强制断开某个 session。
```

### 阶段五：移动端封装

实现内容：

```text
1. iOS Swift wrapper。
2. Android Kotlin wrapper。
3. 状态回调。
4. 错误码映射。
5. App 调用 open_service。
6. WebView 访问本地端口。
```

验收标准：

```text
App 不感知 P2P/Relay 细节，只调用 open_service 后访问本地端口即可。
```

---

## 17. 测试策略

### 17.1 单元测试

```text
1. protocol frame encode/decode。
2. token claims 校验。
3. service registry。
4. relay limiter。
5. traffic stats。
6. session state machine。
```

### 17.2 集成测试

```text
1. Mobile CLI ⇄ Relay ⇄ Agent。
2. Agent ⇄ localhost echo server。
3. Relay stats。
4. Relay disconnect。
5. max_streams 超限。
6. quota 超限。
```

### 17.3 端到端测试

```text
1. 启动本地 HTTP 服务 127.0.0.1:3000。
2. 启动 control-server。
3. 启动 relayd。
4. 启动 agentd。
5. 启动 mobile-cli open_service。
6. curl http://127.0.0.1:18080。
7. 验证响应内容、流量统计和断开行为。
```

---

## 18. MVP 验收清单

### Relay Tunnel

```text
[ ] PC Agent 能读取服务配置。
[ ] PC Agent 能注册设备。
[ ] PC Agent 能注册服务。
[ ] Mobile SDK 能请求访问服务。
[ ] Mobile SDK 能监听本地端口。
[ ] Mobile SDK 能连接 Relay。
[ ] PC Agent 能连接 Relay。
[ ] Relay 能绑定双方 session。
[ ] Mobile 到 PC 的 TCP 数据能转发。
[ ] PC 到 Mobile 的响应能返回。
[ ] Web 服务可以在手机端打开。
```

### Control

```text
[ ] Control Server 能创建设备。
[ ] Control Server 能注册服务。
[ ] Control Server 能创建 session。
[ ] Control Server 能签发 access token。
[ ] Control Server 能签发 relay token。
[ ] Relay 能校验 relay token。
```

### P2P

```text
[ ] Mobile SDK 能获取公网候选地址。
[ ] PC Agent 能获取公网候选地址。
[ ] 双方能交换候选地址。
[ ] 双方能互发 probe。
[ ] P2P 成功后能建立 QUIC。
[ ] P2P 失败后能自动 Relay。
```

### Relay 控制

```text
[ ] Relay 能统计上下行流量。
[ ] Relay 能限制最大带宽。
[ ] Relay 能限制最大 stream 数。
[ ] Relay 能限制最大连接时长。
[ ] Relay 能限制最大流量。
[ ] Relay 能强制断开 session。
[ ] Admin API 能查看 stats。
[ ] Admin API 能断开 session。
```

### Mobile

```text
[ ] mobile-core 能在 CLI 中跑通。
[ ] iOS wrapper 能调用 open_service。
[ ] Android wrapper 能调用 open_service。
[ ] 状态回调能返回 path、rtt、traffic。
[ ] 错误码能映射到平台层。
```

---

## 19. 关键注意事项

### 19.1 先实现 Relay，再实现 P2P

Relay 是可用性底座。

推荐顺序：

```text
Relay Tunnel → Control Server → P2P → Relay 控制能力 → 移动端 wrapper
```

### 19.2 不做系统 VPN

当前目标是访问远程 PC 上的自有服务，不是让手机加入远程 PC 整个内网。

MVP 做 service-level tunnel。

### 19.3 service_id 是核心抽象

Mobile SDK 不应直接访问任意 IP:Port。

推荐：

```text
Mobile SDK 请求 service_id。
PC Agent 根据 service_id 映射到 localhost:port。
Control Server 负责授权。
```

### 19.4 Relay 是控制点

Relay 必须做：

```text
鉴权
限速
限流
流量统计
并发控制
强制断开
```

不要只把 Relay 当成普通 TCP 转发器。

### 19.5 P2P 不保证成功

P2P 成功率受 NAT、运营商、企业网络影响。

必须接受：

```text
P2P 是优化路径。
Relay 是基础路径。
```

### 19.6 Rust 化不要扩大到不必要的 UI 和平台胶水

Rust 化重点是核心链路：

```text
Agent
Relay
Mobile core
Control
Punch
Protocol
Tunnel
```

非核心层保持务实：

```text
Swift/Kotlin wrapper
Admin Web
Infra
CI
```

---

## 20. 最终建议

MVP 不要做得太重。

推荐最小闭环：

```text
Rust PC Agent
+
Rust Relay
+
Rust Mobile SDK core
+
Rust Control Server
+
Rust Punch Server
+
QUIC Stream
+
Local Port Forward
+
Relay Fallback
```

最重要的第一阶段目标：

```text
手机 App 或 mobile-cli 调用 open_service
拿到本地端口 127.0.0.1:18080
然后像访问本地服务一样访问远程 PC 服务
```

后续 P2P、限速、移动端 wrapper 都应围绕这个最小闭环逐步补齐。
