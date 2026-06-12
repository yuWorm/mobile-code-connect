use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use async_trait::async_trait;
use mobilecode_connect_control_client::{
    ControlClientError, CreateSessionRequest, CreateSessionResponse, HttpControlClient,
    HttpControlClientOptions,
};
use mobilecode_connect_protocol::{
    mobile_grant_certificate_fingerprint, ClientId, ControlFrame, DataStreamHeader, DeviceId,
    MobileGrantCredential, PeerRole, PendingGrantSessionStatus, RelayBindFrame, ServiceId,
    SessionId, StreamId,
};
use mobilecode_connect_punch::probe::{establish_p2p_path, P2pPath, P2pPathConfig, P2pPathError};
use mobilecode_connect_tunnel::{
    copy::copy_bidirectional_with_stats,
    quic::{
        make_client_endpoint, make_client_endpoint_from_std_socket,
        make_insecure_client_endpoint_from_std_socket, QuicBiStream, QuicError,
    },
    stats::AtomicTrafficStats,
    stream::{read_control_frame, write_control_frame, write_data_header, TunnelStreamError},
};
use rustls::pki_types::CertificateDer;
use tokio::{
    io::AsyncWriteExt,
    io::{duplex, DuplexStream, ReadBuf},
    net::TcpListener,
    sync::{mpsc, oneshot},
    task::JoinHandle,
};

use crate::status::TunnelTransportStatsHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalForwardHandle {
    handle_id: String,
    device_id: DeviceId,
    service_id: ServiceId,
    local_port: u16,
}

impl LocalForwardHandle {
    pub fn new(
        handle_id: String,
        device_id: DeviceId,
        service_id: ServiceId,
        local_port: u16,
    ) -> Self {
        Self {
            handle_id,
            device_id,
            service_id,
            local_port,
        }
    }

    pub fn handle_id(&self) -> &str {
        &self.handle_id
    }

    pub fn local_port(&self) -> u16 {
        self.local_port
    }

    pub fn device_id(&self) -> &DeviceId {
        &self.device_id
    }

    pub fn service_id(&self) -> &ServiceId {
        &self.service_id
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenForwardRequest {
    pub device_id: DeviceId,
    pub service_id: ServiceId,
    pub local_port: u16,
}

pub type BoxedStream = Box<dyn AsyncReadWrite + Send + Unpin>;

#[async_trait]
pub trait StreamConnector: Send + Sync {
    async fn open_stream(&self, request: &OpenForwardRequest) -> Result<BoxedStream, ForwardError>;
}

pub struct LocalForwarder {
    local_port: u16,
    shutdown_tx: Option<oneshot::Sender<()>>,
    task: JoinHandle<()>,
}

impl LocalForwarder {
    pub async fn bind(
        request: OpenForwardRequest,
        connector: Arc<dyn StreamConnector>,
    ) -> Result<Self, ForwardError> {
        if request.local_port == 0 {
            // Let the OS choose a port for tests and embedders that request ephemeral ports.
        }
        let listener = TcpListener::bind(("127.0.0.1", request.local_port)).await?;
        let local_port = listener.local_addr()?.port();
        let (shutdown_tx, mut shutdown_rx) = oneshot::channel();

        let task = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = &mut shutdown_rx => break,
                    accepted = listener.accept() => {
                        let Ok((local_stream, _)) = accepted else {
                            break;
                        };
                        let connector = Arc::clone(&connector);
                        let request = request.clone();
                        tokio::spawn(async move {
                            if let Ok(remote_stream) = connector.open_stream(&request).await {
                                let _ = copy_bidirectional_with_stats(
                                    local_stream,
                                    remote_stream,
                                    Arc::new(AtomicTrafficStats::default()),
                                ).await;
                            }
                        });
                    }
                }
            }
        });

        Ok(Self {
            local_port,
            shutdown_tx: Some(shutdown_tx),
            task,
        })
    }

    pub fn local_port(&self) -> u16 {
        self.local_port
    }

    pub async fn shutdown(mut self) -> Result<(), ForwardError> {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        self.task.await.map_err(ForwardError::Join)?;
        Ok(())
    }
}

