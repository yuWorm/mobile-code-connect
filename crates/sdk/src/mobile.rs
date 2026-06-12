use std::{fmt, net::SocketAddr, time::Duration};

use mobilecode_connect_control_client::{HttpControlClient, HttpControlClientOptions};
use mobilecode_connect_mobile_core::{
    browser_proxy::{
        browser_proxy_host, classify_browser_proxy_url, BrowserProxy,
        BrowserProxyConfig as CoreBrowserProxyConfig,
        BrowserProxyDirectFallbackPolicy as CoreBrowserProxyDirectFallbackPolicy,
        BrowserProxyStats as CoreBrowserProxyStats, BrowserProxyTarget,
        BrowserProxyUrlClassification as CoreBrowserProxyUrlClassification,
        BrowserProxyUrlKind as CoreBrowserProxyUrlKind, DEFAULT_BROWSER_PROXY_DOMAIN_SUFFIX,
    },
    client::{ControlP2pOrRelayClientConfig, OpenServiceRequest, TunnelClient},
    config::TunnelConfig,
    forward::LocalForwardHandle,
    status::TunnelStatus,
};
use mobilecode_connect_protocol::{
    derive_mobile_grant_secret, ClientId, DeviceId, MobileGrantCredential, MobileInvitePayload,
    MobilePairingRequest, PendingPairingStatus, ServiceId,
};
use rustls::pki_types::CertificateDer;

