use std::{
    collections::HashMap,
    future::Future,
    net::SocketAddr,
    sync::{Arc, RwLock},
};

use mobilecode_connect_auth::TokenKey;
use mobilecode_connect_protocol::{ControlFrame, PeerRole, RelayBindFrame, SessionId};
use mobilecode_connect_tunnel::{
    quic::{make_server_endpoint, QuicBiStream, ServerEndpoint},
    stream::{read_control_frame, write_control_frame},
};
use rustls::pki_types::CertificateDer;
use tokio::{io::AsyncWriteExt, time::Duration};

use crate::{
    bind::{RelayBindRequest, RelayPeerRole, SharedKeyRelayTokenVerifier},
    config::RelayConfig,
    forward::forward_stream_pair_with_limit,
    session::{RelayError, RelaySessionStore},
};

pub struct RelayService {
    session_store: RelaySessionStore,
    connection_store: RelayConnectionStore,
    quic: Option<ServerEndpoint>,
}

impl RelayService {
    pub async fn new(config: RelayConfig) -> Result<Self, RelayError> {
        let verifier = SharedKeyRelayTokenVerifier::new(
            TokenKey::new(config.token_secret),
            config.now_epoch_sec,
        );

        Ok(Self {
            session_store: RelaySessionStore::new(Arc::new(verifier)),
            connection_store: RelayConnectionStore::default(),
            quic: None,
        })
    }

    pub async fn new_quic(config: RelayConfig, bind_addr: SocketAddr) -> Result<Self, RelayError> {
        let mut service = Self::new(config).await?;
        service.quic = Some(make_server_endpoint(bind_addr).await?);
        Ok(service)
    }

    pub fn session_store(&self) -> RelaySessionStore {
        self.session_store.clone()
    }

    pub fn local_addr(&self) -> Option<SocketAddr> {
        self.quic
            .as_ref()
            .and_then(|endpoint| endpoint.local_addr().ok())
    }

    pub fn certificate_der(&self) -> Option<&CertificateDer<'static>> {
        self.quic.as_ref().map(ServerEndpoint::certificate_der)
    }

    pub fn admin_routes(&self) -> axum::Router {
        crate::admin::routes_with_connections(
            self.session_store.clone(),
            self.connection_store.clone(),
        )
    }

    pub async fn run_until<F>(self, shutdown: F) -> Result<(), RelayError>
    where
        F: Future<Output = ()> + Send,
    {
        let RelayService {
            session_store,
            connection_store,
            quic,
        } = self;

        let Some(quic) = quic else {
            shutdown.await;
            return Ok(());
        };

        let endpoint = quic.into_endpoint();
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
                    let session_store = session_store.clone();
                    let connection_store = connection_store.clone();
                    tokio::spawn(async move {
                        let _ = handle_connection(incoming, session_store, connection_store).await;
                    });
                }
            }
        }
    }
}

async fn handle_connection(
    incoming: quinn::Incoming,
    session_store: RelaySessionStore,
    connection_store: RelayConnectionStore,
) -> Result<(), RelayError> {
    let connection = incoming.await?;
    let (send, recv) = connection.accept_bi().await?;
    let mut control_stream = QuicBiStream::new(send, recv);
    let bind = match read_control_frame(&mut control_stream).await? {
        ControlFrame::RelayBind(frame) => frame,
        _ => return Err(RelayError::UnexpectedControlFrame),
    };

    let role = relay_role(&bind);
    session_store.bind(RelayBindRequest {
        role: role.clone(),
        session_id: bind.session_id.clone(),
        token: bind.token.clone(),
    })?;
    connection_store.set(bind.session_id.clone(), role.clone(), connection.clone());

    write_control_frame(
        &mut control_stream,
        &ControlFrame::SessionReady {
            session_id: bind.session_id.clone(),
        },
    )
    .await?;
    control_stream.shutdown().await?;

    match role {
        RelayPeerRole::Mobile => {
            serve_mobile_connection(bind.session_id, connection, session_store, connection_store)
                .await
        }
        RelayPeerRole::Agent => {
            let _ = connection.closed().await;
            Ok(())
        }
    }
}

async fn serve_mobile_connection(
    session_id: SessionId,
    connection: quinn::Connection,
    session_store: RelaySessionStore,
    connection_store: RelayConnectionStore,
) -> Result<(), RelayError> {
    loop {
        let Ok((mobile_send, mobile_recv)) = connection.accept_bi().await else {
            return Ok(());
        };

        let session_store = session_store.clone();
        let connection_store = connection_store.clone();
        let session_id = session_id.clone();
        tokio::spawn(async move {
            let _ = forward_mobile_stream(
                session_id,
                mobile_send,
                mobile_recv,
                session_store,
                connection_store,
            )
            .await;
        });
    }
}

async fn forward_mobile_stream(
    session_id: SessionId,
    mobile_send: quinn::SendStream,
    mobile_recv: quinn::RecvStream,
    session_store: RelaySessionStore,
    connection_store: RelayConnectionStore,
) -> Result<(), RelayError> {
    let permit = session_store.begin_stream(&session_id)?;
    let agent = wait_for_agent_connection(&connection_store, &session_id)
        .await
        .ok_or_else(|| RelayError::AgentConnectionMissing {
            session_id: session_id.clone(),
        })?;
    let (agent_send, agent_recv) = agent.open_bi().await?;
    let outcome = forward_stream_pair_with_limit(
        QuicBiStream::new(mobile_send, mobile_recv),
        QuicBiStream::new(agent_send, agent_recv),
        permit.limiter(),
    )
    .await?;
    permit.finish(outcome.uplink_bytes, outcome.downlink_bytes)?;
    Ok(())
}

async fn wait_for_agent_connection(
    connection_store: &RelayConnectionStore,
    session_id: &SessionId,
) -> Option<quinn::Connection> {
    for _ in 0..100 {
        if let Some(agent) = connection_store.agent(session_id) {
            return Some(agent);
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    None
}

fn relay_role(frame: &RelayBindFrame) -> RelayPeerRole {
    match frame.role {
        PeerRole::Mobile => RelayPeerRole::Mobile,
        PeerRole::Agent => RelayPeerRole::Agent,
    }
}

#[derive(Clone, Default)]
pub(crate) struct RelayConnectionStore {
    connections: Arc<RwLock<HashMap<SessionId, RelayConnections>>>,
}

impl RelayConnectionStore {
    fn set(&self, session_id: SessionId, role: RelayPeerRole, connection: quinn::Connection) {
        let mut connections = self
            .connections
            .write()
            .expect("relay connection lock poisoned");
        let entry = connections.entry(session_id).or_default();
        match role {
            RelayPeerRole::Mobile => entry.mobile = Some(connection),
            RelayPeerRole::Agent => entry.agent = Some(connection),
        }
    }

    fn agent(&self, session_id: &SessionId) -> Option<quinn::Connection> {
        self.connections
            .read()
            .expect("relay connection lock poisoned")
            .get(session_id)
            .and_then(|connections| connections.agent.clone())
    }

    pub(crate) fn close_session(&self, session_id: &SessionId) {
        let connections = self
            .connections
            .write()
            .expect("relay connection lock poisoned")
            .remove(session_id);
        if let Some(connections) = connections {
            if let Some(mobile) = connections.mobile {
                mobile.close(0_u32.into(), b"session closed");
            }
            if let Some(agent) = connections.agent {
                agent.close(0_u32.into(), b"session closed");
            }
        }
    }
}

#[derive(Default)]
struct RelayConnections {
    mobile: Option<quinn::Connection>,
    agent: Option<quinn::Connection>,
}