#[derive(Default)]
pub struct MemoryStreamConnector {
    tx: std::sync::Mutex<Option<mpsc::Sender<DuplexStream>>>,
    rx: tokio::sync::Mutex<Option<mpsc::Receiver<DuplexStream>>>,
}

impl MemoryStreamConnector {
    async fn channel(&self) -> mpsc::Sender<DuplexStream> {
        if let Some(tx) = self.tx.lock().expect("connector lock poisoned").as_ref() {
            return tx.clone();
        }

        let (tx, rx) = mpsc::channel(8);
        *self.tx.lock().expect("connector lock poisoned") = Some(tx.clone());
        *self.rx.lock().await = Some(rx);
        tx
    }

    pub async fn accept(&self) -> Result<DuplexStream, ForwardError> {
        let _ = self.channel().await;
        let mut guard = self.rx.lock().await;
        let rx = guard.as_mut().ok_or(ForwardError::ConnectorClosed)?;
        rx.recv().await.ok_or(ForwardError::ConnectorClosed)
    }
}

#[async_trait]
impl StreamConnector for MemoryStreamConnector {
    async fn open_stream(
        &self,
        _request: &OpenForwardRequest,
    ) -> Result<BoxedStream, ForwardError> {
        let (local, remote) = duplex(1024);
        self.channel()
            .await
            .send(remote)
            .await
            .map_err(|_| ForwardError::ConnectorClosed)?;
        Ok(Box::new(local))
    }
}

#[derive(Debug, Clone)]
pub struct RelayConnectorConfig {
    pub relay_addr: SocketAddr,
    pub server_cert: CertificateDer<'static>,
    pub session_id: SessionId,
    pub token: String,
}

pub struct RelayStreamConnector {
    _endpoint: quinn::Endpoint,
    connection: quinn::Connection,
    session_id: SessionId,
}

impl RelayStreamConnector {
    pub async fn connect(config: RelayConnectorConfig) -> Result<Self, ForwardError> {
        let endpoint = make_client_endpoint(
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
            &[config.server_cert],
        )
        .await?;
        let connection = endpoint.connect(config.relay_addr, "localhost")?.await?;
        bind_mobile(&connection, config.session_id.clone(), config.token).await?;

        Ok(Self {
            _endpoint: endpoint,
            connection,
            session_id: config.session_id,
        })
    }
}

pub struct P2pStreamConnector {
    _endpoint: quinn::Endpoint,
    connection: quinn::Connection,
    session_id: SessionId,
}

impl P2pStreamConnector {
    pub async fn connect_path(session_id: SessionId, path: P2pPath) -> Result<Self, ForwardError> {
        let peer_addr = path.peer_addr;
        let endpoint =
            make_insecure_client_endpoint_from_std_socket(path.socket.into_std()?).await?;
        let connection = endpoint.connect(peer_addr, "localhost")?.await?;

        Ok(Self {
            _endpoint: endpoint,
            connection,
            session_id,
        })
    }

    pub async fn connect_path_with_server_cert(
        session_id: SessionId,
        path: P2pPath,
        server_cert: CertificateDer<'static>,
    ) -> Result<Self, ForwardError> {
        let peer_addr = path.peer_addr;
        let endpoint =
            make_client_endpoint_from_std_socket(path.socket.into_std()?, &[server_cert]).await?;
        let connection = endpoint.connect(peer_addr, "localhost")?.await?;

        Ok(Self {
            _endpoint: endpoint,
            connection,
            session_id,
        })
    }
}

#[derive(Debug, Clone)]
pub struct ControlRelayConnectorConfig {
    pub control_server_url: String,
    pub control_token: Option<String>,
    pub client_id: ClientId,
    pub control_client_options: HttpControlClientOptions,
    pub relay_server_cert: CertificateDer<'static>,
}

pub struct ControlRelayStreamConnector {
    control: HttpControlClient,
    client_id: ClientId,
    relay_server_cert: CertificateDer<'static>,
    transport_stats: TunnelTransportStatsHandle,
}

