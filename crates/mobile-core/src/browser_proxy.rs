use std::{
    io,
    net::IpAddr,
    pin::Pin,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    task::{Context, Poll},
    time::Duration,
};

use quic_tunnel_protocol::{DeviceId, ServiceId};
use quic_tunnel_tunnel::stats::AtomicTrafficStats;
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf},
    net::{TcpListener, TcpStream},
    sync::{oneshot, Semaphore},
    task::JoinHandle,
    time::{sleep, timeout, Instant},
};

use crate::forward::{BoxedStream, OpenForwardRequest, StreamConnector};

const HTTP_IO_BUFFER_LEN: usize = 64 * 1024;
const HTTP_HEAD_INITIAL_CAPACITY: usize = 8 * 1024;
const HTTP_HEAD_READ_BUFFER_LEN: usize = 8 * 1024;
const DEFAULT_MAX_BROWSER_PROXY_CONNECTIONS: usize = 256;
const DEFAULT_REQUEST_HEAD_TIMEOUT: Duration = Duration::from_secs(10);
const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const DEFAULT_TUNNEL_OPEN_TIMEOUT: Duration = Duration::from_secs(15);
const DEFAULT_IDLE_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserProxyTarget {
    pub device_id: DeviceId,
    pub service_id: ServiceId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserProxyConfig {
    pub bind_host: String,
    pub local_port: u16,
    pub domain_suffix: String,
    pub max_connections: usize,
    pub direct_fallback_policy: BrowserProxyDirectFallbackPolicy,
    pub request_head_timeout: Duration,
    pub direct_connect_timeout: Duration,
    pub tunnel_open_timeout: Duration,
    pub idle_timeout: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserProxyDirectFallbackPolicy {
    AllowAll,
    LocalNetworkAndDomain,
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BrowserProxyStats {
    pub accepted_connections: u64,
    pub active_connections: u64,
    pub tunnel_connections: u64,
    pub direct_connections: u64,
    pub forbidden_direct_connections: u64,
    pub tunnel_bytes_to_remote: u64,
    pub tunnel_bytes_to_browser: u64,
    pub direct_bytes_to_remote: u64,
    pub direct_bytes_to_browser: u64,
    pub request_head_timeouts: u64,
    pub tunnel_open_timeouts: u64,
    pub idle_timeout_closures: u64,
    pub direct_connect_failures: u64,
    pub connection_limit_rejections: u64,
    pub request_errors: u64,
}

#[derive(Clone, Default)]
pub struct BrowserProxyStatsHandle {
    counters: Arc<BrowserProxyStatsCounters>,
}

#[derive(Default)]
struct BrowserProxyStatsCounters {
    accepted_connections: AtomicU64,
    active_connections: AtomicU64,
    tunnel_connections: AtomicU64,
    direct_connections: AtomicU64,
    forbidden_direct_connections: AtomicU64,
    tunnel_bytes_to_remote: AtomicU64,
    tunnel_bytes_to_browser: AtomicU64,
    direct_bytes_to_remote: AtomicU64,
    direct_bytes_to_browser: AtomicU64,
    request_head_timeouts: AtomicU64,
    tunnel_open_timeouts: AtomicU64,
    idle_timeout_closures: AtomicU64,
    direct_connect_failures: AtomicU64,
    connection_limit_rejections: AtomicU64,
    request_errors: AtomicU64,
}

impl BrowserProxyStatsHandle {
    pub fn snapshot(&self) -> BrowserProxyStats {
        BrowserProxyStats {
            accepted_connections: self.load(&self.counters.accepted_connections),
            active_connections: self.load(&self.counters.active_connections),
            tunnel_connections: self.load(&self.counters.tunnel_connections),
            direct_connections: self.load(&self.counters.direct_connections),
            forbidden_direct_connections: self.load(&self.counters.forbidden_direct_connections),
            tunnel_bytes_to_remote: self.load(&self.counters.tunnel_bytes_to_remote),
            tunnel_bytes_to_browser: self.load(&self.counters.tunnel_bytes_to_browser),
            direct_bytes_to_remote: self.load(&self.counters.direct_bytes_to_remote),
            direct_bytes_to_browser: self.load(&self.counters.direct_bytes_to_browser),
            request_head_timeouts: self.load(&self.counters.request_head_timeouts),
            tunnel_open_timeouts: self.load(&self.counters.tunnel_open_timeouts),
            idle_timeout_closures: self.load(&self.counters.idle_timeout_closures),
            direct_connect_failures: self.load(&self.counters.direct_connect_failures),
            connection_limit_rejections: self.load(&self.counters.connection_limit_rejections),
            request_errors: self.load(&self.counters.request_errors),
        }
    }

    fn load(&self, counter: &AtomicU64) -> u64 {
        counter.load(Ordering::Relaxed)
    }

    fn increment(counter: &AtomicU64) {
        counter.fetch_add(1, Ordering::Relaxed);
    }

    fn add(counter: &AtomicU64, value: u64) {
        counter.fetch_add(value, Ordering::Relaxed);
    }

    fn decrement(counter: &AtomicU64) {
        counter.fetch_sub(1, Ordering::Relaxed);
    }

    fn accepted(&self) {
        Self::increment(&self.counters.accepted_connections);
    }

    fn active_guard(&self) -> ActiveConnectionGuard {
        Self::increment(&self.counters.active_connections);
        ActiveConnectionGuard {
            stats: self.clone(),
        }
    }

    fn tunnel_connection(&self) {
        Self::increment(&self.counters.tunnel_connections);
    }

    fn direct_connection(&self) {
        Self::increment(&self.counters.direct_connections);
    }

    fn forbidden_direct_connection(&self) {
        Self::increment(&self.counters.forbidden_direct_connections);
    }

    fn transfer(&self, route: ProxyConnectionRoute, to_remote: u64, to_browser: u64) {
        match route {
            ProxyConnectionRoute::Tunnel => {
                Self::add(&self.counters.tunnel_bytes_to_remote, to_remote);
                Self::add(&self.counters.tunnel_bytes_to_browser, to_browser);
            }
            ProxyConnectionRoute::Direct => {
                Self::add(&self.counters.direct_bytes_to_remote, to_remote);
                Self::add(&self.counters.direct_bytes_to_browser, to_browser);
            }
        }
    }

    fn request_head_timeout(&self) {
        Self::increment(&self.counters.request_head_timeouts);
    }

    fn tunnel_open_timeout(&self) {
        Self::increment(&self.counters.tunnel_open_timeouts);
    }

    fn idle_timeout_closure(&self) {
        Self::increment(&self.counters.idle_timeout_closures);
    }

    fn direct_connect_failure(&self) {
        Self::increment(&self.counters.direct_connect_failures);
    }

    fn connection_limit_rejection(&self) {
        Self::increment(&self.counters.connection_limit_rejections);
    }

    fn request_error(&self) {
        Self::increment(&self.counters.request_errors);
    }
}

struct ActiveConnectionGuard {
    stats: BrowserProxyStatsHandle,
}

impl Drop for ActiveConnectionGuard {
    fn drop(&mut self) {
        BrowserProxyStatsHandle::decrement(&self.stats.counters.active_connections);
    }
}

impl Default for BrowserProxyConfig {
    fn default() -> Self {
        Self {
            bind_host: "127.0.0.1".to_string(),
            local_port: 0,
            domain_suffix: ".qtunnel.local".to_string(),
            max_connections: DEFAULT_MAX_BROWSER_PROXY_CONNECTIONS,
            direct_fallback_policy: BrowserProxyDirectFallbackPolicy::LocalNetworkAndDomain,
            request_head_timeout: DEFAULT_REQUEST_HEAD_TIMEOUT,
            direct_connect_timeout: DEFAULT_CONNECT_TIMEOUT,
            tunnel_open_timeout: DEFAULT_TUNNEL_OPEN_TIMEOUT,
            idle_timeout: DEFAULT_IDLE_TIMEOUT,
        }
    }
}

impl BrowserProxyConfig {
    fn validate(&self) -> Result<(), BrowserProxyError> {
        validate_loopback_bind_host(&self.bind_host)?;
        validate_domain_suffix(&self.domain_suffix)?;
        if self.max_connections == 0 {
            return Err(BrowserProxyError::InvalidConfig {
                reason: "max_connections must be greater than 0".to_string(),
            });
        }
        validate_positive_timeout("request_head_timeout", self.request_head_timeout)?;
        validate_positive_timeout("direct_connect_timeout", self.direct_connect_timeout)?;
        validate_positive_timeout("tunnel_open_timeout", self.tunnel_open_timeout)?;
        validate_positive_timeout("idle_timeout", self.idle_timeout)?;
        Ok(())
    }
}

fn validate_positive_timeout(name: &str, timeout: Duration) -> Result<(), BrowserProxyError> {
    if timeout.is_zero() {
        return Err(BrowserProxyError::InvalidConfig {
            reason: format!("{name} must be greater than 0"),
        });
    }
    Ok(())
}

fn validate_loopback_bind_host(bind_host: &str) -> Result<(), BrowserProxyError> {
    let bind_host = bind_host.trim();
    if bind_host.eq_ignore_ascii_case("localhost") {
        return Ok(());
    }
    if bind_host
        .parse::<IpAddr>()
        .map(|ip| ip.is_loopback())
        .unwrap_or(false)
    {
        return Ok(());
    }

    Err(BrowserProxyError::InvalidConfig {
        reason: "bind_host must be a loopback address such as 127.0.0.1 or ::1".to_string(),
    })
}

fn validate_domain_suffix(suffix: &str) -> Result<(), BrowserProxyError> {
    if !suffix.starts_with('.') || suffix.len() <= 1 || suffix.ends_with('.') {
        return Err(BrowserProxyError::InvalidConfig {
            reason: "domain_suffix must look like .qtunnel.local".to_string(),
        });
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserProxyHandle {
    host: String,
    local_port: u16,
}

impl BrowserProxyHandle {
    pub fn new(host: String, local_port: u16) -> Self {
        Self { host, local_port }
    }

    pub fn host(&self) -> &str {
        &self.host
    }

    pub fn local_port(&self) -> u16 {
        self.local_port
    }
}

pub struct BrowserProxy {
    handle: BrowserProxyHandle,
    stats: BrowserProxyStatsHandle,
    shutdown_tx: Option<oneshot::Sender<()>>,
    task: JoinHandle<()>,
}

impl BrowserProxy {
    pub async fn bind(
        config: BrowserProxyConfig,
        connector: Arc<dyn StreamConnector>,
    ) -> Result<Self, BrowserProxyError> {
        config.validate()?;
        let listener = TcpListener::bind((config.bind_host.as_str(), config.local_port)).await?;
        let local_addr = listener.local_addr()?;
        let handle = BrowserProxyHandle::new(config.bind_host.clone(), local_addr.port());
        let stats = BrowserProxyStatsHandle::default();
        let (shutdown_tx, mut shutdown_rx) = oneshot::channel();

        let domain_suffix = config.domain_suffix;
        let runtime_config = Arc::new(BrowserProxyRuntimeConfig {
            request_head_timeout: config.request_head_timeout,
            direct_connect_timeout: config.direct_connect_timeout,
            direct_fallback_policy: config.direct_fallback_policy,
            tunnel_open_timeout: config.tunnel_open_timeout,
            idle_timeout: config.idle_timeout,
            stats: stats.clone(),
        });
        let connection_limit = Arc::new(Semaphore::new(config.max_connections));
        let task = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = &mut shutdown_rx => break,
                    accepted = listener.accept() => {
                        let Ok((browser_stream, _)) = accepted else {
                            break;
                        };
                        runtime_config.stats.accepted();
                        let Ok(connection_permit) = Arc::clone(&connection_limit).try_acquire_owned() else {
                            runtime_config.stats.connection_limit_rejection();
                            tokio::spawn(reject_browser_connection(browser_stream));
                            continue;
                        };
                        let connector = Arc::clone(&connector);
                        let domain_suffix = domain_suffix.clone();
                        let runtime_config = Arc::clone(&runtime_config);
                        tokio::spawn(async move {
                            let _connection_permit = connection_permit;
                            let _active_connection = runtime_config.stats.active_guard();
                            let request_stats = runtime_config.stats.clone();
                            if handle_browser_connection(
                                    browser_stream,
                                    connector,
                                    domain_suffix,
                                    runtime_config,
                                )
                                    .await
                                    .is_err()
                            {
                                request_stats.request_error();
                            }
                        });
                    }
                }
            }
        });

        Ok(Self {
            handle,
            stats,
            shutdown_tx: Some(shutdown_tx),
            task,
        })
    }

    pub fn handle(&self) -> BrowserProxyHandle {
        self.handle.clone()
    }

    pub fn local_port(&self) -> u16 {
        self.handle.local_port()
    }

    pub fn stats(&self) -> BrowserProxyStats {
        self.stats.snapshot()
    }

    pub fn stats_handle(&self) -> BrowserProxyStatsHandle {
        self.stats.clone()
    }

    pub async fn shutdown(mut self) -> Result<(), BrowserProxyError> {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        self.task.await.map_err(BrowserProxyError::Join)?;
        Ok(())
    }
}

struct BrowserProxyRuntimeConfig {
    request_head_timeout: Duration,
    direct_connect_timeout: Duration,
    direct_fallback_policy: BrowserProxyDirectFallbackPolicy,
    tunnel_open_timeout: Duration,
    idle_timeout: Duration,
    stats: BrowserProxyStatsHandle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProxyConnectionRoute {
    Tunnel,
    Direct,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserProxyUrlKind {
    DeviceService,
    ControlServer,
    DirectNetwork,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserProxyUrlClassification {
    pub kind: BrowserProxyUrlKind,
    pub host: String,
    pub target: Option<BrowserProxyTarget>,
}

pub fn parse_qtunnel_host(host: &str) -> Option<BrowserProxyTarget> {
    parse_qtunnel_host_with_suffix(host, ".qtunnel.local")
}

pub fn classify_browser_proxy_url(
    url: &str,
    control_server_url: &str,
    domain_suffix: &str,
) -> Result<BrowserProxyUrlClassification, BrowserProxyError> {
    validate_domain_suffix(domain_suffix)?;
    let parsed = parse_absolute_url(url)?;

    if let Some(target) = parse_qtunnel_host_with_suffix(&parsed.host, domain_suffix) {
        return Ok(BrowserProxyUrlClassification {
            kind: BrowserProxyUrlKind::DeviceService,
            host: parsed.host,
            target: Some(target),
        });
    }

    let control = parse_absolute_url(control_server_url)?;
    if parsed.authority.eq_ignore_ascii_case(&control.authority) {
        return Ok(BrowserProxyUrlClassification {
            kind: BrowserProxyUrlKind::ControlServer,
            host: parsed.host,
            target: None,
        });
    }

    Ok(BrowserProxyUrlClassification {
        kind: BrowserProxyUrlKind::DirectNetwork,
        host: parsed.host,
        target: None,
    })
}

pub fn browser_proxy_host(
    target: &BrowserProxyTarget,
    suffix: &str,
) -> Result<String, BrowserProxyError> {
    validate_domain_suffix(suffix)?;
    Ok(format!(
        "{}.{}{}",
        encode_host_label("s", target.service_id.as_str()),
        encode_host_label("d", target.device_id.as_str()),
        suffix
    ))
}

fn parse_qtunnel_host_with_suffix(host: &str, suffix: &str) -> Option<BrowserProxyTarget> {
    let host = host
        .split_once(':')
        .map(|(host, _)| host)
        .unwrap_or(host)
        .trim_end_matches('.');
    let base = host.strip_suffix(suffix)?;
    let (service_id, device_id) = base.split_once('.')?;
    if let Some(target) = decode_dns_safe_target(service_id, device_id) {
        return Some(target);
    }
    if service_id.is_empty() || device_id.is_empty() || device_id.contains('.') {
        return None;
    }

    Some(BrowserProxyTarget {
        device_id: DeviceId::new(device_id),
        service_id: ServiceId::new(service_id),
    })
}

fn encode_host_label(prefix: &str, value: &str) -> String {
    let mut encoded = String::with_capacity(prefix.len() + 1 + value.len());
    encoded.push_str(prefix);
    encoded.push('-');
    for byte in value.bytes() {
        if byte.is_ascii_lowercase() || byte.is_ascii_digit() {
            encoded.push(byte as char);
        } else if byte == b'-' {
            encoded.push_str("--");
        } else {
            encoded.push('-');
            encoded.push(hex_char(byte >> 4));
            encoded.push(hex_char(byte & 0x0f));
        }
    }
    encoded
}

fn decode_dns_safe_target(service_label: &str, device_label: &str) -> Option<BrowserProxyTarget> {
    let service_id = decode_host_label("s", service_label)?;
    let device_id = decode_host_label("d", device_label)?;
    Some(BrowserProxyTarget {
        device_id: DeviceId::new(device_id),
        service_id: ServiceId::new(service_id),
    })
}

fn decode_host_label(prefix: &str, label: &str) -> Option<String> {
    let encoded = label.strip_prefix(prefix)?.strip_prefix('-')?;
    let bytes = encoded.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        if byte != b'-' {
            decoded.push(byte);
            index += 1;
            continue;
        }

        let next = *bytes.get(index + 1)?;
        if next == b'-' {
            decoded.push(b'-');
            index += 2;
            continue;
        }

        let high = from_hex(next)?;
        let low = from_hex(*bytes.get(index + 2)?)?;
        decoded.push((high << 4) | low);
        index += 3;
    }

    String::from_utf8(decoded)
        .ok()
        .filter(|value| !value.is_empty())
}

fn hex_char(value: u8) -> char {
    match value {
        0..=9 => (b'0' + value) as char,
        10..=15 => (b'a' + value - 10) as char,
        _ => unreachable!("hex nibble out of range"),
    }
}

fn from_hex(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        _ => None,
    }
}

struct ParsedAbsoluteUrl {
    authority: String,
    host: String,
}

fn parse_absolute_url(url: &str) -> Result<ParsedAbsoluteUrl, BrowserProxyError> {
    let url = url.trim();
    let Some((scheme, rest)) = url.split_once("://") else {
        return Err(BrowserProxyError::InvalidRequest {
            reason: "url must be absolute and include a scheme".to_string(),
        });
    };
    if scheme.is_empty()
        || !scheme
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'-' | b'.'))
    {
        return Err(BrowserProxyError::InvalidRequest {
            reason: "url scheme is invalid".to_string(),
        });
    }

    let authority_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..authority_end];
    if authority.is_empty() {
        return Err(BrowserProxyError::InvalidRequest {
            reason: "url host is required".to_string(),
        });
    }
    let authority = authority
        .rsplit_once('@')
        .map(|(_, host)| host)
        .unwrap_or(authority);
    let (normalized_authority, host) = normalize_authority(authority)?;
    Ok(ParsedAbsoluteUrl {
        authority: normalized_authority,
        host,
    })
}

