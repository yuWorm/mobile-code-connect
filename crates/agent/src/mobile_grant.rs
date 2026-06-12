use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};

use mobilecode_connect_protocol::{
    derive_mobile_grant_secret, mobile_grant_certificate_fingerprint, ClientId, DeviceId,
    GrantSessionRequest, MobileGrantCredential, MobileInvitePayload, MobilePairingRequest,
    ServiceId,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreateMobileInviteRequest {
    pub control_url: String,
    pub device_id: mobilecode_connect_protocol::DeviceId,
    pub allowed_services: Vec<ServiceId>,
    pub ttl_sec: u64,
    pub max_uses: u32,
    pub agent_p2p_cert_fingerprint: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MobileGrantRecord {
    pub credential: MobileGrantCredential,
    pub enabled: bool,
    pub created_at: u64,
    pub last_used_at: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MobileInviteSummary {
    pub invite_id: String,
    pub device_id: DeviceId,
    pub allowed_services: Vec<ServiceId>,
    pub expires_at: u64,
    pub max_uses: u32,
    pub uses: u32,
    pub revoked: bool,
    pub agent_p2p_cert_fingerprint: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MobileGrantSummary {
    pub grant_id: String,
    pub device_id: DeviceId,
    pub client_id: ClientId,
    pub allowed_services: Vec<ServiceId>,
    pub enabled: bool,
    pub revocation_version: u64,
    pub created_at: u64,
    pub last_used_at: Option<u64>,
    pub agent_p2p_cert_fingerprint: Option<String>,
}

#[derive(Clone, Debug)]
pub struct MobileGrantManager {
    inner: Arc<RwLock<MobileGrantState>>,
    store_path: Option<Arc<PathBuf>>,
}

impl Default for MobileGrantManager {
    fn default() -> Self {
        Self {
            inner: Arc::new(RwLock::new(MobileGrantState::default())),
            store_path: None,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct MobileGrantState {
    #[serde(default)]
    invites: HashMap<String, MobileInviteRecord>,
    #[serde(default)]
    grants: HashMap<String, MobileGrantRecord>,
    #[serde(default)]
    pairing_approvals: HashMap<String, PairingApprovalRecord>,
    #[serde(default)]
    session_approvals: HashMap<String, SessionApprovalRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct MobileInviteRecord {
    payload: MobileInvitePayload,
    uses: u32,
    revoked: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct PairingApprovalRecord {
    request_fingerprint: String,
    grant_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct SessionApprovalRecord {
    request_fingerprint: String,
}

impl MobileGrantManager {
    pub fn load_or_create_file(path: impl Into<PathBuf>) -> Result<Self, MobileGrantManagerError> {
        let path = path.into();
        let state = match std::fs::read(&path) {
            Ok(body) => serde_json::from_slice(&body)
                .map_err(|error| MobileGrantManagerError::StoreJson(error.to_string()))?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                MobileGrantState::default()
            }
            Err(error) => return Err(MobileGrantManagerError::StoreIo(error.to_string())),
        };

        Ok(Self {
            inner: Arc::new(RwLock::new(state)),
            store_path: Some(Arc::new(path)),
        })
    }

    pub fn create_invite(
        &self,
        request: CreateMobileInviteRequest,
        now: u64,
    ) -> Result<MobileInvitePayload, MobileGrantManagerError> {
        if request.control_url.trim().is_empty()
            || request.device_id.as_str().trim().is_empty()
            || request.allowed_services.is_empty()
            || request
                .allowed_services
                .iter()
                .any(|service_id| service_id.as_str().trim().is_empty())
            || request.ttl_sec == 0
            || request.max_uses == 0
        {
            return Err(MobileGrantManagerError::InvalidInput);
        }

        let invite_id = format!("inv_{}", uuid::Uuid::new_v4().simple());
        let invite_secret = format!("inv_secret_{}", uuid::Uuid::new_v4().simple());
        let payload = MobileInvitePayload {
            version: 1,
            control_url: request.control_url,
            device_id: request.device_id,
            invite_id: invite_id.clone(),
            invite_secret,
            agent_p2p_cert_fingerprint: request.agent_p2p_cert_fingerprint,
            allowed_services: request.allowed_services,
            expires_at: now.saturating_add(request.ttl_sec),
            max_uses: request.max_uses,
        };
        self.mutate_state(|inner| {
            inner.invites.insert(
                invite_id,
                MobileInviteRecord {
                    payload: payload.clone(),
                    uses: 0,
                    revoked: false,
                },
            );
            Ok(())
        })?;
        Ok(payload)
    }

    pub fn approve_pairing(
        &self,
        request: &MobilePairingRequest,
        now: u64,
    ) -> Result<MobileGrantCredential, MobileGrantManagerError> {
        self.mutate_state(|inner| {
            let approval_key = pairing_approval_key(request);
            let request_fingerprint = request_fingerprint(request)?;
            if let Some(approval) = inner.pairing_approvals.get(&approval_key) {
                if approval.request_fingerprint != request_fingerprint {
                    return Err(MobileGrantManagerError::ReplayDetected);
                }
                return inner
                    .grants
                    .get(&approval.grant_id)
                    .map(|record| record.credential.clone())
                    .ok_or(MobileGrantManagerError::GrantNotFound);
            }

            let invite = inner
                .invites
                .get_mut(&request.invite_id)
                .ok_or(MobileGrantManagerError::InviteNotFound)?;
            if invite.payload.device_id != request.device_id {
                return Err(MobileGrantManagerError::ScopeDenied);
            }
            if invite.revoked {
                return Err(MobileGrantManagerError::InviteRevoked);
            }
            if invite.payload.expires_at <= now {
                return Err(MobileGrantManagerError::InviteExpired);
            }
            if invite.uses >= invite.payload.max_uses {
                return Err(MobileGrantManagerError::InviteConsumed);
            }
            if request.requested_services.is_empty()
                || request.requested_services.iter().any(|service_id| {
                    !invite
                        .payload
                        .allowed_services
                        .iter()
                        .any(|allowed| allowed == service_id)
                })
            {
                return Err(MobileGrantManagerError::ScopeDenied);
            }
            request
                .verify(&invite.payload.invite_secret)
                .map_err(|_| MobileGrantManagerError::InvalidProof)?;

            let grant_id = format!("gr_{}", uuid::Uuid::new_v4().simple());
            let grant_secret =
                derive_grant_secret(&invite.payload.invite_secret, &grant_id, &request.client_id)?;
            let credential = MobileGrantCredential {
                version: 1,
                control_url: invite.payload.control_url.clone(),
                device_id: request.device_id.clone(),
                grant_id: grant_id.clone(),
                client_id: request.client_id.clone(),
                allowed_services: request.requested_services.clone(),
                grant_secret,
                revocation_version: 1,
                agent_p2p_cert_fingerprint: invite.payload.agent_p2p_cert_fingerprint.clone(),
            };
            invite.uses = invite.uses.saturating_add(1);
            inner.grants.insert(
                grant_id,
                MobileGrantRecord {
                    credential: credential.clone(),
                    enabled: true,
                    created_at: now,
                    last_used_at: None,
                },
            );
            inner.pairing_approvals.insert(
                approval_key,
                PairingApprovalRecord {
                    request_fingerprint,
                    grant_id: credential.grant_id.clone(),
                },
            );
            Ok(credential)
        })
    }

    pub fn verify_session(
        &self,
        request: &GrantSessionRequest,
        now: u64,
    ) -> Result<MobileGrantCredential, MobileGrantManagerError> {
        self.mutate_state(|inner| {
            let approval_key = session_approval_key(request);
            let request_fingerprint = request_fingerprint(request)?;
            if let Some(approval) = inner.session_approvals.get(&approval_key) {
                if approval.request_fingerprint != request_fingerprint {
                    return Err(MobileGrantManagerError::ReplayDetected);
                }
            }

            let record = inner
                .grants
                .get_mut(&request.grant_id)
                .ok_or(MobileGrantManagerError::GrantNotFound)?;
            if !record.enabled {
                return Err(MobileGrantManagerError::GrantRevoked);
            }
            let credential = record.credential.clone();
            if credential.device_id != request.device_id
                || credential.client_id != request.client_id
            {
                return Err(MobileGrantManagerError::ScopeDenied);
            }
            if credential.revocation_version != request.revocation_version {
                return Err(MobileGrantManagerError::GrantVersionMismatch);
            }
            if !credential
                .allowed_services
                .iter()
                .any(|service_id| service_id == &request.service_id)
            {
                return Err(MobileGrantManagerError::ScopeDenied);
            }
            request
                .verify(&credential.grant_secret)
                .map_err(|_| MobileGrantManagerError::InvalidProof)?;
            record.last_used_at = Some(now);
            inner.session_approvals.insert(
                approval_key,
                SessionApprovalRecord {
                    request_fingerprint,
                },
            );
            Ok(credential)
        })
    }

    pub fn revoke_invite(&self, invite_id: &str) -> Result<(), MobileGrantManagerError> {
        self.mutate_state(|inner| {
            let invite = inner
                .invites
                .get_mut(invite_id)
                .ok_or(MobileGrantManagerError::InviteNotFound)?;
            invite.revoked = true;
            Ok(())
        })
    }

    pub fn revoke_grant(&self, grant_id: &str) -> Result<(), MobileGrantManagerError> {
        self.mutate_state(|inner| {
            let record = inner
                .grants
                .get_mut(grant_id)
                .ok_or(MobileGrantManagerError::GrantNotFound)?;
            record.enabled = false;
            record.credential.revocation_version =
                record.credential.revocation_version.saturating_add(1);
            Ok(())
        })
    }

    pub fn grant(&self, grant_id: &str) -> Option<MobileGrantRecord> {
        self.inner
            .read()
            .expect("mobile grant manager lock poisoned")
            .grants
            .get(grant_id)
            .cloned()
    }

    pub fn list_invites(&self) -> Vec<MobileInviteSummary> {
        let mut invites: Vec<_> = self
            .inner
            .read()
            .expect("mobile grant manager lock poisoned")
            .invites
            .values()
            .map(|record| MobileInviteSummary {
                invite_id: record.payload.invite_id.clone(),
                device_id: record.payload.device_id.clone(),
                allowed_services: record.payload.allowed_services.clone(),
                expires_at: record.payload.expires_at,
                max_uses: record.payload.max_uses,
                uses: record.uses,
                revoked: record.revoked,
                agent_p2p_cert_fingerprint: record.payload.agent_p2p_cert_fingerprint.clone(),
            })
            .collect();
        invites.sort_by(|left, right| left.invite_id.cmp(&right.invite_id));
        invites
    }

    pub fn list_grants(&self) -> Vec<MobileGrantSummary> {
        let mut grants: Vec<_> = self
            .inner
            .read()
            .expect("mobile grant manager lock poisoned")
            .grants
            .values()
            .map(|record| MobileGrantSummary {
                grant_id: record.credential.grant_id.clone(),
                device_id: record.credential.device_id.clone(),
                client_id: record.credential.client_id.clone(),
                allowed_services: record.credential.allowed_services.clone(),
                enabled: record.enabled,
                revocation_version: record.credential.revocation_version,
                created_at: record.created_at,
                last_used_at: record.last_used_at,
                agent_p2p_cert_fingerprint: record.credential.agent_p2p_cert_fingerprint.clone(),
            })
            .collect();
        grants.sort_by(|left, right| left.grant_id.cmp(&right.grant_id));
        grants
    }

    fn mutate_state<T>(
        &self,
        mutate: impl FnOnce(&mut MobileGrantState) -> Result<T, MobileGrantManagerError>,
    ) -> Result<T, MobileGrantManagerError> {
        let mut guard = self
            .inner
            .write()
            .expect("mobile grant manager lock poisoned");
        let mut next = guard.clone();
        let result = mutate(&mut next)?;
        self.persist_state(&next)?;
        *guard = next;
        Ok(result)
    }

    fn persist_state(&self, state: &MobileGrantState) -> Result<(), MobileGrantManagerError> {
        let Some(path) = &self.store_path else {
            return Ok(());
        };
        write_state_file(path, state)
    }
}

fn write_state_file(path: &Path, state: &MobileGrantState) -> Result<(), MobileGrantManagerError> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)
            .map_err(|error| MobileGrantManagerError::StoreIo(error.to_string()))?;
    }
    let body = serde_json::to_vec_pretty(state)
        .map_err(|error| MobileGrantManagerError::StoreJson(error.to_string()))?;
    std::fs::write(path, body)
        .map_err(|error| MobileGrantManagerError::StoreIo(error.to_string()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
            .map_err(|error| MobileGrantManagerError::StoreIo(error.to_string()))?;
    }

    Ok(())
}

fn derive_grant_secret(
    invite_secret: &str,
    grant_id: &str,
    client_id: &mobilecode_connect_protocol::ClientId,
) -> Result<String, MobileGrantManagerError> {
    derive_mobile_grant_secret(invite_secret, grant_id, client_id)
        .map_err(|_| MobileGrantManagerError::InvalidProof)
}

fn pairing_approval_key(request: &MobilePairingRequest) -> String {
    format!(
        "{}\n{}\n{}",
        request.invite_id,
        request.client_id.as_str(),
        request.nonce
    )
}

fn session_approval_key(request: &GrantSessionRequest) -> String {
    format!(
        "{}\n{}\n{}",
        request.grant_id,
        request.client_id.as_str(),
        request.nonce
    )
}

fn request_fingerprint<T>(request: &T) -> Result<String, MobileGrantManagerError>
where
    T: Serialize,
{
    let body = serde_json::to_vec(request)
        .map_err(|error| MobileGrantManagerError::StoreJson(error.to_string()))?;
    Ok(mobile_grant_certificate_fingerprint(body))
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum MobileGrantManagerError {
    #[error("mobile grant input is invalid")]
    InvalidInput,
    #[error("mobile invite was not found")]
    InviteNotFound,
    #[error("mobile invite expired")]
    InviteExpired,
    #[error("mobile invite was revoked")]
    InviteRevoked,
    #[error("mobile invite is already consumed")]
    InviteConsumed,
    #[error("mobile grant scope denied")]
    ScopeDenied,
    #[error("mobile grant proof is invalid")]
    InvalidProof,
    #[error("mobile grant was not found")]
    GrantNotFound,
    #[error("mobile grant was revoked")]
    GrantRevoked,
    #[error("mobile grant revocation version is stale")]
    GrantVersionMismatch,
    #[error("mobile grant replay detected")]
    ReplayDetected,
    #[error("mobile grant store io failed: {0}")]
    StoreIo(String),
    #[error("mobile grant store json failed: {0}")]
    StoreJson(String),
}