impl ControlRelayStreamConnector {
    pub fn new(config: ControlRelayConnectorConfig) -> Result<Self, ForwardError> {
        Ok(Self {
            control: control_client(
                config.control_server_url,
                config.control_token,
                config.control_client_options,
            )?,
            client_id: config.client_id,
            relay_server_cert: config.relay_server_cert,
            transport_stats: TunnelTransportStatsHandle::default(),
        })
    }

    pub fn transport_stats_handle(&self) -> TunnelTransportStatsHandle {
        self.transport_stats.clone()
    }

    pub async fn resolve_relay_config(
        &self,
        request: &OpenForwardRequest,
    ) -> Result<RelayConnectorConfig, ForwardError> {
        let session = self
            .control
            .create_session(CreateSessionRequest {
                client_id: self.client_id.to_string(),
                device_id: request.device_id.clone(),
                service_id: request.service_id.clone(),
            })
            .await?;

        Ok(RelayConnectorConfig {
            relay_addr: parse_socket_addr(&session.relay_addr)?,
            server_cert: self.relay_server_cert.clone(),
            session_id: session.session_id,
            token: session.relay_token,
        })
    }
}

#[derive(Debug, Clone)]
pub struct ControlP2pConnectorConfig {
    pub control_server_url: String,
    pub control_token: Option<String>,
    pub client_id: ClientId,
    pub control_client_options: HttpControlClientOptions,
    pub bind_addr: SocketAddr,
    pub candidate_timeout: Duration,
    pub probe_timeout: Duration,
    pub interval: Duration,
}

pub struct ControlP2pStreamConnector {
    control: HttpControlClient,
    client_id: ClientId,
    bind_addr: SocketAddr,
    candidate_timeout: Duration,
    probe_timeout: Duration,
    interval: Duration,
}

#[derive(Debug, Clone)]
pub struct ControlP2pOrRelayConnectorConfig {
    pub control_server_url: String,
    pub control_token: Option<String>,
    pub mobile_grant: Option<MobileGrantCredential>,
    pub client_id: ClientId,
    pub control_client_options: HttpControlClientOptions,
    pub relay_server_cert: CertificateDer<'static>,
    pub bind_addr: SocketAddr,
    pub candidate_timeout: Duration,
    pub probe_timeout: Duration,
    pub interval: Duration,
    pub relay_fallback_delay: Duration,
}

pub struct ControlP2pOrRelayStreamConnector {
    control: HttpControlClient,
    client_id: ClientId,
    mobile_grant: Option<MobileGrantCredential>,
    relay_server_cert: CertificateDer<'static>,
    bind_addr: SocketAddr,
    candidate_timeout: Duration,
    probe_timeout: Duration,
    interval: Duration,
    relay_fallback_delay: Duration,
    transport_stats: TunnelTransportStatsHandle,
}

impl ControlP2pStreamConnector {
    pub fn new(config: ControlP2pConnectorConfig) -> Result<Self, ForwardError> {
        Ok(Self {
            control: control_client(
                config.control_server_url,
                config.control_token,
                config.control_client_options,
            )?,
            client_id: config.client_id,
            bind_addr: config.bind_addr,
            candidate_timeout: config.candidate_timeout,
            probe_timeout: config.probe_timeout,
            interval: config.interval,
        })
    }

    pub async fn connect_p2p(
        &self,
        request: &OpenForwardRequest,
    ) -> Result<P2pStreamConnector, ForwardError> {
        let session = self
            .control
            .create_session(CreateSessionRequest {
                client_id: self.client_id.to_string(),
                device_id: request.device_id.clone(),
                service_id: request.service_id.clone(),
            })
            .await?;
        let punch_addr = parse_punch_addr(&session.punch_addr)?;
        let path = establish_p2p_path(P2pPathConfig {
            session_id: session.session_id.clone(),
            role: PeerRole::Mobile,
            self_id: self.client_id.to_string(),
            peer_id: request.device_id.to_string(),
            bind_addr: self.bind_addr,
            punch_addr,
            shared_secret: session.relay_token.clone(),
            candidate_timeout: self.candidate_timeout,
            probe_timeout: self.probe_timeout,
            interval: self.interval,
        })
        .await?;

        let server_cert = agent_p2p_cert(&session)?;
        P2pStreamConnector::connect_path_with_server_cert(session.session_id, path, server_cert)
            .await
    }
}