fn normalize_authority(authority: &str) -> Result<(String, String), BrowserProxyError> {
    if let Some(rest) = authority.strip_prefix('[') {
        let Some((host, after_bracket)) = rest.split_once(']') else {
            return Err(BrowserProxyError::InvalidRequest {
                reason: "bracketed IPv6 url host is missing closing bracket".to_string(),
            });
        };
        if host.is_empty() {
            return Err(BrowserProxyError::InvalidRequest {
                reason: "url host is required".to_string(),
            });
        }
        if !after_bracket.is_empty()
            && !after_bracket
                .strip_prefix(':')
                .map(|port| !port.is_empty() && port.bytes().all(|byte| byte.is_ascii_digit()))
                .unwrap_or(false)
        {
            return Err(BrowserProxyError::InvalidRequest {
                reason: "url port is invalid".to_string(),
            });
        }
        let host = host.to_ascii_lowercase();
        return Ok((format!("[{host}]{after_bracket}"), host));
    }

    let (host, port) = match authority.rsplit_once(':') {
        Some((host, port))
            if !host.contains(':')
                && !port.is_empty()
                && port.bytes().all(|byte| byte.is_ascii_digit()) =>
        {
            (host, Some(port))
        }
        Some((_host, port)) if port.is_empty() => {
            return Err(BrowserProxyError::InvalidRequest {
                reason: "url port is invalid".to_string(),
            });
        }
        _ => (authority, None),
    };
    let host = host.trim_end_matches('.');
    if host.is_empty() {
        return Err(BrowserProxyError::InvalidRequest {
            reason: "url host is required".to_string(),
        });
    }
    let host = host.to_ascii_lowercase();
    let authority = if let Some(port) = port {
        format!("{host}:{port}")
    } else {
        host.clone()
    };
    Ok((authority, host))
}

