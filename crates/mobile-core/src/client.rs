use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, RwLock},
    time::Duration,
};

use mobilecode_connect_protocol::{DeviceId, MobileGrantCredential, ServiceId};
use rustls::pki_types::CertificateDer;

use crate::{
    browser_proxy::{BrowserProxy, BrowserProxyConfig},
    config::TunnelConfig,
    forward::{
        ControlP2pOrRelayConnectorConfig, ControlP2pOrRelayStreamConnector,
        ControlRelayConnectorConfig, ControlRelayStreamConnector, ForwardError, LocalForwardHandle,
        LocalForwarder, MemoryStreamConnector, OpenForwardRequest, StreamConnector,
    },
    path::{PathSelector, TunnelPath},
    status::{TunnelState, TunnelStatus, TunnelTransportStatsHandle},
};

#[derive(Clone)]
pub struct TunnelClient {
    state: Arc<RwLock<ClientState>>,
    connector: Arc<dyn StreamConnector>,
}

#[derive(Debug, Clone)]
pub struct ControlP2pOrRelayClientConfig {
    pub relay_server_cert: CertificateDer<'static>,
    pub bind_addr: SocketAddr,
    pub candidate_timeout: Duration,
    pub probe_timeout: Duration,
    pub interval: Duration,
    pub relay_fallback_delay: Duration,
}

struct ClientState {
    config: TunnelConfig,
    preferred_path: TunnelPath,
    transport_stats: TunnelTransportStatsHandle,
    forwards: HashMap<String, ActiveForward>,
    next_handle_id: u64,
}

struct ActiveForward {
    forwarder: LocalForwarder,
}

impl TunnelClient {
    pub async fn start(config: TunnelConfig) -> Result<Self, TunnelError> {
        Self::with_connector(config, Arc::new(MemoryStreamConnector::default())).await
    }

    pub async fn start_with_control(
        config: TunnelConfig,
        relay_server_cert: CertificateDer<'static>,
    ) -> Result<Self, TunnelError> {
        let connector = ControlRelayStreamConnector::new(ControlRelayConnectorConfig {
            control_server_url: config.control_server_url.clone(),
            control_token: Some(config.user_token.clone()),
            client_id: config.client_id.clone(),
            control_client_options: config.control_client_options,
            relay_server_cert,
        })?;
        let transport_stats = connector.transport_stats_handle();
        Self::with_connector_and_transport(
            config,
            Arc::new(connector),
            TunnelPath::Relay,
            transport_stats,
        )
        .await
    }

    pub async fn start_with_control_p2p_or_relay(
        config: TunnelConfig,
        p2p_or_relay: ControlP2pOrRelayClientConfig,
    ) -> Result<Self, TunnelError> {
        let connector = ControlP2pOrRelayStreamConnector::new(ControlP2pOrRelayConnectorConfig {
            control_server_url: config.control_server_url.clone(),
            control_token: Some(config.user_token.clone()),
            mobile_grant: None,
            client_id: config.client_id.clone(),
            control_client_options: config.control_client_options,
            relay_server_cert: p2p_or_relay.relay_server_cert,
            bind_addr: p2p_or_relay.bind_addr,
            candidate_timeout: p2p_or_relay.candidate_timeout,
            probe_timeout: p2p_or_relay.probe_timeout,
            interval: p2p_or_relay.interval,
            relay_fallback_delay: p2p_or_relay.relay_fallback_delay,
        })?;
        let transport_stats = connector.transport_stats_handle();
        Self::with_connector_and_transport(
            config,
            Arc::new(connector),
            TunnelPath::P2p,
            transport_stats,
        )
        .await
    }

    pub async fn start_with_control_p2p_or_relay_mobile_grant(
        config: TunnelConfig,
        grant: MobileGrantCredential,
        p2p_or_relay: ControlP2pOrRelayClientConfig,
    ) -> Result<Self, TunnelError> {
        validate_mobile_grant_config(&config)?;
        let connector = ControlP2pOrRelayStreamConnector::new(ControlP2pOrRelayConnectorConfig {
            control_server_url: config.control_server_url.clone(),
            control_token: None,
            mobile_grant: Some(grant),
            client_id: config.client_id.clone(),
            control_client_options: config.control_client_options,
            relay_server_cert: p2p_or_relay.relay_server_cert,
            bind_addr: p2p_or_relay.bind_addr,
            candidate_timeout: p2p_or_relay.candidate_timeout,
            probe_timeout: p2p_or_relay.probe_timeout,
            interval: p2p_or_relay.interval,
            relay_fallback_delay: p2p_or_relay.relay_fallback_delay,
        })?;
        let transport_stats = connector.transport_stats_handle();
        Self::with_connector_and_transport_unvalidated(
            config,
            Arc::new(connector),
            TunnelPath::P2p,
            transport_stats,
        )
        .await
    }

