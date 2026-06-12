use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use hmac::{Hmac, Mac};
use quic_tunnel_protocol::{ClientId, DeviceId, ServiceId, SessionId, UserId};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelayTokenClaims {
    pub session_id: SessionId,
    pub user_id: UserId,
    pub client_id: ClientId,
    pub device_id: DeviceId,
    pub service_id: ServiceId,
    pub max_bps: u64,
    pub max_streams: u32,
    pub max_duration_sec: u64,
    pub traffic_quota_bytes: u64,
    pub exp: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccessTokenClaims {
    pub session_id: SessionId,
    pub subject: String,
    pub exp: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlRole {
    User,
    Admin,
    Relay,
    Agent,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControlTokenClaims {
    pub user_id: UserId,
    pub subject: String,
    pub role: ControlRole,
    pub exp: u64,
    #[serde(default)]
    pub relay_token_version: Option<u64>,
    #[serde(default)]
    pub credential_id: Option<String>,
    #[serde(default)]
    pub server_credential_version: Option<u64>,
}

#[derive(Clone, Debug)]
pub struct TokenKey(Vec<u8>);

impl TokenKey {
    pub fn new(secret: impl AsRef<[u8]>) -> Self {
        Self(secret.as_ref().to_vec())
    }
}

#[derive(Clone, Debug)]
pub struct TokenSigner {
    key: TokenKey,
}

impl TokenSigner {
    pub fn new(key: TokenKey) -> Self {
        Self { key }
    }

    pub fn sign_relay(&self, claims: &RelayTokenClaims) -> Result<String, TokenError> {
        self.sign(claims)
    }

    pub fn sign_access(&self, claims: &AccessTokenClaims) -> Result<String, TokenError> {
        self.sign(claims)
    }

    pub fn sign_control(&self, claims: &ControlTokenClaims) -> Result<String, TokenError> {
        self.sign(claims)
    }

    pub fn verify_relay(
        &self,
        token: &str,
        now_epoch_sec: u64,
    ) -> Result<RelayTokenClaims, TokenError> {
        let claims: RelayTokenClaims = self.verify(token)?;
        if claims.exp <= now_epoch_sec {
            return Err(TokenError::Expired);
        }
        Ok(claims)
    }

    pub fn verify_control(
        &self,
        token: &str,
        now_epoch_sec: u64,
    ) -> Result<ControlTokenClaims, TokenError> {
        let claims: ControlTokenClaims = self.verify(token)?;
        if claims.exp <= now_epoch_sec {
            return Err(TokenError::Expired);
        }
        Ok(claims)
    }

    fn sign<T>(&self, claims: &T) -> Result<String, TokenError>
    where
        T: Serialize,
    {
        let payload = serde_json::to_vec(claims)?;
        let encoded_payload = URL_SAFE_NO_PAD.encode(payload);
        let signature = self.signature(encoded_payload.as_bytes())?;
        let encoded_signature = URL_SAFE_NO_PAD.encode(signature);

        Ok(format!("{encoded_payload}.{encoded_signature}"))
    }

    fn verify<T>(&self, token: &str) -> Result<T, TokenError>
    where
        T: for<'de> Deserialize<'de>,
    {
        let (encoded_payload, encoded_signature) =
            token.split_once('.').ok_or(TokenError::InvalidFormat)?;

        if encoded_payload.is_empty() || encoded_signature.is_empty() {
            return Err(TokenError::InvalidFormat);
        }

        let signature = URL_SAFE_NO_PAD
            .decode(encoded_signature)
            .map_err(|_| TokenError::InvalidFormat)?;
        let mut mac =
            HmacSha256::new_from_slice(&self.key.0).map_err(|_| TokenError::InvalidKey)?;
        mac.update(encoded_payload.as_bytes());
        mac.verify_slice(&signature)
            .map_err(|_| TokenError::InvalidSignature)?;

        let payload = URL_SAFE_NO_PAD
            .decode(encoded_payload)
            .map_err(|_| TokenError::InvalidFormat)?;
        Ok(serde_json::from_slice(&payload)?)
    }

    fn signature(&self, payload: &[u8]) -> Result<Vec<u8>, TokenError> {
        let mut mac =
            HmacSha256::new_from_slice(&self.key.0).map_err(|_| TokenError::InvalidKey)?;
        mac.update(payload);
        Ok(mac.finalize().into_bytes().to_vec())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TokenError {
    #[error("token format is invalid")]
    InvalidFormat,
    #[error("token key is invalid")]
    InvalidKey,
    #[error("token signature is invalid")]
    InvalidSignature,
    #[error("token is expired")]
    Expired,
    #[error("token json failed: {0}")]
    Json(#[from] serde_json::Error),
}
