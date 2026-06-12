use mobilecode_connect_control_client::HttpControlClientOptions;
use mobilecode_connect_protocol::ClientId;

use crate::client::TunnelError;

#[derive(Debug, Clone)]
pub struct TunnelConfig {
    pub user_token: String,
    pub control_server_url: String,
    pub client_id: ClientId,
    pub control_client_options: HttpControlClientOptions,
}

impl TunnelConfig {
    pub fn validate(&self) -> Result<(), TunnelError> {
        if self.user_token.trim().is_empty() {
            return Err(TunnelError::InvalidConfig {
                reason: "user_token is required".to_string(),
            });
        }

        if self.control_server_url.trim().is_empty() {
            return Err(TunnelError::InvalidConfig {
                reason: "control_server_url is required".to_string(),
            });
        }

        Ok(())
    }
}
