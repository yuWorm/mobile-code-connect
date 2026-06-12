use std::future::Future;

use mobilecode_connect_punch::probe::P2pPath;
use mobilecode_connect_tunnel::quic::{
    make_server_endpoint_from_std_socket, make_server_endpoint_from_std_socket_with_identity,
    P2pQuicIdentity, QuicBiStream, QuicError,
};

use crate::{service_registry::ServiceRegistry, stream_handler::handle_data_stream};

pub struct P2pAgentClient {
    endpoint: quinn::Endpoint,
    registry: ServiceRegistry,
}

impl P2pAgentClient {
    pub async fn from_path(
        path: P2pPath,
        registry: ServiceRegistry,
    ) -> Result<Self, AgentP2pError> {
        let socket = path.socket.into_std()?;
        let server = make_server_endpoint_from_std_socket(socket).await?;
        Ok(Self {
            endpoint: server.into_endpoint(),
            registry,
        })
    }

    pub async fn from_path_with_identity(
        path: P2pPath,
        registry: ServiceRegistry,
        identity: P2pQuicIdentity,
    ) -> Result<Self, AgentP2pError> {
        let socket = path.socket.into_std()?;
        let server = make_server_endpoint_from_std_socket_with_identity(socket, identity).await?;
        Ok(Self {
            endpoint: server.into_endpoint(),
            registry,
        })
    }

    pub async fn run_until<F>(self, shutdown: F) -> Result<(), AgentP2pError>
    where
        F: Future<Output = ()> + Send,
    {
        let P2pAgentClient { endpoint, registry } = self;
        tokio::pin!(shutdown);

        loop {
            tokio::select! {
                _ = &mut shutdown => {
                    endpoint.close(0_u32.into(), b"shutdown");
                    endpoint.wait_idle().await;
                    return Ok(());
                }
                incoming = endpoint.accept() => {
                    let Some(incoming) = incoming else {
                        return Ok(());
                    };
                    let registry = registry.clone();
                    tokio::spawn(async move {
                        if let Ok(connection) = incoming.await {
                            serve_connection(connection, registry).await;
                        }
                    });
                }
            }
        }
    }
}

async fn serve_connection(connection: quinn::Connection, registry: ServiceRegistry) {
    while let Ok((send, recv)) = connection.accept_bi().await {
        let registry = registry.clone();
        tokio::spawn(async move {
            let _ = handle_data_stream(QuicBiStream::new(send, recv), registry).await;
        });
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AgentP2pError {
    #[error("io failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("quic endpoint failed: {0}")]
    Quic(#[from] QuicError),
}
