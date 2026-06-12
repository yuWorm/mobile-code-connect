use std::{future, sync::Arc};

use async_trait::async_trait;
use quic_tunnel_mobile_core::{
    browser_proxy::{
        browser_proxy_host, classify_browser_proxy_url, parse_qtunnel_host, BrowserProxy,
        BrowserProxyConfig, BrowserProxyDirectFallbackPolicy, BrowserProxyStats,
        BrowserProxyTarget, BrowserProxyUrlKind,
    },
    forward::{
        BoxedStream, ForwardError, MemoryStreamConnector, OpenForwardRequest, StreamConnector,
    },
};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    time::{sleep, timeout, Duration},
};

#[test]
fn parses_qtunnel_proxy_hosts() {
    let target = parse_qtunnel_host("svc_web_3000.pc_001.qtunnel.local").unwrap();

    assert_eq!(target.device_id.as_str(), "pc_001");
    assert_eq!(target.service_id.as_str(), "svc_web_3000");
    assert!(parse_qtunnel_host("svc_web_3000.qtunnel.local").is_none());
    assert!(parse_qtunnel_host("svc_web_3000.pc_001.example.com").is_none());
}

#[test]
fn browser_proxy_host_generates_dns_safe_reversible_hosts() {
    let host = browser_proxy_host(
        &BrowserProxyTarget {
            device_id: "pc_001".into(),
            service_id: "svc_web_3000".into(),
        },
        ".qtunnel.local",
    )
    .unwrap();

    assert_eq!(host, "s-svc-5fweb-5f3000.d-pc-5f001.qtunnel.local");
    assert!(host
        .trim_end_matches(".qtunnel.local")
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-' || ch == '.'));

    let target = parse_qtunnel_host(&host).unwrap();
    assert_eq!(target.device_id.as_str(), "pc_001");
    assert_eq!(target.service_id.as_str(), "svc_web_3000");
}

#[test]
fn classifies_browser_proxy_urls_for_device_control_and_direct_targets() {
    let synthetic_host = browser_proxy_host(
        &BrowserProxyTarget {
            device_id: "pc_001".into(),
            service_id: "svc_web_3000".into(),
        },
        ".qtunnel.local",
    )
    .unwrap();

    let device = classify_browser_proxy_url(
        &format!("http://{synthetic_host}/status?q=1"),
        "https://control.example.test/api",
        ".qtunnel.local",
    )
    .unwrap();
    assert_eq!(device.kind, BrowserProxyUrlKind::DeviceService);
    assert_eq!(device.host, synthetic_host);
    let target = device.target.expect("device service target");
    assert_eq!(target.device_id.as_str(), "pc_001");
    assert_eq!(target.service_id.as_str(), "svc_web_3000");

    let control = classify_browser_proxy_url(
        "https://control.example.test/devices",
        "https://control.example.test/api",
        ".qtunnel.local",
    )
    .unwrap();
    assert_eq!(control.kind, BrowserProxyUrlKind::ControlServer);
    assert_eq!(control.host, "control.example.test");
    assert!(control.target.is_none());

    let direct = classify_browser_proxy_url(
        "https://example.com/assets/app.js",
        "https://control.example.test/api",
        ".qtunnel.local",
    )
    .unwrap();
    assert_eq!(direct.kind, BrowserProxyUrlKind::DirectNetwork);
    assert_eq!(direct.host, "example.com");
    assert!(direct.target.is_none());
}

#[tokio::test]
async fn browser_proxy_rejects_non_loopback_bind_hosts() {
    let connector = Arc::new(MemoryStreamConnector::default());
    let result = BrowserProxy::bind(
        BrowserProxyConfig {
            bind_host: "0.0.0.0".to_string(),
            ..BrowserProxyConfig::default()
        },
        connector,
    )
    .await;
    let Err(error) = result else {
        panic!("non-loopback bind host should fail");
    };

    assert!(error.to_string().contains("bind_host"));
}

#[tokio::test]
async fn browser_proxy_rejects_connections_over_the_configured_limit() {
    let connector = Arc::new(MemoryStreamConnector::default());
    let proxy = BrowserProxy::bind(
        BrowserProxyConfig {
            max_connections: 1,
            ..BrowserProxyConfig::default()
        },
        connector.clone(),
    )
    .await
    .unwrap();

    let mut first = TcpStream::connect(("127.0.0.1", proxy.local_port()))
        .await
        .unwrap();
    first
        .write_all(
            b"CONNECT svc_web_3000.pc_001.qtunnel.local:443 HTTP/1.1\r\n\
              Host: svc_web_3000.pc_001.qtunnel.local:443\r\n\
              \r\n",
        )
        .await
        .unwrap();
    let mut established = vec![0_u8; 128];
    let read = first.read(&mut established).await.unwrap();
    assert!(String::from_utf8_lossy(&established[..read])
        .starts_with("HTTP/1.1 200 Connection Established"));
    let _remote = connector.accept().await.unwrap();

    let mut second = TcpStream::connect(("127.0.0.1", proxy.local_port()))
        .await
        .unwrap();
    second
        .write_all(
            b"GET http://svc_api.pc_001.qtunnel.local/ HTTP/1.1\r\n\
              Host: svc_api.pc_001.qtunnel.local\r\n\
              \r\n",
        )
        .await
        .unwrap();
    let mut rejected = vec![0_u8; 128];
    let read = timeout(Duration::from_secs(1), second.read(&mut rejected))
        .await
        .expect("timed out waiting for proxy rejection")
        .expect("read proxy rejection");
    assert!(
        String::from_utf8_lossy(&rejected[..read]).starts_with("HTTP/1.1 503 Service Unavailable")
    );
    assert!(timeout(Duration::from_millis(50), connector.accept())
        .await
        .is_err());

    drop(first);
    proxy.shutdown().await.unwrap();
}

#[tokio::test]
async fn browser_proxy_times_out_slow_request_headers() {
    let connector = Arc::new(MemoryStreamConnector::default());
    let proxy = BrowserProxy::bind(
        BrowserProxyConfig {
            request_head_timeout: Duration::from_millis(30),
            ..BrowserProxyConfig::default()
        },
        connector,
    )
    .await
    .unwrap();
    let mut browser = TcpStream::connect(("127.0.0.1", proxy.local_port()))
        .await
        .unwrap();

    let mut response = vec![0_u8; 128];
    let read = timeout(Duration::from_secs(1), browser.read(&mut response))
        .await
        .expect("timed out waiting for request timeout")
        .expect("read timeout response");
    assert!(String::from_utf8_lossy(&response[..read]).starts_with("HTTP/1.1 408 Request Timeout"));

    proxy.shutdown().await.unwrap();
}

#[tokio::test]
async fn browser_proxy_returns_gateway_timeout_when_tunnel_open_times_out() {
    let connector = Arc::new(HangingStreamConnector);
    let proxy = BrowserProxy::bind(
        BrowserProxyConfig {
            tunnel_open_timeout: Duration::from_millis(30),
            ..BrowserProxyConfig::default()
        },
        connector,
    )
    .await
    .unwrap();
    let mut browser = TcpStream::connect(("127.0.0.1", proxy.local_port()))
        .await
        .unwrap();

    browser
        .write_all(
            b"GET http://svc_api.pc_001.qtunnel.local/ HTTP/1.1\r\n\
              Host: svc_api.pc_001.qtunnel.local\r\n\
              \r\n",
        )
        .await
        .unwrap();
    let mut response = vec![0_u8; 128];
    let read = timeout(Duration::from_secs(1), browser.read(&mut response))
        .await
        .expect("timed out waiting for gateway timeout")
        .expect("read gateway timeout response");
    assert!(String::from_utf8_lossy(&response[..read]).starts_with("HTTP/1.1 504 Gateway Timeout"));

    proxy.shutdown().await.unwrap();
}

#[tokio::test]
async fn browser_proxy_closes_idle_connect_tunnels() {
    let connector = Arc::new(MemoryStreamConnector::default());
    let proxy = BrowserProxy::bind(
        BrowserProxyConfig {
            idle_timeout: Duration::from_millis(30),
            ..BrowserProxyConfig::default()
        },
        connector.clone(),
    )
    .await
    .unwrap();
    let mut browser = TcpStream::connect(("127.0.0.1", proxy.local_port()))
        .await
        .unwrap();

    browser
        .write_all(
            b"CONNECT svc_web_3000.pc_001.qtunnel.local:443 HTTP/1.1\r\n\
              Host: svc_web_3000.pc_001.qtunnel.local:443\r\n\
              \r\n",
        )
        .await
        .unwrap();
    let mut established = vec![0_u8; 128];
    let read = browser.read(&mut established).await.unwrap();
    assert!(String::from_utf8_lossy(&established[..read])
        .starts_with("HTTP/1.1 200 Connection Established"));
    let _remote = connector.accept().await.unwrap();

    let mut closed = [0_u8; 1];
    let read = timeout(Duration::from_secs(1), browser.read(&mut closed))
        .await
        .expect("timed out waiting for idle tunnel close")
        .unwrap_or(0);
    assert_eq!(read, 0);
    let stats = wait_for_proxy_stats(&proxy, |stats| stats.idle_timeout_closures == 1).await;
    assert_eq!(stats.accepted_connections, 1);
    assert_eq!(stats.tunnel_connections, 1);
    assert_eq!(stats.idle_timeout_closures, 1);

    proxy.shutdown().await.unwrap();
}

#[tokio::test]
async fn browser_proxy_rewrites_absolute_form_http_requests_to_origin_form() {
    let connector = Arc::new(MemoryStreamConnector::default());
    let proxy = BrowserProxy::bind(BrowserProxyConfig::default(), connector.clone())
        .await
        .unwrap();
    let mut browser = TcpStream::connect(("127.0.0.1", proxy.local_port()))
        .await
        .unwrap();

    browser
        .write_all(
            b"GET http://svc_web_3000.pc_001.qtunnel.local/path?q=1 HTTP/1.1\r\n\
              Host: svc_web_3000.pc_001.qtunnel.local\r\n\
              User-Agent: test\r\n\
              \r\n",
        )
        .await
        .unwrap();

    let mut remote = connector.accept().await.unwrap();
    let mut forwarded = vec![0_u8; 128];
    let read = remote.read(&mut forwarded).await.unwrap();
    let forwarded = String::from_utf8_lossy(&forwarded[..read]);
    assert!(forwarded.starts_with("GET /path?q=1 HTTP/1.1\r\n"));
    assert!(forwarded.contains("Host: svc_web_3000.pc_001.qtunnel.local\r\n"));

    remote
        .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
        .await
        .unwrap();

    let mut response = vec![0_u8; 64];
    let read = browser.read(&mut response).await.unwrap();
    assert!(String::from_utf8_lossy(&response[..read]).contains("200 OK"));

    proxy.shutdown().await.unwrap();
}

struct HangingStreamConnector;

#[async_trait]
impl StreamConnector for HangingStreamConnector {
    async fn open_stream(
        &self,
        _request: &OpenForwardRequest,
    ) -> Result<BoxedStream, ForwardError> {
        future::pending().await
    }
}

#[tokio::test]
async fn browser_proxy_accepts_dns_safe_encoded_hosts() {
    let connector = Arc::new(MemoryStreamConnector::default());
    let proxy = BrowserProxy::bind(BrowserProxyConfig::default(), connector.clone())
        .await
        .unwrap();
    let host = browser_proxy_host(
        &BrowserProxyTarget {
            device_id: "pc_001".into(),
            service_id: "svc_web_3000".into(),
        },
        ".qtunnel.local",
    )
    .unwrap();
    let mut browser = TcpStream::connect(("127.0.0.1", proxy.local_port()))
        .await
        .unwrap();

    browser
        .write_all(format!("GET http://{host}/status HTTP/1.1\r\nHost: {host}\r\n\r\n").as_bytes())
        .await
        .unwrap();

    let mut remote = connector.accept().await.unwrap();
    let mut forwarded = vec![0_u8; 128];
    let read = remote.read(&mut forwarded).await.unwrap();
    let forwarded = String::from_utf8_lossy(&forwarded[..read]);
    assert!(forwarded.starts_with("GET /status HTTP/1.1\r\n"));
    assert!(forwarded.contains(&format!("Host: {host}\r\n")));

    proxy.shutdown().await.unwrap();
}

#[tokio::test]
async fn browser_proxy_directly_forwards_non_qtunnel_http_without_opening_tunnel_stream() {
    let connector = Arc::new(MemoryStreamConnector::default());
    let direct_listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let direct_port = direct_listener.local_addr().unwrap().port();
    let direct_server = tokio::spawn(async move {
        let (mut stream, _) = direct_listener.accept().await.unwrap();
        let mut request = vec![0_u8; 128];
        let read = stream.read(&mut request).await.unwrap();
        let request = String::from_utf8_lossy(&request[..read]);
        assert!(request.starts_with("GET /direct HTTP/1.1\r\n"));
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 6\r\n\r\ndirect")
            .await
            .unwrap();
    });
    let proxy = BrowserProxy::bind(BrowserProxyConfig::default(), connector.clone())
        .await
        .unwrap();
    let mut browser = TcpStream::connect(("127.0.0.1", proxy.local_port()))
        .await
        .unwrap();

    browser
        .write_all(
            format!(
                "GET http://127.0.0.1:{direct_port}/direct HTTP/1.1\r\nHost: 127.0.0.1:{direct_port}\r\n\r\n"
            )
            .as_bytes(),
        )
        .await
        .unwrap();

    let mut response = vec![0_u8; 128];
    let read = browser.read(&mut response).await.unwrap();
    let response = String::from_utf8_lossy(&response[..read]);
    assert!(response.starts_with("HTTP/1.1 200 OK"));
    assert!(response.contains("direct"));
    assert!(timeout(Duration::from_millis(50), connector.accept())
        .await
        .is_err());

    direct_server.await.unwrap();
    let stats = wait_for_proxy_stats(&proxy, |stats| stats.active_connections == 0).await;
    assert_eq!(stats.accepted_connections, 1);
    assert_eq!(stats.direct_connections, 1);
    assert_eq!(stats.tunnel_connections, 0);
    assert!(stats.direct_bytes_to_remote > 0);
    assert!(stats.direct_bytes_to_browser > 0);
    proxy.shutdown().await.unwrap();
}

#[tokio::test]
async fn browser_proxy_directly_tunnels_non_qtunnel_connect_without_opening_tunnel_stream() {
    let connector = Arc::new(MemoryStreamConnector::default());
    let direct_listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let direct_port = direct_listener.local_addr().unwrap().port();
    let direct_server = tokio::spawn(async move {
        let (mut stream, _) = direct_listener.accept().await.unwrap();
        let mut tunneled = [0_u8; 4];
        stream.read_exact(&mut tunneled).await.unwrap();
        assert_eq!(&tunneled, b"ping");
        stream.write_all(b"pong").await.unwrap();
    });
    let proxy = BrowserProxy::bind(BrowserProxyConfig::default(), connector.clone())
        .await
        .unwrap();
    let mut browser = TcpStream::connect(("127.0.0.1", proxy.local_port()))
        .await
        .unwrap();

    browser
        .write_all(
            format!(
                "CONNECT 127.0.0.1:{direct_port} HTTP/1.1\r\nHost: 127.0.0.1:{direct_port}\r\n\r\n"
            )
            .as_bytes(),
        )
        .await
        .unwrap();

    let mut established = vec![0_u8; 128];
    let read = browser.read(&mut established).await.unwrap();
    assert!(String::from_utf8_lossy(&established[..read])
        .starts_with("HTTP/1.1 200 Connection Established"));

    browser.write_all(b"ping").await.unwrap();
    let mut reply = [0_u8; 4];
    browser.read_exact(&mut reply).await.unwrap();
    assert_eq!(&reply, b"pong");
    browser.shutdown().await.unwrap();
    assert!(timeout(Duration::from_millis(50), connector.accept())
        .await
        .is_err());

    direct_server.await.unwrap();
    let stats = wait_for_proxy_stats(&proxy, |stats| {
        stats.direct_bytes_to_remote == 4 && stats.direct_bytes_to_browser == 4
    })
    .await;
    assert_eq!(stats.accepted_connections, 1);
    assert_eq!(stats.direct_connections, 1);
    assert_eq!(stats.tunnel_connections, 0);
    assert_eq!(stats.direct_bytes_to_remote, 4);
    assert_eq!(stats.direct_bytes_to_browser, 4);
    proxy.shutdown().await.unwrap();
}

#[tokio::test]
async fn browser_proxy_rejects_direct_http_when_direct_fallback_is_disabled() {
    let connector = Arc::new(MemoryStreamConnector::default());
    let direct_listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let direct_port = direct_listener.local_addr().unwrap().port();
    let proxy = BrowserProxy::bind(
        BrowserProxyConfig {
            direct_fallback_policy: BrowserProxyDirectFallbackPolicy::Disabled,
            ..BrowserProxyConfig::default()
        },
        connector.clone(),
    )
    .await
    .unwrap();
    let mut browser = TcpStream::connect(("127.0.0.1", proxy.local_port()))
        .await
        .unwrap();

    browser
        .write_all(
            format!(
                "GET http://127.0.0.1:{direct_port}/direct HTTP/1.1\r\nHost: 127.0.0.1:{direct_port}\r\n\r\n"
            )
            .as_bytes(),
        )
        .await
        .unwrap();

    let mut response = vec![0_u8; 128];
    let read = browser.read(&mut response).await.unwrap();
    assert!(String::from_utf8_lossy(&response[..read]).starts_with("HTTP/1.1 403 Forbidden"));
    assert!(timeout(Duration::from_millis(50), direct_listener.accept())
        .await
        .is_err());
    assert!(timeout(Duration::from_millis(50), connector.accept())
        .await
        .is_err());
    let stats = wait_for_proxy_stats(&proxy, |stats| stats.forbidden_direct_connections == 1).await;
    assert_eq!(stats.accepted_connections, 1);
    assert_eq!(stats.active_connections, 0);
    assert_eq!(stats.forbidden_direct_connections, 1);
    assert_eq!(stats.direct_connections, 0);
    assert_eq!(stats.tunnel_connections, 0);

    proxy.shutdown().await.unwrap();
}

#[tokio::test]
async fn browser_proxy_rejects_public_ip_direct_targets_with_local_network_policy() {
    let connector = Arc::new(MemoryStreamConnector::default());
    let proxy = BrowserProxy::bind(
        BrowserProxyConfig {
            direct_fallback_policy: BrowserProxyDirectFallbackPolicy::LocalNetworkAndDomain,
            ..BrowserProxyConfig::default()
        },
        connector.clone(),
    )
    .await
    .unwrap();
    let mut browser = TcpStream::connect(("127.0.0.1", proxy.local_port()))
        .await
        .unwrap();

    browser
        .write_all(
            b"CONNECT 93.184.216.34:443 HTTP/1.1\r\n\
              Host: 93.184.216.34:443\r\n\
              \r\n",
        )
        .await
        .unwrap();

    let mut response = vec![0_u8; 128];
    let read = browser.read(&mut response).await.unwrap();
    assert!(String::from_utf8_lossy(&response[..read]).starts_with("HTTP/1.1 403 Forbidden"));
    assert!(timeout(Duration::from_millis(50), connector.accept())
        .await
        .is_err());

    proxy.shutdown().await.unwrap();
}

#[tokio::test]
async fn browser_proxy_allows_domain_direct_targets_with_local_network_policy() {
    let connector = Arc::new(MemoryStreamConnector::default());
    let direct_listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let direct_port = direct_listener.local_addr().unwrap().port();
    let direct_server = tokio::spawn(async move {
        let (mut stream, _) = direct_listener.accept().await.unwrap();
        let mut request = vec![0_u8; 128];
        let read = stream.read(&mut request).await.unwrap();
        let request = String::from_utf8_lossy(&request[..read]);
        assert!(request.starts_with("GET /domain HTTP/1.1\r\n"));
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 6\r\n\r\ndomain")
            .await
            .unwrap();
    });
    let proxy = BrowserProxy::bind(
        BrowserProxyConfig {
            direct_fallback_policy: BrowserProxyDirectFallbackPolicy::LocalNetworkAndDomain,
            ..BrowserProxyConfig::default()
        },
        connector.clone(),
    )
    .await
    .unwrap();
    let mut browser = TcpStream::connect(("127.0.0.1", proxy.local_port()))
        .await
        .unwrap();

    browser
        .write_all(
            format!(
                "GET http://localhost:{direct_port}/domain HTTP/1.1\r\nHost: localhost:{direct_port}\r\n\r\n"
            )
            .as_bytes(),
        )
        .await
        .unwrap();

    let mut response = vec![0_u8; 128];
    let read = browser.read(&mut response).await.unwrap();
    let response = String::from_utf8_lossy(&response[..read]);
    assert!(response.starts_with("HTTP/1.1 200 OK"));
    assert!(response.contains("domain"));
    assert!(timeout(Duration::from_millis(50), connector.accept())
        .await
        .is_err());
    let stats = wait_for_proxy_stats(&proxy, |stats| {
        stats.active_connections == 0
            && stats.direct_connections == 1
            && stats.direct_bytes_to_remote > 0
            && stats.direct_bytes_to_browser > 0
    })
    .await;
    assert_eq!(stats.accepted_connections, 1);
    assert_eq!(stats.active_connections, 0);
    assert_eq!(stats.direct_connections, 1);
    assert_eq!(stats.forbidden_direct_connections, 0);
    assert_eq!(stats.tunnel_connections, 0);
    assert!(stats.direct_bytes_to_remote > 0);
    assert!(stats.direct_bytes_to_browser > 0);
    assert_eq!(stats.tunnel_bytes_to_remote, 0);
    assert_eq!(stats.tunnel_bytes_to_browser, 0);

    direct_server.await.unwrap();
    proxy.shutdown().await.unwrap();
}

#[tokio::test]
async fn browser_proxy_counts_tunnel_connections() {
    let connector = Arc::new(MemoryStreamConnector::default());
    let proxy = BrowserProxy::bind(BrowserProxyConfig::default(), connector.clone())
        .await
        .unwrap();
    let mut browser = TcpStream::connect(("127.0.0.1", proxy.local_port()))
        .await
        .unwrap();

    browser
        .write_all(
            b"GET http://svc_api.pc_001.qtunnel.local/status HTTP/1.1\r\n\
              Host: svc_api.pc_001.qtunnel.local\r\n\
              \r\n",
        )
        .await
        .unwrap();

    let mut remote = connector.accept().await.unwrap();
    let forwarded = read_forwarded_request(&mut remote).await;
    assert!(forwarded.starts_with(b"GET /status HTTP/1.1\r\n"));
    remote
        .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
        .await
        .unwrap();
    remote.shutdown().await.unwrap();
    let mut response = vec![0_u8; 64];
    let read = browser.read(&mut response).await.unwrap();
    assert!(String::from_utf8_lossy(&response[..read]).contains("200 OK"));

    let stats = wait_for_proxy_stats(&proxy, |stats| {
        stats.active_connections == 0
            && stats.tunnel_connections == 1
            && stats.tunnel_bytes_to_remote > 0
            && stats.tunnel_bytes_to_browser > 0
    })
    .await;
    assert_eq!(stats.accepted_connections, 1);
    assert_eq!(stats.active_connections, 0);
    assert_eq!(stats.tunnel_connections, 1);
    assert_eq!(stats.direct_connections, 0);
    assert_eq!(stats.forbidden_direct_connections, 0);
    assert!(stats.tunnel_bytes_to_remote > 0);
    assert!(stats.tunnel_bytes_to_browser > 0);
    assert_eq!(stats.direct_bytes_to_remote, 0);
    assert_eq!(stats.direct_bytes_to_browser, 0);

    proxy.shutdown().await.unwrap();
}

#[tokio::test]
async fn browser_proxy_uses_configured_domain_suffix() {
    let connector = Arc::new(MemoryStreamConnector::default());
    let proxy = BrowserProxy::bind(
        BrowserProxyConfig {
            domain_suffix: ".qtunnel.test".to_string(),
            ..BrowserProxyConfig::default()
        },
        connector.clone(),
    )
    .await
    .unwrap();
    let mut browser = TcpStream::connect(("127.0.0.1", proxy.local_port()))
        .await
        .unwrap();

    browser
        .write_all(
            b"GET http://svc_web_3000.pc_001.qtunnel.test/ HTTP/1.1\r\n\
              Host: svc_web_3000.pc_001.qtunnel.test\r\n\
              \r\n",
        )
        .await
        .unwrap();

    let mut remote = connector.accept().await.unwrap();
    let mut forwarded = vec![0_u8; 96];
    let read = remote.read(&mut forwarded).await.unwrap();
    let forwarded = String::from_utf8_lossy(&forwarded[..read]);
    assert!(forwarded.starts_with("GET / HTTP/1.1\r\n"));

    proxy.shutdown().await.unwrap();
}

#[tokio::test]
async fn browser_proxy_rejects_empty_domain_suffix() {
    let connector = Arc::new(MemoryStreamConnector::default());
    let result = BrowserProxy::bind(
        BrowserProxyConfig {
            domain_suffix: String::new(),
            ..BrowserProxyConfig::default()
        },
        connector,
    )
    .await;
    let Err(error) = result else {
        panic!("empty domain suffix should fail");
    };

    assert!(error.to_string().contains("domain_suffix"));
}

#[tokio::test]
async fn browser_proxy_preserves_request_body_already_read_with_headers() {
    let connector = Arc::new(MemoryStreamConnector::default());
    let proxy = BrowserProxy::bind(BrowserProxyConfig::default(), connector.clone())
        .await
        .unwrap();
    let mut browser = TcpStream::connect(("127.0.0.1", proxy.local_port()))
        .await
        .unwrap();

    browser
        .write_all(
            b"POST http://svc_api.pc_001.qtunnel.local/upload HTTP/1.1\r\n\
              Host: svc_api.pc_001.qtunnel.local\r\n\
              Content-Length: 5\r\n\
              \r\n\
              abc\xffz",
        )
        .await
        .unwrap();

    let mut remote = connector.accept().await.unwrap();
    let mut forwarded = vec![0_u8; 160];
    let read = remote.read(&mut forwarded).await.unwrap();
    let forwarded = &forwarded[..read];
    assert!(
        forwarded.starts_with(b"POST /upload HTTP/1.1\r\nHost: svc_api.pc_001.qtunnel.local\r\n")
    );
    assert!(forwarded.ends_with(b"abc\xffz"));

    proxy.shutdown().await.unwrap();
}

#[tokio::test]
async fn browser_proxy_forces_connection_close_on_plain_http_requests() {
    let connector = Arc::new(MemoryStreamConnector::default());
    let proxy = BrowserProxy::bind(BrowserProxyConfig::default(), connector.clone())
        .await
        .unwrap();
    let mut browser = TcpStream::connect(("127.0.0.1", proxy.local_port()))
        .await
        .unwrap();

    browser
        .write_all(
            b"GET http://svc_web_3000.pc_001.qtunnel.local/path HTTP/1.1\r\n\
              Host: svc_web_3000.pc_001.qtunnel.local\r\n\
              Connection: keep-alive\r\n\
              Proxy-Connection: keep-alive\r\n\
              \r\n",
        )
        .await
        .unwrap();

    let mut remote = connector.accept().await.unwrap();
    let forwarded = read_forwarded_request(&mut remote).await;
    let forwarded = String::from_utf8_lossy(&forwarded);
    assert!(forwarded.starts_with("GET /path HTTP/1.1\r\n"));
    assert!(forwarded
        .split("\r\n")
        .any(|line| line.eq_ignore_ascii_case("Connection: close")));
    assert!(!forwarded
        .split("\r\n")
        .any(|line| line.eq_ignore_ascii_case("Connection: keep-alive")));
    assert!(!forwarded
        .split("\r\n")
        .any(|line| line.to_ascii_lowercase().starts_with("proxy-connection:")));

    proxy.shutdown().await.unwrap();
}

#[tokio::test]
async fn browser_proxy_does_not_forward_pipelined_http_request_to_first_remote_stream() {
    let connector = Arc::new(MemoryStreamConnector::default());
    let proxy = BrowserProxy::bind(BrowserProxyConfig::default(), connector.clone())
        .await
        .unwrap();
    let mut browser = TcpStream::connect(("127.0.0.1", proxy.local_port()))
        .await
        .unwrap();

    browser
        .write_all(
            b"GET http://svc_web_3000.pc_001.qtunnel.local/one HTTP/1.1\r\n\
              Host: svc_web_3000.pc_001.qtunnel.local\r\n\
              Connection: keep-alive\r\n\
              \r\n\
              GET http://svc_api.pc_001.qtunnel.local/two HTTP/1.1\r\n\
              Host: svc_api.pc_001.qtunnel.local\r\n\
              \r\n",
        )
        .await
        .unwrap();

    let mut remote = connector.accept().await.unwrap();
    let forwarded = read_forwarded_request(&mut remote).await;
    let forwarded = String::from_utf8_lossy(&forwarded);
    assert!(forwarded.starts_with("GET /one HTTP/1.1\r\n"));
    assert!(!forwarded.contains("/two"));
    assert!(!forwarded.contains("svc_api.pc_001.qtunnel.local"));

    proxy.shutdown().await.unwrap();
}

#[tokio::test]
async fn browser_proxy_forwards_only_declared_content_length_body() {
    let connector = Arc::new(MemoryStreamConnector::default());
    let proxy = BrowserProxy::bind(BrowserProxyConfig::default(), connector.clone())
        .await
        .unwrap();
    let mut browser = TcpStream::connect(("127.0.0.1", proxy.local_port()))
        .await
        .unwrap();

    browser
        .write_all(
            b"POST http://svc_api.pc_001.qtunnel.local/upload HTTP/1.1\r\n\
              Host: svc_api.pc_001.qtunnel.local\r\n\
              Content-Length: 5\r\n\
              \r\n\
              abcdeGET http://svc_api.pc_001.qtunnel.local/next HTTP/1.1\r\n\
              Host: svc_api.pc_001.qtunnel.local\r\n\
              \r\n",
        )
        .await
        .unwrap();

    let mut remote = connector.accept().await.unwrap();
    let forwarded = read_forwarded_request(&mut remote).await;
    let body_start = forwarded
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|position| position + 4)
        .expect("forwarded request has head terminator");
    assert!(forwarded.starts_with(b"POST /upload HTTP/1.1\r\n"));
    assert_eq!(&forwarded[body_start..], b"abcde");

    proxy.shutdown().await.unwrap();
}