async fn reject_browser_connection(mut stream: TcpStream) {
    let _ = stream
        .write_all(
            b"HTTP/1.1 503 Service Unavailable\r\n\
              Connection: close\r\n\
              Content-Length: 0\r\n\
              \r\n",
        )
        .await;
    let _ = stream.shutdown().await;
}

async fn handle_browser_connection(
    mut browser_stream: tokio::net::TcpStream,
    connector: Arc<dyn StreamConnector>,
    domain_suffix: String,
    config: Arc<BrowserProxyRuntimeConfig>,
) -> Result<(), BrowserProxyError> {
    browser_stream.set_nodelay(true)?;
    let request_parts = match timeout(
        config.request_head_timeout,
        read_http_request(&mut browser_stream),
    )
    .await
    {
        Ok(result) => result?,
        Err(_) => {
            config.stats.request_head_timeout();
            write_proxy_error_response(
                &mut browser_stream,
                b"HTTP/1.1 408 Request Timeout\r\nConnection: close\r\nContent-Length: 0\r\n\r\n",
            )
            .await;
            return Ok(());
        }
    };
    let request = ParsedProxyRequest::parse(&request_parts.head)?;
    let http_request = if request.method.eq_ignore_ascii_case("CONNECT") {
        None
    } else {
        Some(PreparedHttpRequest {
            rewritten_head: rewrite_absolute_form_request(&request_parts.head)?,
            body_kind: request_body_kind(&request_parts.head)?,
        })
    };
    let (mut remote_stream, route): (BoxedStream, ProxyConnectionRoute) =
        if let Some(target) = parse_qtunnel_host_with_suffix(&request.host, &domain_suffix) {
            let forward_request = OpenForwardRequest {
                device_id: target.device_id,
                service_id: target.service_id,
                local_port: 0,
            };
            match timeout(
                config.tunnel_open_timeout,
                connector.open_stream(&forward_request),
            )
            .await
            {
                Ok(result) => {
                    let stream = result?;
                    config.stats.tunnel_connection();
                    (stream, ProxyConnectionRoute::Tunnel)
                }
                Err(_) => {
                    config.stats.tunnel_open_timeout();
                    write_gateway_timeout_response(&mut browser_stream).await;
                    return Ok(());
                }
            }
        } else {
            if !direct_fallback_allowed(&request.host, config.direct_fallback_policy) {
                config.stats.forbidden_direct_connection();
                write_direct_fallback_forbidden_response(&mut browser_stream).await;
                return Ok(());
            }
            let direct_stream = match timeout(
                config.direct_connect_timeout,
                TcpStream::connect(request.direct_addr()),
            )
            .await
            {
                Ok(Ok(stream)) => stream,
                Ok(Err(_)) | Err(_) => {
                    config.stats.direct_connect_failure();
                    write_gateway_timeout_response(&mut browser_stream).await;
                    return Ok(());
                }
            };
            direct_stream.set_nodelay(true)?;
            config.stats.direct_connection();
            (Box::new(direct_stream), ProxyConnectionRoute::Direct)
        };

    if request.method.eq_ignore_ascii_case("CONNECT") {
        browser_stream
            .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
            .await?;
        if !request_parts.body_prefix.is_empty() {
            remote_stream.write_all(&request_parts.body_prefix).await?;
        }
    } else {
        let http_request = http_request.expect("non-CONNECT request is prepared");
        let bytes_to_remote = forward_single_http_request(
            &mut browser_stream,
            &mut remote_stream,
            &request_parts,
            http_request,
        )
        .await?;
        let bytes_to_browser =
            copy_response_to_browser(&mut remote_stream, &mut browser_stream).await?;
        config
            .stats
            .transfer(route, bytes_to_remote, bytes_to_browser);
        let _ = browser_stream.shutdown().await;
        let _ = remote_stream.shutdown().await;
        return Ok(());
    }

    let transfer = copy_bidirectional_with_idle_timeout(
        browser_stream,
        remote_stream,
        Arc::new(AtomicTrafficStats::default()),
        config.idle_timeout,
    )
    .await?;
    config
        .stats
        .transfer(route, transfer.to_remote_bytes, transfer.to_browser_bytes);
    if transfer.idle_timeout {
        config.stats.idle_timeout_closure();
    }
    Ok(())
}

