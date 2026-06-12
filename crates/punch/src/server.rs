use std::{future::Future, net::SocketAddr};

use tokio::net::UdpSocket;

use crate::{
    candidate::CandidateStore,
    probe::{PunchPacket, PunchPacketError},
};

pub const PUNCH_BUFFER_SIZE: usize = 64 * 1024;

#[derive(Debug)]
pub struct PunchServer {
    socket: UdpSocket,
    store: CandidateStore,
}

impl PunchServer {
    pub async fn bind(bind_addr: SocketAddr) -> Result<Self, PunchError> {
        Ok(Self {
            socket: UdpSocket::bind(bind_addr).await?,
            store: CandidateStore::default(),
        })
    }

    pub fn local_addr(&self) -> Result<SocketAddr, PunchError> {
        Ok(self.socket.local_addr()?)
    }

    pub fn store(&self) -> CandidateStore {
        self.store.clone()
    }

    pub async fn run_until<F>(self, shutdown: F) -> Result<(), PunchError>
    where
        F: Future<Output = ()> + Send,
    {
        let mut buf = [0_u8; PUNCH_BUFFER_SIZE];
        tokio::pin!(shutdown);

        loop {
            tokio::select! {
                _ = &mut shutdown => return Ok(()),
                received = self.socket.recv_from(&mut buf) => {
                    let (len, peer_addr) = received?;
                    self.handle_datagram(&buf[..len], peer_addr).await?;
                }
            }
        }
    }

    async fn handle_datagram(&self, bytes: &[u8], peer_addr: SocketAddr) -> Result<(), PunchError> {
        let response = match PunchPacket::decode(bytes) {
            Ok(PunchPacket::Hello(hello)) => {
                self.store.record_observed(
                    hello.session_id.clone(),
                    hello.role,
                    hello.peer_id,
                    peer_addr,
                );
                PunchPacket::Candidates {
                    session_id: hello.session_id.clone(),
                    candidates: self.store.list(&hello.session_id),
                }
            }
            Ok(_) => PunchPacket::Error {
                message: "unexpected punch packet".to_string(),
            },
            Err(error) => PunchPacket::Error {
                message: error.to_string(),
            },
        };

        self.socket.send_to(&response.encode()?, peer_addr).await?;
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PunchError {
    #[error("udp failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("packet failed: {0}")]
    Packet(#[from] PunchPacketError),
}