impl ControlP2pOrRelayStreamConnector {
    pub fn new(config: ControlP2pOrRelayConnectorConfig) -> Result<Self, ForwardError> {
        Ok(Self {
            control: control_client(
                config.control_server_url,
                config.control_token,
                config.control_client_options,
            )?,
            client_id: config.client_id,
            mobile_grant: config.mobile_grant,
            relay_server_cert: config.relay_server_cert,
            bind_addr: config.bind_addr,
            candidate_timeout: config.candidate_timeout,
            probe_timeout: config.probe_timeout,
            interval: config.interval,
            relay_fallback_delay: config.relay_fallback_delay,
            transport_stats: TunnelTransportStatsHandle::default(),
        })
    }

    pub fn transport_stats_handle(&self) -> TunnelTransportStatsHandle {
        self.transport_stats.clone()
    }

    async fn create_session(
        &self,
        request: &OpenForwardRequest,
    ) -> Result<CreateSessionResponse, ForwardError> {
        if let Some(grant) = &self.mobile_grant {
            return self.create_grant_session(request, grant).await;
        }

        Ok(self
            .control
            .create_session(CreateSessionRequest {
                client_id: self.client_id.to_string(),
                device_id: request.device_id.clone(),
                service_id: request.service_id.clone(),
            })
            .await?)
    }

    async fn create_grant_session(
        &self,
        request: &OpenForwardRequest,
        grant: &MobileGrantCredential,
    ) -> Result<CreateSessionResponse, ForwardError> {
        if grant.client_id != self.client_id
            || grant.device_id != request.device_id
            || !grant.allows(&request.service_id, grant.revocation_version)
        {
            return Err(ForwardError::MobileGrantScopeDenied);
        }

        let session_request = grant
            .sign_session_request(
                request.service_id.clone(),
                format!("nonce_{}", uuid::Uuid::new_v4().simple()),
            )
            .map_err(|_| ForwardError::InvalidMobileGrant)?;
        let started = self.control.start_grant_session(session_request).await?;
        let poll_interval = Duration::from_millis(started.poll_interval_ms.max(1));

        loop {
            let poll = self
                .control
                .grant_session_result(&started.pending_session_id)
                .await?;
            match poll.status {
                PendingGrantSessionStatus::Pending => {
                    tokio::time::sleep(grant_session_pending_sleep(
                        poll_interval,
                        started.expires_at,
                        current_epoch_sec(),
                    )?)
                    .await;
                }
                PendingGrantSessionStatus::Approved => {
                    let session = poll
                        .session
                        .ok_or(ForwardError::GrantSessionApprovedWithoutSession)?;
                    validate_grant_session_p2p_fingerprint(grant, &session)?;
                    return Ok(session);
                }
                PendingGrantSessionStatus::Denied => return Err(ForwardError::GrantSessionDenied),
                PendingGrantSessionStatus::Expired => {
                    return Err(ForwardError::GrantSessionExpired)
                }
            }
        }
    }
}

fn grant_session_pending_sleep(
    poll_interval: Duration,
    expires_at_epoch_sec: u64,
    now_epoch_sec: u64,
) -> Result<Duration, ForwardError> {
    let remaining_secs = expires_at_epoch_sec
        .checked_sub(now_epoch_sec)
        .filter(|remaining| *remaining > 0)
        .ok_or(ForwardError::GrantSessionExpired)?;
    Ok(poll_interval.min(Duration::from_secs(remaining_secs)))
}

fn current_epoch_sec() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn control_client(
    control_server_url: String,
    control_token: Option<String>,
    options: HttpControlClientOptions,
) -> Result<HttpControlClient, ControlClientError> {
    if let Some(token) = control_token {
        HttpControlClient::with_optional_bearer_token_and_options(
            control_server_url,
            token,
            options,
        )
    } else {
        HttpControlClient::with_options(control_server_url, options)
    }
}