async fn write_gateway_timeout_response(stream: &mut TcpStream) {
    write_proxy_error_response(
        stream,
        b"HTTP/1.1 504 Gateway Timeout\r\nConnection: close\r\nContent-Length: 0\r\n\r\n",
    )
    .await;
}

async fn write_direct_fallback_forbidden_response(stream: &mut TcpStream) {
    write_proxy_error_response(
        stream,
        b"HTTP/1.1 403 Forbidden\r\nConnection: close\r\nContent-Length: 0\r\n\r\n",
    )
    .await;
}

async fn write_proxy_error_response(stream: &mut TcpStream, response: &[u8]) {
    let _ = stream.write_all(response).await;
    let _ = stream.shutdown().await;
}

fn direct_fallback_allowed(host: &str, policy: BrowserProxyDirectFallbackPolicy) -> bool {
    match policy {
        BrowserProxyDirectFallbackPolicy::AllowAll => true,
        BrowserProxyDirectFallbackPolicy::Disabled => false,
        BrowserProxyDirectFallbackPolicy::LocalNetworkAndDomain => {
            direct_host_is_domain_or_local_network(host)
        }
    }
}

fn direct_host_is_domain_or_local_network(host: &str) -> bool {
    let host = direct_host_without_port(host);
    match host.parse::<IpAddr>() {
        Ok(ip) => is_local_network_ip(ip),
        Err(_) => true,
    }
}

