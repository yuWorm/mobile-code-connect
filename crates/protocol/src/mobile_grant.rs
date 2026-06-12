use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{ClientId, DeviceId, ServiceId};

type HmacSha256 = Hmac<Sha256>;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MobileInvitePayload {
    pub version: u32,
    pub control_url: String,
    pub device_id: DeviceId,
    pub invite_id: String,
    pub invite_secret: String,
    pub agent_p2p_cert_fingerprint: Option<String>,
    pub allowed_services: Vec<ServiceId>,
    pub expires_at: u64,
    pub max_uses: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MobilePairingRequest {
    pub device_id: DeviceId,
    pub invite_id: String,
    pub client_id: ClientId,
    pub requested_services: Vec<ServiceId>,
    pub nonce: String,
    pub proof: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MobileGrantCredential {
    pub version: u32,
    pub control_url: String,
    pub device_id: DeviceId,
    pub grant_id: String,
    pub client_id: ClientId,
    pub allowed_services: Vec<ServiceId>,
    pub grant_secret: String,
    pub revocation_version: u64,
    #[serde(default)]
    pub agent_p2p_cert_fingerprint: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrantSessionRequest {
    pub client_id: ClientId,
    pub device_id: DeviceId,
    pub service_id: ServiceId,
    pub grant_id: String,
    pub revocation_version: u64,
    pub nonce: String,
    pub proof: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PendingPairingStatus {
    Pending,
    Approved,
    Denied,
    Expired,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PendingGrantSessionStatus {
    Pending,
    Approved,
    Denied,
    Expired,
}

impl MobilePairingRequest {
    pub fn unsigned_payload(&self) -> MobilePairingUnsignedPayload {
        MobilePairingUnsignedPayload {
            device_id: self.device_id.clone(),
            invite_id: self.invite_id.clone(),
            client_id: self.client_id.clone(),
            requested_services: self.requested_services.clone(),
            nonce: self.nonce.clone(),
        }
    }

    pub fn proof_for(
        device_id: DeviceId,
        invite_id: String,
        client_id: ClientId,
        requested_services: Vec<ServiceId>,
        nonce: String,
        secret: impl AsRef<[u8]>,
    ) -> Result<String, MobileGrantError> {
        sign_hmac_proof(
            &MobilePairingUnsignedPayload {
                device_id,
                invite_id,
                client_id,
                requested_services,
                nonce,
            },
            secret,
        )
    }

    pub fn verify(&self, secret: impl AsRef<[u8]>) -> Result<(), MobileGrantError> {
        verify_hmac_proof(&self.unsigned_payload(), secret, &self.proof)
    }
}

impl GrantSessionRequest {
    pub fn unsigned_payload(&self) -> GrantSessionUnsignedPayload {
        GrantSessionUnsignedPayload {
            client_id: self.client_id.clone(),
            device_id: self.device_id.clone(),
            service_id: self.service_id.clone(),
            grant_id: self.grant_id.clone(),
            revocation_version: self.revocation_version,
            nonce: self.nonce.clone(),
        }
    }

    pub fn proof_for(
        client_id: ClientId,
        device_id: DeviceId,
        service_id: ServiceId,
        grant_id: String,
        revocation_version: u64,
        nonce: String,
        secret: impl AsRef<[u8]>,
    ) -> Result<String, MobileGrantError> {
        sign_hmac_proof(
            &GrantSessionUnsignedPayload {
                client_id,
                device_id,
                service_id,
                grant_id,
                revocation_version,
                nonce,
            },
            secret,
        )
    }

    pub fn verify(&self, secret: impl AsRef<[u8]>) -> Result<(), MobileGrantError> {
        verify_hmac_proof(&self.unsigned_payload(), secret, &self.proof)
    }
}

impl MobileGrantCredential {
    pub fn allows(&self, service_id: &ServiceId, revocation_version: u64) -> bool {
        self.revocation_version == revocation_version
            && self
                .allowed_services
                .iter()
                .any(|allowed| allowed == service_id)
    }

    pub fn sign_session_request(
        &self,
        service_id: ServiceId,
        nonce: String,
    ) -> Result<GrantSessionRequest, MobileGrantError> {
        let proof = GrantSessionRequest::proof_for(
            self.client_id.clone(),
            self.device_id.clone(),
            service_id.clone(),
            self.grant_id.clone(),
            self.revocation_version,
            nonce.clone(),
            &self.grant_secret,
        )?;
        Ok(GrantSessionRequest {
            client_id: self.client_id.clone(),
            device_id: self.device_id.clone(),
            service_id,
            grant_id: self.grant_id.clone(),
            revocation_version: self.revocation_version,
            nonce,
            proof,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct MobilePairingUnsignedPayload {
    pub device_id: DeviceId,
    pub invite_id: String,
    pub client_id: ClientId,
    pub requested_services: Vec<ServiceId>,
    pub nonce: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct GrantSessionUnsignedPayload {
    pub client_id: ClientId,
    pub device_id: DeviceId,
    pub service_id: ServiceId,
    pub grant_id: String,
    pub revocation_version: u64,
    pub nonce: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct MobileGrantSecretDerivationPayload {
    pub purpose: &'static str,
    pub grant_id: String,
    pub client_id: ClientId,
}

pub fn derive_mobile_grant_secret(
    invite_secret: impl AsRef<[u8]>,
    grant_id: impl Into<String>,
    client_id: &ClientId,
) -> Result<String, MobileGrantError> {
    sign_hmac_proof(
        &MobileGrantSecretDerivationPayload {
            purpose: "qtunnel-mobile-grant-secret-v1",
            grant_id: grant_id.into(),
            client_id: client_id.clone(),
        },
        invite_secret,
    )
}

pub fn mobile_grant_certificate_fingerprint(cert_der: impl AsRef<[u8]>) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(cert_der.as_ref()))
}

pub fn sign_hmac_proof<T>(value: &T, secret: impl AsRef<[u8]>) -> Result<String, MobileGrantError>
where
    T: Serialize,
{
    let payload = serde_json::to_vec(value)?;
    let mut mac =
        HmacSha256::new_from_slice(secret.as_ref()).map_err(|_| MobileGrantError::InvalidKey)?;
    mac.update(&payload);
    Ok(URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes()))
}

pub fn verify_hmac_proof<T>(
    value: &T,
    secret: impl AsRef<[u8]>,
    proof: &str,
) -> Result<(), MobileGrantError>
where
    T: Serialize,
{
    let expected = sign_hmac_proof(value, secret)?;
    if expected == proof {
        Ok(())
    } else {
        Err(MobileGrantError::InvalidProof)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum MobileGrantError {
    #[error("mobile grant key is invalid")]
    InvalidKey,
    #[error("mobile grant proof is invalid")]
    InvalidProof,
    #[error("mobile grant json failed: {0}")]
    Json(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pairing_proof_is_stable_and_secret_dependent() {
        let proof = MobilePairingRequest::proof_for(
            DeviceId::new("pc_001"),
            "inv_001".to_string(),
            ClientId::new("mobile_001"),
            vec![ServiceId::new("web")],
            "nonce".to_string(),
            "secret-a",
        )
        .unwrap();
        let request = MobilePairingRequest {
            device_id: DeviceId::new("pc_001"),
            invite_id: "inv_001".to_string(),
            client_id: ClientId::new("mobile_001"),
            requested_services: vec![ServiceId::new("web")],
            nonce: "nonce".to_string(),
            proof: proof.clone(),
        };

        assert!(request.verify("secret-a").is_ok());
        assert!(request.verify("secret-b").is_err());
        assert_eq!(
            proof,
            MobilePairingRequest::proof_for(
                DeviceId::new("pc_001"),
                "inv_001".to_string(),
                ClientId::new("mobile_001"),
                vec![ServiceId::new("web")],
                "nonce".to_string(),
                "secret-a",
            )
            .unwrap()
        );
    }

    #[test]
    fn grant_allows_only_scoped_services_and_matching_version() {
        let grant = MobileGrantCredential {
            version: 1,
            control_url: "https://control.example.test".to_string(),
            device_id: DeviceId::new("pc_001"),
            grant_id: "gr_001".to_string(),
            client_id: ClientId::new("mobile_001"),
            allowed_services: vec![ServiceId::new("web")],
            grant_secret: "grant-secret".to_string(),
            revocation_version: 1,
            agent_p2p_cert_fingerprint: None,
        };

        assert!(grant.allows(&ServiceId::new("web"), 1));
        assert!(!grant.allows(&ServiceId::new("ssh"), 1));
        assert!(!grant.allows(&ServiceId::new("web"), 2));
    }

    #[test]
    fn grant_session_request_uses_grant_secret_proof() {
        let grant = MobileGrantCredential {
            version: 1,
            control_url: "https://control.example.test".to_string(),
            device_id: DeviceId::new("pc_001"),
            grant_id: "gr_001".to_string(),
            client_id: ClientId::new("mobile_001"),
            allowed_services: vec![ServiceId::new("web")],
            grant_secret: "grant-secret".to_string(),
            revocation_version: 1,
            agent_p2p_cert_fingerprint: None,
        };

        let request = grant
            .sign_session_request(ServiceId::new("web"), "nonce".to_string())
            .unwrap();

        assert!(request.verify("grant-secret").is_ok());
        assert!(request.verify("other-secret").is_err());
        assert_eq!(request.service_id, ServiceId::new("web"));
    }

    #[test]
    fn mobile_grant_secret_derivation_is_stable_and_invite_secret_dependent() {
        let secret =
            derive_mobile_grant_secret("invite-secret-a", "gr_001", &ClientId::new("mobile_001"))
                .unwrap();

        assert_eq!(
            secret,
            derive_mobile_grant_secret("invite-secret-a", "gr_001", &ClientId::new("mobile_001"),)
                .unwrap()
        );
        assert_ne!(
            secret,
            derive_mobile_grant_secret("invite-secret-b", "gr_001", &ClientId::new("mobile_001"),)
                .unwrap()
        );
    }

    #[test]
    fn mobile_grant_certificate_fingerprint_is_stable() {
        assert_eq!(
            mobile_grant_certificate_fingerprint(b"agent-cert-der"),
            "rkDEvNT8Ati6xNe6Ckk2hzZYxOtGkbLpYkOPv0qyL8o"
        );
    }

    #[test]
    fn mobile_grant_credential_deserializes_without_fingerprint() {
        let grant: MobileGrantCredential = serde_json::from_str(
            r#"{
                "version": 1,
                "control_url": "https://control.example.test",
                "device_id": "pc_001",
                "grant_id": "gr_001",
                "client_id": "mobile_001",
                "allowed_services": ["web"],
                "grant_secret": "grant-secret",
                "revocation_version": 1
            }"#,
        )
        .unwrap();

        assert_eq!(grant.agent_p2p_cert_fingerprint, None);
    }
}
