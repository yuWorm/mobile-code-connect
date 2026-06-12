use std::{
    sync::{Arc, RwLock},
    time::Duration,
};

use quic_tunnel_control_client::{ControlClientError, HttpControlClient, HttpControlClientOptions};
use quic_tunnel_protocol::{
    derive_mobile_grant_secret, ClientId, DeviceId, MobileGrantCredential, MobileGrantError,
    MobileInvitePayload, MobilePairingRequest, PendingPairingStatus, ServiceId,
};
use rustls::pki_types::CertificateDer;
use tokio::runtime::{Builder, Runtime};

use crate::{
    browser_proxy::{
        browser_proxy_host, classify_browser_proxy_url, BrowserProxy, BrowserProxyConfig,
        BrowserProxyDirectFallbackPolicy, BrowserProxyHandle, BrowserProxyStats,
        BrowserProxyStatsHandle, BrowserProxyTarget, BrowserProxyUrlClassification,
        BrowserProxyUrlKind,
    },
    client::{ControlP2pOrRelayClientConfig, OpenServiceRequest, TunnelClient},
    config::TunnelConfig,
    forward::LocalForwardHandle,
    path::TunnelPath,
    status::{TunnelState, TunnelStatus, TunnelTransportStats},
};

#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiMobileTunnelConfig {
    pub user_token: String,
    pub control_server_url: String,
    pub client_id: String,
    pub control_request_timeout_ms: Option<u64>,
    pub control_max_retries: u32,
    pub control_retry_backoff_ms: u64,
}

impl FfiMobileTunnelConfig {
    pub fn new(user_token: String, control_server_url: String, client_id: String) -> Self {
        Self {
            user_token,
            control_server_url,
            client_id,
            control_request_timeout_ms: None,
            control_max_retries: 0,
            control_retry_backoff_ms: 0,
        }
    }

    pub fn with_control_request_timeout_ms(mut self, timeout_ms: Option<u64>) -> Self {
        self.control_request_timeout_ms = timeout_ms;
        self
    }

    pub fn with_control_max_retries(mut self, max_retries: u32) -> Self {
        self.control_max_retries = max_retries;
        self
    }

    pub fn with_control_retry_backoff_ms(mut self, retry_backoff_ms: u64) -> Self {
        self.control_retry_backoff_ms = retry_backoff_ms;
        self
    }

    pub(crate) fn into_tunnel_config(self) -> Result<TunnelConfig, FfiMobileError> {
        let mut control_client_options = HttpControlClientOptions::default()
            .with_max_retries(self.control_max_retries)
            .with_retry_backoff(Duration::from_millis(self.control_retry_backoff_ms));
        if let Some(timeout_ms) = self.control_request_timeout_ms {
            control_client_options =
                control_client_options.with_request_timeout(Duration::from_millis(timeout_ms));
        }

        Ok(TunnelConfig {
            user_token: self.user_token,
            control_server_url: self.control_server_url,
            client_id: ClientId::new(self.client_id),
            control_client_options,
        })
    }
}