fn direct_host_without_port(host: &str) -> &str {
    let host = host.trim();
    if let Some(rest) = host.strip_prefix('[') {
        if let Some((inside, _)) = rest.split_once(']') {
            return inside;
        }
    }

    if let Some((candidate, port)) = host.rsplit_once(':') {
        if !candidate.contains(':')
            && !candidate.is_empty()
            && !port.is_empty()
            && port.bytes().all(|byte| byte.is_ascii_digit())
        {
            return candidate.trim_end_matches('.');
        }
    }

    host.trim_end_matches('.')
}

fn is_local_network_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => ip.is_loopback() || ip.is_private() || ip.is_link_local(),
        IpAddr::V6(ip) => ip.is_loopback() || ip.is_unique_local() || ip.is_unicast_link_local(),
    }
}

async fn copy_bidirectional_with_idle_timeout<A, B>(
    uplink: A,
    downlink: B,
    stats: Arc<AtomicTrafficStats>,
    idle_timeout: Duration,
) -> io::Result<BidirectionalTransferStats>
where
    A: AsyncRead + AsyncWrite + Unpin,
    B: AsyncRead + AsyncWrite + Unpin,
{
    let activity = Arc::new(Mutex::new(Instant::now()));
    let to_remote_bytes = Arc::new(AtomicU64::new(0));
    let to_browser_bytes = Arc::new(AtomicU64::new(0));
    let mut uplink =
        ActivityStream::new(uplink, Arc::clone(&activity), Arc::clone(&to_remote_bytes));
    let mut downlink = ActivityStream::new(
        downlink,
        Arc::clone(&activity),
        Arc::clone(&to_browser_bytes),
    );

    stats.begin_stream();
    let copy_result = tokio::select! {
        result = tokio::io::copy_bidirectional(&mut uplink, &mut downlink) => {
            Some(result)
        }
        _ = wait_until_idle(Arc::clone(&activity), idle_timeout) => {
            None
        }
    };
    stats.end_stream();

    let idle_timeout = copy_result.is_none();
    if let Some(result) = copy_result {
        let _ = result?;
    }

    let to_remote_bytes = to_remote_bytes.load(Ordering::Relaxed);
    let to_browser_bytes = to_browser_bytes.load(Ordering::Relaxed);
    stats.add_uplink(to_remote_bytes);
    stats.add_downlink(to_browser_bytes);
    Ok(BidirectionalTransferStats {
        to_remote_bytes,
        to_browser_bytes,
        idle_timeout,
    })
}

async fn wait_until_idle(activity: Arc<Mutex<Instant>>, idle_timeout: Duration) {
    loop {
        let elapsed = activity
            .lock()
            .expect("browser proxy activity lock poisoned")
            .elapsed();
        if elapsed >= idle_timeout {
            return;
        }
        sleep(idle_timeout - elapsed).await;
    }
}

struct BidirectionalTransferStats {
    to_remote_bytes: u64,
    to_browser_bytes: u64,
    idle_timeout: bool,
}

struct ActivityStream<S> {
    inner: S,
    activity: Arc<Mutex<Instant>>,
    bytes_read: Arc<AtomicU64>,
}

