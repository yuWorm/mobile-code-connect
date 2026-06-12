use std::{
    future::Future,
    net::{IpAddr, Ipv4Addr, SocketAddr},
};

use mobilecode_connect_protocol::{ControlFrame, PeerRole, RelayBindFrame, SessionId};
use mobilecode_connect_tunnel::{
    quic::{make_client_endpoint, QuicBiStream, QuicError},
    stream::{read_control_frame, write_control_frame, TunnelStreamError},
};
use rustls::pki_types::CertificateDer;
use tokio::io::AsyncWriteExt;

use crate::{service_registry::ServiceRegistry, stream_handler::handle_data_stream};

#[derive(Debug, Clone)]
pub struct RelayClientConfig;

#[derive(Debug, Clone)]
pub struct RelayAgentConfig {
    pub relay_addr: SocketAddr,
    pub server_cert: CertificateDer<'static>,
    pub session_id: SessionId,
    pub token: String,
    pub registry: ServiceRegistry,
}

pub struct RelayAgentClient {
    endpoint: quinn::Endpoint,
    connection: quinn::Connection,
    registry: ServiceRegistry,
}

impl RelayAgentClient {
    pub async fn connect(config: RelayAgentConfig) -> Result<Self, AgentRelayError> {
        let endpoint = make_client_endpoint(
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
            &[config.server_cert],
        )
        .await?;
        let connection = endpoint.connect(config.relay_addr, "localhost")?.await?;
        bind_agent(&connection, config.session_id.clone(), config.token).await?;

        Ok(Self {
            endpoint,
            connection,
            registry: config.registry,
        })
    }

    pub async fn run_until<F>(self, shutdown: F) -> Result<(), AgentRelayError>
    where
        F: Future<Output = ()> + Send,
    {
        let RelayAgentClient {
            endpoint,
            connection,
            registry,
        } = self;
        tokio::pin!(shutdown);

        loop {
            tokio::select! {
                _ = &mut shutdown => {
                    connection.close(0_u32.into(), b"shutdown");
                    endpoint.wait_idle().await;
                    return Ok(());
                }
                accepted = connection.accept_bi() => {
                    let Ok((send, recv)) = accepted else {
                        return Ok(());
                    };
                    let registry = registry.clone();
                    tokio::spawn(async move {
                        let _ = handle_data_stream(QuicBiStream::new(send, recv), registry).await;
                    });
                }
            }
        }
    }
}

async fn bind_agent(
    connection: &quinn::Connection,
    session_id: SessionId,
    token: String,
) -> Result<(), AgentRelayError> {
    let (send, recv) = connection.open_bi().await?;
    let mut stream = QuicBiStream::new(send, recv);
    write_control_frame(
        &mut stream,
        &ControlFrame::RelayBind(RelayBindFrame {
            role: PeerRole::Agent,
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
        _ => Err(AgentRelayError::UnexpectedControlFrame),
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AgentRelayError {
    #[error("quic endpoint failed: {0}")]
    Quic(#[from] QuicError),
    #[error("quic connect failed: {0}")]
    Connect(#[from] quinn::ConnectError),
    #[error("quic connection failed: {0}")]
    Connection(#[from] quinn::ConnectionError),
    #[error("stream failed: {0}")]
    Stream(#[from] TunnelStreamError),
    #[error("io failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("unexpected control frame during agent relay bind")]
    UnexpectedControlFrame,
}
