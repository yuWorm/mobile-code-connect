use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::Arc,
    time::Duration,
};

use mobilecode_connect_agent::{
    config::{AgentConfig, ServiceConfig},
    mobile_grant::{CreateMobileInviteRequest, MobileGrantManager},
    runtime::{Agent, AgentControlRuntime, AgentControlRuntimeConfig, AgentP2pRuntimeConfig},
    service_registry::ServiceRegistry,
};
use mobilecode_connect_control::{routes::routes, state::ControlState};
use mobilecode_connect_control_client::{
    CreateSessionRequest, HttpControlClient, HttpControlClientOptions,
};
use mobilecode_connect_mobile_core::{
    browser_proxy::{browser_proxy_host, BrowserProxyTarget},
    client::{OpenServiceRequest, TunnelClient},
    config::TunnelConfig,
    forward::{
        ControlP2pConnectorConfig, ControlP2pOrRelayConnectorConfig,
        ControlP2pOrRelayStreamConnector, ControlP2pStreamConnector, OpenForwardRequest,
        P2pStreamConnector, StreamConnector,
    },
};
use mobilecode_connect_protocol::{
    mobile_grant_certificate_fingerprint, ClientId, DeviceId, MobilePairingRequest, PeerRole,
    ServiceId, ServiceProtocol, SessionId,
};
use mobilecode_connect_punch::{
    probe::{establish_p2p_path, P2pPathConfig},
    server::PunchServer,
};
use mobilecode_connect_tunnel::quic::generate_self_signed_server_identity;
use rustls::pki_types::CertificateDer;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::oneshot,
};

#[tokio::test]
async fn agent_control_runtime_claims_session_and_serves_p2p_quic() {
    let punch = PunchServer::bind(local_addr(0)).await.unwrap();
    let punch_addr = punch.local_addr().unwrap();
    let (punch_shutdown_tx, punch_shutdown_rx) = oneshot::channel();
    let punch_task = tokio::spawn(punch.run_until(async {
        let _ = punch_shutdown_rx.await;
    }));

    let echo = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let echo_port = echo.local_addr().unwrap().port();
    let echo_task = tokio::spawn(async move {
        let (mut stream, _) = echo.accept().await.unwrap();
        let mut payload = [0_u8; 5];
        stream.read_exact(&mut payload).await.unwrap();
        stream.write_all(b"world").await.unwrap();
    });

    let control = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let control_addr = control.local_addr().unwrap();
    let control_url = format!("http://{control_addr}");
    let state = ControlState::new("dev-secret", "127.0.0.1:4443", punch_addr.to_string());
    let control_task = tokio::spawn(async move {
        axum::serve(control, routes(state)).await.unwrap();
    });

    let service = service_config(echo_port);
    let p2p_identity = generate_self_signed_server_identity().unwrap();
    let p2p_cert = p2p_identity.certificate_der().clone();
    Agent::register_with_control(AgentConfig {
        device_id: DeviceId::new("pc_001"),
        control_server: control_url.clone(),
        auth_token: "agent-token".to_string(),
        services: vec![service.clone()],
        p2p_certificate_der: Some(p2p_cert.as_ref().to_vec()),
    })
    .await
    .unwrap();

    let control_client = HttpControlClient::new(control_url.clone()).unwrap();
    let session = control_client
        .create_session(CreateSessionRequest {
            client_id: "mobile_001".to_string(),
            device_id: DeviceId::new("pc_001"),
            service_id: ServiceId::new("svc_web_3000"),
        })
        .await
        .unwrap();

    let registry = ServiceRegistry::new(vec![service]).unwrap();
    let mut agent = AgentControlRuntime::new(AgentControlRuntimeConfig {
        control_server_url: control_url.clone(),
        auth_token: "agent-token".to_string(),
        device_id: DeviceId::new("pc_001"),
        relay_server_cert: CertificateDer::from(vec![1, 2, 3]),
        registry,
        poll_interval: Duration::from_millis(20),
        p2p: Some(AgentP2pRuntimeConfig {
            bind_addr: local_addr(0),
            candidate_timeout: Duration::from_secs(1),
            probe_timeout: Duration::from_secs(1),
            interval: Duration::from_millis(10),
            server_identity: Some(p2p_identity),
        }),
        mobile_grants: None,
    })
    .unwrap();

    let agent_task = tokio::spawn(async move {
        let started = agent.poll_once().await.unwrap();
        (agent, started)
    });
    let mobile_path = establish_p2p_path(p2p_config(
        session.session_id.clone(),
        PeerRole::Mobile,
        "mobile_001",
        "pc_001",
        punch_addr,
        session.relay_token.clone(),
    ));

    let (agent_result, mobile_path) = tokio::join!(agent_task, mobile_path);
    let (mut agent, started) = agent_result.unwrap();
    let mobile_path = mobile_path.unwrap();
    assert_eq!(started, vec![session.session_id.clone()]);

    let connector = P2pStreamConnector::connect_path_with_server_cert(
        session.session_id,
        mobile_path,
        p2p_cert,
    )
    .await
    .unwrap();
    let client = TunnelClient::with_connector(
        TunnelConfig {
            user_token: "user-token".to_string(),
            control_server_url: control_url,
            client_id: ClientId::new("mobile_001"),
            control_client_options: HttpControlClientOptions::default(),
        },
        Arc::new(connector),
    )
    .await
    .unwrap();

    let port_probe = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let local_port = port_probe.local_addr().unwrap().port();
    drop(port_probe);
    let handle = client
        .open_service(OpenServiceRequest {
            device_id: DeviceId::new("pc_001"),
            service_id: ServiceId::new("svc_web_3000"),
            local_port,
        })
        .await
        .unwrap();

    let mut local = TcpStream::connect(("127.0.0.1", handle.local_port()))
        .await
        .unwrap();
    local.write_all(b"hello").await.unwrap();
    let mut response = [0_u8; 5];
    local.read_exact(&mut response).await.unwrap();
    assert_eq!(&response, b"world");

    client
        .close_service(handle.handle_id().to_string())
        .await
        .unwrap();
    agent.shutdown().await;
    let _ = punch_shutdown_tx.send(());
    punch_task.await.unwrap().unwrap();
    echo_task.await.unwrap();
    control_task.abort();
}

