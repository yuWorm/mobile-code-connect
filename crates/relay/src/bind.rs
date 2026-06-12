use mobilecode_connect_auth::{RelayTokenClaims, TokenError, TokenKey, TokenSigner};
use mobilecode_connect_protocol::SessionId;

use crate::session::RelayError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RelayBindRequest {
    pub role: RelayPeerRole,
    pub session_id: SessionId,
    pub token: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RelayPeerRole {
    Mobile,
    Agent,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RelayBindStatus {
    Waiting,
    Ready,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RelayPeer {
    pub role: RelayPeerRole,
}

impl RelayPeer {
    pub fn new(role: RelayPeerRole) -> Self {
        Self { role }
    }
}

pub trait RelayTokenVerifier: Send + Sync {
    fn verify(&self, token: &str) -> Result<RelayTokenClaims, RelayError>;
}

#[derive(Clone, Debug)]
pub struct SharedKeyRelayTokenVerifier {
    signer: TokenSigner,
    now_epoch_sec: u64,
}

impl SharedKeyRelayTokenVerifier {
    pub fn new(key: TokenKey, now_epoch_sec: u64) -> Self {
        Self {
            signer: TokenSigner::new(key),
            now_epoch_sec,
        }
    }
}

impl RelayTokenVerifier for SharedKeyRelayTokenVerifier {
    fn verify(&self, token: &str) -> Result<RelayTokenClaims, RelayError> {
        self.signer
            .verify_relay(token, self.now_epoch_sec)
            .map_err(RelayError::from)
    }
}

impl From<TokenError> for RelayError {
    fn from(source: TokenError) -> Self {
        RelayError::TokenInvalid { source }
    }
}