impl<S> ActivityStream<S> {
    fn new(inner: S, activity: Arc<Mutex<Instant>>, bytes_read: Arc<AtomicU64>) -> Self {
        Self {
            inner,
            activity,
            bytes_read,
        }
    }

    fn mark_active(&self) {
        *self
            .activity
            .lock()
            .expect("browser proxy activity lock poisoned") = Instant::now();
    }
}

impl<S> AsyncRead for ActivityStream<S>
where
    S: AsyncRead + Unpin,
{
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let before = buf.filled().len();
        let result = Pin::new(&mut self.inner).poll_read(cx, buf);
        if matches!(result, Poll::Ready(Ok(()))) && buf.filled().len() > before {
            self.bytes_read
                .fetch_add((buf.filled().len() - before) as u64, Ordering::Relaxed);
            self.mark_active();
        }
        result
    }
}

impl<S> AsyncWrite for ActivityStream<S>
where
    S: AsyncWrite + Unpin,
{
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let result = Pin::new(&mut self.inner).poll_write(cx, buf);
        if let Poll::Ready(Ok(written)) = result {
            if written > 0 {
                self.mark_active();
            }
            return Poll::Ready(Ok(written));
        }
        result
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

struct PreparedHttpRequest {
    rewritten_head: Vec<u8>,
    body_kind: RequestBodyKind,
}

async fn forward_single_http_request(
    browser_stream: &mut TcpStream,
    remote_stream: &mut BoxedStream,
    request_parts: &HttpRequestParts,
    http_request: PreparedHttpRequest,
) -> Result<u64, BrowserProxyError> {
    let mut written = http_request.rewritten_head.len() as u64;
    remote_stream
        .write_all(&http_request.rewritten_head)
        .await?;

    let mut body_input = RequestBodyInput::new(&request_parts.body_prefix, browser_stream);
    match http_request.body_kind {
        RequestBodyKind::Fixed(length) => {
            written += body_input.copy_exact_to(remote_stream, length).await?;
        }
        RequestBodyKind::Chunked => {
            written += copy_chunked_body(&mut body_input, remote_stream).await?;
        }
    }

    remote_stream.flush().await?;
    Ok(written)
}

async fn copy_response_to_browser(
    remote_stream: &mut BoxedStream,
    browser_stream: &mut TcpStream,
) -> Result<u64, BrowserProxyError> {
    let mut total = 0_u64;
    let mut buffer = vec![0_u8; HTTP_IO_BUFFER_LEN];
    loop {
        let read = remote_stream.read(&mut buffer).await?;
        if read == 0 {
            break;
        }
        browser_stream.write_all(&buffer[..read]).await?;
        total += read as u64;
    }
    browser_stream.flush().await?;
    Ok(total)
}

enum RequestBodyKind {
    Fixed(usize),
    Chunked,
}

struct RequestBodyInput<'a> {
    buffer: Vec<u8>,
    offset: usize,
    stream: &'a mut TcpStream,
}

impl<'a> RequestBodyInput<'a> {
    fn new(prefix: &'a [u8], stream: &'a mut TcpStream) -> Self {
        let mut buffer = Vec::with_capacity(prefix.len().max(HTTP_HEAD_READ_BUFFER_LEN));
        buffer.extend_from_slice(prefix);
        Self {
            buffer,
            offset: 0,
            stream,
        }
    }

    fn unread(&self) -> &[u8] {
        &self.buffer[self.offset..]
    }

    fn compact_buffer(&mut self) {
        if self.offset == 0 {
            return;
        }
        if self.offset >= self.buffer.len() {
            self.buffer.clear();
            self.offset = 0;
            return;
        }
        if self.offset > HTTP_IO_BUFFER_LEN || self.offset > self.buffer.len() / 2 {
            let remaining = self.buffer.len() - self.offset;
            self.buffer.copy_within(self.offset.., 0);
            self.buffer.truncate(remaining);
            self.offset = 0;
        }
    }

    async fn fill_buffer(&mut self) -> Result<(), BrowserProxyError> {
        self.compact_buffer();
        let start = self.buffer.len();
        self.buffer.resize(start + HTTP_HEAD_READ_BUFFER_LEN, 0);
        let read = self.stream.read(&mut self.buffer[start..]).await?;
        if read == 0 {
            self.buffer.truncate(start);
            return Err(BrowserProxyError::InvalidRequest {
                reason: "connection closed before request body completed".to_string(),
            });
        }
        self.buffer.truncate(start + read);
        Ok(())
    }

    fn consume_vec(&mut self, len: usize) -> Vec<u8> {
        let start = self.offset;
        let end = start + len;
        let bytes = self.buffer[start..end].to_vec();
        self.offset = end;
        self.compact_buffer();
        bytes
    }

    async fn read_exact_vec(&mut self, len: usize) -> Result<Vec<u8>, BrowserProxyError> {
        while self.unread().len() < len {
            self.fill_buffer().await?;
        }
        Ok(self.consume_vec(len))
    }

    async fn read_crlf_line(&mut self, max_len: usize) -> Result<Vec<u8>, BrowserProxyError> {
        loop {
            if let Some(position) = find_crlf(self.unread()) {
                let line_len = position + 2;
                if line_len > max_len {
                    return Err(BrowserProxyError::InvalidRequest {
                        reason: "chunked request line is too large".to_string(),
                    });
                }
                return Ok(self.consume_vec(line_len));
            }

            if self.unread().len() >= max_len {
                return Err(BrowserProxyError::InvalidRequest {
                    reason: "chunked request line is too large".to_string(),
                });
            }
            self.fill_buffer().await?;
        }
    }