#[tokio::test]
async fn mobile_control_connector_creates_session_and_opens_p2p_stream() {
    let punch = PunchServer::bind(local_addr(0)).await.unwrap();
    let punch_addr = punch.local_addr().unwrap();
    let (punch_shutdown_tx, punch_shutdown_rx) = oneshot::channel();
    let punch_task = tokio::spawn(punch.run_until(async {
        let _ = punch_shutdown_rx.await;
    }));

    let echo = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let echo_port = echo.local_addr().unwrap().port();
    let echo_task = tokio::spawn(async move {
        let (mut stream, _) = echo.accept().await.unwrap();
        let mut payload = [0_u8; 5];
        stream.read_exact(&mut payload).await.unwrap();
        stream.write_all(b"world").await.unwrap();
    });

    let control = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let control_addr = control.local_addr().unwrap();
    let control_url = format!("http://{control_addr}");
    let state = ControlState::new("dev-secret", "127.0.0.1:4443", punch_addr.to_string());
    let control_task = tokio::spawn(async move {
        axum::serve(control, routes(state)).await.unwrap();
    });

    let service = service_config(echo_port);
    let p2p_identity = generate_self_signed_server_identity().unwrap();
    let p2p_cert = p2p_identity.certificate_der().clone();
    Agent::register_with_control(AgentConfig {
        device_id: DeviceId::new("pc_001"),
        control_server: control_url.clone(),
        auth_token: "agent-token".to_string(),
        services: vec![service.clone()],
        p2p_certificate_der: Some(p2p_cert.as_ref().to_vec()),
    })
    .await
    .unwrap();

    let registry = ServiceRegistry::new(vec![service]).unwrap();
    let agent = AgentControlRuntime::new(AgentControlRuntimeConfig {
        control_server_url: control_url.clone(),
        auth_token: "agent-token".to_string(),
        device_id: DeviceId::new("pc_001"),
        relay_server_cert: CertificateDer::from(vec![1, 2, 3]),
        registry,
        poll_interval: Duration::from_millis(10),
        p2p: Some(AgentP2pRuntimeConfig {
            bind_addr: local_addr(0),
            candidate_timeout: Duration::from_secs(1),
            probe_timeout: Duration::from_secs(1),
            interval: Duration::from_millis(10),
            server_identity: Some(p2p_identity),
        }),
        mobile_grants: None,
    })
    .unwrap();
    let (agent_shutdown_tx, agent_shutdown_rx) = oneshot::channel();
    let agent_task = tokio::spawn(agent.run_until(async {
        let _ = agent_shutdown_rx.await;
    }));

    let connector = ControlP2pStreamConnector::new(ControlP2pConnectorConfig {
        control_server_url: control_url.clone(),
        control_token: None,
        client_id: ClientId::new("mobile_001"),
        control_client_options: HttpControlClientOptions::default(),
        bind_addr: local_addr(0),
        candidate_timeout: Duration::from_secs(1),
        probe_timeout: Duration::from_secs(1),
        interval: Duration::from_millis(10),
    })
    .unwrap();
    let client = TunnelClient::with_connector(
        TunnelConfig {
            user_token: "user-token".to_string(),
            control_server_url: control_url,
            client_id: ClientId::new("mobile_001"),
            control_client_options: HttpControlClientOptions::default(),
        },
        Arc::new(connector),
    )
    .await
    .unwrap();

    let port_probe = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let local_port = port_probe.local_addr().unwrap().port();
    drop(port_probe);
    let handle = client
        .open_service(OpenServiceRequest {
            device_id: DeviceId::new("pc_001"),
            service_id: ServiceId::new("svc_web_3000"),
            local_port,
        })
        .await
        .unwrap();

    let mut local = TcpStream::connect(("127.0.0.1", handle.local_port()))
        .await
        .unwrap();
    local.write_all(b"hello").await.unwrap();
    let mut response = [0_u8; 5];
    local.read_exact(&mut response).await.unwrap();
    assert_eq!(&response, b"world");

    client
        .close_service(handle.handle_id().to_string())
        .await
        .unwrap();
    let _ = agent_shutdown_tx.send(());
    let _ = punch_shutdown_tx.send(());
    agent_task.await.unwrap().unwrap();
    punch_task.await.unwrap().unwrap();
    echo_task.await.unwrap();
    control_task.abort();
}