use crate::{
    store::{MobileGrantStore, TokenStore},
    SdkError,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MobileTunnelConfig {
    pub control_server_url: String,
    pub client_id: ClientId,
    pub control_client_options: HttpControlClientOptions,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenServiceInput {
    pub device_id: DeviceId,
    pub service_id: ServiceId,
    pub local_port: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserProxyConfig {
    pub bind_host: String,
    pub local_port: u16,
    pub domain_suffix: String,
    pub max_connections: usize,
    pub direct_fallback_policy: BrowserProxyDirectFallbackPolicy,
    pub request_head_timeout: Duration,
    pub direct_connect_timeout: Duration,
    pub tunnel_open_timeout: Duration,
    pub idle_timeout: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserProxyDirectFallbackPolicy {
    AllowAll,
    LocalNetworkAndDomain,
    Disabled,
}

impl From<BrowserProxyDirectFallbackPolicy> for CoreBrowserProxyDirectFallbackPolicy {
    fn from(policy: BrowserProxyDirectFallbackPolicy) -> Self {
        match policy {
            BrowserProxyDirectFallbackPolicy::AllowAll => Self::AllowAll,
            BrowserProxyDirectFallbackPolicy::LocalNetworkAndDomain => Self::LocalNetworkAndDomain,
            BrowserProxyDirectFallbackPolicy::Disabled => Self::Disabled,
        }
    }
}

impl Default for BrowserProxyConfig {
    fn default() -> Self {
        Self {
            bind_host: "127.0.0.1".to_string(),
            local_port: 0,
            domain_suffix: DEFAULT_BROWSER_PROXY_DOMAIN_SUFFIX.to_string(),
            max_connections: 256,
            direct_fallback_policy: BrowserProxyDirectFallbackPolicy::LocalNetworkAndDomain,
            request_head_timeout: Duration::from_secs(10),
            direct_connect_timeout: Duration::from_secs(10),
            tunnel_open_timeout: Duration::from_secs(15),
            idle_timeout: Duration::from_secs(120),
        }
    }
}

impl From<BrowserProxyConfig> for CoreBrowserProxyConfig {
    fn from(config: BrowserProxyConfig) -> Self {
        Self {
            bind_host: config.bind_host,
            local_port: config.local_port,
            domain_suffix: config.domain_suffix,
            max_connections: config.max_connections,
            direct_fallback_policy: config.direct_fallback_policy.into(),
            request_head_timeout: config.request_head_timeout,
            direct_connect_timeout: config.direct_connect_timeout,
            tunnel_open_timeout: config.tunnel_open_timeout,
            idle_timeout: config.idle_timeout,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BrowserProxyStats {
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

impl From<CoreBrowserProxyStats> for BrowserProxyStats {
    fn from(stats: CoreBrowserProxyStats) -> Self {
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

pub struct BrowserProxyHandle {
    proxy: BrowserProxy,
    host: String,
    local_port: u16,
}

impl BrowserProxyHandle {
    pub fn host(&self) -> &str {
        &self.host
    }

    pub fn local_port(&self) -> u16 {
        self.local_port
    }

    pub fn stats(&self) -> BrowserProxyStats {
        self.proxy.stats().into()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserProxyRouteKind {
    DeviceService,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserProxyUrlKind {
    DeviceService,
    ControlServer,
    DirectNetwork,
}

impl From<CoreBrowserProxyUrlKind> for BrowserProxyUrlKind {
    fn from(kind: CoreBrowserProxyUrlKind) -> Self {
        match kind {
            CoreBrowserProxyUrlKind::DeviceService => Self::DeviceService,
            CoreBrowserProxyUrlKind::ControlServer => Self::ControlServer,
            CoreBrowserProxyUrlKind::DirectNetwork => Self::DirectNetwork,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserProxyUrlClassification {
    pub kind: BrowserProxyUrlKind,
    pub host: String,
    pub target: Option<BrowserProxyTarget>,
}

impl From<CoreBrowserProxyUrlClassification> for BrowserProxyUrlClassification {
    fn from(classification: CoreBrowserProxyUrlClassification) -> Self {
        Self {
            kind: classification.kind.into(),
            host: classification.host,
            target: classification.target,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserProxyRoute {
    pub kind: BrowserProxyRouteKind,
    pub device_id: DeviceId,
    pub service_id: ServiceId,
    pub scheme: String,
    pub host: String,
}

impl BrowserProxyRoute {
    pub fn origin(&self) -> String {
        format!("{}://{}", self.scheme, self.host)
    }

    pub fn http_url(&self, path_and_query: impl AsRef<str>) -> String {
        format!(
            "{}{}",
            self.origin(),
            normalized_url_path(path_and_query.as_ref())
        )
    }
}

#[derive(Debug, Clone)]
pub struct P2pOrRelayTunnelConfig {
    pub relay_server_cert: CertificateDer<'static>,
    pub bind_addr: SocketAddr,
    pub candidate_timeout: Duration,
    pub probe_timeout: Duration,
    pub interval: Duration,
    pub relay_fallback_delay: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MobileGrantPairingInput {
    pub invite: MobileInvitePayload,
    pub client_id: ClientId,
    pub requested_services: Vec<ServiceId>,
    pub nonce: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MobileGrantPairingSession {
    pub pending_pairing_id: String,
    pub poll_interval_ms: u64,
    pub expires_at: u64,
    pub invite: MobileInvitePayload,
    pub client_id: ClientId,
    pub requested_services: Vec<ServiceId>,
}

pub struct MobileTunnelSdk {
    client: TunnelClient,
}

impl fmt::Debug for MobileTunnelSdk {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MobileTunnelSdk").finish_non_exhaustive()
    }
}

impl MobileTunnelSdk {
    pub async fn start_in_memory<S>(
        config: MobileTunnelConfig,
        token_store: S,
    ) -> Result<Self, SdkError>
    where
        S: TokenStore,
    {
        Ok(Self {
            client: TunnelClient::start(tunnel_config(config, &token_store).await?).await?,
        })
    }

    pub async fn start_with_control<S>(
        config: MobileTunnelConfig,
        token_store: S,
        relay_server_cert: CertificateDer<'static>,
    ) -> Result<Self, SdkError>
    where
        S: TokenStore,
    {
        Ok(Self {
            client: TunnelClient::start_with_control(
                tunnel_config(config, &token_store).await?,
                relay_server_cert,
            )
            .await?,
        })
    }

    pub async fn start_with_control_p2p_or_relay<S>(
        config: MobileTunnelConfig,
        token_store: S,
        p2p_or_relay: P2pOrRelayTunnelConfig,
    ) -> Result<Self, SdkError>
    where
        S: TokenStore,
    {
        Ok(Self {
            client: TunnelClient::start_with_control_p2p_or_relay(
                tunnel_config(config, &token_store).await?,
                ControlP2pOrRelayClientConfig {
                    relay_server_cert: p2p_or_relay.relay_server_cert,
                    bind_addr: p2p_or_relay.bind_addr,
                    candidate_timeout: p2p_or_relay.candidate_timeout,
                    probe_timeout: p2p_or_relay.probe_timeout,
                    interval: p2p_or_relay.interval,
                    relay_fallback_delay: p2p_or_relay.relay_fallback_delay,
                },
            )
            .await?,
        })
    }

    pub async fn start_with_mobile_grant(
        config: MobileTunnelConfig,
        grant: MobileGrantCredential,
        p2p_or_relay: P2pOrRelayTunnelConfig,
    ) -> Result<Self, SdkError> {
        validate_mobile_grant_start(&config, &grant)?;
        Ok(Self {
            client: TunnelClient::start_with_control_p2p_or_relay_mobile_grant(
                TunnelConfig {
                    user_token: String::new(),
                    control_server_url: config.control_server_url,
                    client_id: config.client_id,
                    control_client_options: config.control_client_options,
                },
                grant,
                ControlP2pOrRelayClientConfig {
                    relay_server_cert: p2p_or_relay.relay_server_cert,
                    bind_addr: p2p_or_relay.bind_addr,
                    candidate_timeout: p2p_or_relay.candidate_timeout,
                    probe_timeout: p2p_or_relay.probe_timeout,
                    interval: p2p_or_relay.interval,
                    relay_fallback_delay: p2p_or_relay.relay_fallback_delay,
                },
            )
            .await?,
        })
    }

    pub async fn start_mobile_grant_pairing(
        input: MobileGrantPairingInput,
        control_client_options: HttpControlClientOptions,
    ) -> Result<MobileGrantPairingSession, SdkError> {
        validate_mobile_grant_pairing_input(&input)?;
        let proof = MobilePairingRequest::proof_for(
            input.invite.device_id.clone(),
            input.invite.invite_id.clone(),
            input.client_id.clone(),
            input.requested_services.clone(),
            input.nonce.clone(),
            &input.invite.invite_secret,
        )?;
        let request = MobilePairingRequest {
            device_id: input.invite.device_id.clone(),
            invite_id: input.invite.invite_id.clone(),
            client_id: input.client_id.clone(),
            requested_services: input.requested_services.clone(),
            nonce: input.nonce.clone(),
            proof,
        };
        let control =
            HttpControlClient::with_options(&input.invite.control_url, control_client_options)?;
        let started = control.start_mobile_pairing(request).await?;
        Ok(MobileGrantPairingSession {
            pending_pairing_id: started.pending_pairing_id,
            poll_interval_ms: started.poll_interval_ms,
            expires_at: started.expires_at,
            invite: input.invite,
            client_id: input.client_id,
            requested_services: input.requested_services,
        })
    }

    pub async fn complete_mobile_grant_pairing_once<S>(
        pairing: MobileGrantPairingSession,
        grant_store: S,
        control_client_options: HttpControlClientOptions,
    ) -> Result<Option<MobileGrantCredential>, SdkError>
    where
        S: MobileGrantStore,
    {
        let control =
            HttpControlClient::with_options(&pairing.invite.control_url, control_client_options)?;
        let result = control
            .mobile_pairing_result(&pairing.pending_pairing_id)
            .await?;
        match result.status {
            PendingPairingStatus::Pending => Ok(None),
            PendingPairingStatus::Denied => Err(SdkError::MobileGrantPairingDenied),
            PendingPairingStatus::Expired => Err(SdkError::MobileGrantPairingExpired),
            PendingPairingStatus::Approved => {
                let metadata = result
                    .grant
                    .ok_or(SdkError::MobileGrantPairingApprovedWithoutGrant)?;
                if metadata.device_id != pairing.invite.device_id
                    || metadata.client_id != pairing.client_id
                    || metadata.allowed_services.iter().any(|service_id| {
                        !pairing
                            .requested_services
                            .iter()
                            .any(|requested| requested == service_id)
                    })
                {
                    return Err(SdkError::InvalidConfig {
                        reason: "approved mobile grant metadata does not match pairing request"
                            .to_string(),
                    });
                }
                let grant_secret = derive_mobile_grant_secret(
                    &pairing.invite.invite_secret,
                    metadata.grant_id.clone(),
                    &metadata.client_id,
                )?;
                let grant = MobileGrantCredential {
                    version: metadata.version,
                    control_url: pairing.invite.control_url,
                    device_id: metadata.device_id,
                    grant_id: metadata.grant_id,
                    client_id: metadata.client_id,
                    allowed_services: metadata.allowed_services,
                    grant_secret,
                    revocation_version: metadata.revocation_version,
                    agent_p2p_cert_fingerprint: pairing.invite.agent_p2p_cert_fingerprint,
                };
                grant_store.save_mobile_grant(grant.clone()).await?;
                Ok(Some(grant))
            }
        }
    }

    pub async fn open_service(
        &self,
        input: OpenServiceInput,
    ) -> Result<LocalForwardHandle, SdkError> {
        Ok(self
            .client
            .open_service(OpenServiceRequest {
                device_id: input.device_id,
                service_id: input.service_id,
                local_port: input.local_port,
            })
            .await?)
    }

    pub async fn close_service(&self, handle_id: impl Into<String>) -> Result<(), SdkError> {
        self.client.close_service(handle_id.into()).await?;
        Ok(())
    }

    pub async fn start_browser_proxy(
        &self,
        config: BrowserProxyConfig,
    ) -> Result<BrowserProxyHandle, SdkError> {
        let proxy = self.client.start_browser_proxy(config.into()).await?;
        let handle = proxy.handle();
        Ok(BrowserProxyHandle {
            host: handle.host().to_string(),
            local_port: handle.local_port(),
            proxy,
        })
    }

    pub async fn close_browser_proxy(&self, handle: BrowserProxyHandle) -> Result<(), SdkError> {
        handle.proxy.shutdown().await?;
        Ok(())
    }

    pub fn browser_proxy_device_service_route(
        &self,
        device_id: DeviceId,
        service_id: ServiceId,
    ) -> Result<BrowserProxyRoute, SdkError> {
        browser_proxy_device_service_route_with_suffix(
            device_id,
            service_id,
            DEFAULT_BROWSER_PROXY_DOMAIN_SUFFIX,
        )
    }

    pub fn classify_browser_proxy_url(
        &self,
        url: impl AsRef<str>,
    ) -> Result<BrowserProxyUrlClassification, SdkError> {
        classify_browser_proxy_url_with_defaults(url, self.client.config().control_server_url)
    }

    pub fn status(&self) -> TunnelStatus {
        self.client.status()
    }

    pub fn inner(&self) -> &TunnelClient {
        &self.client
    }
}

fn validate_mobile_grant_start(
    config: &MobileTunnelConfig,
    grant: &MobileGrantCredential,
) -> Result<(), SdkError> {
    if config.control_server_url.trim().is_empty() {
        return Err(SdkError::InvalidConfig {
            reason: "control_server_url is required".to_string(),
        });
    }
    if grant.control_url.trim() != config.control_server_url.trim() {
        return Err(SdkError::InvalidConfig {
            reason: "grant control_url must match tunnel control_server_url".to_string(),
        });
    }
    if grant.client_id != config.client_id {
        return Err(SdkError::InvalidConfig {
            reason: "grant client_id must match tunnel client_id".to_string(),
        });
    }
    if grant.grant_secret.trim().is_empty()
        || grant.grant_id.trim().is_empty()
        || grant.allowed_services.is_empty()
    {
        return Err(SdkError::InvalidConfig {
            reason: "mobile grant credential is incomplete".to_string(),
        });
    }
    Ok(())
}

fn validate_mobile_grant_pairing_input(input: &MobileGrantPairingInput) -> Result<(), SdkError> {
    if input.invite.version != 1
        || input.invite.control_url.trim().is_empty()
        || input.invite.device_id.as_str().trim().is_empty()
        || input.invite.invite_id.trim().is_empty()
        || input.invite.invite_secret.trim().is_empty()
        || input.client_id.as_str().trim().is_empty()
        || input.requested_services.is_empty()
        || input.nonce.trim().is_empty()
    {
        return Err(SdkError::InvalidConfig {
            reason: "mobile grant pairing input is incomplete".to_string(),
        });
    }
    if input.requested_services.iter().any(|service_id| {
        service_id.as_str().trim().is_empty()
            || !input
                .invite
                .allowed_services
                .iter()
                .any(|allowed| allowed == service_id)
    }) {
        return Err(SdkError::InvalidConfig {
            reason: "requested services must be within invite scope".to_string(),
        });
    }
    Ok(())
}

async fn tunnel_config<S>(
    config: MobileTunnelConfig,
    token_store: &S,
) -> Result<TunnelConfig, SdkError>
where
    S: TokenStore,
{
    let token = token_store
        .load_token()
        .await?
        .ok_or(SdkError::NotAuthenticated)?;
    Ok(TunnelConfig {
        user_token: token.access_token,
        control_server_url: config.control_server_url,
        client_id: config.client_id,
        control_client_options: config.control_client_options,
    })
}

pub fn browser_proxy_device_service_route_with_suffix(
    device_id: DeviceId,
    service_id: ServiceId,
    domain_suffix: impl AsRef<str>,
) -> Result<BrowserProxyRoute, SdkError> {
    let host = browser_proxy_host(
        &BrowserProxyTarget {
            device_id: device_id.clone(),
            service_id: service_id.clone(),
        },
        domain_suffix.as_ref(),
    )?;
    Ok(BrowserProxyRoute {
        kind: BrowserProxyRouteKind::DeviceService,
        device_id,
        service_id,
        scheme: "http".to_string(),
        host,
    })
}

pub fn classify_browser_proxy_url_with_defaults(
    url: impl AsRef<str>,
    control_server_url: impl AsRef<str>,
) -> Result<BrowserProxyUrlClassification, SdkError> {
    classify_browser_proxy_url(
        url.as_ref(),
        control_server_url.as_ref(),
        DEFAULT_BROWSER_PROXY_DOMAIN_SUFFIX,
    )
    .map(BrowserProxyUrlClassification::from)
    .map_err(SdkError::from)
}

pub fn classify_browser_proxy_url_with_domain_suffix(
    url: impl AsRef<str>,
    control_server_url: impl AsRef<str>,
    domain_suffix: impl AsRef<str>,
) -> Result<BrowserProxyUrlClassification, SdkError> {
    classify_browser_proxy_url(
        url.as_ref(),
        control_server_url.as_ref(),
        domain_suffix.as_ref(),
    )
    .map(BrowserProxyUrlClassification::from)
    .map_err(SdkError::from)
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
