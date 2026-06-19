use std::{fmt, time::Duration};

use mobilecode_connect_auth::ControlRole;
use mobilecode_connect_protocol::{
    ClientId, Device, DeviceId, GrantSessionRequest, MobilePairingRequest,
    PendingGrantSessionStatus, PendingPairingStatus, RelayLimits, Service, ServiceId, SessionId,
    TrafficStats, UserId,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    time::{sleep, timeout},
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateSessionRequest {
    pub client_id: String,
    pub device_id: DeviceId,
    pub service_id: ServiceId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateSessionResponse {
    pub session_id: SessionId,
    pub access_token: String,
    pub relay_token: String,
    pub relay_addr: String,
    pub punch_addr: String,
    pub agent_p2p_cert_der: Option<Vec<u8>>,
    pub expire_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartMobilePairingResponse {
    pub pending_pairing_id: String,
    pub poll_interval_ms: u64,
    pub expires_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingMobilePairingRequest {
    pub pending_pairing_id: String,
    pub request: MobilePairingRequest,
    pub expires_at: u64,
    pub status: PendingPairingStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApproveMobilePairingRequest {
    pub grant_id: String,
    pub allowed_services: Vec<ServiceId>,
    pub revocation_version: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DenyMobileGrantRequest {
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovedMobileGrantMetadata {
    pub version: u32,
    pub device_id: DeviceId,
    pub grant_id: String,
    pub client_id: ClientId,
    pub allowed_services: Vec<ServiceId>,
    pub revocation_version: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MobilePairingPollResponse {
    pub pending_pairing_id: String,
    pub status: PendingPairingStatus,
    pub grant: Option<ApprovedMobileGrantMetadata>,
    pub denied_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartGrantSessionResponse {
    pub pending_session_id: String,
    pub poll_interval_ms: u64,
    pub expires_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingGrantSessionRequest {
    pub pending_session_id: String,
    pub request: GrantSessionRequest,
    pub expires_at: u64,
    pub status: PendingGrantSessionStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApproveGrantSessionRequest {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrantSessionPollResponse {
    pub pending_session_id: String,
    pub status: PendingGrantSessionStatus,
    pub session: Option<CreateSessionResponse>,
    pub denied_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceAccessGrant {
    pub device_id: DeviceId,
    pub user_id: UserId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrantDeviceAccessRequest {
    pub user_id: UserId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegisterP2pCertificateRequest {
    pub certificate_der: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegisterUserRequest {
    pub email: String,
    pub password: String,
    pub display_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdatePasswordRequest {
    pub current_password: Option<String>,
    pub new_password: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthResponse {
    pub user_id: UserId,
    pub access_token: String,
    pub expire_at: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OAuthProvider {
    #[serde(rename = "github")]
    GitHub,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OAuthIdentity {
    pub provider: OAuthProvider,
    pub provider_user_id: String,
    pub user_id: UserId,
    pub email: String,
    pub login: String,
    pub avatar_url: String,
    pub created_epoch_sec: u64,
    pub updated_epoch_sec: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServerAuthMode {
    Browser,
    DeviceCode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServerAuthStatus {
    Pending,
    Approved,
    Denied,
    Expired,
    Consumed,
    AuthorizationPending,
    SlowDown,
    AccessDenied,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartServerAuthRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_id: Option<DeviceId>,
    pub device_name: String,
    pub server_public_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrowserServerAuthStartResponse {
    pub session_id: String,
    pub auth_url: String,
    pub expires_in: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrowserServerAuthApprovalResponse {
    pub session_id: String,
    pub server_auth_code: String,
    pub status: ServerAuthStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrowserServerAuthExchangeRequest {
    pub session_id: String,
    pub server_auth_code: String,
    pub server_public_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceServerAuthStartResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub verification_uri_complete: String,
    pub expires_in: u64,
    pub interval: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceServerAuthApprovalResponse {
    pub user_code: String,
    pub status: ServerAuthStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerAuthSessionDetail {
    pub session_id: String,
    pub mode: ServerAuthMode,
    pub status: ServerAuthStatus,
    pub device_id: DeviceId,
    pub device_name: String,
    pub server_public_key_fingerprint: String,
    pub expires_epoch_sec: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PollServerAuthRequest {
    pub device_code: String,
    pub server_public_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceServerAuthPollResponse {
    pub status: ServerAuthStatus,
    pub interval: u64,
    pub credential: Option<ServerCredentialResponse>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerCredentialResponse {
    pub credential_id: String,
    pub device_id: DeviceId,
    pub server_token: String,
    pub token_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerCredentialSummary {
    pub credential_id: String,
    pub user_id: UserId,
    pub device_id: DeviceId,
    pub device_name: String,
    pub enabled: bool,
    pub token_version: u64,
    pub created_epoch_sec: u64,
    pub last_used_epoch_sec: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateServerCredentialStatusRequest {
    pub enabled: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DashboardSummary {
    pub users: DashboardUserStats,
    pub devices: DashboardDeviceStats,
    pub controllers: DashboardControllerStats,
    pub sessions: DashboardSessionStats,
    pub relays: DashboardRelayStats,
    pub usage: DashboardUsageStats,
    pub recent_audit_logs: Vec<AuditLogEntry>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DashboardUserStats {
    pub total: u64,
    pub enabled: u64,
    pub admins: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DashboardDeviceStats {
    pub total: u64,
    pub online: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DashboardControllerStats {
    pub total: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DashboardSessionStats {
    pub total: u64,
    pub pending: u64,
    pub claimed: u64,
    pub bound: u64,
    pub closed: u64,
    pub expired: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DashboardRelayStats {
    pub total: u64,
    pub healthy: u64,
    pub unhealthy: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DashboardUsageStats {
    pub actual_uplink_bytes: u64,
    pub actual_downlink_bytes: u64,
    pub actual_total_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub total: u64,
    pub limit: u32,
    pub offset: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdminListQuery {
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    pub sort: Option<String>,
    pub q: Option<String>,
    pub role: Option<String>,
    pub enabled: Option<bool>,
    pub status: Option<String>,
    pub user_id: Option<UserId>,
    pub device_id: Option<DeviceId>,
    pub healthy: Option<bool>,
    pub action: Option<String>,
    pub target_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditLogEntry {
    pub audit_id: String,
    pub actor_user_id: UserId,
    pub actor_subject: String,
    pub actor_role: ControlRole,
    pub action: String,
    pub target_type: String,
    pub target_id: String,
    pub message: String,
    pub created_epoch_sec: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserSummary {
    pub user_id: UserId,
    pub email: String,
    pub display_name: String,
    pub role: ControlRole,
    pub enabled: bool,
    pub plan_id: String,
    pub controller_count: u32,
    pub device_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserDetail {
    pub user: UserSummary,
    pub plan: Plan,
    pub controllers: Vec<ControllerDevice>,
    pub devices: Vec<Device>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserUsageSummary {
    pub user_id: UserId,
    pub email: String,
    pub plan_id: String,
    #[serde(default)]
    pub current_period_started_epoch_sec: u64,
    pub max_controller_devices: u32,
    pub controller_count: u32,
    pub device_count: u32,
    pub session_count: u64,
    pub pending_sessions: u64,
    pub claimed_sessions: u64,
    pub bound_sessions: u64,
    pub closed_sessions: u64,
    pub expired_sessions: u64,
    pub current_session_quota_bytes: u64,
    pub relay_quota_granted_bytes: u64,
    pub actual_uplink_bytes: u64,
    pub actual_downlink_bytes: u64,
    pub actual_total_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserUsagePeriod {
    pub user_id: UserId,
    pub current_period_started_epoch_sec: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdminSessionSummary {
    pub session_id: SessionId,
    pub user_id: UserId,
    pub user_email: String,
    pub device_id: DeviceId,
    pub device_name: String,
    pub service_id: ServiceId,
    pub service_name: String,
    pub client_id: ClientId,
    pub status: AgentSessionStatus,
    pub relay_addr: String,
    pub punch_addr: String,
    pub expire_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelaySessionUsageReport {
    pub session_id: SessionId,
    pub stats: TrafficStats,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportRelaySessionUsageRequest {
    pub relay_id: String,
    pub sessions: Vec<RelaySessionUsageReport>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateUserRequest {
    pub email: String,
    pub password: String,
    pub display_name: String,
    pub role: ControlRole,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateUserStatusRequest {
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateUserRoleRequest {
    pub role: ControlRole,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControllerDevice {
    pub user_id: UserId,
    pub client_id: ClientId,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegisterControllerDeviceRequest {
    pub client_id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Plan {
    pub plan_id: String,
    pub name: String,
    pub max_controller_devices: u32,
    pub relay_limits: RelayLimits,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateUserPlanRequest {
    pub plan: Plan,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdatePlanCatalogRequest {
    pub plan: Plan,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssignUserPlanRequest {
    pub plan_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelayCredential {
    pub relay_id: String,
    pub enabled: bool,
    pub token_version: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateRelayCredentialRequest {
    pub relay_id: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateRelayCredentialStatusRequest {
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateRelayBootstrapRequest {
    pub relay_id: String,
    pub control_url: String,
    pub relay_addr: String,
    #[serde(default)]
    pub admin_addr: String,
    pub capacity_streams: u32,
    pub heartbeat_interval_sec: u64,
    pub ttl_sec: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelayBootstrapResponse {
    pub bootstrap_id: String,
    pub relay_id: String,
    pub control_url: String,
    pub expires_epoch_sec: u64,
    pub install_command: String,
    #[serde(default)]
    pub no_service_install_command: String,
    pub bootstrap_token: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelayBootstrapExchangeRequest {
    pub bootstrap_token: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelayBootstrapExchangeResponse {
    pub control_url: String,
    pub control_token: String,
    pub relay_id: String,
    pub token_secret: String,
    pub relay_addr: String,
    #[serde(default)]
    pub admin_addr: String,
    pub capacity_streams: u32,
    pub heartbeat_interval_sec: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegisterRelayRequest {
    pub relay_id: String,
    pub relay_addr: String,
    #[serde(default)]
    pub admin_addr: String,
    pub capacity_streams: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateRelayRequest {
    pub relay_addr: String,
    #[serde(default)]
    pub admin_addr: String,
    pub capacity_streams: u32,
    pub healthy: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RelayHealthStatus {
    #[default]
    Healthy,
    Degraded,
    Unhealthy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RelayHealthReport {
    pub status: RelayHealthStatus,
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub relay_version: String,
    #[serde(default)]
    pub uptime_sec: u64,
    #[serde(default)]
    pub active_sessions: u64,
    #[serde(default)]
    pub active_streams: u64,
    #[serde(default)]
    pub total_uplink_bytes: u64,
    #[serde(default)]
    pub total_downlink_bytes: u64,
    #[serde(default)]
    pub total_bytes: u64,
    #[serde(default)]
    pub data_plane_bound: bool,
    #[serde(default)]
    pub admin_bound: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelaySessionSnapshot {
    pub session_id: SessionId,
    pub state: String,
    pub mobile_bound: bool,
    pub agent_bound: bool,
    pub limits: RelayLimits,
    pub stats: TrafficStats,
    #[serde(default)]
    pub last_seen_epoch_sec: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelayCommandKind {
    DisconnectSession,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelayCommandStatus {
    Pending,
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelayCommand {
    pub command_id: String,
    pub relay_id: String,
    pub kind: RelayCommandKind,
    pub session_id: Option<SessionId>,
    pub status: RelayCommandStatus,
    pub requested_epoch_sec: u64,
    pub updated_epoch_sec: u64,
    #[serde(default)]
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportRelayCommandResultRequest {
    pub status: RelayCommandStatus,
    #[serde(default)]
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportRelayHealthRequest {
    pub relay_addr: String,
    #[serde(default)]
    pub admin_addr: String,
    pub capacity_streams: u32,
    pub health: RelayHealthReport,
    #[serde(default)]
    pub sessions: Vec<RelaySessionSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelayNode {
    pub relay_id: String,
    pub relay_addr: String,
    #[serde(default)]
    pub admin_addr: String,
    pub capacity_streams: u32,
    pub healthy: bool,
    #[serde(default)]
    pub last_seen_epoch_sec: u64,
    #[serde(default)]
    pub health_status: RelayHealthStatus,
    #[serde(default)]
    pub health_reason: String,
    #[serde(default)]
    pub relay_version: String,
    #[serde(default)]
    pub uptime_sec: u64,
    #[serde(default)]
    pub active_sessions: u64,
    #[serde(default)]
    pub active_streams: u64,
    #[serde(default)]
    pub total_uplink_bytes: u64,
    #[serde(default)]
    pub total_downlink_bytes: u64,
    #[serde(default)]
    pub total_bytes: u64,
    #[serde(default)]
    pub data_plane_bound: bool,
    #[serde(default)]
    pub admin_bound: bool,
    #[serde(default)]
    pub last_health_report_epoch_sec: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentSessionAssignment {
    pub session_id: SessionId,
    #[serde(default = "empty_user_id")]
    pub user_id: UserId,
    pub device_id: DeviceId,
    pub service_id: ServiceId,
    pub client_id: ClientId,
    pub relay_token: String,
    pub relay_addr: String,
    pub punch_addr: String,
    pub expire_at: u64,
    pub status: AgentSessionStatus,
    #[serde(default)]
    pub grant_id: Option<String>,
    #[serde(default)]
    pub grant_revocation_version: Option<u64>,
    #[serde(default)]
    pub grant_service_id: Option<ServiceId>,
}

fn empty_user_id() -> UserId {
    UserId::new("")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentSessionStatus {
    Pending,
    Claimed,
    Bound,
    Closed,
    Expired,
}

#[derive(Debug, Clone)]
pub struct HttpControlClient {
    endpoint: ControlEndpoint,
    bearer_token: Option<String>,
    options: HttpControlClientOptions,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HttpControlClientOptions {
    request_timeout: Option<Duration>,
    max_retries: u32,
    retry_backoff: Duration,
}

impl HttpControlClientOptions {
    pub fn request_timeout(&self) -> Option<Duration> {
        self.request_timeout
    }

    pub fn max_retries(&self) -> u32 {
        self.max_retries
    }

    pub fn retry_backoff(&self) -> Duration {
        self.retry_backoff
    }

    pub fn with_request_timeout(mut self, timeout: Duration) -> Self {
        self.request_timeout = Some(timeout);
        self
    }

    pub fn without_request_timeout(mut self) -> Self {
        self.request_timeout = None;
        self
    }

    pub fn with_max_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = max_retries;
        self
    }

    pub fn with_retry_backoff(mut self, retry_backoff: Duration) -> Self {
        self.retry_backoff = retry_backoff;
        self
    }
}

impl Default for HttpControlClientOptions {
    fn default() -> Self {
        Self {
            request_timeout: None,
            max_retries: 0,
            retry_backoff: Duration::from_millis(0),
        }
    }
}

impl HttpControlClient {
    pub fn new(base_url: impl AsRef<str>) -> Result<Self, ControlClientError> {
        Self::with_options(base_url, HttpControlClientOptions::default())
    }

    pub fn with_options(
        base_url: impl AsRef<str>,
        options: HttpControlClientOptions,
    ) -> Result<Self, ControlClientError> {
        Ok(Self {
            endpoint: ControlEndpoint::parse(base_url.as_ref())?,
            bearer_token: None,
            options,
        })
    }

    pub fn with_bearer_token(
        base_url: impl AsRef<str>,
        bearer_token: impl Into<String>,
    ) -> Result<Self, ControlClientError> {
        Self::with_bearer_token_and_options(
            base_url,
            bearer_token,
            HttpControlClientOptions::default(),
        )
    }

    pub fn with_bearer_token_and_options(
        base_url: impl AsRef<str>,
        bearer_token: impl Into<String>,
        options: HttpControlClientOptions,
    ) -> Result<Self, ControlClientError> {
        Ok(Self {
            endpoint: ControlEndpoint::parse(base_url.as_ref())?,
            bearer_token: Some(bearer_token.into()),
            options,
        })
    }

    pub fn set_bearer_token(&mut self, bearer_token: impl Into<String>) {
        self.bearer_token = Some(bearer_token.into());
    }

    pub fn options(&self) -> &HttpControlClientOptions {
        &self.options
    }

    pub fn with_optional_bearer_token(
        base_url: impl AsRef<str>,
        bearer_token: impl AsRef<str>,
    ) -> Result<Self, ControlClientError> {
        Self::with_optional_bearer_token_and_options(
            base_url,
            bearer_token,
            HttpControlClientOptions::default(),
        )
    }

    pub fn with_optional_bearer_token_and_options(
        base_url: impl AsRef<str>,
        bearer_token: impl AsRef<str>,
        options: HttpControlClientOptions,
    ) -> Result<Self, ControlClientError> {
        let token = bearer_token.as_ref().trim();
        if looks_like_signed_token(token) {
            Self::with_bearer_token_and_options(base_url, token.to_string(), options)
        } else {
            Self::with_options(base_url, options)
        }
    }

    pub async fn register_user(
        &self,
        request: RegisterUserRequest,
    ) -> Result<AuthResponse, ControlClientError> {
        self.post_json("/auth/register", &request).await
    }

    pub async fn login(&self, request: LoginRequest) -> Result<AuthResponse, ControlClientError> {
        self.post_json("/auth/login", &request).await
    }

    pub async fn update_password(
        &self,
        request: UpdatePasswordRequest,
    ) -> Result<(), ControlClientError> {
        self.post_empty("/auth/password", &request).await
    }

    pub async fn list_oauth_identities(&self) -> Result<Page<OAuthIdentity>, ControlClientError> {
        self.get_json("/oauth/identities").await
    }

    pub async fn list_oauth_identities_with_query(
        &self,
        query: AdminListQuery,
    ) -> Result<Page<OAuthIdentity>, ControlClientError> {
        self.get_json(&admin_query_path("/oauth/identities", query))
            .await
    }

    pub async fn oauth_identity(
        &self,
        provider: OAuthProvider,
        provider_user_id: &str,
    ) -> Result<OAuthIdentity, ControlClientError> {
        self.get_json(&format!(
            "/oauth/identities/{}/{}",
            oauth_provider_path(provider),
            provider_user_id
        ))
        .await
    }

    pub async fn unlink_oauth_identity(
        &self,
        provider: OAuthProvider,
        provider_user_id: &str,
    ) -> Result<(), ControlClientError> {
        self.delete_empty(&format!(
            "/oauth/identities/{}/{}",
            oauth_provider_path(provider),
            provider_user_id
        ))
        .await
    }

    pub async fn start_browser_server_auth(
        &self,
        request: StartServerAuthRequest,
    ) -> Result<BrowserServerAuthStartResponse, ControlClientError> {
        self.post_json("/server-auth/browser/start", &request).await
    }

    pub async fn browser_server_auth_session_detail(
        &self,
        session_id: &str,
    ) -> Result<ServerAuthSessionDetail, ControlClientError> {
        self.get_json(&format!(
            "/server-auth/browser/session?session_id={}",
            encode_query_value(session_id)
        ))
        .await
    }

    pub async fn approve_browser_server_auth(
        &self,
        session_id: &str,
    ) -> Result<BrowserServerAuthApprovalResponse, ControlClientError> {
        self.get_json(&format!(
            "/server-auth/browser/approve?session_id={}",
            encode_query_value(session_id)
        ))
        .await
    }

    pub async fn exchange_browser_server_auth(
        &self,
        request: BrowserServerAuthExchangeRequest,
    ) -> Result<ServerCredentialResponse, ControlClientError> {
        self.post_json("/server-auth/browser/exchange", &request)
            .await
    }

    pub async fn start_device_server_auth(
        &self,
        request: StartServerAuthRequest,
    ) -> Result<DeviceServerAuthStartResponse, ControlClientError> {
        self.post_json("/server-auth/device/start", &request).await
    }

    pub async fn device_server_auth_session_detail(
        &self,
        user_code: &str,
    ) -> Result<ServerAuthSessionDetail, ControlClientError> {
        self.get_json(&format!(
            "/server-auth/device/session?user_code={}",
            encode_query_value(user_code)
        ))
        .await
    }

    pub async fn approve_device_server_auth(
        &self,
        user_code: &str,
    ) -> Result<DeviceServerAuthApprovalResponse, ControlClientError> {
        self.get_json(&format!(
            "/server-auth/device?user_code={}",
            encode_query_value(user_code)
        ))
        .await
    }

    pub async fn deny_device_server_auth(
        &self,
        user_code: &str,
    ) -> Result<DeviceServerAuthApprovalResponse, ControlClientError> {
        self.get_json(&format!(
            "/server-auth/device?user_code={}&decision=deny",
            encode_query_value(user_code)
        ))
        .await
    }

    pub async fn poll_device_server_auth(
        &self,
        request: PollServerAuthRequest,
    ) -> Result<DeviceServerAuthPollResponse, ControlClientError> {
        self.post_json("/server-auth/device/poll", &request).await
    }

    pub async fn list_server_credentials(
        &self,
    ) -> Result<Page<ServerCredentialSummary>, ControlClientError> {
        self.get_json("/server-credentials").await
    }

    pub async fn list_server_credentials_with_query(
        &self,
        query: AdminListQuery,
    ) -> Result<Page<ServerCredentialSummary>, ControlClientError> {
        self.get_json(&admin_query_path("/server-credentials", query))
            .await
    }

    pub async fn server_credential(
        &self,
        credential_id: &str,
    ) -> Result<ServerCredentialSummary, ControlClientError> {
        self.get_json(&format!("/server-credentials/{credential_id}"))
            .await
    }

    pub async fn rotate_server_credential(
        &self,
        credential_id: &str,
    ) -> Result<ServerCredentialResponse, ControlClientError> {
        self.post_no_body_json(&format!("/server-credentials/{credential_id}/rotate"))
            .await
    }

    pub async fn update_server_credential_status(
        &self,
        credential_id: &str,
        request: UpdateServerCredentialStatusRequest,
    ) -> Result<ServerCredentialSummary, ControlClientError> {
        self.post_json(
            &format!("/server-credentials/{credential_id}/status"),
            &request,
        )
        .await
    }

    pub async fn dashboard(&self) -> Result<DashboardSummary, ControlClientError> {
        self.get_json("/dashboard").await
    }

    pub async fn register_controller(
        &self,
        request: RegisterControllerDeviceRequest,
    ) -> Result<ControllerDevice, ControlClientError> {
        self.post_json("/controllers/register", &request).await
    }

    pub async fn list_controllers(&self) -> Result<Page<ControllerDevice>, ControlClientError> {
        self.get_json("/controllers").await
    }

    pub async fn list_controllers_with_query(
        &self,
        query: AdminListQuery,
    ) -> Result<Page<ControllerDevice>, ControlClientError> {
        self.get_json(&admin_query_path("/controllers", query))
            .await
    }

    pub async fn remove_controller(&self, client_id: &ClientId) -> Result<(), ControlClientError> {
        self.delete_empty(&format!("/controllers/{client_id}"))
            .await
    }

    pub async fn list_users(&self) -> Result<Page<UserSummary>, ControlClientError> {
        self.get_json("/users").await
    }

    pub async fn list_users_with_query(
        &self,
        query: AdminListQuery,
    ) -> Result<Page<UserSummary>, ControlClientError> {
        self.get_json(&admin_query_path("/users", query)).await
    }

    pub async fn audit_logs(&self) -> Result<Page<AuditLogEntry>, ControlClientError> {
        self.get_json("/audit-logs").await
    }

    pub async fn audit_logs_with_query(
        &self,
        query: AdminListQuery,
    ) -> Result<Page<AuditLogEntry>, ControlClientError> {
        self.get_json(&admin_query_path("/audit-logs", query)).await
    }

    pub async fn user_usage_summaries(&self) -> Result<Page<UserUsageSummary>, ControlClientError> {
        self.get_json("/usage/users").await
    }

    pub async fn user_usage_summaries_with_query(
        &self,
        query: AdminListQuery,
    ) -> Result<Page<UserUsageSummary>, ControlClientError> {
        self.get_json(&admin_query_path("/usage/users", query))
            .await
    }

    pub async fn report_relay_session_usage(
        &self,
        request: ReportRelaySessionUsageRequest,
    ) -> Result<(), ControlClientError> {
        self.post_empty("/usage/relay-sessions", &request).await
    }

    pub async fn reset_user_usage_period(
        &self,
        user_id: &UserId,
    ) -> Result<UserUsagePeriod, ControlClientError> {
        self.post_no_body_json(&format!("/usage/users/{user_id}/reset"))
            .await
    }

    pub async fn create_user(
        &self,
        request: CreateUserRequest,
    ) -> Result<UserSummary, ControlClientError> {
        self.post_json("/users", &request).await
    }

    pub async fn user(&self, user_id: &UserId) -> Result<UserDetail, ControlClientError> {
        self.get_json(&format!("/users/{user_id}")).await
    }

    pub async fn update_user_status(
        &self,
        user_id: &UserId,
        request: UpdateUserStatusRequest,
    ) -> Result<UserSummary, ControlClientError> {
        self.post_json(&format!("/users/{user_id}/status"), &request)
            .await
    }

    pub async fn update_user_role(
        &self,
        user_id: &UserId,
        request: UpdateUserRoleRequest,
    ) -> Result<UserSummary, ControlClientError> {
        self.post_json(&format!("/users/{user_id}/role"), &request)
            .await
    }

    pub async fn current_plan(&self) -> Result<Plan, ControlClientError> {
        self.get_json("/plans/current").await
    }

    pub async fn plan_catalog(&self) -> Result<Page<Plan>, ControlClientError> {
        self.get_json("/plans/catalog").await
    }

    pub async fn plan_catalog_with_query(
        &self,
        query: AdminListQuery,
    ) -> Result<Page<Plan>, ControlClientError> {
        self.get_json(&admin_query_path("/plans/catalog", query))
            .await
    }

    pub async fn catalog_plan(&self, plan_id: &str) -> Result<Plan, ControlClientError> {
        self.get_json(&format!("/plans/catalog/{plan_id}")).await
    }

    pub async fn update_catalog_plan(
        &self,
        request: UpdatePlanCatalogRequest,
    ) -> Result<Plan, ControlClientError> {
        self.post_json("/plans/catalog", &request).await
    }

    pub async fn user_plan(&self, user_id: &UserId) -> Result<Plan, ControlClientError> {
        self.get_json(&format!("/plans/users/{user_id}")).await
    }

    pub async fn assign_user_plan(
        &self,
        user_id: &UserId,
        request: AssignUserPlanRequest,
    ) -> Result<Plan, ControlClientError> {
        self.post_json(&format!("/plans/users/{user_id}/assign"), &request)
            .await
    }

    pub async fn update_user_plan(
        &self,
        user_id: &UserId,
        request: UpdateUserPlanRequest,
    ) -> Result<Plan, ControlClientError> {
        self.post_json(&format!("/plans/users/{user_id}"), &request)
            .await
    }

    pub async fn list_relay_credentials(
        &self,
    ) -> Result<Page<RelayCredential>, ControlClientError> {
        self.get_json("/relay-credentials").await
    }

    pub async fn list_relay_credentials_with_query(
        &self,
        query: AdminListQuery,
    ) -> Result<Page<RelayCredential>, ControlClientError> {
        self.get_json(&admin_query_path("/relay-credentials", query))
            .await
    }

    pub async fn create_relay_credential(
        &self,
        request: CreateRelayCredentialRequest,
    ) -> Result<RelayCredential, ControlClientError> {
        self.post_json("/relay-credentials", &request).await
    }

    pub async fn relay_credential(
        &self,
        relay_id: &str,
    ) -> Result<RelayCredential, ControlClientError> {
        self.get_json(&format!("/relay-credentials/{relay_id}"))
            .await
    }

    pub async fn update_relay_credential_status(
        &self,
        relay_id: &str,
        request: UpdateRelayCredentialStatusRequest,
    ) -> Result<RelayCredential, ControlClientError> {
        self.post_json(&format!("/relay-credentials/{relay_id}/status"), &request)
            .await
    }

    pub async fn rotate_relay_credential(
        &self,
        relay_id: &str,
    ) -> Result<RelayCredential, ControlClientError> {
        self.post_no_body_json(&format!("/relay-credentials/{relay_id}/rotate"))
            .await
    }

    pub async fn create_relay_bootstrap(
        &self,
        request: CreateRelayBootstrapRequest,
    ) -> Result<RelayBootstrapResponse, ControlClientError> {
        self.post_json("/relay-bootstraps", &request).await
    }

    pub async fn exchange_relay_bootstrap(
        &self,
        bootstrap_id: &str,
        request: RelayBootstrapExchangeRequest,
    ) -> Result<RelayBootstrapExchangeResponse, ControlClientError> {
        self.post_json(
            &format!("/relay-bootstraps/{bootstrap_id}/exchange"),
            &request,
        )
        .await
    }

    pub async fn register_relay(
        &self,
        request: RegisterRelayRequest,
    ) -> Result<RelayNode, ControlClientError> {
        self.post_json("/relays/register", &request).await
    }

    pub async fn update_relay(
        &self,
        relay_id: &str,
        request: UpdateRelayRequest,
    ) -> Result<RelayNode, ControlClientError> {
        self.post_json(&format!("/relays/{relay_id}"), &request)
            .await
    }

    pub async fn report_relay_health(
        &self,
        relay_id: &str,
        request: ReportRelayHealthRequest,
    ) -> Result<RelayNode, ControlClientError> {
        self.post_json(&format!("/relays/{relay_id}/health"), &request)
            .await
    }

    pub async fn relay(&self, relay_id: &str) -> Result<RelayNode, ControlClientError> {
        self.get_json(&format!("/relays/{relay_id}")).await
    }

    pub async fn relay_sessions(
        &self,
        relay_id: &str,
    ) -> Result<Page<RelaySessionSnapshot>, ControlClientError> {
        self.get_json(&format!("/relays/{relay_id}/sessions")).await
    }

    pub async fn relay_sessions_with_query(
        &self,
        relay_id: &str,
        query: AdminListQuery,
    ) -> Result<Page<RelaySessionSnapshot>, ControlClientError> {
        self.get_json(&admin_query_path(
            &format!("/relays/{relay_id}/sessions"),
            query,
        ))
        .await
    }

    pub async fn request_relay_session_disconnect(
        &self,
        relay_id: &str,
        session_id: &SessionId,
    ) -> Result<RelayCommand, ControlClientError> {
        self.post_no_body_json(&format!(
            "/relays/{relay_id}/sessions/{session_id}/disconnect"
        ))
        .await
    }

    pub async fn pending_relay_commands(
        &self,
        relay_id: &str,
    ) -> Result<Vec<RelayCommand>, ControlClientError> {
        self.get_json(&format!("/relays/{relay_id}/commands")).await
    }

    pub async fn report_relay_command_result(
        &self,
        relay_id: &str,
        command_id: &str,
        request: ReportRelayCommandResultRequest,
    ) -> Result<RelayCommand, ControlClientError> {
        self.post_json(
            &format!("/relays/{relay_id}/commands/{command_id}/result"),
            &request,
        )
        .await
    }

    pub async fn remove_relay(&self, relay_id: &str) -> Result<(), ControlClientError> {
        self.delete_empty(&format!("/relays/{relay_id}")).await
    }

    pub async fn list_relays(&self) -> Result<Page<RelayNode>, ControlClientError> {
        self.get_json("/relays").await
    }

    pub async fn list_relays_with_query(
        &self,
        query: AdminListQuery,
    ) -> Result<Page<RelayNode>, ControlClientError> {
        self.get_json(&admin_query_path("/relays", query)).await
    }

    pub async fn register_device(&self, device: Device) -> Result<(), ControlClientError> {
        self.post_empty("/agent/register", &device).await
    }

    pub async fn register_services(
        &self,
        services: Vec<Service>,
    ) -> Result<(), ControlClientError> {
        self.post_empty("/agent/services", &services).await
    }

    pub async fn register_p2p_certificate(
        &self,
        device_id: &DeviceId,
        certificate_der: Vec<u8>,
    ) -> Result<(), ControlClientError> {
        self.post_empty(
            &format!("/agent/devices/{device_id}/p2p-cert"),
            &RegisterP2pCertificateRequest { certificate_der },
        )
        .await
    }

    pub async fn list_devices(&self) -> Result<Vec<Device>, ControlClientError> {
        self.get_json("/mobile/devices").await
    }

    pub async fn list_controlled_devices(&self) -> Result<Page<Device>, ControlClientError> {
        self.get_json("/devices").await
    }

    pub async fn list_controlled_devices_with_query(
        &self,
        query: AdminListQuery,
    ) -> Result<Page<Device>, ControlClientError> {
        self.get_json(&admin_query_path("/devices", query)).await
    }

    pub async fn controlled_device(
        &self,
        device_id: &DeviceId,
    ) -> Result<Device, ControlClientError> {
        self.get_json(&format!("/devices/{device_id}")).await
    }

    pub async fn device_access_grants(
        &self,
        device_id: &DeviceId,
    ) -> Result<Page<DeviceAccessGrant>, ControlClientError> {
        self.get_json(&format!("/devices/{device_id}/access")).await
    }

    pub async fn device_access_grants_with_query(
        &self,
        device_id: &DeviceId,
        query: AdminListQuery,
    ) -> Result<Page<DeviceAccessGrant>, ControlClientError> {
        self.get_json(&admin_query_path(
            &format!("/devices/{device_id}/access"),
            query,
        ))
        .await
    }

    pub async fn grant_device_access(
        &self,
        device_id: &DeviceId,
        request: GrantDeviceAccessRequest,
    ) -> Result<DeviceAccessGrant, ControlClientError> {
        self.post_json(&format!("/devices/{device_id}/access"), &request)
            .await
    }

    pub async fn revoke_device_access(
        &self,
        device_id: &DeviceId,
        user_id: &UserId,
    ) -> Result<(), ControlClientError> {
        self.delete_empty(&format!("/devices/{device_id}/access/{user_id}"))
            .await
    }

    pub async fn remove_controlled_device(
        &self,
        device_id: &DeviceId,
    ) -> Result<(), ControlClientError> {
        self.delete_empty(&format!("/devices/{device_id}")).await
    }

    pub async fn list_device_services(
        &self,
        device_id: &DeviceId,
    ) -> Result<Vec<Service>, ControlClientError> {
        self.get_json(&format!("/mobile/devices/{device_id}/services"))
            .await
    }

    pub async fn start_mobile_pairing(
        &self,
        request: MobilePairingRequest,
    ) -> Result<StartMobilePairingResponse, ControlClientError> {
        self.post_json("/agent-grants/pairing/start", &request)
            .await
    }

    pub async fn mobile_pairing_result(
        &self,
        pending_pairing_id: &str,
    ) -> Result<MobilePairingPollResponse, ControlClientError> {
        self.get_json(&format!("/agent-grants/pairing/{pending_pairing_id}"))
            .await
    }

    pub async fn list_agent_sessions(
        &self,
        device_id: &DeviceId,
    ) -> Result<Vec<AgentSessionAssignment>, ControlClientError> {
        self.get_json(&format!("/agent/devices/{device_id}/sessions"))
            .await
    }

    pub async fn list_mobile_pairing_requests(
        &self,
        device_id: &DeviceId,
    ) -> Result<Vec<PendingMobilePairingRequest>, ControlClientError> {
        self.get_json(&format!("/agent/devices/{device_id}/pairing-requests"))
            .await
    }

    pub async fn admin_sessions(&self) -> Result<Page<AdminSessionSummary>, ControlClientError> {
        self.get_json("/sessions").await
    }

    pub async fn admin_sessions_with_query(
        &self,
        query: AdminListQuery,
    ) -> Result<Page<AdminSessionSummary>, ControlClientError> {
        self.get_json(&admin_query_path("/sessions", query)).await
    }

    pub async fn claim_agent_session(
        &self,
        session_id: &SessionId,
    ) -> Result<AgentSessionAssignment, ControlClientError> {
        self.post_no_body_json(&format!("/agent/sessions/{session_id}/claim"))
            .await
    }

    pub async fn mark_agent_session_bound(
        &self,
        session_id: &SessionId,
    ) -> Result<AgentSessionAssignment, ControlClientError> {
        self.post_no_body_json(&format!("/agent/sessions/{session_id}/bound"))
            .await
    }

    pub async fn approve_mobile_pairing(
        &self,
        pending_pairing_id: &str,
        request: ApproveMobilePairingRequest,
    ) -> Result<MobilePairingPollResponse, ControlClientError> {
        self.post_json(
            &format!("/agent/pairing/{pending_pairing_id}/approve"),
            &request,
        )
        .await
    }

    pub async fn deny_mobile_pairing(
        &self,
        pending_pairing_id: &str,
        request: DenyMobileGrantRequest,
    ) -> Result<MobilePairingPollResponse, ControlClientError> {
        self.post_json(
            &format!("/agent/pairing/{pending_pairing_id}/deny"),
            &request,
        )
        .await
    }

    pub async fn start_grant_session(
        &self,
        request: GrantSessionRequest,
    ) -> Result<StartGrantSessionResponse, ControlClientError> {
        self.post_json("/agent-grants/sessions/start", &request)
            .await
    }

    pub async fn grant_session_result(
        &self,
        pending_session_id: &str,
    ) -> Result<GrantSessionPollResponse, ControlClientError> {
        self.get_json(&format!("/agent-grants/sessions/{pending_session_id}"))
            .await
    }

    pub async fn list_grant_session_requests(
        &self,
        device_id: &DeviceId,
    ) -> Result<Vec<PendingGrantSessionRequest>, ControlClientError> {
        self.get_json(&format!(
            "/agent/devices/{device_id}/grant-session-requests"
        ))
        .await
    }

    pub async fn approve_grant_session(
        &self,
        pending_session_id: &str,
    ) -> Result<GrantSessionPollResponse, ControlClientError> {
        self.post_json(
            &format!("/agent/grant-sessions/{pending_session_id}/approve"),
            &ApproveGrantSessionRequest {},
        )
        .await
    }

    pub async fn deny_grant_session(
        &self,
        pending_session_id: &str,
        request: DenyMobileGrantRequest,
    ) -> Result<GrantSessionPollResponse, ControlClientError> {
        self.post_json(
            &format!("/agent/grant-sessions/{pending_session_id}/deny"),
            &request,
        )
        .await
    }

    pub async fn close_session(
        &self,
        session_id: &SessionId,
    ) -> Result<AgentSessionAssignment, ControlClientError> {
        self.post_no_body_json(&format!("/sessions/{session_id}/close"))
            .await
    }

    pub async fn create_session(
        &self,
        request: CreateSessionRequest,
    ) -> Result<CreateSessionResponse, ControlClientError> {
        self.post_json("/sessions", &request).await
    }

    async fn post_empty<T: Serialize>(
        &self,
        path: &str,
        payload: &T,
    ) -> Result<(), ControlClientError> {
        let body = serde_json::to_vec(payload)?;
        let response = self.request("POST", path, Some(body)).await?;
        if response.status_code.is_success() {
            Ok(())
        } else {
            Err(ControlClientError::HttpStatus {
                status_code: response.status_code,
                body: response.body,
            })
        }
    }

    async fn post_json<T, R>(&self, path: &str, payload: &T) -> Result<R, ControlClientError>
    where
        T: Serialize,
        R: DeserializeOwned,
    {
        let body = serde_json::to_vec(payload)?;
        self.decode_json(self.request("POST", path, Some(body)).await?)
    }

    async fn post_no_body_json<R>(&self, path: &str) -> Result<R, ControlClientError>
    where
        R: DeserializeOwned,
    {
        self.decode_json(self.request("POST", path, None).await?)
    }

    async fn delete_empty(&self, path: &str) -> Result<(), ControlClientError> {
        let response = self.request("DELETE", path, None).await?;
        if response.status_code.is_success() {
            Ok(())
        } else {
            Err(ControlClientError::HttpStatus {
                status_code: response.status_code,
                body: response.body,
            })
        }
    }

    async fn get_json<R>(&self, path: &str) -> Result<R, ControlClientError>
    where
        R: DeserializeOwned,
    {
        self.decode_json(self.request("GET", path, None).await?)
    }

    fn decode_json<R>(&self, response: HttpResponse) -> Result<R, ControlClientError>
    where
        R: DeserializeOwned,
    {
        if !response.status_code.is_success() {
            return Err(ControlClientError::HttpStatus {
                status_code: response.status_code,
                body: response.body,
            });
        }

        Ok(serde_json::from_slice(&response.body)?)
    }

    async fn request(
        &self,
        method: &str,
        path: &str,
        body: Option<Vec<u8>>,
    ) -> Result<HttpResponse, ControlClientError> {
        let body = body.unwrap_or_default();
        let mut attempt = 0;
        loop {
            let result = self.request_once(method, path, body.clone()).await;
            match result {
                Ok(response)
                    if response.status_code.is_retryable()
                        && attempt < self.options.max_retries =>
                {
                    attempt += 1;
                    self.sleep_before_retry().await;
                }
                Ok(response) => return Ok(response),
                Err(error) if error.is_retryable() && attempt < self.options.max_retries => {
                    attempt += 1;
                    self.sleep_before_retry().await;
                }
                Err(error) => return Err(error),
            }
        }
    }

    async fn request_once(
        &self,
        method: &str,
        path: &str,
        body: Vec<u8>,
    ) -> Result<HttpResponse, ControlClientError> {
        if let Some(request_timeout) = self.options.request_timeout {
            timeout(
                request_timeout,
                self.request_once_without_timeout(method, path, body),
            )
            .await
            .map_err(|_| ControlClientError::Timeout {
                timeout: request_timeout,
            })?
        } else {
            self.request_once_without_timeout(method, path, body).await
        }
    }

    async fn request_once_without_timeout(
        &self,
        method: &str,
        path: &str,
        body: Vec<u8>,
    ) -> Result<HttpResponse, ControlClientError> {
        let mut stream = TcpStream::connect(self.endpoint.addr()).await?;
        let request = format!(
            "{method} {path} HTTP/1.1\r\nHost: {}\r\n{}Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            self.endpoint.host_header(),
            self.authorization_header(),
            body.len(),
        );
        stream.write_all(request.as_bytes()).await?;
        stream.write_all(&body).await?;
        stream.flush().await?;

        let mut bytes = Vec::new();
        stream.read_to_end(&mut bytes).await?;
        parse_response(&bytes)
    }

    async fn sleep_before_retry(&self) {
        if !self.options.retry_backoff.is_zero() {
            sleep(self.options.retry_backoff).await;
        }
    }

    fn authorization_header(&self) -> String {
        self.bearer_token
            .as_ref()
            .map(|token| format!("Authorization: Bearer {token}\r\n"))
            .unwrap_or_default()
    }
}

fn looks_like_signed_token(token: &str) -> bool {
    let Some((payload, signature)) = token.split_once('.') else {
        return false;
    };
    !payload.is_empty() && !signature.is_empty()
}

fn admin_query_path(path: &str, query: AdminListQuery) -> String {
    let mut params = Vec::new();
    if let Some(limit) = query.limit {
        push_query_param(&mut params, "limit", limit);
    }
    if let Some(offset) = query.offset {
        push_query_param(&mut params, "offset", offset);
    }
    if let Some(sort) = query
        .sort
        .as_deref()
        .map(str::trim)
        .filter(|sort| !sort.is_empty())
    {
        push_query_param(&mut params, "sort", sort);
    }
    if let Some(q) = query.q.as_deref().map(str::trim).filter(|q| !q.is_empty()) {
        push_query_param(&mut params, "q", q);
    }
    if let Some(role) = query
        .role
        .as_deref()
        .map(str::trim)
        .filter(|role| !role.is_empty())
    {
        push_query_param(&mut params, "role", role);
    }
    if let Some(enabled) = query.enabled {
        push_query_param(&mut params, "enabled", enabled);
    }
    if let Some(status) = query
        .status
        .as_deref()
        .map(str::trim)
        .filter(|status| !status.is_empty())
    {
        push_query_param(&mut params, "status", status);
    }
    if let Some(user_id) = query.user_id {
        push_query_param(&mut params, "user_id", user_id);
    }
    if let Some(device_id) = query.device_id {
        push_query_param(&mut params, "device_id", device_id);
    }
    if let Some(healthy) = query.healthy {
        push_query_param(&mut params, "healthy", healthy);
    }
    if let Some(action) = query
        .action
        .as_deref()
        .map(str::trim)
        .filter(|action| !action.is_empty())
    {
        push_query_param(&mut params, "action", action);
    }
    if let Some(target_type) = query
        .target_type
        .as_deref()
        .map(str::trim)
        .filter(|target_type| !target_type.is_empty())
    {
        push_query_param(&mut params, "target_type", target_type);
    }
    if params.is_empty() {
        path.to_string()
    } else {
        format!("{path}?{}", params.join("&"))
    }
}

fn oauth_provider_path(provider: OAuthProvider) -> &'static str {
    match provider {
        OAuthProvider::GitHub => "github",
    }
}

fn push_query_param(params: &mut Vec<String>, name: &str, value: impl fmt::Display) {
    params.push(format!("{name}={}", encode_query_value(&value.to_string())));
}

fn encode_query_value(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                encoded.push(byte as char);
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ControlEndpoint {
    host: String,
    port: u16,
}

impl ControlEndpoint {
    fn parse(base_url: &str) -> Result<Self, ControlClientError> {
        let address =
            base_url
                .strip_prefix("http://")
                .ok_or_else(|| ControlClientError::InvalidBaseUrl {
                    value: base_url.to_string(),
                    reason: "only http:// URLs are supported".to_string(),
                })?;
        let address = address.trim_end_matches('/');
        let (host, port) =
            address
                .rsplit_once(':')
                .ok_or_else(|| ControlClientError::InvalidBaseUrl {
                    value: base_url.to_string(),
                    reason: "missing host:port".to_string(),
                })?;
        if host.trim().is_empty() {
            return Err(ControlClientError::InvalidBaseUrl {
                value: base_url.to_string(),
                reason: "host must not be empty".to_string(),
            });
        }
        let port = port
            .parse::<u16>()
            .map_err(|_| ControlClientError::InvalidBaseUrl {
                value: base_url.to_string(),
                reason: "port must be a valid u16".to_string(),
            })?;

        Ok(Self {
            host: host.to_string(),
            port,
        })
    }

    fn addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    fn host_header(&self) -> String {
        self.addr()
    }
}

struct HttpResponse {
    status_code: StatusCode,
    body: Vec<u8>,
}

fn parse_response(bytes: &[u8]) -> Result<HttpResponse, ControlClientError> {
    let header_end =
        find_header_end(bytes).ok_or_else(|| ControlClientError::MalformedResponse {
            reason: format!("missing header terminator in {} bytes", bytes.len()),
        })?;
    let headers = std::str::from_utf8(&bytes[..header_end]).map_err(|_| {
        ControlClientError::MalformedResponse {
            reason: "headers are not valid utf-8".to_string(),
        }
    })?;
    let status_line =
        headers
            .lines()
            .next()
            .ok_or_else(|| ControlClientError::MalformedResponse {
                reason: "missing status line".to_string(),
            })?;
    let status_code = status_line
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| ControlClientError::MalformedResponse {
            reason: "missing status code".to_string(),
        })?
        .parse::<u16>()
        .map_err(|_| ControlClientError::MalformedResponse {
            reason: "status code is not numeric".to_string(),
        })?;

    Ok(HttpResponse {
        status_code: StatusCode(status_code),
        body: bytes[header_end + 4..].to_vec(),
    })
}

fn find_header_end(bytes: &[u8]) -> Option<usize> {
    bytes.windows(4).position(|window| window == b"\r\n\r\n")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatusCode(u16);

impl StatusCode {
    pub fn from_u16(value: u16) -> Self {
        Self(value)
    }

    pub fn as_u16(self) -> u16 {
        self.0
    }

    fn is_success(self) -> bool {
        (200..300).contains(&self.0)
    }

    fn is_retryable(self) -> bool {
        matches!(self.0, 408 | 429 | 500 | 502 | 503 | 504)
    }
}

impl fmt::Display for StatusCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ControlClientError {
    #[error("invalid control base url {value}: {reason}")]
    InvalidBaseUrl { value: String, reason: String },
    #[error("io failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("request timed out after {timeout:?}")]
    Timeout { timeout: Duration },
    #[error("json failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("http status {status_code}: {body:?}")]
    HttpStatus {
        status_code: StatusCode,
        body: Vec<u8>,
    },
    #[error("malformed http response: {reason}")]
    MalformedResponse { reason: String },
}

impl ControlClientError {
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::Io(_) | Self::Timeout { .. } => true,
            Self::HttpStatus { status_code, .. } => status_code.is_retryable(),
            Self::InvalidBaseUrl { .. } | Self::Json(_) | Self::MalformedResponse { .. } => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn admin_query_path_encodes_supported_filters() {
        let path = admin_query_path(
            "/users",
            AdminListQuery {
                limit: Some(50),
                offset: Some(10),
                sort: Some("-created_epoch_sec".to_string()),
                q: Some("alice admin".to_string()),
                role: Some("admin".to_string()),
                enabled: Some(true),
                status: Some("bound".to_string()),
                user_id: Some(UserId::new("user_abc")),
                device_id: Some(DeviceId::new("device_123")),
                healthy: Some(false),
                action: Some("relay.register".to_string()),
                target_type: Some("relay pool".to_string()),
            },
        );

        assert_eq!(
            path,
            "/users?limit=50&offset=10&sort=-created_epoch_sec&q=alice%20admin&role=admin&enabled=true&status=bound&user_id=user_abc&device_id=device_123&healthy=false&action=relay.register&target_type=relay%20pool"
        );
    }

    #[test]
    fn http_client_exposes_query_methods_for_paginated_admin_lists() {
        let client = HttpControlClient::new("http://127.0.0.1:1").unwrap();
        let query = AdminListQuery {
            limit: Some(1),
            q: Some("needle".to_string()),
            ..AdminListQuery::default()
        };
        let device_id = DeviceId::new("device_001");

        std::mem::drop(client.list_controllers_with_query(query.clone()));
        std::mem::drop(client.list_users_with_query(query.clone()));
        std::mem::drop(client.audit_logs_with_query(query.clone()));
        std::mem::drop(client.user_usage_summaries_with_query(query.clone()));
        std::mem::drop(client.admin_sessions_with_query(query.clone()));
        std::mem::drop(client.plan_catalog_with_query(query.clone()));
        std::mem::drop(client.list_relay_credentials_with_query(query.clone()));
        std::mem::drop(client.list_relays_with_query(query.clone()));
        std::mem::drop(client.list_controlled_devices_with_query(query.clone()));
        std::mem::drop(client.device_access_grants_with_query(&device_id, query));
    }

    #[test]
    fn relay_bootstrap_models_roundtrip_and_client_methods_are_exposed() {
        let request = CreateRelayBootstrapRequest {
            relay_id: "relay_auto".to_string(),
            control_url: "https://control.example.com".to_string(),
            relay_addr: "relay.example.com:4443".to_string(),
            admin_addr: "127.0.0.1:9090".to_string(),
            capacity_streams: 128,
            heartbeat_interval_sec: 30,
            ttl_sec: 900,
        };
        assert_eq!(
            serde_json::from_str::<CreateRelayBootstrapRequest>(
                &serde_json::to_string(&request).unwrap()
            )
            .unwrap(),
            request
        );

        let response = RelayBootstrapResponse {
            bootstrap_id: "rb_001".to_string(),
            relay_id: "relay_auto".to_string(),
            control_url: "https://control.example.com".to_string(),
            expires_epoch_sec: 1_781_097_600,
            install_command: "curl -fsSL https://control.example.com/install-relayd.sh | sudo sh"
                .to_string(),
            no_service_install_command:
                "curl -fsSL https://control.example.com/install-relayd.sh | sudo sh -s -- --no-service"
                    .to_string(),
            bootstrap_token: "shown-once".to_string(),
        };
        assert_eq!(
            serde_json::from_str::<RelayBootstrapResponse>(
                &serde_json::to_string(&response).unwrap()
            )
            .unwrap(),
            response
        );
        let legacy_response_json = serde_json::json!({
            "bootstrap_id": "rb_001",
            "relay_id": "relay_auto",
            "control_url": "https://control.example.com",
            "expires_epoch_sec": 1_781_097_600_u64,
            "install_command": "curl -fsSL https://control.example.com/install-relayd.sh | sudo sh",
            "bootstrap_token": "shown-once",
        });
        let legacy_response =
            serde_json::from_value::<RelayBootstrapResponse>(legacy_response_json).unwrap();
        assert_eq!(legacy_response.no_service_install_command, "");

        let exchange_request = RelayBootstrapExchangeRequest {
            bootstrap_token: "shown-once".to_string(),
        };
        assert_eq!(
            serde_json::from_str::<RelayBootstrapExchangeRequest>(
                &serde_json::to_string(&exchange_request).unwrap()
            )
            .unwrap(),
            exchange_request
        );

        let exchange_response = RelayBootstrapExchangeResponse {
            control_url: "https://control.example.com".to_string(),
            control_token: "relay-control-token".to_string(),
            relay_id: "relay_auto".to_string(),
            token_secret: "relay-data-plane-secret".to_string(),
            relay_addr: "relay.example.com:4443".to_string(),
            admin_addr: "127.0.0.1:9090".to_string(),
            capacity_streams: 128,
            heartbeat_interval_sec: 30,
        };
        assert_eq!(
            serde_json::from_str::<RelayBootstrapExchangeResponse>(
                &serde_json::to_string(&exchange_response).unwrap()
            )
            .unwrap(),
            exchange_response
        );

        let client = HttpControlClient::new("http://127.0.0.1:1").unwrap();
        std::mem::drop(client.create_relay_bootstrap(request));
        std::mem::drop(client.exchange_relay_bootstrap("rb_001", exchange_request));
    }

    #[test]
    fn relay_live_ops_models_roundtrip_and_client_methods_are_exposed() {
        let session_id = SessionId::new("sess_live_ops_001");
        let snapshot = RelaySessionSnapshot {
            session_id: session_id.clone(),
            state: "ready".to_string(),
            mobile_bound: true,
            agent_bound: true,
            limits: RelayLimits {
                max_bps: 8_192,
                max_streams: 16,
                max_duration_sec: 3_600,
                traffic_quota_bytes: 1_048_576,
            },
            stats: TrafficStats {
                session_id: Some(session_id.clone()),
                uplink_bytes: 1,
                downlink_bytes: 2,
                total_bytes: 3,
                duration_sec: 4,
                active_streams: 5,
            },
            last_seen_epoch_sec: 1_781_097_600,
        };
        assert_eq!(
            serde_json::from_str::<RelaySessionSnapshot>(
                &serde_json::to_string(&snapshot).unwrap()
            )
            .unwrap(),
            snapshot
        );

        let command = RelayCommand {
            command_id: "rc_001".to_string(),
            relay_id: "relay_live_ops".to_string(),
            kind: RelayCommandKind::DisconnectSession,
            session_id: Some(session_id.clone()),
            status: RelayCommandStatus::Pending,
            requested_epoch_sec: 1_781_097_600,
            updated_epoch_sec: 1_781_097_600,
            message: String::new(),
        };
        assert_eq!(
            serde_json::from_str::<RelayCommand>(&serde_json::to_string(&command).unwrap())
                .unwrap(),
            command
        );

        let result = ReportRelayCommandResultRequest {
            status: RelayCommandStatus::Succeeded,
            message: "session closed locally".to_string(),
        };
        assert_eq!(
            serde_json::from_str::<ReportRelayCommandResultRequest>(
                &serde_json::to_string(&result).unwrap()
            )
            .unwrap(),
            result
        );

        let legacy_health = serde_json::json!({
            "relay_addr": "relay.example.com:4443",
            "capacity_streams": 128,
            "health": {
                "status": "healthy",
                "total_uplink_bytes": 1_u64,
                "total_downlink_bytes": 2_u64,
                "total_bytes": 3_u64,
                "data_plane_bound": true
            }
        });
        let legacy_health =
            serde_json::from_value::<ReportRelayHealthRequest>(legacy_health).unwrap();
        assert!(legacy_health.sessions.is_empty());

        let client = HttpControlClient::new("http://127.0.0.1:1").unwrap();
        let query = AdminListQuery {
            status: Some("ready".to_string()),
            ..AdminListQuery::default()
        };
        std::mem::drop(client.relay_sessions("relay_live_ops"));
        std::mem::drop(client.relay_sessions_with_query("relay_live_ops", query));
        std::mem::drop(client.request_relay_session_disconnect("relay_live_ops", &session_id));
        std::mem::drop(client.pending_relay_commands("relay_live_ops"));
        std::mem::drop(client.report_relay_command_result("relay_live_ops", "rc_001", result));
    }

    #[test]
    fn http_client_options_default_disable_timeout_and_retry() {
        let options = HttpControlClientOptions::default();

        assert_eq!(options.request_timeout(), None);
        assert_eq!(options.max_retries(), 0);
        assert_eq!(options.retry_backoff(), Duration::from_millis(0));
    }

    #[test]
    fn http_client_constructors_accept_timeout_and_retry_options() {
        let options = HttpControlClientOptions::default()
            .with_request_timeout(Duration::from_secs(5))
            .with_max_retries(2)
            .with_retry_backoff(Duration::from_millis(25));

        let client = HttpControlClient::with_options("http://127.0.0.1:1", options).unwrap();
        let bearer = HttpControlClient::with_bearer_token_and_options(
            "http://127.0.0.1:1",
            "token",
            options,
        )
        .unwrap();
        let optional = HttpControlClient::with_optional_bearer_token_and_options(
            "http://127.0.0.1:1",
            "payload.signature",
            options,
        )
        .unwrap();

        assert_eq!(client.options(), &options);
        assert_eq!(bearer.options(), &options);
        assert_eq!(optional.options(), &options);
    }

    #[test]
    fn retryable_control_errors_are_classified() {
        assert!(ControlClientError::Timeout {
            timeout: Duration::from_secs(5)
        }
        .is_retryable());
        assert!(ControlClientError::Io(std::io::Error::new(
            std::io::ErrorKind::ConnectionReset,
            "reset"
        ))
        .is_retryable());
        assert!(ControlClientError::HttpStatus {
            status_code: StatusCode::from_u16(503),
            body: Vec::new(),
        }
        .is_retryable());
        assert!(ControlClientError::HttpStatus {
            status_code: StatusCode::from_u16(429),
            body: Vec::new(),
        }
        .is_retryable());
        assert!(!ControlClientError::HttpStatus {
            status_code: StatusCode::from_u16(403),
            body: Vec::new(),
        }
        .is_retryable());
    }

    #[test]
    fn http_client_exposes_browser_server_auth_methods() {
        let client = HttpControlClient::new("http://127.0.0.1:1").unwrap();
        let start = StartServerAuthRequest {
            device_id: Some(DeviceId::new("pc_001")),
            device_name: "Office PC".to_string(),
            server_public_key: "base64url-public-key".to_string(),
        };
        let exchange = BrowserServerAuthExchangeRequest {
            session_id: "srv_auth_001".to_string(),
            server_auth_code: "one-time-code".to_string(),
            server_public_key: "base64url-public-key".to_string(),
        };

        std::mem::drop(client.start_browser_server_auth(start));
        std::mem::drop(client.approve_browser_server_auth("srv_auth_001"));
        std::mem::drop(client.exchange_browser_server_auth(exchange));
    }

    #[test]
    fn http_client_exposes_password_update_method() {
        let client = HttpControlClient::new("http://127.0.0.1:1").unwrap();

        std::mem::drop(client.update_password(UpdatePasswordRequest {
            current_password: Some("password-123".to_string()),
            new_password: "new-password-123".to_string(),
        }));
    }

    #[test]
    fn http_client_exposes_device_code_server_auth_methods() {
        let client = HttpControlClient::new("http://127.0.0.1:1").unwrap();
        let start = StartServerAuthRequest {
            device_id: Some(DeviceId::new("pc_001")),
            device_name: "Office PC".to_string(),
            server_public_key: "base64url-public-key".to_string(),
        };
        let poll = PollServerAuthRequest {
            device_code: "raw-device-code".to_string(),
            server_public_key: "base64url-public-key".to_string(),
        };

        std::mem::drop(client.start_device_server_auth(start));
        std::mem::drop(client.approve_device_server_auth("ABCD-EFGH"));
        std::mem::drop(client.deny_device_server_auth("ABCD-EFGH"));
        std::mem::drop(client.poll_device_server_auth(poll));
    }

    #[test]
    fn http_client_exposes_server_credential_methods() {
        let client = HttpControlClient::new("http://127.0.0.1:1").unwrap();

        std::mem::drop(client.list_server_credentials());
        std::mem::drop(client.list_server_credentials_with_query(AdminListQuery {
            user_id: Some(UserId::new("user_001")),
            device_id: Some(DeviceId::new("server_001")),
            enabled: Some(true),
            ..AdminListQuery::default()
        }));
        std::mem::drop(client.server_credential("srv_cred_001"));
        std::mem::drop(client.rotate_server_credential("srv_cred_001"));
        std::mem::drop(client.update_server_credential_status(
            "srv_cred_001",
            UpdateServerCredentialStatusRequest { enabled: false },
        ));
    }

    #[test]
    fn http_client_exposes_oauth_identity_methods() {
        let client = HttpControlClient::new("http://127.0.0.1:1").unwrap();

        std::mem::drop(client.list_oauth_identities());
        std::mem::drop(client.list_oauth_identities_with_query(AdminListQuery {
            user_id: Some(UserId::new("user_001")),
            q: Some("octocat".to_string()),
            ..AdminListQuery::default()
        }));
        std::mem::drop(client.oauth_identity(OAuthProvider::GitHub, "123456"));
        std::mem::drop(client.unlink_oauth_identity(OAuthProvider::GitHub, "123456"));
    }

    #[test]
    fn oauth_server_auth_dtos_roundtrip() {
        let identity = OAuthIdentity {
            provider: OAuthProvider::GitHub,
            provider_user_id: "123456".to_string(),
            user_id: UserId::new("user_github"),
            email: "user@example.com".to_string(),
            login: "octocat".to_string(),
            avatar_url: "https://avatars.githubusercontent.com/u/123456".to_string(),
            created_epoch_sec: 1_767_000_000,
            updated_epoch_sec: 1_767_000_001,
        };
        let identity_json = serde_json::to_string(&identity).unwrap();
        assert!(identity_json.contains(r#""provider":"github""#));
        assert_eq!(
            serde_json::from_str::<OAuthIdentity>(&identity_json).unwrap(),
            identity
        );

        let start = StartServerAuthRequest {
            device_id: Some(DeviceId::new("pc_001")),
            device_name: "Office PC".to_string(),
            server_public_key: "base64url-public-key".to_string(),
        };
        assert_eq!(
            serde_json::from_str::<StartServerAuthRequest>(&serde_json::to_string(&start).unwrap())
                .unwrap(),
            start
        );

        let browser = BrowserServerAuthStartResponse {
            session_id: "srv_auth_001".to_string(),
            auth_url:
                "https://control.example.com/server-auth/browser/approve?session_id=srv_auth_001"
                    .to_string(),
            expires_in: 600,
        };
        assert_eq!(
            serde_json::from_str::<BrowserServerAuthStartResponse>(
                &serde_json::to_string(&browser).unwrap()
            )
            .unwrap(),
            browser
        );

        let approval = BrowserServerAuthApprovalResponse {
            session_id: "srv_auth_001".to_string(),
            server_auth_code: "one-time-code".to_string(),
            status: ServerAuthStatus::Approved,
        };
        assert_eq!(
            serde_json::from_str::<BrowserServerAuthApprovalResponse>(
                &serde_json::to_string(&approval).unwrap()
            )
            .unwrap(),
            approval
        );

        let device = DeviceServerAuthStartResponse {
            device_code: "raw-device-code".to_string(),
            user_code: "ABCD-EFGH".to_string(),
            verification_uri: "https://control.example.com/server-auth/device".to_string(),
            verification_uri_complete:
                "https://control.example.com/server-auth/device?user_code=ABCD-EFGH".to_string(),
            expires_in: 600,
            interval: 5,
        };
        assert_eq!(
            serde_json::from_str::<DeviceServerAuthStartResponse>(
                &serde_json::to_string(&device).unwrap()
            )
            .unwrap(),
            device
        );

        let device_approval = DeviceServerAuthApprovalResponse {
            user_code: "ABCD-EFGH".to_string(),
            status: ServerAuthStatus::Approved,
        };
        assert_eq!(
            serde_json::from_str::<DeviceServerAuthApprovalResponse>(
                &serde_json::to_string(&device_approval).unwrap()
            )
            .unwrap(),
            device_approval
        );

        let poll = PollServerAuthRequest {
            device_code: "raw-device-code".to_string(),
            server_public_key: "base64url-public-key".to_string(),
        };
        assert_eq!(
            serde_json::from_str::<PollServerAuthRequest>(&serde_json::to_string(&poll).unwrap())
                .unwrap(),
            poll
        );

        let credential = ServerCredentialResponse {
            credential_id: "srv_cred_001".to_string(),
            device_id: DeviceId::new("pc_001"),
            server_token: "control-token".to_string(),
            token_type: "bearer".to_string(),
        };
        assert_eq!(
            serde_json::from_str::<ServerCredentialResponse>(
                &serde_json::to_string(&credential).unwrap()
            )
            .unwrap(),
            credential
        );

        let poll_response = DeviceServerAuthPollResponse {
            status: ServerAuthStatus::Approved,
            interval: 5,
            credential: Some(credential.clone()),
        };
        assert_eq!(
            serde_json::from_str::<DeviceServerAuthPollResponse>(
                &serde_json::to_string(&poll_response).unwrap()
            )
            .unwrap(),
            poll_response
        );

        let summary = ServerCredentialSummary {
            credential_id: "srv_cred_001".to_string(),
            user_id: UserId::new("user_github"),
            device_id: DeviceId::new("pc_001"),
            device_name: "Office PC".to_string(),
            enabled: true,
            token_version: 1,
            created_epoch_sec: 1_767_000_000,
            last_used_epoch_sec: Some(1_767_000_100),
        };
        assert_eq!(
            serde_json::from_str::<ServerCredentialSummary>(
                &serde_json::to_string(&summary).unwrap()
            )
            .unwrap(),
            summary
        );

        let status = UpdateServerCredentialStatusRequest { enabled: false };
        assert_eq!(
            serde_json::from_str::<UpdateServerCredentialStatusRequest>(
                &serde_json::to_string(&status).unwrap()
            )
            .unwrap(),
            status
        );

        assert_eq!(
            serde_json::to_string(&ServerAuthMode::DeviceCode).unwrap(),
            r#""device_code""#
        );
        assert_eq!(
            serde_json::to_string(&ServerAuthStatus::AuthorizationPending).unwrap(),
            r#""authorization_pending""#
        );
    }
}