    async fn copy_exact_to(
        &mut self,
        remote_stream: &mut BoxedStream,
        mut remaining: usize,
    ) -> Result<u64, BrowserProxyError> {
        let mut copied = 0_u64;
        while remaining > 0 {
            if !self.unread().is_empty() {
                let available = self.unread().len();
                let take = available.min(remaining);
                remote_stream.write_all(&self.unread()[..take]).await?;
                self.offset += take;
                remaining -= take;
                copied += take as u64;
                self.compact_buffer();
                continue;
            }

            let mut buffer = vec![0_u8; HTTP_IO_BUFFER_LEN];
            let read_len = buffer.len().min(remaining);
            let read = self.stream.read(&mut buffer[..read_len]).await?;
            if read == 0 {
                return Err(BrowserProxyError::InvalidRequest {
                    reason: "connection closed before request body completed".to_string(),
                });
            }
            remote_stream.write_all(&buffer[..read]).await?;
            remaining -= read;
            copied += read as u64;
        }

        Ok(copied)
    }
}

async fn copy_chunked_body(
    body_input: &mut RequestBodyInput<'_>,
    remote_stream: &mut BoxedStream,
) -> Result<u64, BrowserProxyError> {
    const MAX_CHUNK_LINE_LEN: usize = 8 * 1024;
    const MAX_TRAILER_BYTES: usize = 64 * 1024;
    let mut copied = 0_u64;

    loop {
        let chunk_line = body_input.read_crlf_line(MAX_CHUNK_LINE_LEN).await?;
        let chunk_size = parse_chunk_size(&chunk_line)?;
        remote_stream.write_all(&chunk_line).await?;
        copied += chunk_line.len() as u64;

        if chunk_size == 0 {
            let mut trailer_bytes = 0;
            loop {
                let trailer_line = body_input.read_crlf_line(MAX_CHUNK_LINE_LEN).await?;
                trailer_bytes += trailer_line.len();
                if trailer_bytes > MAX_TRAILER_BYTES {
                    return Err(BrowserProxyError::InvalidRequest {
                        reason: "chunked request trailers are too large".to_string(),
                    });
                }

                let is_final_line = trailer_line == b"\r\n";
                remote_stream.write_all(&trailer_line).await?;
                copied += trailer_line.len() as u64;
                if is_final_line {
                    return Ok(copied);
                }
            }
        }

        copied += body_input.copy_exact_to(remote_stream, chunk_size).await?;
        let chunk_end = body_input.read_exact_vec(2).await?;
        if chunk_end != b"\r\n" {
            return Err(BrowserProxyError::InvalidRequest {
                reason: "chunked request chunk is missing CRLF terminator".to_string(),
            });
        }
        remote_stream.write_all(&chunk_end).await?;
        copied += chunk_end.len() as u64;
    }
}

fn parse_chunk_size(chunk_line: &[u8]) -> Result<usize, BrowserProxyError> {
    let line =
        std::str::from_utf8(chunk_line).map_err(|error| BrowserProxyError::InvalidRequest {
            reason: format!("chunked request line is not utf-8: {error}"),
        })?;
    let Some(line) = line.strip_suffix("\r\n") else {
        return Err(BrowserProxyError::InvalidRequest {
            reason: "chunked request line is missing CRLF".to_string(),
        });
    };
    let size = line.split_once(';').map(|(size, _)| size).unwrap_or(line);
    let size = size.trim();
    if size.is_empty() {
        return Err(BrowserProxyError::InvalidRequest {
            reason: "chunked request chunk size is empty".to_string(),
        });
    }

    usize::from_str_radix(size, 16).map_err(|error| BrowserProxyError::InvalidRequest {
        reason: format!("invalid chunked request chunk size: {error}"),
    })
}

fn find_crlf(buffer: &[u8]) -> Option<usize> {
    buffer.windows(2).position(|window| window == b"\r\n")
}

struct HttpRequestParts {
    head: Vec<u8>,
    body_prefix: Vec<u8>,
}

async fn read_http_request(
    stream: &mut tokio::net::TcpStream,
) -> Result<HttpRequestParts, BrowserProxyError> {
    const MAX_HEAD_LEN: usize = 64 * 1024;
    let mut buffer = Vec::with_capacity(HTTP_HEAD_INITIAL_CAPACITY);
    let mut read_buffer = [0_u8; HTTP_HEAD_READ_BUFFER_LEN];
    let mut scan_start = 0_usize;
    loop {
        let read = stream.read(&mut read_buffer).await?;
        if read == 0 {
            return Err(BrowserProxyError::InvalidRequest {
                reason: "connection closed before request head".to_string(),
            });
        }
        buffer.extend_from_slice(&read_buffer[..read]);
        let search_start = scan_start.saturating_sub(3);
        if let Some(head_end) = find_request_head_end(&buffer, search_start) {
            let body_prefix = buffer.split_off(head_end);
            return Ok(HttpRequestParts {
                head: buffer,
                body_prefix,
            });
        }
        scan_start = buffer.len();
        if buffer.len() > MAX_HEAD_LEN {
            return Err(BrowserProxyError::InvalidRequest {
                reason: "request head is too large".to_string(),
            });
        }
    }
}

fn find_request_head_end(buffer: &[u8], start: usize) -> Option<usize> {
    buffer[start..]
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|position| start + position + 4)
}

#[derive(Debug)]
struct ParsedProxyRequest {
    method: String,
    target: String,
    version: String,
    host: String,
}