#[tokio::test]
async fn browser_proxy_forwards_declared_request_body_written_after_headers() {
    let connector = Arc::new(MemoryStreamConnector::default());
    let proxy = BrowserProxy::bind(BrowserProxyConfig::default(), connector.clone())
        .await
        .unwrap();
    let mut browser = TcpStream::connect(("127.0.0.1", proxy.local_port()))
        .await
        .unwrap();

    browser
        .write_all(
            b"POST http://svc_api.pc_001.qtunnel.local/upload HTTP/1.1\r\n\
              Host: svc_api.pc_001.qtunnel.local\r\n\
              Content-Length: 7\r\n\
              \r\n\
              abc",
        )
        .await
        .unwrap();

    let mut remote = connector.accept().await.unwrap();
    browser.write_all(b"defg").await.unwrap();

    let forwarded = read_forwarded_request(&mut remote).await;
    let body_start = forwarded
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|position| position + 4)
        .expect("forwarded request has head terminator");
    assert!(forwarded.starts_with(b"POST /upload HTTP/1.1\r\n"));
    assert_eq!(&forwarded[body_start..], b"abcdefg");

    proxy.shutdown().await.unwrap();
}

#[tokio::test]
async fn browser_proxy_forwards_chunked_request_body_without_pipelined_bytes() {
    let connector = Arc::new(MemoryStreamConnector::default());
    let proxy = BrowserProxy::bind(BrowserProxyConfig::default(), connector.clone())
        .await
        .unwrap();
    let mut browser = TcpStream::connect(("127.0.0.1", proxy.local_port()))
        .await
        .unwrap();

    browser
        .write_all(
            b"POST http://svc_api.pc_001.qtunnel.local/upload HTTP/1.1\r\n\
              Host: svc_api.pc_001.qtunnel.local\r\n\
              Transfer-Encoding: chunked\r\n\
              \r\n\
              4\r\nWiki\r\n5\r\npedia\r\n0\r\n\r\n\
              GET http://svc_api.pc_001.qtunnel.local/next HTTP/1.1\r\n\
              Host: svc_api.pc_001.qtunnel.local\r\n\
              \r\n",
        )
        .await
        .unwrap();

    let mut remote = connector.accept().await.unwrap();
    let forwarded = read_forwarded_request(&mut remote).await;
    assert!(forwarded.starts_with(
        b"POST /upload HTTP/1.1\r\nHost: svc_api.pc_001.qtunnel.local\r\nTransfer-Encoding: chunked\r\n"
    ));
    assert!(forwarded.ends_with(b"4\r\nWiki\r\n5\r\npedia\r\n0\r\n\r\n"));
    assert!(!String::from_utf8_lossy(&forwarded).contains("/next"));

    proxy.shutdown().await.unwrap();
}

