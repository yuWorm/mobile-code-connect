use std::net::SocketAddr;

#[derive(Debug, Clone)]
pub struct PunchConfig {
    pub bind: SocketAddr,
}
