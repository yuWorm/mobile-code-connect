use mobilecode_connect_auth::{
    AccessTokenClaims, ControlRole, ControlTokenClaims, RelayTokenClaims, TokenError, TokenKey,
    TokenSigner,
};
use mobilecode_connect_protocol::{ClientId, DeviceId, RelayLimits, ServiceId, SessionId, UserId};

#[derive(Debug, Clone)]
pub struct TokenIssuer {
    signer: TokenSigner,
}

impl TokenIssuer {
    pub fn new(secret: impl AsRef<[u8]>) -> Self {
        Self {
            signer: TokenSigner::new(TokenKey::new(secret)),
        }
    }

    pub fn issue_access_token(
        &self,
        session_id: SessionId,
        subject: impl Into<String>,
        exp: u64,
    ) -> Result<String, TokenError> {
        self.signer.sign_access(&AccessTokenClaims {
            session_id,
            subject: subject.into(),
            exp,
        })
    }

    pub fn issue_control_token(
        &self,
        user_id: UserId,
        subject: impl Into<String>,
        role: ControlRole,
        exp: u64,
    ) -> Result<String, TokenError> {
        self.signer.sign_control(&ControlTokenClaims {
            user_id,
            subject: subject.into(),
            role,
            exp,
            relay_token_version: None,
            credential_id: None,
            server_credential_version: None,
        })
    }

    pub fn issue_relay_control_token(
        &self,
        user_id: UserId,
        relay_id: impl Into<String>,
        token_version: u64,
        exp: u64,
    ) -> Result<String, TokenError> {
        self.signer.sign_control(&ControlTokenClaims {
            user_id,
            subject: relay_id.into(),
            role: ControlRole::Relay,
            exp,
            relay_token_version: Some(token_version),
            credential_id: None,
            server_credential_version: None,
        })
    }

    pub fn issue_agent_control_token(
        &self,
        user_id: UserId,
        credential_id: impl Into<String>,
        token_version: u64,
        exp: u64,
    ) -> Result<String, TokenError> {
        let credential_id = credential_id.into();
        self.signer.sign_control(&ControlTokenClaims {
            user_id,
            subject: credential_id.clone(),
            role: ControlRole::Agent,
            exp,
            relay_token_version: None,
            credential_id: Some(credential_id),
            server_credential_version: Some(token_version),
        })
    }

    pub fn issue_relay_token(
        &self,
        user_id: UserId,
        session_id: SessionId,
        client_id: ClientId,
        device_id: DeviceId,
        service_id: ServiceId,
        limits: RelayLimits,
        exp: u64,
    ) -> Result<String, TokenError> {
        self.signer.sign_relay(&RelayTokenClaims {
            session_id,
            user_id,
            client_id,
            device_id,
            service_id,
            max_bps: limits.max_bps,
            max_streams: limits.max_streams,
            max_duration_sec: limits.max_duration_sec,
            traffic_quota_bytes: limits.traffic_quota_bytes,
            exp,
        })
    }
}