#[tokio::test]
async fn mobile_grant_browser_proxy_reaches_agent_service_over_p2p_or_relay() {
    let punch = PunchServer::bind(local_addr(0)).await.unwrap();
    let punch_addr = punch.local_addr().unwrap();
    let (punch_shutdown_tx, punch_shutdown_rx) = oneshot::channel();
    let punch_task = tokio::spawn(punch.run_until(async {
        let _ = punch_shutdown_rx.await;
    }));

    let http = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let http_port = http.local_addr().unwrap().port();
    let (http_seen_tx, mut http_seen_rx) = oneshot::channel();
    let http_task = tokio::spawn(async move {
        let (mut stream, _) = http.accept().await.unwrap();
        let mut request = Vec::new();
        let mut buf = [0_u8; 1024];
        loop {
            let read = stream.read(&mut buf).await.unwrap();
            if read == 0 {
                break;
            }
            request.extend_from_slice(&buf[..read]);
            if request.windows(4).any(|window| window == b"\r\n\r\n") {
                break;
            }
        }
        let _ = http_seen_tx.send(());
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
            .await
            .unwrap();
        stream.shutdown().await.unwrap();
    });

    let control = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let control_addr = control.local_addr().unwrap();
    let control_url = format!("http://{control_addr}");
    let state = ControlState::new("dev-secret", "127.0.0.1:9", punch_addr.to_string());
    let control_task = tokio::spawn(async move {
        axum::serve(control, routes(state)).await.unwrap();
    });

    let service_id = ServiceId::new("svc_web_3000");
    let service = service_config(http_port);
    let p2p_identity = generate_self_signed_server_identity().unwrap();
    let p2p_cert = p2p_identity.certificate_der().clone();
    let p2p_fingerprint = mobile_grant_certificate_fingerprint(p2p_cert.as_ref());
    // Grant sessions poll at a 1s cadence; keep probing above that boundary.
    let grant_p2p_timeout = Duration::from_secs(2);
    Agent::register_with_control(AgentConfig {
        device_id: DeviceId::new("pc_001"),
        control_server: control_url.clone(),
        auth_token: "agent-token".to_string(),
        services: vec![service.clone()],
        p2p_certificate_der: Some(p2p_cert.as_ref().to_vec()),
    })
    .await
    .unwrap();

    let grants = MobileGrantManager::default();
    let invite = grants
        .create_invite(
            CreateMobileInviteRequest {
                control_url: control_url.clone(),
                device_id: DeviceId::new("pc_001"),
                allowed_services: vec![service_id.clone()],
                ttl_sec: 60,
                max_uses: 1,
                agent_p2p_cert_fingerprint: Some(p2p_fingerprint.clone()),
            },
            1_000,
        )
        .unwrap();
    let client_id = ClientId::new("mobile_001");
    let proof = MobilePairingRequest::proof_for(
        DeviceId::new("pc_001"),
        invite.invite_id.clone(),
        client_id.clone(),
        vec![service_id.clone()],
        "pairing-nonce".to_string(),
        &invite.invite_secret,
    )
    .unwrap();
    let grant = grants
        .approve_pairing(
            &MobilePairingRequest {
                device_id: DeviceId::new("pc_001"),
                invite_id: invite.invite_id,
                client_id: client_id.clone(),
                requested_services: vec![service_id.clone()],
                nonce: "pairing-nonce".to_string(),
                proof,
            },
            1_001,
        )
        .unwrap();

    let registry = ServiceRegistry::new(vec![service]).unwrap();
    let agent = AgentControlRuntime::new(AgentControlRuntimeConfig {
        control_server_url: control_url.clone(),
        auth_token: "agent-token".to_string(),
        device_id: DeviceId::new("pc_001"),
        relay_server_cert: CertificateDer::from(vec![1, 2, 3]),
        registry,
        poll_interval: Duration::from_millis(10),
        p2p: Some(AgentP2pRuntimeConfig {
            bind_addr: local_addr(0),
            candidate_timeout: grant_p2p_timeout,
            probe_timeout: grant_p2p_timeout,
            interval: Duration::from_millis(10),
            server_identity: Some(p2p_identity),
        }),
        mobile_grants: Some(grants),
    })
    .unwrap();
    let (agent_shutdown_tx, agent_shutdown_rx) = oneshot::channel();
    let agent_task = tokio::spawn(agent.run_until(async {
        let _ = agent_shutdown_rx.await;
    }));

    let client = tokio::time::timeout(
        Duration::from_secs(5),
        TunnelClient::start_with_control_p2p_or_relay_mobile_grant(
            TunnelConfig {
                user_token: String::new(),
                control_server_url: control_url,
                client_id,
                control_client_options: HttpControlClientOptions::default(),
            },
            grant,
            mobilecode_connect_mobile_core::client::ControlP2pOrRelayClientConfig {
                relay_server_cert: CertificateDer::from(vec![1, 2, 3]),
                bind_addr: local_addr(0),
                candidate_timeout: grant_p2p_timeout,
                probe_timeout: grant_p2p_timeout,
                interval: Duration::from_millis(10),
                relay_fallback_delay: Duration::from_millis(50),
            },
        ),
    )
    .await
    .expect("mobile grant p2p-or-relay client start timed out")
    .unwrap();
    let proxy = tokio::time::timeout(
        Duration::from_secs(2),
        client.start_browser_proxy(Default::default()),
    )
    .await
    .expect("browser proxy start timed out")
    .unwrap();
    let host = browser_proxy_host(
        &BrowserProxyTarget {
            device_id: DeviceId::new("pc_001"),
            service_id,
        },
        ".qtunnel.local",
    )
    .unwrap();
    let proxy_handle = proxy.handle();
    let mut browser = tokio::time::timeout(
        Duration::from_secs(2),
        TcpStream::connect((proxy_handle.host(), proxy_handle.local_port())),
    )
    .await
    .expect("browser proxy TCP connect timed out")
    .unwrap();
    browser
        .write_all(
            format!("GET http://{host}/ HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n")
                .as_bytes(),
        )
        .await
        .unwrap();
    let mut response = vec![0_u8; 128];
    let read = tokio::time::timeout(Duration::from_secs(10), browser.read(&mut response))
        .await
        .unwrap()
        .unwrap();
    let response = String::from_utf8_lossy(&response[..read]);
    let http_seen = http_seen_rx.try_recv().is_ok();
    let browser_stats = proxy.stats();
    let client_status = client.status();
    assert!(
        response.contains("200 OK"),
        "response was: {response}; http_seen={http_seen}; browser_stats={browser_stats:?}; client_status={client_status:?}"
    );
    assert!(
        response.contains("ok"),
        "response was: {response}; http_seen={http_seen}; browser_stats={browser_stats:?}; client_status={client_status:?}"
    );

    drop(proxy);
    client.shutdown().await.unwrap();
    let _ = agent_shutdown_tx.send(());
    let _ = punch_shutdown_tx.send(());
    agent_task.await.unwrap().unwrap();
    punch_task.await.unwrap().unwrap();
    http_task.await.unwrap();
    control_task.abort();
}