#[tokio::test]
async fn browser_proxy_forwards_chunked_body_written_after_headers() {
    let connector = Arc::new(MemoryStreamConnector::default());
    let proxy = BrowserProxy::bind(BrowserProxyConfig::default(), connector.clone())
        .await
        .unwrap();
    let mut browser = TcpStream::connect(("127.0.0.1", proxy.local_port()))
        .await
        .unwrap();

    browser
        .write_all(
            b"POST http://svc_api.pc_001.qtunnel.local/upload HTTP/1.1\r\n\
              Host: svc_api.pc_001.qtunnel.local\r\n\
              Transfer-Encoding: chunked\r\n\
              \r\n\
              3\r\nabc\r\n",
        )
        .await
        .unwrap();

    let mut remote = connector.accept().await.unwrap();
    browser.write_all(b"2\r\nde\r\n0\r\n\r\n").await.unwrap();

    let forwarded = read_forwarded_request(&mut remote).await;
    assert!(forwarded.starts_with(
        b"POST /upload HTTP/1.1\r\nHost: svc_api.pc_001.qtunnel.local\r\nTransfer-Encoding: chunked\r\n"
    ));
    assert!(forwarded.ends_with(b"3\r\nabc\r\n2\r\nde\r\n0\r\n\r\n"));

    proxy.shutdown().await.unwrap();
}