    pub async fn with_connector(
        config: TunnelConfig,
        connector: Arc<dyn StreamConnector>,
    ) -> Result<Self, TunnelError> {
        Self::with_connector_and_transport(
            config,
            connector,
            PathSelector::default().select(),
            TunnelTransportStatsHandle::default(),
        )
        .await
    }

    async fn with_connector_and_transport(
        config: TunnelConfig,
        connector: Arc<dyn StreamConnector>,
        preferred_path: TunnelPath,
        transport_stats: TunnelTransportStatsHandle,
    ) -> Result<Self, TunnelError> {
        config.validate()?;

        Ok(Self {
            state: Arc::new(RwLock::new(ClientState {
                config,
                preferred_path,
                transport_stats,
                forwards: HashMap::new(),
                next_handle_id: 1,
            })),
            connector,
        })
    }

    async fn with_connector_and_transport_unvalidated(
        config: TunnelConfig,
        connector: Arc<dyn StreamConnector>,
        preferred_path: TunnelPath,
        transport_stats: TunnelTransportStatsHandle,
    ) -> Result<Self, TunnelError> {
        Ok(Self {
            state: Arc::new(RwLock::new(ClientState {
                config,
                preferred_path,
                transport_stats,
                forwards: HashMap::new(),
                next_handle_id: 1,
            })),
            connector,
        })
    }

    pub async fn open_service(
        &self,
        request: OpenServiceRequest,
    ) -> Result<LocalForwardHandle, TunnelError> {
        request.validate()?;

        let forwarder = LocalForwarder::bind(
            OpenForwardRequest {
                device_id: request.device_id.clone(),
                service_id: request.service_id.clone(),
                local_port: request.local_port,
            },
            Arc::clone(&self.connector),
        )
        .await?;

        let mut state = self.state.write().expect("mobile client lock poisoned");
        let handle_id = format!("forward_{}", state.next_handle_id);
        state.next_handle_id += 1;

        let handle = LocalForwardHandle::new(
            handle_id.clone(),
            request.device_id,
            request.service_id,
            forwarder.local_port(),
        );
        state
            .forwards
            .insert(handle_id, ActiveForward { forwarder });

        Ok(handle)
    }

    pub async fn close_service(&self, handle_id: String) -> Result<(), TunnelError> {
        let active = {
            let mut state = self.state.write().expect("mobile client lock poisoned");
            state
                .forwards
                .remove(&handle_id)
                .ok_or(TunnelError::ForwardNotFound { handle_id })?
        };
        active.forwarder.shutdown().await?;
        Ok(())
    }

    pub async fn shutdown(&self) -> Result<(), TunnelError> {
        let forwards = {
            let mut state = self.state.write().expect("mobile client lock poisoned");
            std::mem::take(&mut state.forwards)
        };

        for active in forwards.into_values() {
            active.forwarder.shutdown().await?;
        }

        Ok(())
    }

    pub async fn start_browser_proxy(
        &self,
        config: BrowserProxyConfig,
    ) -> Result<BrowserProxy, TunnelError> {
        BrowserProxy::bind(config, Arc::clone(&self.connector))
            .await
            .map_err(TunnelError::BrowserProxy)
    }

    pub fn status(&self) -> TunnelStatus {
        let state = self.state.read().expect("mobile client lock poisoned");
        let transport = state.transport_stats.snapshot();
        TunnelStatus {
            state: TunnelState::Started,
            path: transport
                .last_successful_path
                .unwrap_or(state.preferred_path),
            rtt_ms: None,
            uplink_bytes: 0,
            downlink_bytes: 0,
            active_forwards: state.forwards.len(),
            transport,
        }
    }

    pub fn config(&self) -> TunnelConfig {
        self.state
            .read()
            .expect("mobile client lock poisoned")
            .config
            .clone()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenServiceRequest {
    pub device_id: DeviceId,
    pub service_id: ServiceId,
    pub local_port: u16,
}

impl OpenServiceRequest {
    fn validate(&self) -> Result<(), TunnelError> {
        Ok(())
    }
}

fn validate_mobile_grant_config(config: &TunnelConfig) -> Result<(), TunnelError> {
    if config.control_server_url.trim().is_empty() {
        return Err(TunnelError::InvalidConfig {
            reason: "control_server_url is required".to_string(),
        });
    }
    if config.client_id.as_str().trim().is_empty() {
        return Err(TunnelError::InvalidConfig {
            reason: "client_id is required".to_string(),
        });
    }
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum TunnelError {
    #[error("invalid tunnel config: {reason}")]
    InvalidConfig { reason: String },
    #[error("invalid open_service request: {reason}")]
    InvalidOpenServiceRequest { reason: String },
    #[error("local forward handle not found: {handle_id}")]
    ForwardNotFound { handle_id: String },
    #[error("local forward failed: {0}")]
    Forward(#[from] ForwardError),
    #[error("browser proxy failed: {0}")]
    BrowserProxy(#[from] crate::browser_proxy::BrowserProxyError),
}