#[tokio::test]
async fn mobile_control_p2p_connector_rejects_wrong_registered_agent_cert() {
    let punch = PunchServer::bind(local_addr(0)).await.unwrap();
    let punch_addr = punch.local_addr().unwrap();
    let (punch_shutdown_tx, punch_shutdown_rx) = oneshot::channel();
    let punch_task = tokio::spawn(punch.run_until(async {
        let _ = punch_shutdown_rx.await;
    }));

    let control = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let control_addr = control.local_addr().unwrap();
    let control_url = format!("http://{control_addr}");
    let state = ControlState::new("dev-secret", "127.0.0.1:4443", punch_addr.to_string());
    let control_task = tokio::spawn(async move {
        axum::serve(control, routes(state)).await.unwrap();
    });

    let agent_identity = generate_self_signed_server_identity().unwrap();
    let wrong_cert = generate_self_signed_server_identity()
        .unwrap()
        .certificate_der()
        .as_ref()
        .to_vec();
    let service = service_config(3000);
    Agent::register_with_control(AgentConfig {
        device_id: DeviceId::new("pc_001"),
        control_server: control_url.clone(),
        auth_token: "agent-token".to_string(),
        services: vec![service.clone()],
        p2p_certificate_der: Some(wrong_cert),
    })
    .await
    .unwrap();

    let registry = ServiceRegistry::new(vec![service]).unwrap();
    let agent = AgentControlRuntime::new(AgentControlRuntimeConfig {
        control_server_url: control_url.clone(),
        auth_token: "agent-token".to_string(),
        device_id: DeviceId::new("pc_001"),
        relay_server_cert: CertificateDer::from(vec![1, 2, 3]),
        registry,
        poll_interval: Duration::from_millis(10),
        p2p: Some(AgentP2pRuntimeConfig {
            bind_addr: local_addr(0),
            candidate_timeout: Duration::from_secs(1),
            probe_timeout: Duration::from_secs(1),
            interval: Duration::from_millis(10),
            server_identity: Some(agent_identity),
        }),
        mobile_grants: None,
    })
    .unwrap();
    let (agent_shutdown_tx, agent_shutdown_rx) = oneshot::channel();
    let agent_task = tokio::spawn(agent.run_until(async {
        let _ = agent_shutdown_rx.await;
    }));

    let connector = ControlP2pStreamConnector::new(ControlP2pConnectorConfig {
        control_server_url: control_url,
        control_token: None,
        client_id: ClientId::new("mobile_001"),
        control_client_options: HttpControlClientOptions::default(),
        bind_addr: local_addr(0),
        candidate_timeout: Duration::from_secs(1),
        probe_timeout: Duration::from_secs(1),
        interval: Duration::from_millis(10),
    })
    .unwrap();

    let result = connector
        .connect_p2p(&OpenForwardRequest {
            device_id: DeviceId::new("pc_001"),
            service_id: ServiceId::new("svc_web_3000"),
            local_port: 18080,
        })
        .await;
    assert!(result.is_err());

    let _ = agent_shutdown_tx.send(());
    let _ = punch_shutdown_tx.send(());
    agent_task.await.unwrap().unwrap();
    punch_task.await.unwrap().unwrap();
    control_task.abort();
}