impl ParsedProxyRequest {
    fn parse(head: &[u8]) -> Result<Self, BrowserProxyError> {
        let head =
            std::str::from_utf8(head).map_err(|error| BrowserProxyError::InvalidRequest {
                reason: format!("request head is not utf-8: {error}"),
            })?;
        let mut lines = head.split("\r\n");
        let request_line = lines
            .next()
            .ok_or_else(|| BrowserProxyError::InvalidRequest {
                reason: "missing request line".to_string(),
            })?;
        let mut request_parts = request_line.split_whitespace();
        let method = request_parts
            .next()
            .ok_or_else(|| BrowserProxyError::InvalidRequest {
                reason: "missing method".to_string(),
            })?
            .to_string();
        let target = request_parts
            .next()
            .ok_or_else(|| BrowserProxyError::InvalidRequest {
                reason: "missing target".to_string(),
            })?
            .to_string();
        let version = request_parts
            .next()
            .ok_or_else(|| BrowserProxyError::InvalidRequest {
                reason: "missing version".to_string(),
            })?
            .to_string();

        let host = if method.eq_ignore_ascii_case("CONNECT") {
            target
                .split_once(':')
                .map(|(host, _)| host)
                .unwrap_or(&target)
                .to_string()
        } else if let Some(host) = absolute_form_host(&target) {
            host.to_string()
        } else {
            lines
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    if name.eq_ignore_ascii_case("host") {
                        Some(value.trim().to_string())
                    } else {
                        None
                    }
                })
                .ok_or_else(|| BrowserProxyError::InvalidRequest {
                    reason: "missing host".to_string(),
                })?
        };

        Ok(Self {
            method,
            target,
            version,
            host,
        })
    }

    fn direct_addr(&self) -> String {
        if self.method.eq_ignore_ascii_case("CONNECT") {
            return self.target.clone();
        }
        if self.host.contains(':') {
            self.host.clone()
        } else if self.target.starts_with("https://") {
            format!("{}:443", self.host)
        } else {
            format!("{}:80", self.host)
        }
    }
}

fn rewrite_absolute_form_request(head: &[u8]) -> Result<Vec<u8>, BrowserProxyError> {
    let head = std::str::from_utf8(head).map_err(|error| BrowserProxyError::InvalidRequest {
        reason: format!("request head is not utf-8: {error}"),
    })?;
    let (_request_line, rest) =
        head.split_once("\r\n")
            .ok_or_else(|| BrowserProxyError::InvalidRequest {
                reason: "missing request line terminator".to_string(),
            })?;
    let request = ParsedProxyRequest::parse(head.as_bytes())?;
    let origin_target = absolute_form_origin(&request.target).unwrap_or(&request.target);

    let mut rewritten = Vec::with_capacity(head.len());
    rewritten.extend_from_slice(
        format!(
            "{} {} {}\r\n",
            request.method, origin_target, request.version
        )
        .as_bytes(),
    );

    let mut wrote_connection_close = false;
    for line in rest.split("\r\n") {
        if line.is_empty() {
            if !wrote_connection_close {
                rewritten.extend_from_slice(b"Connection: close\r\n");
            }
            rewritten.extend_from_slice(b"\r\n");
            return Ok(rewritten);
        }

        let Some((name, _)) = line.split_once(':') else {
            return Err(BrowserProxyError::InvalidRequest {
                reason: format!("malformed header line: {line}"),
            });
        };

        if name.eq_ignore_ascii_case("connection") {
            if !wrote_connection_close {
                rewritten.extend_from_slice(b"Connection: close\r\n");
                wrote_connection_close = true;
            }
            continue;
        }
        if name.eq_ignore_ascii_case("proxy-connection") || name.eq_ignore_ascii_case("keep-alive")
        {
            continue;
        }

        rewritten.extend_from_slice(line.as_bytes());
        rewritten.extend_from_slice(b"\r\n");
    }

    Err(BrowserProxyError::InvalidRequest {
        reason: "missing request head terminator".to_string(),
    })
}

fn request_body_kind(head: &[u8]) -> Result<RequestBodyKind, BrowserProxyError> {
    let head = std::str::from_utf8(head).map_err(|error| BrowserProxyError::InvalidRequest {
        reason: format!("request head is not utf-8: {error}"),
    })?;
    let mut length = None;
    let mut has_chunked_transfer_encoding = false;

    for line in head.split("\r\n").skip(1) {
        if line.is_empty() {
            break;
        }
        let Some((name, value)) = line.split_once(':') else {
            return Err(BrowserProxyError::InvalidRequest {
                reason: format!("malformed header line: {line}"),
            });
        };

        if name.eq_ignore_ascii_case("content-length") {
            let parsed = value.trim().parse::<usize>().map_err(|error| {
                BrowserProxyError::InvalidRequest {
                    reason: format!("invalid content-length: {error}"),
                }
            })?;
            if let Some(existing) = length {
                if existing != parsed {
                    return Err(BrowserProxyError::InvalidRequest {
                        reason: "conflicting content-length headers".to_string(),
                    });
                }
            } else {
                length = Some(parsed);
            }
        } else if name.eq_ignore_ascii_case("transfer-encoding")
            && value
                .split(',')
                .any(|coding| coding.trim().eq_ignore_ascii_case("chunked"))
        {
            has_chunked_transfer_encoding = true;
        }
    }

    if has_chunked_transfer_encoding {
        if length.is_some() {
            return Err(BrowserProxyError::InvalidRequest {
                reason: "chunked request must not include content-length".to_string(),
            });
        }
        return Ok(RequestBodyKind::Chunked);
    }

    Ok(RequestBodyKind::Fixed(length.unwrap_or(0)))
}

fn absolute_form_host(target: &str) -> Option<&str> {
    let without_scheme = target
        .strip_prefix("http://")
        .or_else(|| target.strip_prefix("https://"))?;
    let end = without_scheme
        .find(['/', '?', '#'])
        .unwrap_or(without_scheme.len());
    Some(&without_scheme[..end])
}

fn absolute_form_origin(target: &str) -> Option<&str> {
    let without_scheme = target
        .strip_prefix("http://")
        .or_else(|| target.strip_prefix("https://"))?;
    let start = without_scheme
        .find(['/', '?', '#'])
        .unwrap_or(without_scheme.len());
    let origin = &without_scheme[start..];
    if origin.is_empty() {
        Some("/")
    } else {
        Some(origin)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum BrowserProxyError {
    #[error("io failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("proxy task failed: {0}")]
    Join(#[from] tokio::task::JoinError),
    #[error("forward failed: {0}")]
    Forward(#[from] crate::forward::ForwardError),
    #[error("invalid request: {reason}")]
    InvalidRequest { reason: String },
    #[error("invalid browser proxy config: {reason}")]
    InvalidConfig { reason: String },
    #[error("unsupported qtunnel host: {host}")]
    UnsupportedHost { host: String },
}