#[async_trait]
impl StreamConnector for ControlRelayStreamConnector {
    async fn open_stream(&self, request: &OpenForwardRequest) -> Result<BoxedStream, ForwardError> {
        let relay_config = match self.resolve_relay_config(request).await {
            Ok(config) => config,
            Err(error) => {
                self.transport_stats.relay_failure();
                return Err(error);
            }
        };
        let relay = match RelayStreamConnector::connect(relay_config).await {
            Ok(relay) => relay,
            Err(error) => {
                self.transport_stats.relay_failure();
                return Err(error);
            }
        };
        let result = relay.open_stream(request).await;
        match &result {
            Ok(_) => self.transport_stats.relay_connection(),
            Err(_) => self.transport_stats.relay_failure(),
        }
        result
    }
}

#[async_trait]
impl StreamConnector for RelayStreamConnector {
    async fn open_stream(&self, request: &OpenForwardRequest) -> Result<BoxedStream, ForwardError> {
        let stream = open_quic_data_stream(&self.connection, &self.session_id, request).await?;
        Ok(Box::new(ConnectionBackedStream::new(
            self._endpoint.clone(),
            self.connection.clone(),
            stream,
        )))
    }
}

#[async_trait]
impl StreamConnector for P2pStreamConnector {
    async fn open_stream(&self, request: &OpenForwardRequest) -> Result<BoxedStream, ForwardError> {
        let stream = open_quic_data_stream(&self.connection, &self.session_id, request).await?;
        Ok(Box::new(ConnectionBackedStream::new(
            self._endpoint.clone(),
            self.connection.clone(),
            stream,
        )))
    }
}

#[async_trait]
impl StreamConnector for ControlP2pStreamConnector {
    async fn open_stream(&self, request: &OpenForwardRequest) -> Result<BoxedStream, ForwardError> {
        let p2p = self.connect_p2p(request).await?;
        p2p.open_stream(request).await
    }
}

#[async_trait]
impl StreamConnector for ControlP2pOrRelayStreamConnector {
    async fn open_stream(&self, request: &OpenForwardRequest) -> Result<BoxedStream, ForwardError> {
        let session = self.create_session(request).await?;
        let p2p = open_p2p_stream_from_session_with_stats(
            &session,
            request,
            self.client_id.clone(),
            self.bind_addr,
            self.candidate_timeout,
            self.probe_timeout,
            self.interval,
            self.transport_stats.clone(),
        );
        tokio::pin!(p2p);

        let fallback_delay = tokio::time::sleep(self.relay_fallback_delay);
        tokio::pin!(fallback_delay);

        tokio::select! {
            p2p_result = &mut p2p => {
                return match p2p_result {
                    Ok(stream) => Ok(stream),
                    Err(_) => {
                        self.transport_stats.relay_fallback();
                        open_relay_stream_from_session_with_stats(
                        &session,
                        request,
                        self.relay_server_cert.clone(),
                        self.transport_stats.clone(),
                    ).await
                    },
                };
            }
            _ = &mut fallback_delay => {}
        }

        self.transport_stats.relay_fallback();
        let relay = open_relay_stream_from_session_with_stats(
            &session,
            request,
            self.relay_server_cert.clone(),
            self.transport_stats.clone(),
        );
        tokio::pin!(relay);

        tokio::select! {
            p2p_result = &mut p2p => {
                match p2p_result {
                    Ok(stream) => Ok(stream),
                    Err(_) => relay.await,
                }
            }
            relay_result = &mut relay => {
                match relay_result {
                    Ok(stream) => Ok(stream),
                    Err(relay_error) => match p2p.await {
                        Ok(stream) => Ok(stream),
                        Err(_) => Err(relay_error),
                    },
                }
            }
        }
    }
}