#[tokio::test]
async fn browser_proxy_forwards_chunked_body_with_split_chunk_lines() {
    let connector = Arc::new(MemoryStreamConnector::default());
    let proxy = BrowserProxy::bind(BrowserProxyConfig::default(), connector.clone())
        .await
        .unwrap();
    let mut browser = TcpStream::connect(("127.0.0.1", proxy.local_port()))
        .await
        .unwrap();

    browser
        .write_all(
            b"POST http://svc_api.pc_001.qtunnel.local/upload HTTP/1.1\r\n\
              Host: svc_api.pc_001.qtunnel.local\r\n\
              Transfer-Encoding: chunked\r\n\
              \r\n\
              A",
        )
        .await
        .unwrap();

    let mut remote = connector.accept().await.unwrap();
    browser
        .write_all(b"\r\n0123456789\r\n0\r\nTrailer: yes\r\n\r\n")
        .await
        .unwrap();

    let forwarded = read_forwarded_request(&mut remote).await;
    assert!(forwarded.starts_with(
        b"POST /upload HTTP/1.1\r\nHost: svc_api.pc_001.qtunnel.local\r\nTransfer-Encoding: chunked\r\n"
    ));
    assert!(forwarded.ends_with(b"A\r\n0123456789\r\n0\r\nTrailer: yes\r\n\r\n"));

    proxy.shutdown().await.unwrap();
}

