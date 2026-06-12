use mobilecode_connect_protocol::{DeviceId, ServiceId, ServiceProtocol};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub device_id: DeviceId,
    pub control_server: String,
    pub auth_token: String,
    pub services: Vec<ServiceConfig>,
    pub p2p_certificate_der: Option<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceConfig {
    pub service_id: ServiceId,
    pub name: String,
    pub protocol: ServiceProtocol,
    pub target_host: String,
    pub target_port: u16,
}

impl ServiceConfig {
    pub fn target_addr(&self) -> String {
        format!("{}:{}", self.target_host, self.target_port)
    }
}