async fn open_p2p_stream_from_session_with_stats(
    session: &CreateSessionResponse,
    request: &OpenForwardRequest,
    client_id: ClientId,
    bind_addr: SocketAddr,
    candidate_timeout: Duration,
    probe_timeout: Duration,
    interval: Duration,
    transport_stats: TunnelTransportStatsHandle,
) -> Result<BoxedStream, ForwardError> {
    transport_stats.p2p_attempt();
    let result = open_p2p_stream_from_session(
        session,
        request,
        client_id,
        bind_addr,
        candidate_timeout,
        probe_timeout,
        interval,
    )
    .await;
    match &result {
        Ok(_) => transport_stats.p2p_connection(),
        Err(_) => transport_stats.p2p_failure(),
    }
    result
}

async fn open_relay_stream_from_session_with_stats(
    session: &CreateSessionResponse,
    request: &OpenForwardRequest,
    relay_server_cert: CertificateDer<'static>,
    transport_stats: TunnelTransportStatsHandle,
) -> Result<BoxedStream, ForwardError> {
    let result = open_relay_stream_from_session(session, request, relay_server_cert).await;
    match &result {
        Ok(_) => transport_stats.relay_connection(),
        Err(_) => transport_stats.relay_failure(),
    }
    result
}

async fn open_p2p_stream_from_session(
    session: &CreateSessionResponse,
    request: &OpenForwardRequest,
    client_id: ClientId,
    bind_addr: SocketAddr,
    candidate_timeout: Duration,
    probe_timeout: Duration,
    interval: Duration,
) -> Result<BoxedStream, ForwardError> {
    let punch_addr = parse_punch_addr(&session.punch_addr)?;
    let path = establish_p2p_path(P2pPathConfig {
        session_id: session.session_id.clone(),
        role: PeerRole::Mobile,
        self_id: client_id.to_string(),
        peer_id: request.device_id.to_string(),
        bind_addr,
        punch_addr,
        shared_secret: session.relay_token.clone(),
        candidate_timeout,
        probe_timeout,
        interval,
    })
    .await?;
    let server_cert = agent_p2p_cert(session)?;
    let p2p = P2pStreamConnector::connect_path_with_server_cert(
        session.session_id.clone(),
        path,
        server_cert,
    )
    .await?;
    p2p.open_stream(request).await
}

async fn open_relay_stream_from_session(
    session: &CreateSessionResponse,
    request: &OpenForwardRequest,
    relay_server_cert: CertificateDer<'static>,
) -> Result<BoxedStream, ForwardError> {
    let relay = RelayStreamConnector::connect(RelayConnectorConfig {
        relay_addr: parse_socket_addr(&session.relay_addr)?,
        server_cert: relay_server_cert,
        session_id: session.session_id.clone(),
        token: session.relay_token.clone(),
    })
    .await?;
    relay.open_stream(request).await
}

async fn open_quic_data_stream(
    connection: &quinn::Connection,
    session_id: &SessionId,
    request: &OpenForwardRequest,
) -> Result<QuicBiStream, ForwardError> {
    let (send, recv) = connection.open_bi().await?;
    let mut stream = QuicBiStream::new(send, recv);
    write_data_header(
        &mut stream,
        &DataStreamHeader {
            stream_id: StreamId::new(format!("stream_{}", uuid::Uuid::new_v4())),
            session_id: session_id.clone(),
            service_id: request.service_id.clone(),
        },
    )
    .await?;
    stream.flush().await?;
    Ok(stream)
}

struct ConnectionBackedStream {
    _endpoint: quinn::Endpoint,
    _connection: quinn::Connection,
    stream: QuicBiStream,
}

impl ConnectionBackedStream {
    fn new(endpoint: quinn::Endpoint, connection: quinn::Connection, stream: QuicBiStream) -> Self {
        Self {
            _endpoint: endpoint,
            _connection: connection,
            stream,
        }
    }
}

impl tokio::io::AsyncRead for ConnectionBackedStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.stream).poll_read(cx, buf)
    }
}

impl tokio::io::AsyncWrite for ConnectionBackedStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut self.stream).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.stream).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.stream).poll_shutdown(cx)
    }
}