#[tokio::test]
async fn browser_proxy_rejects_chunked_request_with_content_length() {
    let connector = Arc::new(MemoryStreamConnector::default());
    let proxy = BrowserProxy::bind(BrowserProxyConfig::default(), connector.clone())
        .await
        .unwrap();
    let mut browser = TcpStream::connect(("127.0.0.1", proxy.local_port()))
        .await
        .unwrap();

    browser
        .write_all(
            b"POST http://svc_api.pc_001.qtunnel.local/upload HTTP/1.1\r\n\
              Host: svc_api.pc_001.qtunnel.local\r\n\
              Transfer-Encoding: chunked\r\n\
              Content-Length: 4\r\n\
              \r\n\
              0\r\n\r\n",
        )
        .await
        .unwrap();

    assert!(timeout(Duration::from_millis(50), connector.accept())
        .await
        .is_err());

    proxy.shutdown().await.unwrap();
}

#[tokio::test]
async fn browser_proxy_connect_establishes_a_raw_tunnel() {
    let connector = Arc::new(MemoryStreamConnector::default());
    let proxy = BrowserProxy::bind(BrowserProxyConfig::default(), connector.clone())
        .await
        .unwrap();
    let mut browser = TcpStream::connect(("127.0.0.1", proxy.local_port()))
        .await
        .unwrap();

    browser
        .write_all(
            b"CONNECT svc_web_3000.pc_001.qtunnel.local:443 HTTP/1.1\r\n\
              Host: svc_web_3000.pc_001.qtunnel.local:443\r\n\
              \r\n",
        )
        .await
        .unwrap();

    let mut established = vec![0_u8; 128];
    let read = browser.read(&mut established).await.unwrap();
    assert!(String::from_utf8_lossy(&established[..read])
        .starts_with("HTTP/1.1 200 Connection Established"));

    let mut remote = connector.accept().await.unwrap();
    browser.write_all(b"tls-bytes").await.unwrap();
    let mut tunneled = [0_u8; 9];
    remote.read_exact(&mut tunneled).await.unwrap();
    assert_eq!(&tunneled, b"tls-bytes");

    remote.write_all(b"reply").await.unwrap();
    let mut reply = [0_u8; 5];
    browser.read_exact(&mut reply).await.unwrap();
    assert_eq!(&reply, b"reply");

    proxy.shutdown().await.unwrap();
}