#[uniffi::export]
pub fn mobile_tunnel_config(
    user_token: String,
    control_server_url: String,
    client_id: String,
) -> FfiMobileTunnelConfig {
    FfiMobileTunnelConfig::new(user_token, control_server_url, client_id)
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiP2pOrRelayConfig {
    pub relay_server_cert_der: Vec<u8>,
    pub bind_addr: String,
    pub candidate_timeout_ms: u64,
    pub probe_timeout_ms: u64,
    pub interval_ms: u64,
    pub relay_fallback_delay_ms: u64,
}

impl FfiP2pOrRelayConfig {
    pub fn with_defaults(relay_server_cert_der: Vec<u8>) -> Self {
        Self {
            relay_server_cert_der,
            bind_addr: "0.0.0.0:0".to_string(),
            candidate_timeout_ms: 1500,
            probe_timeout_ms: 1500,
            interval_ms: 25,
            relay_fallback_delay_ms: 300,
        }
    }

    fn into_client_config(self) -> Result<ControlP2pOrRelayClientConfig, FfiMobileError> {
        Ok(ControlP2pOrRelayClientConfig {
            relay_server_cert: CertificateDer::from(self.relay_server_cert_der),
            bind_addr: self
                .bind_addr
                .parse()
                .map_err(|error| FfiMobileError::InvalidConfig {
                    reason: format!("invalid bind_addr: {error}"),
                })?,
            candidate_timeout: Duration::from_millis(self.candidate_timeout_ms),
            probe_timeout: Duration::from_millis(self.probe_timeout_ms),
            interval: Duration::from_millis(self.interval_ms),
            relay_fallback_delay: Duration::from_millis(self.relay_fallback_delay_ms),
        })
    }
}

#[uniffi::export]
pub fn p2p_or_relay_config_with_defaults(relay_server_cert_der: Vec<u8>) -> FfiP2pOrRelayConfig {
    FfiP2pOrRelayConfig::with_defaults(relay_server_cert_der)
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FfiMobileGrantPairingOptions {
    pub control_request_timeout_ms: Option<u64>,
    pub control_max_retries: u32,
    pub control_retry_backoff_ms: u64,
}

impl Default for FfiMobileGrantPairingOptions {
    fn default() -> Self {
        Self {
            control_request_timeout_ms: Some(5_000),
            control_max_retries: 2,
            control_retry_backoff_ms: 100,
        }
    }
}

impl FfiMobileGrantPairingOptions {
    fn into_control_options(self) -> HttpControlClientOptions {
        let mut options = HttpControlClientOptions::default()
            .with_max_retries(self.control_max_retries)
            .with_retry_backoff(Duration::from_millis(self.control_retry_backoff_ms));
        if let Some(timeout_ms) = self.control_request_timeout_ms {
            options = options.with_request_timeout(Duration::from_millis(timeout_ms));
        }
        options
    }
}

#[uniffi::export]
pub fn mobile_grant_pairing_options_with_defaults() -> FfiMobileGrantPairingOptions {
    FfiMobileGrantPairingOptions::default()
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FfiMobileInvitePayload {
    pub version: u32,
    pub control_url: String,
    pub device_id: String,
    pub invite_id: String,
    pub invite_secret: String,
    pub agent_p2p_cert_fingerprint: Option<String>,
    pub allowed_services: Vec<String>,
    pub expires_at: u64,
    pub max_uses: u32,
}

impl FfiMobileInvitePayload {
    fn into_mobile_invite(self) -> MobileInvitePayload {
        MobileInvitePayload {
            version: self.version,
            control_url: self.control_url,
            device_id: DeviceId::new(self.device_id),
            invite_id: self.invite_id,
            invite_secret: self.invite_secret,
            agent_p2p_cert_fingerprint: self.agent_p2p_cert_fingerprint,
            allowed_services: self
                .allowed_services
                .into_iter()
                .map(ServiceId::new)
                .collect(),
            expires_at: self.expires_at,
            max_uses: self.max_uses,
        }
    }
}

impl From<MobileInvitePayload> for FfiMobileInvitePayload {
    fn from(invite: MobileInvitePayload) -> Self {
        Self {
            version: invite.version,
            control_url: invite.control_url,
            device_id: invite.device_id.as_str().to_string(),
            invite_id: invite.invite_id,
            invite_secret: invite.invite_secret,
            agent_p2p_cert_fingerprint: invite.agent_p2p_cert_fingerprint,
            allowed_services: invite
                .allowed_services
                .into_iter()
                .map(|service_id| service_id.as_str().to_string())
                .collect(),
            expires_at: invite.expires_at,
            max_uses: invite.max_uses,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FfiMobileGrantPairingSession {
    pub pending_pairing_id: String,
    pub poll_interval_ms: u64,
    pub expires_at: u64,
    pub invite: FfiMobileInvitePayload,
    pub client_id: String,
    pub requested_services: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum FfiMobileGrantPairingStatus {
    Pending,
    Approved,
    Denied,
    Expired,
}

impl From<PendingPairingStatus> for FfiMobileGrantPairingStatus {
    fn from(status: PendingPairingStatus) -> Self {
        match status {
            PendingPairingStatus::Pending => Self::Pending,
            PendingPairingStatus::Approved => Self::Approved,
            PendingPairingStatus::Denied => Self::Denied,
            PendingPairingStatus::Expired => Self::Expired,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FfiMobileGrantPairingPollResult {
    pub status: FfiMobileGrantPairingStatus,
    pub grant: Option<FfiMobileGrantCredential>,
    pub denied_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FfiMobileGrantCredential {
    pub version: u32,
    pub control_url: String,
    pub device_id: String,
    pub grant_id: String,
    pub client_id: String,
    pub allowed_services: Vec<String>,
    pub grant_secret: String,
    pub revocation_version: u64,
    pub agent_p2p_cert_fingerprint: Option<String>,
}

impl From<FfiMobileGrantCredential> for MobileGrantCredential {
    fn from(grant: FfiMobileGrantCredential) -> Self {
        Self {
            version: grant.version,
            control_url: grant.control_url,
            device_id: DeviceId::new(grant.device_id),
            grant_id: grant.grant_id,
            client_id: ClientId::new(grant.client_id),
            allowed_services: grant
                .allowed_services
                .into_iter()
                .map(ServiceId::new)
                .collect(),
            grant_secret: grant.grant_secret,
            revocation_version: grant.revocation_version,
            agent_p2p_cert_fingerprint: grant.agent_p2p_cert_fingerprint,
        }
    }
}

impl From<MobileGrantCredential> for FfiMobileGrantCredential {
    fn from(grant: MobileGrantCredential) -> Self {
        Self {
            version: grant.version,
            control_url: grant.control_url,
            device_id: grant.device_id.as_str().to_string(),
            grant_id: grant.grant_id,
            client_id: grant.client_id.as_str().to_string(),
            allowed_services: grant
                .allowed_services
                .into_iter()
                .map(|service_id| service_id.as_str().to_string())
                .collect(),
            grant_secret: grant.grant_secret,
            revocation_version: grant.revocation_version,
            agent_p2p_cert_fingerprint: grant.agent_p2p_cert_fingerprint,
        }
    }
}

#[uniffi::export]
pub fn mobile_grant_credential_to_json(
    grant: FfiMobileGrantCredential,
) -> Result<String, FfiMobileError> {
    serde_json::to_string(&MobileGrantCredential::from(grant)).map_err(|error| {
        FfiMobileError::InvalidConfig {
            reason: format!("mobile grant credential json serialization failed: {error}"),
        }
    })
}

#[uniffi::export]
pub fn mobile_grant_credential_from_json(
    json: String,
) -> Result<FfiMobileGrantCredential, FfiMobileError> {
    serde_json::from_str::<MobileGrantCredential>(&json)
        .map(FfiMobileGrantCredential::from)
        .map_err(|error| FfiMobileError::InvalidConfig {
            reason: format!("mobile grant credential json deserialization failed: {error}"),
        })
}

#[uniffi::export]
pub fn start_mobile_grant_pairing(
    invite: FfiMobileInvitePayload,
    client_id: String,
    requested_services: Vec<String>,
    nonce: String,
    options: FfiMobileGrantPairingOptions,
) -> Result<FfiMobileGrantPairingSession, FfiMobileError> {
    validate_mobile_grant_pairing_input(&invite, &client_id, &requested_services, &nonce)?;
    let invite_internal = invite.clone().into_mobile_invite();
    let client_id_value = client_id;
    let client_id = ClientId::new(client_id_value.clone());
    let requested_service_ids: Vec<_> = requested_services
        .iter()
        .cloned()
        .map(ServiceId::new)
        .collect();
    let proof = MobilePairingRequest::proof_for(
        invite_internal.device_id.clone(),
        invite_internal.invite_id.clone(),
        client_id.clone(),
        requested_service_ids.clone(),
        nonce.clone(),
        &invite_internal.invite_secret,
    )?;
    let request = MobilePairingRequest {
        device_id: invite_internal.device_id,
        invite_id: invite_internal.invite_id,
        client_id,
        requested_services: requested_service_ids,
        nonce,
        proof,
    };
    let runtime = mobile_runtime()?;
    let started = runtime.block_on(async {
        let control = HttpControlClient::with_options(
            &invite_internal.control_url,
            options.into_control_options(),
        )?;
        control.start_mobile_pairing(request).await
    })?;
    Ok(FfiMobileGrantPairingSession {
        pending_pairing_id: started.pending_pairing_id,
        poll_interval_ms: started.poll_interval_ms,
        expires_at: started.expires_at,
        invite,
        client_id: client_id_value,
        requested_services,
    })
}

#[uniffi::export]
pub fn poll_mobile_grant_pairing_once(
    pairing: FfiMobileGrantPairingSession,
    options: FfiMobileGrantPairingOptions,
) -> Result<FfiMobileGrantPairingPollResult, FfiMobileError> {
    validate_mobile_grant_pairing_session(&pairing)?;
    let invite = pairing.invite.clone().into_mobile_invite();
    let runtime = mobile_runtime()?;
    let result = runtime.block_on(async {
        let control =
            HttpControlClient::with_options(&invite.control_url, options.into_control_options())?;
        control
            .mobile_pairing_result(&pairing.pending_pairing_id)
            .await
    })?;
    let status = FfiMobileGrantPairingStatus::from(result.status.clone());
    let grant = match result.status {
        PendingPairingStatus::Approved => {
            let metadata = result.grant.ok_or_else(|| FfiMobileError::Tunnel {
                reason: "mobile grant pairing was approved without grant metadata".to_string(),
            })?;
            let client_id = ClientId::new(pairing.client_id.clone());
            let requested_services: Vec<_> = pairing
                .requested_services
                .iter()
                .cloned()
                .map(ServiceId::new)
                .collect();
            if metadata.device_id != invite.device_id
                || metadata.client_id != client_id
                || metadata.allowed_services.iter().any(|service_id| {
                    !requested_services
                        .iter()
                        .any(|requested| requested == service_id)
                })
            {
                return Err(FfiMobileError::InvalidConfig {
                    reason: "approved mobile grant metadata does not match pairing request"
                        .to_string(),
                });
            }
            let grant_secret = derive_mobile_grant_secret(
                &invite.invite_secret,
                metadata.grant_id.clone(),
                &metadata.client_id,
            )?;
            Some(
                MobileGrantCredential {
                    version: metadata.version,
                    control_url: invite.control_url,
                    device_id: metadata.device_id,
                    grant_id: metadata.grant_id,
                    client_id: metadata.client_id,
                    allowed_services: metadata.allowed_services,
                    grant_secret,
                    revocation_version: metadata.revocation_version,
                    agent_p2p_cert_fingerprint: invite.agent_p2p_cert_fingerprint,
                }
                .into(),
            )
        }
        _ => None,
    };
    Ok(FfiMobileGrantPairingPollResult {
        status,
        grant,
        denied_reason: result.denied_reason,
    })
}

fn validate_mobile_grant_pairing_input(
    invite: &FfiMobileInvitePayload,
    client_id: &str,
    requested_services: &[String],
    nonce: &str,
) -> Result<(), FfiMobileError> {
    if invite.control_url.trim().is_empty()
        || invite.device_id.trim().is_empty()
        || invite.invite_id.trim().is_empty()
        || invite.invite_secret.trim().is_empty()
        || client_id.trim().is_empty()
        || nonce.trim().is_empty()
        || requested_services.is_empty()
        || requested_services
            .iter()
            .any(|service_id| service_id.trim().is_empty())
    {
        return Err(FfiMobileError::InvalidConfig {
            reason: "mobile grant pairing input is incomplete".to_string(),
        });
    }
    if requested_services.iter().any(|service_id| {
        !invite
            .allowed_services
            .iter()
            .any(|allowed| allowed == service_id)
    }) {
        return Err(FfiMobileError::InvalidConfig {
            reason: "mobile grant pairing requested service is outside invite scope".to_string(),
        });
    }
    Ok(())
}

fn validate_mobile_grant_pairing_session(
    pairing: &FfiMobileGrantPairingSession,
) -> Result<(), FfiMobileError> {
    if pairing.pending_pairing_id.trim().is_empty()
        || pairing.client_id.trim().is_empty()
        || pairing.requested_services.is_empty()
    {
        return Err(FfiMobileError::InvalidConfig {
            reason: "mobile grant pairing session is incomplete".to_string(),
        });
    }
    validate_mobile_grant_pairing_input(
        &pairing.invite,
        &pairing.client_id,
        &pairing.requested_services,
        "poll",
    )
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiOpenServiceRequest {
    pub device_id: String,
    pub service_id: String,
    pub local_port: u16,
}

impl From<FfiOpenServiceRequest> for OpenServiceRequest {
    fn from(request: FfiOpenServiceRequest) -> Self {
        Self {
            device_id: DeviceId::new(request.device_id),
            service_id: ServiceId::new(request.service_id),
            local_port: request.local_port,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FfiForwardHandle {
    pub handle_id: String,
    pub device_id: String,
    pub service_id: String,
    pub local_port: u16,
}

impl From<LocalForwardHandle> for FfiForwardHandle {
    fn from(handle: LocalForwardHandle) -> Self {
        Self {
            handle_id: handle.handle_id().to_string(),
            device_id: handle.device_id().as_str().to_string(),
            service_id: handle.service_id().as_str().to_string(),
            local_port: handle.local_port(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FfiBrowserProxyConfig {
    pub bind_host: String,
    pub local_port: u16,
    pub domain_suffix: String,
    pub max_connections: u64,
    pub direct_fallback_policy: FfiBrowserProxyDirectFallbackPolicy,
    pub request_head_timeout_ms: u64,
    pub direct_connect_timeout_ms: u64,
    pub tunnel_open_timeout_ms: u64,
    pub idle_timeout_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FfiBrowserProxyStats {
    pub accepted_connections: u64,
    pub active_connections: u64,
    pub tunnel_connections: u64,
    pub direct_connections: u64,
    pub forbidden_direct_connections: u64,
    pub tunnel_bytes_to_remote: u64,
    pub tunnel_bytes_to_browser: u64,
    pub direct_bytes_to_remote: u64,
    pub direct_bytes_to_browser: u64,
    pub request_head_timeouts: u64,
    pub tunnel_open_timeouts: u64,
    pub idle_timeout_closures: u64,
    pub direct_connect_failures: u64,
    pub connection_limit_rejections: u64,
    pub request_errors: u64,
}

impl From<BrowserProxyStats> for FfiBrowserProxyStats {
    fn from(stats: BrowserProxyStats) -> Self {
        Self {
            accepted_connections: stats.accepted_connections,
            active_connections: stats.active_connections,
            tunnel_connections: stats.tunnel_connections,
            direct_connections: stats.direct_connections,
            forbidden_direct_connections: stats.forbidden_direct_connections,
            tunnel_bytes_to_remote: stats.tunnel_bytes_to_remote,
            tunnel_bytes_to_browser: stats.tunnel_bytes_to_browser,
            direct_bytes_to_remote: stats.direct_bytes_to_remote,
            direct_bytes_to_browser: stats.direct_bytes_to_browser,
            request_head_timeouts: stats.request_head_timeouts,
            tunnel_open_timeouts: stats.tunnel_open_timeouts,
            idle_timeout_closures: stats.idle_timeout_closures,
            direct_connect_failures: stats.direct_connect_failures,
            connection_limit_rejections: stats.connection_limit_rejections,
            request_errors: stats.request_errors,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum FfiBrowserProxyDirectFallbackPolicy {
    AllowAll,
    LocalNetworkAndDomain,
    Disabled,
}

impl From<FfiBrowserProxyDirectFallbackPolicy> for BrowserProxyDirectFallbackPolicy {
    fn from(policy: FfiBrowserProxyDirectFallbackPolicy) -> Self {
        match policy {
            FfiBrowserProxyDirectFallbackPolicy::AllowAll => Self::AllowAll,
            FfiBrowserProxyDirectFallbackPolicy::LocalNetworkAndDomain => {
                Self::LocalNetworkAndDomain
            }
            FfiBrowserProxyDirectFallbackPolicy::Disabled => Self::Disabled,
        }
    }
}

impl Default for FfiBrowserProxyConfig {
    fn default() -> Self {
        Self {
            bind_host: "127.0.0.1".to_string(),
            local_port: 0,
            domain_suffix: ".qtunnel.local".to_string(),
            max_connections: 256,
            direct_fallback_policy: FfiBrowserProxyDirectFallbackPolicy::LocalNetworkAndDomain,
            request_head_timeout_ms: 10_000,
            direct_connect_timeout_ms: 10_000,
            tunnel_open_timeout_ms: 15_000,
            idle_timeout_ms: 120_000,
        }
    }
}

impl FfiBrowserProxyConfig {
    fn into_browser_proxy_config(self) -> Result<BrowserProxyConfig, FfiMobileError> {
        Ok(BrowserProxyConfig {
            bind_host: self.bind_host,
            local_port: self.local_port,
            domain_suffix: self.domain_suffix,
            max_connections: usize::try_from(self.max_connections).map_err(|_| {
                FfiMobileError::InvalidConfig {
                    reason: "max_connections is too large for this platform".to_string(),
                }
            })?,
            direct_fallback_policy: self.direct_fallback_policy.into(),
            request_head_timeout: Duration::from_millis(self.request_head_timeout_ms),
            direct_connect_timeout: Duration::from_millis(self.direct_connect_timeout_ms),
            tunnel_open_timeout: Duration::from_millis(self.tunnel_open_timeout_ms),
            idle_timeout: Duration::from_millis(self.idle_timeout_ms),
        })
    }
}

#[uniffi::export]
pub fn browser_proxy_config_with_defaults() -> FfiBrowserProxyConfig {
    FfiBrowserProxyConfig::default()
}

#[uniffi::export]
pub fn browser_proxy_host_with_suffix(
    device_id: String,
    service_id: String,
    domain_suffix: String,
) -> Result<String, FfiMobileError> {
    browser_proxy_host(
        &BrowserProxyTarget {
            device_id: DeviceId::new(device_id),
            service_id: ServiceId::new(service_id),
        },
        &domain_suffix,
    )
    .map_err(FfiMobileError::from)
}

#[uniffi::export]
pub fn browser_proxy_host_for_service(
    device_id: String,
    service_id: String,
) -> Result<String, FfiMobileError> {
    browser_proxy_host_with_suffix(device_id, service_id, ".qtunnel.local".to_string())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum FfiBrowserProxyRouteKind {
    DeviceService,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FfiBrowserProxyRoute {
    pub kind: FfiBrowserProxyRouteKind,
    pub device_id: String,
    pub service_id: String,
    pub scheme: String,
    pub host: String,
}

impl FfiBrowserProxyRoute {
    pub fn origin(&self) -> String {
        format!("{}://{}", self.scheme, self.host)
    }

    pub fn http_url(&self, path_and_query: String) -> String {
        format!("{}{}", self.origin(), normalized_url_path(&path_and_query))
    }
}

#[uniffi::export]
pub fn browser_proxy_route_origin(route: FfiBrowserProxyRoute) -> String {
    route.origin()
}

#[uniffi::export]
pub fn browser_proxy_route_http_url(route: FfiBrowserProxyRoute, path_and_query: String) -> String {
    route.http_url(path_and_query)
}

#[uniffi::export]
pub fn browser_proxy_device_service_route(
    device_id: String,
    service_id: String,
) -> Result<FfiBrowserProxyRoute, FfiMobileError> {
    browser_proxy_device_service_route_with_suffix(
        device_id,
        service_id,
        ".qtunnel.local".to_string(),
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum FfiBrowserProxyUrlKind {
    DeviceService,
    ControlServer,
    DirectNetwork,
}

impl From<BrowserProxyUrlKind> for FfiBrowserProxyUrlKind {
    fn from(kind: BrowserProxyUrlKind) -> Self {
        match kind {
            BrowserProxyUrlKind::DeviceService => Self::DeviceService,
            BrowserProxyUrlKind::ControlServer => Self::ControlServer,
            BrowserProxyUrlKind::DirectNetwork => Self::DirectNetwork,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FfiBrowserProxyUrlClassification {
    pub kind: FfiBrowserProxyUrlKind,
    pub host: String,
    pub device_id: Option<String>,
    pub service_id: Option<String>,
}

impl From<BrowserProxyUrlClassification> for FfiBrowserProxyUrlClassification {
    fn from(classification: BrowserProxyUrlClassification) -> Self {
        let (device_id, service_id) = classification
            .target
            .map(|target| {
                (
                    Some(target.device_id.as_str().to_string()),
                    Some(target.service_id.as_str().to_string()),
                )
            })
            .unwrap_or((None, None));
        Self {
            kind: classification.kind.into(),
            host: classification.host,
            device_id,
            service_id,
        }
    }
}

#[uniffi::export]
pub fn browser_proxy_classify_url(
    url: String,
    control_server_url: String,
    domain_suffix: String,
) -> Result<FfiBrowserProxyUrlClassification, FfiMobileError> {
    classify_browser_proxy_url(&url, &control_server_url, &domain_suffix)
        .map(FfiBrowserProxyUrlClassification::from)
        .map_err(FfiMobileError::from)
}

#[uniffi::export]
pub fn browser_proxy_classify_url_with_defaults(
    url: String,
    control_server_url: String,
) -> Result<FfiBrowserProxyUrlClassification, FfiMobileError> {
    browser_proxy_classify_url(url, control_server_url, ".qtunnel.local".to_string())
}

#[uniffi::export]
pub fn browser_proxy_device_service_route_with_suffix(
    device_id: String,
    service_id: String,
    domain_suffix: String,
) -> Result<FfiBrowserProxyRoute, FfiMobileError> {
    let host = browser_proxy_host_for_service_with_suffix(&device_id, &service_id, &domain_suffix)?;
    Ok(FfiBrowserProxyRoute {
        kind: FfiBrowserProxyRouteKind::DeviceService,
        device_id,
        service_id,
        scheme: "http".to_string(),
        host,
    })
}

fn browser_proxy_host_for_service_with_suffix(
    device_id: &str,
    service_id: &str,
    domain_suffix: &str,
) -> Result<String, FfiMobileError> {
    browser_proxy_host(
        &BrowserProxyTarget {
            device_id: DeviceId::new(device_id),
            service_id: ServiceId::new(service_id),
        },
        domain_suffix,
    )
    .map_err(FfiMobileError::from)
}

fn normalized_url_path(path_and_query: &str) -> String {
    if path_and_query.is_empty() {
        return "/".to_string();
    }
    if path_and_query.starts_with('/') {
        path_and_query.to_string()
    } else {
        format!("/{path_and_query}")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum FfiTunnelPath {
    Relay,
    P2p,
}

impl From<TunnelPath> for FfiTunnelPath {
    fn from(path: TunnelPath) -> Self {
        match path {
            TunnelPath::Relay => Self::Relay,
            TunnelPath::P2p => Self::P2p,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum FfiTunnelState {
    Started,
    Connected,
    Closed,
}

impl From<TunnelState> for FfiTunnelState {
    fn from(state: TunnelState) -> Self {
        match state {
            TunnelState::Started => Self::Started,
            TunnelState::Connected => Self::Connected,
            TunnelState::Closed => Self::Closed,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FfiTunnelStatus {
    pub state: FfiTunnelState,
    pub path: FfiTunnelPath,
    pub rtt_ms: Option<u64>,
    pub uplink_bytes: u64,
    pub downlink_bytes: u64,
    pub active_forwards: u64,
    pub transport: FfiTunnelTransportStats,
}

impl From<TunnelStatus> for FfiTunnelStatus {
    fn from(status: TunnelStatus) -> Self {
        Self {
            state: status.state.into(),
            path: status.path.into(),
            rtt_ms: status.rtt_ms,
            uplink_bytes: status.uplink_bytes,
            downlink_bytes: status.downlink_bytes,
            active_forwards: status.active_forwards as u64,
            transport: status.transport.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FfiTunnelTransportStats {
    pub p2p_attempts: u64,
    pub p2p_connections: u64,
    pub p2p_failures: u64,
    pub relay_fallbacks: u64,
    pub relay_connections: u64,
    pub relay_failures: u64,
    pub last_successful_path: Option<FfiTunnelPath>,
}

impl From<TunnelTransportStats> for FfiTunnelTransportStats {
    fn from(stats: TunnelTransportStats) -> Self {
        Self {
            p2p_attempts: stats.p2p_attempts,
            p2p_connections: stats.p2p_connections,
            p2p_failures: stats.p2p_failures,
            relay_fallbacks: stats.relay_fallbacks,
            relay_connections: stats.relay_connections,
            relay_failures: stats.relay_failures,
            last_successful_path: stats.last_successful_path.map(FfiTunnelPath::from),
        }
    }
}

#[derive(Debug, thiserror::Error, uniffi::Error)]
#[uniffi(flat_error)]
pub enum FfiMobileError {
    #[error("invalid config: {reason}")]
    InvalidConfig { reason: String },
    #[error("runtime failed: {reason}")]
    Runtime { reason: String },
    #[error("tunnel failed: {reason}")]
    Tunnel { reason: String },
    #[error("mobile tunnel is closed")]
    Closed,
}

impl From<crate::client::TunnelError> for FfiMobileError {
    fn from(error: crate::client::TunnelError) -> Self {
        match error {
            crate::client::TunnelError::InvalidConfig { reason }
            | crate::client::TunnelError::InvalidOpenServiceRequest { reason } => {
                Self::InvalidConfig { reason }
            }
            other => Self::Tunnel {
                reason: other.to_string(),
            },
        }
    }
}

impl From<ControlClientError> for FfiMobileError {
    fn from(error: ControlClientError) -> Self {
        Self::Tunnel {
            reason: error.to_string(),
        }
    }
}

impl From<MobileGrantError> for FfiMobileError {
    fn from(error: MobileGrantError) -> Self {
        Self::InvalidConfig {
            reason: error.to_string(),
        }
    }
}

impl From<crate::browser_proxy::BrowserProxyError> for FfiMobileError {
    fn from(error: crate::browser_proxy::BrowserProxyError) -> Self {
        Self::Tunnel {
            reason: error.to_string(),
        }
    }
}

#[derive(uniffi::Object)]
pub struct FfiMobileTunnel {
    runtime: Runtime,
    client: RwLock<Option<TunnelClient>>,
    browser_proxies: RwLock<Vec<Arc<FfiBrowserProxy>>>,
}

#[derive(uniffi::Object)]
pub struct FfiBrowserProxy {
    runtime: Runtime,
    proxy: RwLock<Option<BrowserProxy>>,
    handle: BrowserProxyHandle,
    stats: BrowserProxyStatsHandle,
}

#[uniffi::export]
impl FfiMobileTunnel {
    #[uniffi::constructor(name = "start_in_memory")]
    pub fn start_in_memory(config: FfiMobileTunnelConfig) -> Result<Arc<Self>, FfiMobileError> {
        let runtime = mobile_runtime()?;
        let client = runtime.block_on(TunnelClient::start(config.into_tunnel_config()?))?;
        Ok(Arc::new(Self {
            runtime,
            client: RwLock::new(Some(client)),
            browser_proxies: RwLock::new(Vec::new()),
        }))
    }

    #[uniffi::constructor(name = "start_with_control_relay")]
    pub fn start_with_control_relay(
        config: FfiMobileTunnelConfig,
        relay_server_cert_der: Vec<u8>,
    ) -> Result<Arc<Self>, FfiMobileError> {
        let runtime = mobile_runtime()?;
        let client = runtime.block_on(TunnelClient::start_with_control(
            config.into_tunnel_config()?,
            CertificateDer::from(relay_server_cert_der),
        ))?;
        Ok(Arc::new(Self {
            runtime,
            client: RwLock::new(Some(client)),
            browser_proxies: RwLock::new(Vec::new()),
        }))
    }

    #[uniffi::constructor(name = "start_with_control_p2p_or_relay")]
    pub fn start_with_control_p2p_or_relay(
        config: FfiMobileTunnelConfig,
        p2p_or_relay: FfiP2pOrRelayConfig,
    ) -> Result<Arc<Self>, FfiMobileError> {
        let runtime = mobile_runtime()?;
        let client = runtime.block_on(TunnelClient::start_with_control_p2p_or_relay(
            config.into_tunnel_config()?,
            p2p_or_relay.into_client_config()?,
        ))?;
        Ok(Arc::new(Self {
            runtime,
            client: RwLock::new(Some(client)),
            browser_proxies: RwLock::new(Vec::new()),
        }))
    }

    #[uniffi::constructor(name = "start_with_mobile_grant")]
    pub fn start_with_mobile_grant(
        config: FfiMobileTunnelConfig,
        grant: FfiMobileGrantCredential,
        p2p_or_relay: FfiP2pOrRelayConfig,
    ) -> Result<Arc<Self>, FfiMobileError> {
        let runtime = mobile_runtime()?;
        let client =
            runtime.block_on(TunnelClient::start_with_control_p2p_or_relay_mobile_grant(
                config.into_tunnel_config()?,
                grant.into(),
                p2p_or_relay.into_client_config()?,
            ))?;
        Ok(Arc::new(Self {
            runtime,
            client: RwLock::new(Some(client)),
            browser_proxies: RwLock::new(Vec::new()),
        }))
    }

    pub fn open_service(
        &self,
        request: FfiOpenServiceRequest,
    ) -> Result<FfiForwardHandle, FfiMobileError> {
        let client = self.client()?;
        let handle = self
            .runtime
            .block_on(client.open_service(request.into()))
            .map_err(FfiMobileError::from)?;
        Ok(handle.into())
    }

    pub fn start_browser_proxy(&self) -> Result<Arc<FfiBrowserProxy>, FfiMobileError> {
        self.start_browser_proxy_with_config(FfiBrowserProxyConfig::default())
    }

    pub fn start_browser_proxy_with_config(
        &self,
        config: FfiBrowserProxyConfig,
    ) -> Result<Arc<FfiBrowserProxy>, FfiMobileError> {
        let client = self.client()?;
        let proxy = self
            .runtime
            .block_on(client.start_browser_proxy(config.into_browser_proxy_config()?))
            .map_err(FfiMobileError::from)?;
        let handle = proxy.handle();
        let stats = proxy.stats_handle();
        let ffi_proxy = Arc::new(FfiBrowserProxy {
            runtime: mobile_runtime()?,
            proxy: RwLock::new(Some(proxy)),
            handle,
            stats,
        });
        self.browser_proxies
            .write()
            .map_err(|_| FfiMobileError::Runtime {
                reason: "browser proxy lock poisoned".to_string(),
            })?
            .push(Arc::clone(&ffi_proxy));
        Ok(ffi_proxy)
    }

    pub fn close_service(&self, handle_id: String) -> Result<(), FfiMobileError> {
        let client = self.client()?;
        self.runtime
            .block_on(client.close_service(handle_id))
            .map_err(FfiMobileError::from)
    }

    pub fn close_all_services(&self) -> Result<(), FfiMobileError> {
        let client = self.client()?;
        self.runtime
            .block_on(client.shutdown())
            .map_err(FfiMobileError::from)
    }

    pub fn is_closed(&self) -> bool {
        self.client
            .read()
            .map(|guard| guard.is_none())
            .unwrap_or(true)
    }

    pub fn status(&self) -> FfiTunnelStatus {
        self.client
            .read()
            .ok()
            .and_then(|guard| guard.as_ref().map(TunnelClient::status))
            .map(FfiTunnelStatus::from)
            .unwrap_or_else(closed_status)
    }

    pub fn status_result(&self) -> Result<FfiTunnelStatus, FfiMobileError> {
        Ok(self.client()?.status().into())
    }

    pub fn shutdown(&self) -> Result<(), FfiMobileError> {
        let proxies = {
            let mut guard = self
                .browser_proxies
                .write()
                .map_err(|_| FfiMobileError::Runtime {
                    reason: "browser proxy lock poisoned".to_string(),
                })?;
            std::mem::take(&mut *guard)
        };
        for proxy in proxies {
            proxy.close()?;
        }

        let client = {
            let mut guard = self.client.write().map_err(|_| FfiMobileError::Runtime {
                reason: "mobile tunnel lock poisoned".to_string(),
            })?;
            let Some(client) = guard.take() else {
                return Ok(());
            };
            client
        };

        self.runtime
            .block_on(client.shutdown())
            .map_err(FfiMobileError::from)
    }
}

#[uniffi::export]
impl FfiBrowserProxy {
    pub fn host(&self) -> String {
        self.handle.host().to_string()
    }

    pub fn port(&self) -> u16 {
        self.handle.local_port()
    }

    pub fn is_closed(&self) -> bool {
        self.proxy
            .read()
            .map(|guard| guard.is_none())
            .unwrap_or(true)
    }

    pub fn stats(&self) -> FfiBrowserProxyStats {
        self.stats.snapshot().into()
    }

    pub fn close(&self) -> Result<(), FfiMobileError> {
        let proxy = {
            let mut guard = self.proxy.write().map_err(|_| FfiMobileError::Runtime {
                reason: "browser proxy lock poisoned".to_string(),
            })?;
            let Some(proxy) = guard.take() else {
                return Ok(());
            };
            proxy
        };

        self.runtime
            .block_on(proxy.shutdown())
            .map_err(FfiMobileError::from)
    }
}

impl FfiMobileTunnel {
    fn client(&self) -> Result<TunnelClient, FfiMobileError> {
        self.client
            .read()
            .map_err(|_| FfiMobileError::Runtime {
                reason: "mobile tunnel lock poisoned".to_string(),
            })?
            .as_ref()
            .cloned()
            .ok_or(FfiMobileError::Closed)
    }
}

fn mobile_runtime() -> Result<Runtime, FfiMobileError> {
    Builder::new_multi_thread()
        .enable_all()
        .worker_threads(2)
        .thread_name("quic-mobile-ffi")
        .build()
        .map_err(|error| FfiMobileError::Runtime {
            reason: error.to_string(),
        })
}

fn closed_status() -> FfiTunnelStatus {
    FfiTunnelStatus {
        state: FfiTunnelState::Closed,
        path: FfiTunnelPath::Relay,
        rtt_ms: None,
        uplink_bytes: 0,
        downlink_bytes: 0,
        active_forwards: 0,
        transport: TunnelTransportStats::default().into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> FfiMobileTunnelConfig {
        FfiMobileTunnelConfig::new(
            "token_123".to_string(),
            "https://control.example.test".to_string(),
            "mobile_001".to_string(),
        )
        .with_control_request_timeout_ms(Some(2500))
        .with_control_max_retries(2)
        .with_control_retry_backoff_ms(100)
    }

    #[test]
    fn ffi_config_constructor_sets_mobile_defaults() {
        let config = mobile_tunnel_config(
            "token_123".to_string(),
            "https://control.example.test".to_string(),
            "mobile_001".to_string(),
        );

        assert_eq!(config.user_token, "token_123");
        assert_eq!(config.control_server_url, "https://control.example.test");
        assert_eq!(config.client_id, "mobile_001");
        assert_eq!(config.control_request_timeout_ms, None);
        assert_eq!(config.control_max_retries, 0);
        assert_eq!(config.control_retry_backoff_ms, 0);
    }

    #[test]
    fn p2p_or_relay_default_constructor_sets_mobile_defaults() {
        let config = p2p_or_relay_config_with_defaults(vec![1, 2, 3]);

        assert_eq!(config.relay_server_cert_der, vec![1, 2, 3]);
        assert_eq!(config.bind_addr, "0.0.0.0:0");
        assert_eq!(config.candidate_timeout_ms, 1500);
        assert_eq!(config.probe_timeout_ms, 1500);
        assert_eq!(config.interval_ms, 25);
        assert_eq!(config.relay_fallback_delay_ms, 300);
    }

    #[test]
    fn mobile_grant_pairing_options_default_constructor_sets_mobile_defaults() {
        let options = mobile_grant_pairing_options_with_defaults();

        assert_eq!(options.control_request_timeout_ms, Some(5_000));
        assert_eq!(options.control_max_retries, 2);
        assert_eq!(options.control_retry_backoff_ms, 100);
    }

    #[test]
    fn mobile_grant_invite_payload_converts_with_p2p_fingerprint() {
        let invite = FfiMobileInvitePayload {
            version: 1,
            control_url: "http://127.0.0.1:4242".to_string(),
            device_id: "pc_001".to_string(),
            invite_id: "inv_001".to_string(),
            invite_secret: "invite-secret".to_string(),
            agent_p2p_cert_fingerprint: Some("cert-fp".to_string()),
            allowed_services: vec!["svc_web".to_string()],
            expires_at: 1_000,
            max_uses: 1,
        };

        let internal = invite.clone().into_mobile_invite();
        assert_eq!(
            internal.agent_p2p_cert_fingerprint.as_deref(),
            Some("cert-fp")
        );
        assert_eq!(FfiMobileInvitePayload::from(internal), invite);
    }

    #[test]
    fn mobile_grant_credential_json_roundtrips_all_fields() {
        let grant = FfiMobileGrantCredential {
            version: 1,
            control_url: "http://127.0.0.1:4242".to_string(),
            device_id: "pc_001".to_string(),
            grant_id: "gr_001".to_string(),
            client_id: "phone_001".to_string(),
            allowed_services: vec!["svc_web".to_string(), "svc_api".to_string()],
            grant_secret: "grant-secret".to_string(),
            revocation_version: 7,
            agent_p2p_cert_fingerprint: Some("p2p-fingerprint".to_string()),
        };

        let json = mobile_grant_credential_to_json(grant.clone()).expect("credential serializes");
        assert!(json.contains("\"grant_id\""));
        assert!(json.contains("\"agent_p2p_cert_fingerprint\""));

        let decoded =
            mobile_grant_credential_from_json(json).expect("credential deserializes from json");
        assert_eq!(decoded, grant);
    }

    #[test]
    fn start_mobile_grant_pairing_rejects_incomplete_input_before_network() {
        let error = start_mobile_grant_pairing(
            FfiMobileInvitePayload {
                version: 1,
                control_url: "http://127.0.0.1:4242".to_string(),
                device_id: "pc_001".to_string(),
                invite_id: "inv_001".to_string(),
                invite_secret: "invite-secret".to_string(),
                agent_p2p_cert_fingerprint: None,
                allowed_services: vec!["svc_web".to_string()],
                expires_at: 1_000,
                max_uses: 1,
            },
            String::new(),
            vec!["svc_web".to_string()],
            "nonce".to_string(),
            mobile_grant_pairing_options_with_defaults(),
        )
        .expect_err("empty client id should be rejected before network");

        assert!(error.to_string().contains("incomplete"));
    }

    #[test]
    fn browser_proxy_default_config_exposes_mobile_timeouts() {
        let config = browser_proxy_config_with_defaults();

        assert_eq!(config.bind_host, "127.0.0.1");
        assert_eq!(config.local_port, 0);
        assert_eq!(config.domain_suffix, ".qtunnel.local");
        assert_eq!(config.max_connections, 256);
        assert_eq!(
            config.direct_fallback_policy,
            FfiBrowserProxyDirectFallbackPolicy::LocalNetworkAndDomain
        );
        assert_eq!(config.request_head_timeout_ms, 10_000);
        assert_eq!(config.direct_connect_timeout_ms, 10_000);
        assert_eq!(config.tunnel_open_timeout_ms, 15_000);
        assert_eq!(config.idle_timeout_ms, 120_000);
    }

    #[test]
    fn ffi_config_converts_to_internal_tunnel_config() {
        let internal = config().into_tunnel_config().expect("config converts");

        assert_eq!(internal.user_token, "token_123");
        assert_eq!(internal.control_server_url, "https://control.example.test");
        assert_eq!(internal.client_id.as_str(), "mobile_001");
        assert_eq!(
            internal
                .control_client_options
                .request_timeout()
                .map(|timeout| timeout.as_millis()),
            Some(2500)
        );
        assert_eq!(internal.control_client_options.max_retries(), 2);
        assert_eq!(
            internal.control_client_options.retry_backoff().as_millis(),
            100
        );
    }

    #[test]
    fn in_memory_tunnel_opens_statuses_closes_and_shuts_down() {
        let tunnel = FfiMobileTunnel::start_in_memory(config()).expect("start tunnel");
        assert!(!tunnel.is_closed());

        let initial = tunnel.status();
        assert_eq!(initial.state, FfiTunnelState::Started);
        assert_eq!(initial.path, FfiTunnelPath::Relay);
        assert_eq!(initial.active_forwards, 0);

        let handle = tunnel
            .open_service(FfiOpenServiceRequest {
                device_id: "pc_001".to_string(),
                service_id: "ssh".to_string(),
                local_port: 0,
            })
            .expect("open service");

        assert_eq!(handle.device_id, "pc_001");
        assert_eq!(handle.service_id, "ssh");
        assert!(handle.handle_id.starts_with("forward_"));
        assert!(handle.local_port > 0);

        let opened = tunnel.status();
        assert_eq!(opened.active_forwards, 1);

        tunnel
            .close_service(handle.handle_id.clone())
            .expect("close service");
        assert_eq!(tunnel.status().active_forwards, 0);

        tunnel.shutdown().expect("shutdown");
        assert!(tunnel.is_closed());
        let error = tunnel.status_result().expect_err("closed tunnel errors");
        assert!(error.to_string().contains("closed"));
    }

    #[test]
    fn close_all_services_keeps_tunnel_open() {
        let tunnel = FfiMobileTunnel::start_in_memory(config()).expect("start tunnel");
        let first = tunnel
            .open_service(FfiOpenServiceRequest {
                device_id: "pc_001".to_string(),
                service_id: "ssh".to_string(),
                local_port: 0,
            })
            .expect("open first service");
        let second = tunnel
            .open_service(FfiOpenServiceRequest {
                device_id: "pc_001".to_string(),
                service_id: "web".to_string(),
                local_port: 0,
            })
            .expect("open second service");

        assert_eq!(tunnel.status().active_forwards, 2);
        tunnel.close_all_services().expect("close all services");

        assert_eq!(tunnel.status().active_forwards, 0);
        assert!(!tunnel.is_closed());
        std::net::TcpListener::bind(("127.0.0.1", first.local_port)).expect("first port released");
        std::net::TcpListener::bind(("127.0.0.1", second.local_port))
            .expect("second port released");
    }

    #[test]
    fn start_with_control_relay_constructor_accepts_relay_certificate_bytes() {
        let mut config = config();
        config.control_server_url = "http://127.0.0.1:4242".to_string();
        let tunnel =
            FfiMobileTunnel::start_with_control_relay(config, vec![1, 2, 3]).expect("start");

        assert_eq!(tunnel.status().path, FfiTunnelPath::Relay);
        tunnel.shutdown().expect("shutdown");
    }

    #[test]
    fn start_with_control_p2p_or_relay_reports_p2p_preferred_transport() {
        let mut config = config();
        config.control_server_url = "http://127.0.0.1:4242".to_string();
        let tunnel = FfiMobileTunnel::start_with_control_p2p_or_relay(
            config,
            p2p_or_relay_config_with_defaults(vec![1, 2, 3]),
        )
        .expect("start");

        let status = tunnel.status();
        assert_eq!(status.path, FfiTunnelPath::P2p);
        assert_eq!(status.transport.p2p_attempts, 0);
        assert_eq!(status.transport.p2p_connections, 0);
        assert_eq!(status.transport.p2p_failures, 0);
        assert_eq!(status.transport.relay_fallbacks, 0);
        assert_eq!(status.transport.relay_connections, 0);
        assert_eq!(status.transport.relay_failures, 0);
        assert_eq!(status.transport.last_successful_path, None);

        tunnel.shutdown().expect("shutdown");
    }

    #[test]
    fn start_with_mobile_grant_reports_p2p_preferred_transport() {
        let mut config = config();
        config.user_token = String::new();
        config.control_server_url = "http://127.0.0.1:4242".to_string();
        config.client_id = "phone_001".to_string();
        let tunnel = FfiMobileTunnel::start_with_mobile_grant(
            config,
            FfiMobileGrantCredential {
                version: 1,
                control_url: "http://127.0.0.1:4242".to_string(),
                device_id: "pc_001".to_string(),
                grant_id: "gr_001".to_string(),
                client_id: "phone_001".to_string(),
                allowed_services: vec!["svc_web".to_string()],
                grant_secret: "grant-secret".to_string(),
                revocation_version: 1,
                agent_p2p_cert_fingerprint: None,
            },
            p2p_or_relay_config_with_defaults(vec![1, 2, 3]),
        )
        .expect("start");

        let status = tunnel.status();
        assert_eq!(status.path, FfiTunnelPath::P2p);
        assert_eq!(status.transport.p2p_attempts, 0);

        tunnel.shutdown().expect("shutdown");
    }

    #[test]
    fn closing_unknown_handle_returns_tunnel_error() {
        let tunnel = FfiMobileTunnel::start_in_memory(config()).expect("start tunnel");

        let error = tunnel
            .close_service("missing".to_string())
            .expect_err("unknown handle should fail");

        assert!(error.to_string().contains("missing"));
    }

    #[test]
    fn shutdown_closes_active_forwards() {
        let tunnel = FfiMobileTunnel::start_in_memory(config()).expect("start tunnel");
        let handle = tunnel
            .open_service(FfiOpenServiceRequest {
                device_id: "pc_001".to_string(),
                service_id: "ssh".to_string(),
                local_port: 0,
            })
            .expect("open service");

        assert_eq!(tunnel.status().active_forwards, 1);
        tunnel.shutdown().expect("shutdown");

        std::net::TcpListener::bind(("127.0.0.1", handle.local_port))
            .expect("shutdown releases local port");
    }

    #[test]
    fn browser_proxy_handle_reports_endpoint_and_closes() {
        let tunnel = FfiMobileTunnel::start_in_memory(config()).expect("start tunnel");
        let proxy = tunnel.start_browser_proxy().expect("start browser proxy");

        assert_eq!(proxy.host(), "127.0.0.1");
        assert!(proxy.port() > 0);
        assert!(!proxy.is_closed());
        let stats = proxy.stats();
        assert_eq!(stats.accepted_connections, 0);
        assert_eq!(stats.active_connections, 0);
        assert_eq!(stats.direct_connections, 0);
        assert_eq!(stats.tunnel_connections, 0);
        assert_eq!(stats.forbidden_direct_connections, 0);
        assert_eq!(stats.tunnel_bytes_to_remote, 0);
        assert_eq!(stats.tunnel_bytes_to_browser, 0);
        assert_eq!(stats.direct_bytes_to_remote, 0);
        assert_eq!(stats.direct_bytes_to_browser, 0);
        assert_eq!(stats.idle_timeout_closures, 0);

        proxy.close().expect("close browser proxy");
        assert!(proxy.is_closed());
    }

    #[test]
    fn browser_proxy_can_start_with_mobile_config() {
        let tunnel = FfiMobileTunnel::start_in_memory(config()).expect("start tunnel");
        let proxy = tunnel
            .start_browser_proxy_with_config(FfiBrowserProxyConfig {
                bind_host: "127.0.0.1".to_string(),
                local_port: 0,
                domain_suffix: ".qtunnel.test".to_string(),
                max_connections: 64,
                direct_fallback_policy: FfiBrowserProxyDirectFallbackPolicy::LocalNetworkAndDomain,
                request_head_timeout_ms: 1_000,
                direct_connect_timeout_ms: 1_000,
                tunnel_open_timeout_ms: 1_000,
                idle_timeout_ms: 5_000,
            })
            .expect("start browser proxy");

        assert_eq!(proxy.host(), "127.0.0.1");
        assert!(proxy.port() > 0);

        proxy.close().expect("close browser proxy");
    }

    #[test]
    fn browser_proxy_host_helpers_generate_dns_safe_hosts() {
        assert_eq!(
            browser_proxy_host_for_service("pc_001".to_string(), "svc_web_3000".to_string())
                .expect("host generated"),
            "s-svc-5fweb-5f3000.d-pc-5f001.qtunnel.local"
        );
        assert_eq!(
            browser_proxy_host_with_suffix(
                "pc_001".to_string(),
                "svc_web_3000".to_string(),
                ".qtunnel.test".to_string(),
            )
            .expect("host generated"),
            "s-svc-5fweb-5f3000.d-pc-5f001.qtunnel.test"
        );
        let error = browser_proxy_host_with_suffix(
            "pc_001".to_string(),
            "svc_web_3000".to_string(),
            String::new(),
        )
        .expect_err("invalid suffix should fail");
        assert!(error.to_string().contains("domain_suffix"));
    }

    #[test]
    fn browser_proxy_device_service_route_describes_synthetic_url() {
        let route =
            browser_proxy_device_service_route("pc_001".to_string(), "svc_web_3000".to_string())
                .expect("route generated");

        assert_eq!(route.kind, FfiBrowserProxyRouteKind::DeviceService);
        assert_eq!(route.device_id, "pc_001");
        assert_eq!(route.service_id, "svc_web_3000");
        assert_eq!(route.scheme, "http");
        assert_eq!(route.host, "s-svc-5fweb-5f3000.d-pc-5f001.qtunnel.local");
        assert_eq!(
            route.origin(),
            "http://s-svc-5fweb-5f3000.d-pc-5f001.qtunnel.local"
        );
        assert_eq!(
            route.http_url("/status?q=1".to_string()),
            "http://s-svc-5fweb-5f3000.d-pc-5f001.qtunnel.local/status?q=1"
        );
        assert_eq!(
            route.http_url("status".to_string()),
            "http://s-svc-5fweb-5f3000.d-pc-5f001.qtunnel.local/status"
        );
    }

    #[test]
    fn browser_proxy_route_helpers_are_exportable_functions() {
        let route =
            browser_proxy_device_service_route("pc_001".to_string(), "svc_web_3000".to_string())
                .expect("route generated");

        assert_eq!(
            browser_proxy_route_origin(route.clone()),
            "http://s-svc-5fweb-5f3000.d-pc-5f001.qtunnel.local"
        );
        assert_eq!(
            browser_proxy_route_http_url(route, "status?q=1".to_string()),
            "http://s-svc-5fweb-5f3000.d-pc-5f001.qtunnel.local/status?q=1"
        );
    }

    #[test]
    fn browser_proxy_classifies_urls_for_mobile_callers() {
        let synthetic =
            browser_proxy_device_service_route("pc_001".to_string(), "svc_web_3000".to_string())
                .expect("route generated")
                .http_url("/status".to_string());

        let device = browser_proxy_classify_url_with_defaults(
            synthetic,
            "https://control.example.test/api".to_string(),
        )
        .expect("classified device route");
        assert_eq!(device.kind, FfiBrowserProxyUrlKind::DeviceService);
        assert_eq!(device.device_id.as_deref(), Some("pc_001"));
        assert_eq!(device.service_id.as_deref(), Some("svc_web_3000"));

        let control = browser_proxy_classify_url_with_defaults(
            "https://control.example.test/devices".to_string(),
            "https://control.example.test/api".to_string(),
        )
        .expect("classified control route");
        assert_eq!(control.kind, FfiBrowserProxyUrlKind::ControlServer);
        assert!(control.device_id.is_none());
        assert!(control.service_id.is_none());

        let direct = browser_proxy_classify_url_with_defaults(
            "https://example.com/app.js".to_string(),
            "https://control.example.test/api".to_string(),
        )
        .expect("classified direct route");
        assert_eq!(direct.kind, FfiBrowserProxyUrlKind::DirectNetwork);
    }

    #[test]
    fn tunnel_shutdown_closes_active_browser_proxy() {
        let tunnel = FfiMobileTunnel::start_in_memory(config()).expect("start tunnel");
        let proxy = tunnel.start_browser_proxy().expect("start browser proxy");
        let port = proxy.port();

        tunnel.shutdown().expect("shutdown tunnel");

        assert!(proxy.is_closed());
        std::net::TcpListener::bind(("127.0.0.1", port)).expect("proxy port released");
    }
}