fn parse_socket_addr(value: &str) -> Result<SocketAddr, ForwardError> {
    value
        .parse::<SocketAddr>()
        .map_err(|_| ForwardError::InvalidRelayAddress {
            value: value.to_string(),
        })
}

fn parse_punch_addr(value: &str) -> Result<SocketAddr, ForwardError> {
    value
        .parse::<SocketAddr>()
        .map_err(|_| ForwardError::InvalidPunchAddress {
            value: value.to_string(),
        })
}

fn agent_p2p_cert(
    session: &CreateSessionResponse,
) -> Result<CertificateDer<'static>, ForwardError> {
    session
        .agent_p2p_cert_der
        .clone()
        .map(CertificateDer::from)
        .ok_or_else(|| ForwardError::MissingAgentP2pCertificate {
            session_id: session.session_id.clone(),
        })
}

fn validate_grant_session_p2p_fingerprint(
    grant: &MobileGrantCredential,
    session: &CreateSessionResponse,
) -> Result<(), ForwardError> {
    let Some(expected) = grant.agent_p2p_cert_fingerprint.as_deref() else {
        return Ok(());
    };
    let cert_der = session.agent_p2p_cert_der.as_ref().ok_or_else(|| {
        ForwardError::MobileGrantP2pCertificateMissing {
            grant_id: grant.grant_id.clone(),
            session_id: session.session_id.clone(),
        }
    })?;
    let actual = mobile_grant_certificate_fingerprint(cert_der);
    if actual == expected {
        Ok(())
    } else {
        Err(ForwardError::MobileGrantP2pFingerprintMismatch {
            grant_id: grant.grant_id.clone(),
            session_id: session.session_id.clone(),
            expected: expected.to_string(),
            actual,
        })
    }
}