#[tokio::test]
async fn browser_proxy_shutdown_releases_the_proxy_port() {
    let connector = Arc::new(MemoryStreamConnector::default());
    let proxy = BrowserProxy::bind(BrowserProxyConfig::default(), connector)
        .await
        .unwrap();
    let port = proxy.local_port();

    proxy.shutdown().await.unwrap();
    TcpListener::bind(("127.0.0.1", port))
        .await
        .expect("proxy port released");
}

async fn read_forwarded_request<S>(stream: &mut S) -> Vec<u8>
where
    S: AsyncRead + Unpin,
{
    sleep(Duration::from_millis(20)).await;

    let mut forwarded = Vec::new();
    let mut chunk = [0_u8; 256];
    let read = timeout(Duration::from_secs(1), stream.read(&mut chunk))
        .await
        .expect("timed out waiting for forwarded request")
        .expect("read forwarded request");
    assert!(read > 0, "forwarded request stream closed before data");
    forwarded.extend_from_slice(&chunk[..read]);

    loop {
        match timeout(Duration::from_millis(30), stream.read(&mut chunk)).await {
            Ok(Ok(0)) | Err(_) => break,
            Ok(Ok(read)) => forwarded.extend_from_slice(&chunk[..read]),
            Ok(Err(error)) => panic!("read forwarded request: {error}"),
        }
    }

    forwarded
}

async fn wait_for_proxy_stats(
    proxy: &BrowserProxy,
    matches: impl Fn(&BrowserProxyStats) -> bool,
) -> BrowserProxyStats {
    let mut last = proxy.stats();
    for _ in 0..100 {
        if matches(&last) {
            return last;
        }
        sleep(Duration::from_millis(10)).await;
        last = proxy.stats();
    }

    panic!("timed out waiting for proxy stats, last snapshot: {last:?}");
}