#[tokio::test]
async fn mobile_grant_connector_rejects_agent_p2p_fingerprint_mismatch() {
    let control = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let control_addr = control.local_addr().unwrap();
    let control_url = format!("http://{control_addr}");
    let state = ControlState::new("dev-secret", "127.0.0.1:9", "127.0.0.1:9");
    let control_task = tokio::spawn(async move {
        axum::serve(control, routes(state)).await.unwrap();
    });

    let service = service_config(3000);
    let registered_cert = vec![9, 9, 9, 9];
    Agent::register_with_control(AgentConfig {
        device_id: DeviceId::new("pc_001"),
        control_server: control_url.clone(),
        auth_token: "agent-token".to_string(),
        services: vec![service.clone()],
        p2p_certificate_der: Some(registered_cert),
    })
    .await
    .unwrap();

    let service_id = ServiceId::new("svc_web_3000");
    let client_id = ClientId::new("mobile_001");
    let grants = MobileGrantManager::default();
    let invite = grants
        .create_invite(
            CreateMobileInviteRequest {
                control_url: control_url.clone(),
                device_id: DeviceId::new("pc_001"),
                allowed_services: vec![service_id.clone()],
                ttl_sec: 60,
                max_uses: 1,
                agent_p2p_cert_fingerprint: Some(mobile_grant_certificate_fingerprint([
                    1_u8, 2, 3, 4,
                ])),
            },
            1_000,
        )
        .unwrap();
    let proof = MobilePairingRequest::proof_for(
        DeviceId::new("pc_001"),
        invite.invite_id.clone(),
        client_id.clone(),
        vec![service_id.clone()],
        "pairing-nonce".to_string(),
        &invite.invite_secret,
    )
    .unwrap();
    let grant = grants
        .approve_pairing(
            &MobilePairingRequest {
                device_id: DeviceId::new("pc_001"),
                invite_id: invite.invite_id,
                client_id: client_id.clone(),
                requested_services: vec![service_id.clone()],
                nonce: "pairing-nonce".to_string(),
                proof,
            },
            1_001,
        )
        .unwrap();

    let control_client = HttpControlClient::new(control_url.clone()).unwrap();
    let connector = ControlP2pOrRelayStreamConnector::new(ControlP2pOrRelayConnectorConfig {
        control_server_url: control_url.clone(),
        control_token: None,
        client_id,
        control_client_options: HttpControlClientOptions::default(),
        mobile_grant: Some(grant),
        relay_server_cert: CertificateDer::from(vec![1, 2, 3]),
        bind_addr: local_addr(0),
        candidate_timeout: Duration::from_millis(20),
        probe_timeout: Duration::from_millis(20),
        interval: Duration::from_millis(5),
        relay_fallback_delay: Duration::from_millis(1),
    })
    .unwrap();
    let open_task = tokio::spawn(async move {
        connector
            .open_stream(&OpenForwardRequest {
                device_id: DeviceId::new("pc_001"),
                service_id,
                local_port: 18080,
            })
            .await
    });

    let pending_session_id = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let pending = control_client
                .list_grant_session_requests(&DeviceId::new("pc_001"))
                .await
                .unwrap();
            if let Some(request) = pending.first() {
                break request.pending_session_id.clone();
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    control_client
        .approve_grant_session(&pending_session_id)
        .await
        .unwrap();

    let result = tokio::time::timeout(Duration::from_secs(2), open_task)
        .await
        .unwrap()
        .unwrap();
    let error = match result {
        Ok(_) => panic!("expected fingerprint mismatch"),
        Err(error) => error,
    };
    assert!(
        error.to_string().contains("fingerprint"),
        "expected fingerprint error, got {error}"
    );

    control_task.abort();
}

fn service_config(target_port: u16) -> ServiceConfig {
    ServiceConfig {
        service_id: ServiceId::new("svc_web_3000"),
        name: "Dev Web".to_string(),
        protocol: ServiceProtocol::Tcp,
        target_host: "127.0.0.1".to_string(),
        target_port,
    }
}

fn p2p_config(
    session_id: SessionId,
    role: PeerRole,
    self_id: &str,
    peer_id: &str,
    punch_addr: SocketAddr,
    shared_secret: String,
) -> P2pPathConfig {
    P2pPathConfig {
        session_id,
        role,
        self_id: self_id.to_string(),
        peer_id: peer_id.to_string(),
        bind_addr: local_addr(0),
        punch_addr,
        shared_secret,
        candidate_timeout: Duration::from_secs(1),
        probe_timeout: Duration::from_secs(1),
        interval: Duration::from_millis(10),
    }
}

fn local_addr(port: u16) -> SocketAddr {
    SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port)
}