async fn bind_mobile(
    connection: &quinn::Connection,
    session_id: SessionId,
    token: String,
) -> Result<(), ForwardError> {
    let (send, recv) = connection.open_bi().await?;
    let mut stream = QuicBiStream::new(send, recv);
    write_control_frame(
        &mut stream,
        &ControlFrame::RelayBind(RelayBindFrame {
            role: PeerRole::Mobile,
            session_id: session_id.clone(),
            token,
        }),
    )
    .await?;
    stream.shutdown().await?;

    match read_control_frame(&mut stream).await? {
        ControlFrame::SessionReady {
            session_id: accepted,
        } if accepted == session_id => Ok(()),
        _ => Err(ForwardError::UnexpectedControlFrame),
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ForwardError {
    #[error("io failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("forward task failed: {0}")]
    Join(#[from] tokio::task::JoinError),
    #[error("stream connector is closed")]
    ConnectorClosed,
    #[error("quic endpoint failed: {0}")]
    Quic(#[from] QuicError),
    #[error("quic connect failed: {0}")]
    Connect(#[from] quinn::ConnectError),
    #[error("quic connection failed: {0}")]
    Connection(#[from] quinn::ConnectionError),
    #[error("stream failed: {0}")]
    Stream(#[from] TunnelStreamError),
    #[error("control client failed: {0}")]
    Control(#[from] ControlClientError),
    #[error("punch path failed: {0}")]
    P2pPath(#[from] P2pPathError),
    #[error("invalid relay address from control: {value}")]
    InvalidRelayAddress { value: String },
    #[error("invalid punch address from control: {value}")]
    InvalidPunchAddress { value: String },
    #[error("missing agent p2p certificate for session {session_id}")]
    MissingAgentP2pCertificate { session_id: SessionId },
    #[error("unexpected control frame during mobile relay bind")]
    UnexpectedControlFrame,
    #[error("mobile grant is invalid")]
    InvalidMobileGrant,
    #[error("mobile grant does not allow requested service")]
    MobileGrantScopeDenied,
    #[error("mobile grant session was denied by agent")]
    GrantSessionDenied,
    #[error("mobile grant session expired before approval")]
    GrantSessionExpired,
    #[error("mobile grant session was approved without session credentials")]
    GrantSessionApprovedWithoutSession,
    #[error("mobile grant {grant_id} requires agent p2p certificate for session {session_id}")]
    MobileGrantP2pCertificateMissing {
        grant_id: String,
        session_id: SessionId,
    },
    #[error("mobile grant {grant_id} agent p2p fingerprint mismatch for session {session_id}: expected {expected}, got {actual}")]
    MobileGrantP2pFingerprintMismatch {
        grant_id: String,
        session_id: SessionId,
        expected: String,
        actual: String,
    },
}

pub trait AsyncReadWrite: tokio::io::AsyncRead + tokio::io::AsyncWrite {}

impl<T> AsyncReadWrite for T where T: tokio::io::AsyncRead + tokio::io::AsyncWrite {}

#[cfg(test)]
mod tests {
    use super::*;
    use mobilecode_connect_protocol::mobile_grant_certificate_fingerprint;

    #[test]
    fn grant_session_accepts_matching_agent_p2p_fingerprint() {
        let grant = mobile_grant(Some(mobile_grant_certificate_fingerprint([1_u8, 2, 3])));
        let session = create_session(Some(vec![1, 2, 3]));

        validate_grant_session_p2p_fingerprint(&grant, &session).unwrap();
    }

    #[test]
    fn grant_session_rejects_mismatched_agent_p2p_fingerprint() {
        let grant = mobile_grant(Some(mobile_grant_certificate_fingerprint([1_u8, 2, 3])));
        let session = create_session(Some(vec![9, 9, 9]));

        assert!(matches!(
            validate_grant_session_p2p_fingerprint(&grant, &session),
            Err(ForwardError::MobileGrantP2pFingerprintMismatch { .. })
        ));
    }

    #[test]
    fn grant_session_rejects_missing_agent_p2p_cert_when_fingerprint_is_bound() {
        let grant = mobile_grant(Some(mobile_grant_certificate_fingerprint([1_u8, 2, 3])));
        let session = create_session(None);

        assert!(matches!(
            validate_grant_session_p2p_fingerprint(&grant, &session),
            Err(ForwardError::MobileGrantP2pCertificateMissing { .. })
        ));
    }

    #[test]
    fn grant_session_allows_missing_agent_p2p_cert_when_no_fingerprint_is_bound() {
        let grant = mobile_grant(None);
        let session = create_session(None);

        validate_grant_session_p2p_fingerprint(&grant, &session).unwrap();
    }

    #[test]
    fn grant_session_pending_sleep_caps_poll_interval_at_expiry() {
        let sleep = grant_session_pending_sleep(Duration::from_secs(5), 11, 10)
            .expect("pending session should still be valid");

        assert_eq!(sleep, Duration::from_secs(1));
    }

    #[test]
    fn grant_session_pending_sleep_expires_at_or_before_now() {
        assert!(matches!(
            grant_session_pending_sleep(Duration::from_secs(5), 10, 10),
            Err(ForwardError::GrantSessionExpired)
        ));
        assert!(matches!(
            grant_session_pending_sleep(Duration::from_secs(5), 9, 10),
            Err(ForwardError::GrantSessionExpired)
        ));
    }

    fn mobile_grant(agent_p2p_cert_fingerprint: Option<String>) -> MobileGrantCredential {
        MobileGrantCredential {
            version: 1,
            control_url: "http://127.0.0.1:4242".to_string(),
            device_id: DeviceId::new("pc_001"),
            grant_id: "gr_001".to_string(),
            client_id: ClientId::new("mobile_001"),
            allowed_services: vec![ServiceId::new("svc_web_3000")],
            grant_secret: "grant-secret".to_string(),
            revocation_version: 1,
            agent_p2p_cert_fingerprint,
        }
    }

    fn create_session(agent_p2p_cert_der: Option<Vec<u8>>) -> CreateSessionResponse {
        CreateSessionResponse {
            session_id: SessionId::new("sess_001"),
            access_token: "access-token".to_string(),
            relay_token: "relay-token".to_string(),
            relay_addr: "127.0.0.1:4443".to_string(),
            punch_addr: "127.0.0.1:3478".to_string(),
            agent_p2p_cert_der,
            expire_at: 1_000,
        }
    }
}
