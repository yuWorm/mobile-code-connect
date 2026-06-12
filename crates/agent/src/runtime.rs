#[derive(Debug)]
pub struct Agent;

use std::{
    collections::HashMap,
    future::Future,
    net::SocketAddr,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use mobilecode_connect_control_client::{
    AgentSessionAssignment, ApproveMobilePairingRequest, ControlClientError,
    DenyMobileGrantRequest, HttpControlClient,
};
use mobilecode_connect_protocol::{
    ClientId, Device, DeviceId, DeviceStatus, PeerRole, Service, ServiceId, SessionId, UserId,
};
use mobilecode_connect_punch::probe::{establish_p2p_path, P2pPathConfig, P2pPathError};
use mobilecode_connect_tunnel::quic::P2pQuicIdentity;
use rustls::pki_types::CertificateDer;
use tokio::{sync::oneshot, task::JoinHandle};

use crate::{
    config::{AgentConfig, ServiceConfig},
    mobile_grant::MobileGrantManager,
    p2p_client::{AgentP2pError, P2pAgentClient},
    relay_client::{AgentRelayError, RelayAgentClient, RelayAgentConfig},
    service_registry::ServiceRegistry,
};

impl Agent {
    pub async fn register_with_control(config: AgentConfig) -> Result<(), AgentError> {
        let client = HttpControlClient::with_optional_bearer_token(
            &config.control_server,
            &config.auth_token,
        )?;
        client.register_device(device_from_config(&config)).await?;
        if let Some(certificate_der) = config.p2p_certificate_der.clone() {
            client
                .register_p2p_certificate(&config.device_id, certificate_der)
                .await?;
        }
        client
            .register_services(services_from_config(&config.device_id, config.services))
            .await?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct AgentControlRuntimeConfig {
    pub control_server_url: String,
    pub auth_token: String,
    pub device_id: DeviceId,
    pub relay_server_cert: CertificateDer<'static>,
    pub registry: ServiceRegistry,
    pub poll_interval: Duration,
    pub p2p: Option<AgentP2pRuntimeConfig>,
    pub mobile_grants: Option<MobileGrantManager>,
}

#[derive(Debug, Clone)]
pub struct AgentP2pRuntimeConfig {
    pub bind_addr: SocketAddr,
    pub candidate_timeout: Duration,
    pub probe_timeout: Duration,
    pub interval: Duration,
    pub server_identity: Option<P2pQuicIdentity>,
}

pub struct AgentControlRuntime {
    control: HttpControlClient,
    device_id: DeviceId,
    relay_server_cert: CertificateDer<'static>,
    registry: ServiceRegistry,
    poll_interval: Duration,
    p2p: Option<AgentP2pRuntimeConfig>,
    mobile_grants: Option<MobileGrantManager>,
    active: HashMap<SessionId, ActiveAgentSession>,
}

struct ActiveAgentSession {
    shutdown_tx: oneshot::Sender<()>,
    task: JoinHandle<()>,
    grant_metadata: Option<ActiveMobileGrantMetadata>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ActiveMobileGrantMetadata {
    device_id: DeviceId,
    client_id: ClientId,
    service_id: ServiceId,
    grant_id: String,
    revocation_version: u64,
}

impl AgentControlRuntime {
    pub fn new(config: AgentControlRuntimeConfig) -> Result<Self, AgentError> {
        Ok(Self {
            control: HttpControlClient::with_optional_bearer_token(
                config.control_server_url,
                &config.auth_token,
            )?,
            device_id: config.device_id,
            relay_server_cert: config.relay_server_cert,
            registry: config.registry,
            poll_interval: config.poll_interval,
            p2p: config.p2p,
            mobile_grants: config.mobile_grants,
            active: HashMap::new(),
        })
    }

    pub async fn poll_once(&mut self) -> Result<Vec<SessionId>, AgentError> {
        self.active.retain(|_, active| !active.task.is_finished());
        self.close_revoked_mobile_grant_sessions().await;
        let assignments = self.control.list_agent_sessions(&self.device_id).await?;
        let mut started = Vec::new();

        for assignment in assignments {
            if self.active.contains_key(&assignment.session_id) {
                continue;
            }

            let claimed = match self
                .control
                .claim_agent_session(&assignment.session_id)
                .await
            {
                Ok(claimed) => claimed,
                Err(ControlClientError::HttpStatus { status_code, .. })
                    if status_code.as_u16() == 409 =>
                {
                    continue;
                }
                Err(error) => return Err(error.into()),
            };

            let session_id = claimed.session_id.clone();
            if self.validate_grant_assignment(&claimed).is_err() {
                let _ = self.control.close_session(&session_id).await;
                continue;
            }
            let active = self.start_claimed_session(claimed).await?;
            self.control.mark_agent_session_bound(&session_id).await?;
            self.active.insert(session_id.clone(), active);
            started.push(session_id);
        }

        self.process_mobile_grant_requests().await?;

        Ok(started)
    }

    async fn close_revoked_mobile_grant_sessions(&mut self) {
        let session_ids = revoked_mobile_grant_session_ids(
            self.active
                .iter()
                .map(|(session_id, active)| (session_id, active.grant_metadata.as_ref())),
            self.mobile_grants.as_ref(),
        );

        for session_id in session_ids {
            if let Some(active) = self.active.remove(&session_id) {
                let _ = self.control.close_session(&session_id).await;
                let _ = active.shutdown_tx.send(());
                let _ = active.task.await;
            }
        }
    }

    async fn process_mobile_grant_requests(&self) -> Result<(), AgentError> {
        let Some(grants) = &self.mobile_grants else {
            return Ok(());
        };

        for pending in self
            .control
            .list_mobile_pairing_requests(&self.device_id)
            .await?
        {
            match grants.approve_pairing(&pending.request, current_epoch_sec()) {
                Ok(grant) => {
                    self.control
                        .approve_mobile_pairing(
                            &pending.pending_pairing_id,
                            ApproveMobilePairingRequest {
                                grant_id: grant.grant_id,
                                allowed_services: grant.allowed_services,
                                revocation_version: grant.revocation_version,
                            },
                        )
                        .await?;
                }
                Err(error) => {
                    self.control
                        .deny_mobile_pairing(
                            &pending.pending_pairing_id,
                            DenyMobileGrantRequest {
                                reason: Some(error.to_string()),
                            },
                        )
                        .await?;
                }
            }
        }

        for pending in self
            .control
            .list_grant_session_requests(&self.device_id)
            .await?
        {
            match grants.verify_session(&pending.request, current_epoch_sec()) {
                Ok(_) => {
                    self.control
                        .approve_grant_session(&pending.pending_session_id)
                        .await?;
                }
                Err(error) => {
                    self.control
                        .deny_grant_session(
                            &pending.pending_session_id,
                            DenyMobileGrantRequest {
                                reason: Some(error.to_string()),
                            },
                        )
                        .await?;
                }
            }
        }

        Ok(())
    }

    fn validate_grant_assignment(
        &self,
        claimed: &AgentSessionAssignment,
    ) -> Result<(), AgentError> {
        let (Some(grant_id), Some(revocation_version), Some(grant_service_id)) = (
            claimed.grant_id.as_deref(),
            claimed.grant_revocation_version,
            claimed.grant_service_id.as_ref(),
        ) else {
            if claimed.grant_id.is_none()
                && claimed.grant_revocation_version.is_none()
                && claimed.grant_service_id.is_none()
            {
                return Ok(());
            }
            return Err(AgentError::MobileGrantRejected {
                reason: "incomplete grant metadata".to_string(),
            });
        };

        let grants =
            self.mobile_grants
                .as_ref()
                .ok_or_else(|| AgentError::MobileGrantRejected {
                    reason: "grant manager unavailable".to_string(),
                })?;
        let record = grants
            .grant(grant_id)
            .ok_or_else(|| AgentError::MobileGrantRejected {
                reason: "grant not found".to_string(),
            })?;
        if !record.enabled {
            return Err(AgentError::MobileGrantRejected {
                reason: "grant revoked".to_string(),
            });
        }
        if record.credential.device_id != claimed.device_id
            || record.credential.client_id != claimed.client_id
            || record.credential.revocation_version != revocation_version
            || grant_service_id != &claimed.service_id
            || !record
                .credential
                .allowed_services
                .iter()
                .any(|service_id| service_id == grant_service_id)
        {
            return Err(AgentError::MobileGrantRejected {
                reason: "grant metadata mismatch".to_string(),
            });
        }
        Ok(())
    }

    async fn start_claimed_session(
        &self,
        claimed: AgentSessionAssignment,
    ) -> Result<ActiveAgentSession, AgentError> {
        let grant_metadata = active_mobile_grant_metadata_from_assignment(&claimed);
        let mut active = if let Some(p2p) = &self.p2p {
            match self.start_p2p_session(claimed.clone(), p2p.clone()).await {
                Ok(active) => Ok(active),
                Err(_) => self.start_relay_session(claimed).await,
            }
        } else {
            self.start_relay_session(claimed).await
        }?;
        active.grant_metadata = grant_metadata;
        Ok(active)
    }

    async fn start_relay_session(
        &self,
        claimed: AgentSessionAssignment,
    ) -> Result<ActiveAgentSession, AgentError> {
        let relay_addr = claimed.relay_addr.parse::<SocketAddr>().map_err(|_| {
            AgentError::InvalidRelayAddress {
                value: claimed.relay_addr.clone(),
            }
        })?;
        let agent = RelayAgentClient::connect(RelayAgentConfig {
            relay_addr,
            server_cert: self.relay_server_cert.clone(),
            session_id: claimed.session_id,
            token: claimed.relay_token,
            registry: self.registry.clone(),
        })
        .await?;
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let task = tokio::spawn(async move {
            let _ = agent
                .run_until(async {
                    let _ = shutdown_rx.await;
                })
                .await;
        });

        Ok(ActiveAgentSession {
            shutdown_tx,
            task,
            grant_metadata: None,
        })
    }

    async fn start_p2p_session(
        &self,
        claimed: AgentSessionAssignment,
        config: AgentP2pRuntimeConfig,
    ) -> Result<ActiveAgentSession, AgentError> {
        let punch_addr = claimed.punch_addr.parse::<SocketAddr>().map_err(|_| {
            AgentError::InvalidPunchAddress {
                value: claimed.punch_addr.clone(),
            }
        })?;
        let path = establish_p2p_path(P2pPathConfig {
            session_id: claimed.session_id,
            role: PeerRole::Agent,
            self_id: self.device_id.to_string(),
            peer_id: claimed.client_id.to_string(),
            bind_addr: config.bind_addr,
            punch_addr,
            shared_secret: claimed.relay_token,
            candidate_timeout: config.candidate_timeout,
            probe_timeout: config.probe_timeout,
            interval: config.interval,
        })
        .await?;
        let agent = if let Some(identity) = config.server_identity {
            P2pAgentClient::from_path_with_identity(path, self.registry.clone(), identity).await?
        } else {
            P2pAgentClient::from_path(path, self.registry.clone()).await?
        };
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let task = tokio::spawn(async move {
            let _ = agent
                .run_until(async {
                    let _ = shutdown_rx.await;
                })
                .await;
        });

        Ok(ActiveAgentSession {
            shutdown_tx,
            task,
            grant_metadata: None,
        })
    }

    pub async fn run_until<F>(mut self, shutdown: F) -> Result<(), AgentError>
    where
        F: Future<Output = ()> + Send,
    {
        let mut interval = tokio::time::interval(self.poll_interval);
        tokio::pin!(shutdown);

        loop {
            tokio::select! {
                _ = &mut shutdown => {
                    self.shutdown().await;
                    return Ok(());
                }
                _ = interval.tick() => {
                    self.poll_once().await?;
                }
            }
        }
    }

    pub async fn shutdown(&mut self) {
        let active = std::mem::take(&mut self.active);
        for (session_id, session) in active {
            let _ = self.control.close_session(&session_id).await;
            let _ = session.shutdown_tx.send(());
            let _ = session.task.await;
        }
    }
}

fn device_from_config(config: &AgentConfig) -> Device {
    Device {
        device_id: config.device_id.clone(),
        user_id: UserId::new("user_001"),
        name: config.device_id.to_string(),
        status: DeviceStatus::Online,
        agent_version: env!("CARGO_PKG_VERSION").to_string(),
    }
}

fn services_from_config(
    device_id: &mobilecode_connect_protocol::DeviceId,
    services: Vec<ServiceConfig>,
) -> Vec<Service> {
    services
        .into_iter()
        .map(|service| Service {
            service_id: service.service_id,
            device_id: device_id.clone(),
            name: service.name,
            protocol: service.protocol,
            target_host: service.target_host,
            target_port: service.target_port,
        })
        .collect()
}

fn current_epoch_sec() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

fn active_mobile_grant_metadata_from_assignment(
    assignment: &AgentSessionAssignment,
) -> Option<ActiveMobileGrantMetadata> {
    Some(ActiveMobileGrantMetadata {
        device_id: assignment.device_id.clone(),
        client_id: assignment.client_id.clone(),
        service_id: assignment.grant_service_id.as_ref()?.clone(),
        grant_id: assignment.grant_id.as_ref()?.clone(),
        revocation_version: assignment.grant_revocation_version?,
    })
}

fn active_mobile_grant_metadata_is_valid(
    grants: Option<&MobileGrantManager>,
    metadata: &ActiveMobileGrantMetadata,
) -> bool {
    let Some(record) = grants.and_then(|grants| grants.grant(&metadata.grant_id)) else {
        return false;
    };

    record.enabled
        && record.credential.device_id == metadata.device_id
        && record.credential.client_id == metadata.client_id
        && record.credential.revocation_version == metadata.revocation_version
        && record
            .credential
            .allowed_services
            .iter()
            .any(|service_id| service_id == &metadata.service_id)
}

fn revoked_mobile_grant_session_ids<'a, I>(
    sessions: I,
    grants: Option<&MobileGrantManager>,
) -> Vec<SessionId>
where
    I: IntoIterator<Item = (&'a SessionId, Option<&'a ActiveMobileGrantMetadata>)>,
{
    sessions
        .into_iter()
        .filter_map(|(session_id, metadata)| {
            let metadata = metadata?;
            (!active_mobile_grant_metadata_is_valid(grants, metadata)).then(|| session_id.clone())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mobile_grant::CreateMobileInviteRequest;
    use mobilecode_connect_protocol::MobilePairingRequest;

    #[test]
    fn active_mobile_grant_metadata_is_invalid_after_local_revoke() {
        let grants = MobileGrantManager::default();
        let device_id = DeviceId::new("pc_active_revoke");
        let service_id = ServiceId::new("svc_web_3000");
        let client_id = ClientId::new("mobile_001");
        let invite = grants
            .create_invite(
                CreateMobileInviteRequest {
                    control_url: "http://127.0.0.1".to_string(),
                    device_id: device_id.clone(),
                    allowed_services: vec![service_id.clone()],
                    ttl_sec: 4_102_444_800,
                    max_uses: 1,
                    agent_p2p_cert_fingerprint: None,
                },
                1_000,
            )
            .unwrap();
        let proof = MobilePairingRequest::proof_for(
            device_id.clone(),
            invite.invite_id.clone(),
            client_id.clone(),
            vec![service_id.clone()],
            "pairing-nonce".to_string(),
            &invite.invite_secret,
        )
        .unwrap();
        let grant = grants
            .approve_pairing(
                &MobilePairingRequest {
                    device_id: device_id.clone(),
                    invite_id: invite.invite_id,
                    client_id: client_id.clone(),
                    requested_services: vec![service_id.clone()],
                    nonce: "pairing-nonce".to_string(),
                    proof,
                },
                1_001,
            )
            .unwrap();
        let metadata = ActiveMobileGrantMetadata {
            device_id,
            client_id,
            service_id,
            grant_id: grant.grant_id.clone(),
            revocation_version: grant.revocation_version,
        };

        assert!(active_mobile_grant_metadata_is_valid(
            Some(&grants),
            &metadata
        ));

        grants.revoke_grant(&grant.grant_id).unwrap();

        assert!(!active_mobile_grant_metadata_is_valid(
            Some(&grants),
            &metadata
        ));
    }

    #[test]
    fn revoked_mobile_grant_session_ids_ignore_regular_sessions() {
        let grants = MobileGrantManager::default();
        let metadata = ActiveMobileGrantMetadata {
            device_id: DeviceId::new("pc_active_revoke"),
            client_id: ClientId::new("mobile_001"),
            service_id: ServiceId::new("svc_web_3000"),
            grant_id: "missing-grant".to_string(),
            revocation_version: 1,
        };

        let revoked = revoked_mobile_grant_session_ids(
            [
                (&SessionId::new("sess_regular"), None),
                (&SessionId::new("sess_grant"), Some(&metadata)),
            ],
            Some(&grants),
        );

        assert_eq!(revoked, vec![SessionId::new("sess_grant")]);
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error("control client failed: {0}")]
    Control(#[from] ControlClientError),
    #[error("relay client failed: {0}")]
    Relay(#[from] AgentRelayError),
    #[error("agent p2p failed: {0}")]
    P2p(#[from] AgentP2pError),
    #[error("punch path failed: {0}")]
    P2pPath(#[from] P2pPathError),
    #[error("invalid relay address from control: {value}")]
    InvalidRelayAddress { value: String },
    #[error("invalid punch address from control: {value}")]
    InvalidPunchAddress { value: String },
    #[error("mobile grant assignment rejected: {reason}")]
    MobileGrantRejected { reason: String },
}
