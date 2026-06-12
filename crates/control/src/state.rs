use std::{
    collections::HashMap,
    path::Path,
    sync::{Arc, RwLock},
    time::{SystemTime, UNIX_EPOCH},
};

use quic_tunnel_auth::{ControlRole, ControlTokenClaims, TokenKey, TokenSigner};
use quic_tunnel_control_client::{
    AdminSessionSummary, AgentSessionAssignment, AgentSessionStatus, ApproveMobilePairingRequest,
    ApprovedMobileGrantMetadata, AssignUserPlanRequest, AuditLogEntry, AuthResponse,
    BrowserServerAuthApprovalResponse, BrowserServerAuthExchangeRequest,
    BrowserServerAuthStartResponse, ControllerDevice, CreateRelayBootstrapRequest,
    CreateRelayCredentialRequest, CreateSessionRequest, CreateSessionResponse, CreateUserRequest,
    DashboardControllerStats, DashboardDeviceStats, DashboardRelayStats, DashboardSessionStats,
    DashboardSummary, DashboardUsageStats, DashboardUserStats, DenyMobileGrantRequest,
    DeviceAccessGrant, DeviceServerAuthApprovalResponse, DeviceServerAuthPollResponse,
    DeviceServerAuthStartResponse, GrantDeviceAccessRequest, GrantSessionPollResponse,
    LoginRequest, MobilePairingPollResponse, OAuthIdentity, OAuthProvider,
    PendingGrantSessionRequest, PendingMobilePairingRequest, Plan, PollServerAuthRequest,
    RegisterControllerDeviceRequest, RegisterRelayRequest, RegisterUserRequest,
    RelayBootstrapExchangeRequest, RelayBootstrapExchangeResponse, RelayBootstrapResponse,
    RelayCommand, RelayCommandKind, RelayCommandStatus, RelayCredential, RelayHealthReport,
    RelayHealthStatus, RelayNode, RelaySessionSnapshot, ReportRelayCommandResultRequest,
    ReportRelayHealthRequest, ReportRelaySessionUsageRequest, ServerAuthMode, ServerAuthStatus,
    ServerCredentialResponse, ServerCredentialSummary, StartGrantSessionResponse,
    StartMobilePairingResponse, StartServerAuthRequest, UpdatePasswordRequest,
    UpdatePlanCatalogRequest, UpdateRelayCredentialStatusRequest, UpdateRelayRequest,
    UpdateServerCredentialStatusRequest, UpdateUserPlanRequest, UpdateUserRoleRequest,
    UpdateUserStatusRequest, UserDetail, UserSummary, UserUsagePeriod, UserUsageSummary,
};
use quic_tunnel_protocol::{
    ClientId, Device, DeviceId, DeviceStatus, GrantSessionRequest, MobilePairingRequest,
    PendingGrantSessionStatus, PendingPairingStatus, RelayLimits, Service, ServiceId, SessionId,
    UserId,
};
use sha2::{Digest, Sha256};

use crate::{
    oauth::{
        pkce_challenge, secret_hash, GitHubOAuthClient, GitHubOAuthConfig, GitHubUserProfile,
        OAuthError, OAuthStart, UnavailableGitHubOAuthClient,
    },
    token_issuer::TokenIssuer,
};
use crate::{
    sqlite_store::{SqliteControlStore, SqliteStoreError},
    store::{
        InMemoryControlStore, OAuthLoginSession, PendingGrantSessionRecord,
        PendingMobilePairingRecord, RelayBootstrapRecord, RelaySessionUsageRecord,
        ServerAuthSession, ServerCredential, UserAccount,
    },
};

const CONTROL_TOKEN_EXP: u64 = 4_102_444_800;
const CONTROL_TOKEN_NOW: u64 = 1_767_000_000;
const RELAY_HEARTBEAT_TIMEOUT_SEC: u64 = 90;
const OAUTH_LOGIN_SESSION_TTL_SEC: u64 = 600;
const SERVER_AUTH_SESSION_TTL_SEC: u64 = 600;
const SERVER_AUTH_POLL_INTERVAL_SEC: u64 = 5;
const MOBILE_GRANT_PENDING_TTL_SEC: u64 = 300;
const MOBILE_GRANT_POLL_INTERVAL_MS: u64 = 1_000;
const RELAY_BOOTSTRAP_MAX_TTL_SEC: u64 = 86_400;

#[derive(Default)]
struct UsageAccumulator {
    session_count: u64,
    pending_sessions: u64,
    claimed_sessions: u64,
    bound_sessions: u64,
    closed_sessions: u64,
    expired_sessions: u64,
    relay_quota_granted_bytes: u64,
    actual_uplink_bytes: u64,
    actual_downlink_bytes: u64,
    actual_total_bytes: u64,
}

#[derive(Clone)]
struct AgentSessionGrantMetadata {
    grant_id: String,
    revocation_version: u64,
    service_id: ServiceId,
}

fn session_user_id_locked(device: &Device, assignment: &AgentSessionAssignment) -> UserId {
    if assignment.user_id.as_str().is_empty() {
        device.user_id.clone()
    } else {
        assignment.user_id.clone()
    }
}

fn user_can_access_device_locked(
    inner: &crate::store::ControlStore,
    user_id: &UserId,
    device_id: &DeviceId,
) -> bool {
    inner
        .devices
        .get(device_id)
        .map(|device| &device.user_id == user_id)
        .unwrap_or(false)
        || inner
            .device_access_grants
            .get(device_id)
            .map(|grants| {
                grants
                    .iter()
                    .any(|granted_user_id| granted_user_id == user_id)
            })
            .unwrap_or(false)
}

fn user_actual_relay_usage_total_locked(
    inner: &crate::store::ControlStore,
    user_id: &UserId,
) -> u64 {
    let mut actual_total_bytes = 0_u64;
    for (device_id, assignments) in &inner.agent_sessions {
        let Some(device) = inner.devices.get(device_id) else {
            continue;
        };
        for assignment in assignments {
            if session_user_id_locked(device, assignment) != *user_id {
                continue;
            }
            if let Some(actual) = inner.relay_session_usage.get(&assignment.session_id) {
                actual_total_bytes = actual_total_bytes.saturating_add(actual.stats.total_bytes);
            }
        }
    }
    actual_total_bytes
}

fn push_audit_log_locked(
    inner: &mut crate::store::ControlStore,
    actor_user_id: UserId,
    actor_subject: impl Into<String>,
    actor_role: ControlRole,
    action: impl Into<String>,
    target_type: impl Into<String>,
    target_id: impl Into<String>,
    message: impl Into<String>,
) {
    inner.audit_logs.push(AuditLogEntry {
        audit_id: format!("audit_{}", uuid::Uuid::new_v4()),
        actor_user_id,
        actor_subject: actor_subject.into(),
        actor_role,
        action: action.into(),
        target_type: target_type.into(),
        target_id: target_id.into(),
        message: message.into(),
        created_epoch_sec: current_epoch_sec(),
    });
}

#[derive(Clone)]
pub struct ControlState {
    store: InMemoryControlStore,
    persistence: Option<SqliteControlStore>,
    token_secret: String,
    relay_addr: String,
    punch_addr: String,
    strict_auth: bool,
    relay_health_now_epoch_sec: Arc<RwLock<u64>>,
    server_auth_now_epoch_sec: Arc<RwLock<Option<u64>>>,
    github_oauth_config: Option<GitHubOAuthConfig>,
    github_oauth_client: Arc<dyn GitHubOAuthClient>,
}

impl ControlState {
    pub fn new(
        token_secret: impl Into<String>,
        relay_addr: impl Into<String>,
        punch_addr: impl Into<String>,
    ) -> Self {
        let state = Self {
            store: InMemoryControlStore::default(),
            persistence: None,
            token_secret: token_secret.into(),
            relay_addr: relay_addr.into(),
            punch_addr: punch_addr.into(),
            strict_auth: false,
            relay_health_now_epoch_sec: Arc::new(RwLock::new(current_epoch_sec())),
            server_auth_now_epoch_sec: Arc::new(RwLock::new(None)),
            github_oauth_config: None,
            github_oauth_client: Arc::new(UnavailableGitHubOAuthClient),
        };
        state.seed_defaults();
        state
    }

    pub fn new_sqlite(
        token_secret: impl Into<String>,
        relay_addr: impl Into<String>,
        punch_addr: impl Into<String>,
        db_path: impl AsRef<Path>,
    ) -> Result<Self, ControlPersistenceError> {
        let persistence = SqliteControlStore::open(db_path)?;
        let store = InMemoryControlStore::default();
        if let Some(snapshot) = persistence.load_snapshot()? {
            store.replace(snapshot);
        }

        let state = Self {
            store,
            persistence: Some(persistence),
            token_secret: token_secret.into(),
            relay_addr: relay_addr.into(),
            punch_addr: punch_addr.into(),
            strict_auth: false,
            relay_health_now_epoch_sec: Arc::new(RwLock::new(current_epoch_sec())),
            server_auth_now_epoch_sec: Arc::new(RwLock::new(None)),
            github_oauth_config: None,
            github_oauth_client: Arc::new(UnavailableGitHubOAuthClient),
        };
        state.seed_defaults();
        state.persist()?;
        Ok(state)
    }

    pub fn token_secret(&self) -> &str {
        &self.token_secret
    }

    pub fn relay_addr(&self) -> &str {
        &self.relay_addr
    }

    pub fn punch_addr(&self) -> &str {
        &self.punch_addr
    }

    pub fn with_strict_auth(mut self, strict_auth: bool) -> Self {
        self.strict_auth = strict_auth;
        self
    }

    pub fn with_github_oauth_config(mut self, config: GitHubOAuthConfig) -> Self {
        self.github_oauth_config = Some(config);
        self
    }

    pub fn with_github_oauth_client(mut self, client: Arc<dyn GitHubOAuthClient>) -> Self {
        self.github_oauth_client = client;
        self
    }

    pub fn strict_auth(&self) -> bool {
        self.strict_auth
    }

    pub fn with_relay_health_now_epoch_sec(self, now_epoch_sec: u64) -> Self {
        self.set_relay_health_now_epoch_sec(now_epoch_sec);
        {
            let mut inner = self.store.write();
            for relay in inner.relays.values_mut() {
                relay.last_seen_epoch_sec = now_epoch_sec;
            }
        }
        self
    }

    pub fn set_relay_health_now_epoch_sec(&self, now_epoch_sec: u64) {
        *self
            .relay_health_now_epoch_sec
            .write()
            .expect("relay health clock lock poisoned") = now_epoch_sec;
    }

    fn relay_health_now_epoch_sec(&self) -> u64 {
        *self
            .relay_health_now_epoch_sec
            .read()
            .expect("relay health clock lock poisoned")
    }

    pub fn with_server_auth_now_epoch_sec(self, now_epoch_sec: u64) -> Self {
        self.set_server_auth_now_epoch_sec(now_epoch_sec);
        self
    }

    pub fn set_server_auth_now_epoch_sec(&self, now_epoch_sec: u64) {
        *self
            .server_auth_now_epoch_sec
            .write()
            .expect("server auth clock lock poisoned") = Some(now_epoch_sec);
    }

    fn server_auth_now_epoch_sec(&self) -> u64 {
        let override_now = *self
            .server_auth_now_epoch_sec
            .read()
            .expect("server auth clock lock poisoned");
        override_now.unwrap_or_else(current_epoch_sec)
    }

    pub fn default_user_id(&self) -> UserId {
        UserId::new("user_001")
    }

    pub fn issue_admin_token(
        &self,
        subject: impl Into<String>,
    ) -> Result<String, ControlAuthError> {
        TokenIssuer::new(self.token_secret())
            .issue_control_token(
                self.default_user_id(),
                subject,
                ControlRole::Admin,
                CONTROL_TOKEN_EXP,
            )
            .map_err(|_| ControlAuthError::TokenIssueFailed)
    }

    pub fn issue_relay_token(
        &self,
        relay_id: impl Into<String>,
    ) -> Result<String, ControlAuthError> {
        let relay_id = relay_id.into();
        if relay_id.trim().is_empty() {
            return Err(ControlAuthError::InvalidInput);
        }
        let token_version = {
            let inner = self.store.read();
            match inner.relay_credentials.get(&relay_id) {
                Some(credential) if credential.enabled => credential.token_version,
                Some(_) => return Err(ControlAuthError::InvalidCredentials),
                None => 1,
            }
        };
        TokenIssuer::new(self.token_secret())
            .issue_relay_control_token(
                self.default_user_id(),
                relay_id,
                token_version,
                CONTROL_TOKEN_EXP,
            )
            .map_err(|_| ControlAuthError::TokenIssueFailed)
    }

    fn seed_defaults(&self) {
        let default_user_id = self.default_user_id();
        let mut inner = self.store.write();
        inner
            .plans
            .entry(default_user_id.clone())
            .or_insert_with(default_plan);
        inner
            .plan_catalog
            .entry("free".to_string())
            .or_insert_with(default_plan);
        inner
            .relays
            .entry("relay_default".to_string())
            .or_insert_with(|| RelayNode {
                relay_id: "relay_default".to_string(),
                relay_addr: self.relay_addr.clone(),
                admin_addr: String::new(),
                capacity_streams: 1,
                healthy: true,
                last_seen_epoch_sec: self.relay_health_now_epoch_sec(),
                health_status: RelayHealthStatus::Healthy,
                health_reason: String::new(),
                relay_version: String::new(),
                uptime_sec: 0,
                active_sessions: 0,
                active_streams: 0,
                total_uplink_bytes: 0,
                total_downlink_bytes: 0,
                total_bytes: 0,
                data_plane_bound: true,
                admin_bound: false,
                last_health_report_epoch_sec: self.relay_health_now_epoch_sec(),
            });
    }

    fn persist(&self) -> Result<(), ControlPersistenceError> {
        let Some(persistence) = &self.persistence else {
            return Ok(());
        };
        persistence.save_snapshot(&self.store.snapshot())?;
        Ok(())
    }

    pub fn register_user(
        &self,
        request: RegisterUserRequest,
    ) -> Result<AuthResponse, ControlAuthError> {
        validate_email_password(&request.email, &request.password)?;
        let email = normalized_email(&request.email);
        let mut inner = self.store.write();
        if inner.user_ids_by_email.contains_key(&email) {
            return Err(ControlAuthError::EmailAlreadyRegistered);
        }

        let user_id = UserId::new(format!("user_{}", uuid::Uuid::new_v4()));
        let account = UserAccount {
            user_id: user_id.clone(),
            email: email.clone(),
            display_name: request.display_name,
            password_hash: password_hash(&email, &request.password),
            role: ControlRole::User,
            enabled: true,
        };
        inner
            .user_ids_by_email
            .insert(email.clone(), user_id.clone());
        inner.users.insert(user_id.clone(), account);
        inner.plans.insert(user_id.clone(), default_plan());
        drop(inner);
        self.persist()
            .map_err(|_| ControlAuthError::PersistenceFailed)?;

        self.auth_response(user_id, email, ControlRole::User)
    }

    pub fn bootstrap_admin_user(
        &self,
        request: RegisterUserRequest,
    ) -> Result<AuthResponse, ControlAuthError> {
        validate_email_password(&request.email, &request.password)?;
        let email = normalized_email(&request.email);
        let password_hash = password_hash(&email, &request.password);
        let user_id = {
            let mut inner = self.store.write();
            if let Some(user_id) = inner.user_ids_by_email.get(&email).cloned() {
                let account = inner
                    .users
                    .get_mut(&user_id)
                    .ok_or(ControlAuthError::InvalidCredentials)?;
                account.password_hash = password_hash;
                account.role = ControlRole::Admin;
                account.email = email.clone();
                account.display_name = request.display_name.clone();
                account.enabled = true;
                inner
                    .plans
                    .entry(user_id.clone())
                    .or_insert_with(default_plan);
                user_id
            } else {
                let user_id = UserId::new(format!("admin_{}", uuid::Uuid::new_v4()));
                let account = UserAccount {
                    user_id: user_id.clone(),
                    email: email.clone(),
                    display_name: request.display_name,
                    password_hash,
                    role: ControlRole::Admin,
                    enabled: true,
                };
                inner
                    .user_ids_by_email
                    .insert(email.clone(), user_id.clone());
                inner.users.insert(user_id.clone(), account);
                inner.plans.insert(user_id.clone(), default_plan());
                user_id
            }
        };
        self.persist()
            .map_err(|_| ControlAuthError::PersistenceFailed)?;

        self.auth_response(user_id, email, ControlRole::Admin)
    }

    pub fn login(&self, request: LoginRequest) -> Result<AuthResponse, ControlAuthError> {
        let email = normalized_email(&request.email);
        let inner = self.store.read();
        let user_id = inner
            .user_ids_by_email
            .get(&email)
            .ok_or(ControlAuthError::InvalidCredentials)?;
        let account = inner
            .users
            .get(user_id)
            .ok_or(ControlAuthError::InvalidCredentials)?;
        if !account.enabled {
            return Err(ControlAuthError::InvalidCredentials);
        }
        if account.password_hash != password_hash(&email, &request.password) {
            return Err(ControlAuthError::InvalidCredentials);
        }
        let user_id = account.user_id.clone();
        let role = account.role;
        drop(inner);

        self.auth_response(user_id, email, role)
    }

    pub fn update_password(
        &self,
        actor: &ControlTokenClaims,
        request: UpdatePasswordRequest,
    ) -> Result<(), ControlAuthError> {
        if request.new_password.len() < 8 {
            return Err(ControlAuthError::InvalidInput);
        }
        if matches!(actor.role, ControlRole::Relay | ControlRole::Agent) {
            return Err(ControlAuthError::InvalidCredentials);
        }

        {
            let mut inner = self.store.write();
            let (target_id, action, message) = {
                let account = inner
                    .users
                    .get_mut(&actor.user_id)
                    .ok_or(ControlAuthError::InvalidCredentials)?;
                if !account.enabled {
                    return Err(ControlAuthError::InvalidCredentials);
                }

                let had_password = !account.password_hash.trim().is_empty();
                if had_password {
                    let current_password = request
                        .current_password
                        .as_deref()
                        .ok_or(ControlAuthError::InvalidCredentials)?;
                    if account.password_hash != password_hash(&account.email, current_password) {
                        return Err(ControlAuthError::InvalidCredentials);
                    }
                }

                account.password_hash = password_hash(&account.email, &request.new_password);
                let action = if had_password {
                    "auth.password.change"
                } else {
                    "auth.password.set"
                };
                let message = if had_password {
                    "changed account password"
                } else {
                    "set account password"
                };
                (account.user_id.clone(), action, message)
            };

            push_audit_log_locked(
                &mut inner,
                actor.user_id.clone(),
                actor.subject.clone(),
                actor.role,
                action,
                "user",
                target_id.to_string(),
                message,
            );
        }
        self.persist()
            .map_err(|_| ControlAuthError::PersistenceFailed)
    }

    pub fn upsert_oauth_identity(
        &self,
        identity: OAuthIdentity,
    ) -> Result<(), ControlPersistenceError> {
        let key = oauth_identity_key(identity.provider, &identity.provider_user_id);
        self.store.write().oauth_identities.insert(key, identity);
        self.persist()
    }

    pub fn oauth_identity(
        &self,
        provider: OAuthProvider,
        provider_user_id: &str,
    ) -> Option<OAuthIdentity> {
        self.store
            .read()
            .oauth_identities
            .get(&oauth_identity_key(provider, provider_user_id))
            .cloned()
    }

    pub fn oauth_identities(&self) -> Vec<OAuthIdentity> {
        let mut identities: Vec<_> = self
            .store
            .read()
            .oauth_identities
            .values()
            .cloned()
            .collect();
        identities.sort_by(|left, right| {
            oauth_provider_key(left.provider)
                .cmp(oauth_provider_key(right.provider))
                .then_with(|| left.provider_user_id.cmp(&right.provider_user_id))
        });
        identities
    }

    pub fn oauth_identities_for_user(&self, user_id: &UserId) -> Vec<OAuthIdentity> {
        let mut identities: Vec<_> = self
            .store
            .read()
            .oauth_identities
            .values()
            .filter(|identity| &identity.user_id == user_id)
            .cloned()
            .collect();
        identities.sort_by(|left, right| {
            oauth_provider_key(left.provider)
                .cmp(oauth_provider_key(right.provider))
                .then_with(|| left.provider_user_id.cmp(&right.provider_user_id))
        });
        identities
    }

    pub fn oauth_identity_for_actor(
        &self,
        actor: &ControlTokenClaims,
        provider: OAuthProvider,
        provider_user_id: &str,
    ) -> Result<OAuthIdentity, ControlPlaneError> {
        if provider_user_id.trim().is_empty() {
            return Err(ControlPlaneError::InvalidInput);
        }
        let identity = self
            .store
            .read()
            .oauth_identities
            .get(&oauth_identity_key(provider, provider_user_id))
            .filter(|identity| {
                actor.role == ControlRole::Admin
                    || (actor.role == ControlRole::User && identity.user_id == actor.user_id)
            })
            .cloned()
            .ok_or(ControlPlaneError::OAuthIdentityNotFound)?;
        Ok(identity)
    }

    pub fn unlink_oauth_identity(
        &self,
        actor: &ControlTokenClaims,
        provider: OAuthProvider,
        provider_user_id: &str,
    ) -> Result<OAuthIdentity, ControlPlaneError> {
        if provider_user_id.trim().is_empty() {
            return Err(ControlPlaneError::InvalidInput);
        }
        let key = oauth_identity_key(provider, provider_user_id);
        let removed = {
            let mut inner = self.store.write();
            let identity = inner
                .oauth_identities
                .get(&key)
                .filter(|identity| {
                    actor.role == ControlRole::Admin
                        || (actor.role == ControlRole::User && identity.user_id == actor.user_id)
                })
                .cloned()
                .ok_or(ControlPlaneError::OAuthIdentityNotFound)?;
            let account = inner
                .users
                .get(&identity.user_id)
                .ok_or(ControlPlaneError::UserNotFound)?;
            let has_password_login = !account.password_hash.trim().is_empty();
            let has_other_oauth_login = inner.oauth_identities.values().any(|candidate| {
                candidate.user_id == identity.user_id
                    && !(candidate.provider == identity.provider
                        && candidate.provider_user_id == identity.provider_user_id)
            });
            if !has_password_login && !has_other_oauth_login {
                return Err(ControlPlaneError::OAuthIdentityLastLoginMethod);
            }
            let removed = inner
                .oauth_identities
                .remove(&key)
                .ok_or(ControlPlaneError::OAuthIdentityNotFound)?;
            push_audit_log_locked(
                &mut inner,
                actor.user_id.clone(),
                actor.subject.clone(),
                actor.role,
                "oauth_identity.unlink",
                "oauth_identity",
                format!(
                    "{}:{}",
                    oauth_provider_key(removed.provider),
                    removed.provider_user_id
                ),
                format!(
                    "unlinked {} identity {}",
                    oauth_provider_key(removed.provider),
                    removed.provider_user_id
                ),
            );
            removed
        };
        self.persist()
            .map_err(|_| ControlPlaneError::PersistenceFailed)?;
        Ok(removed)
    }

    pub fn start_github_oauth(
        &self,
        redirect_uri: Option<String>,
    ) -> Result<OAuthStart, ControlAuthError> {
        let config = self
            .github_oauth_config
            .as_ref()
            .ok_or(ControlAuthError::OAuthNotConfigured)?;
        let state = format!("oauth_state_{}", uuid::Uuid::new_v4());
        let pkce_verifier = format!("oauth_verifier_{}", uuid::Uuid::new_v4());
        let code_challenge = pkce_challenge(&pkce_verifier);
        let authorization_url = config.authorize_url(&state, &code_challenge);
        let now = current_epoch_sec();
        let session = OAuthLoginSession {
            session_id: format!("oauth_login_{}", uuid::Uuid::new_v4()),
            provider: "github".to_string(),
            state_hash: secret_hash(&state),
            pkce_verifier,
            redirect_uri,
            expires_epoch_sec: now + OAUTH_LOGIN_SESSION_TTL_SEC,
            created_epoch_sec: now,
        };

        self.store
            .write()
            .oauth_login_sessions
            .insert(session.state_hash.clone(), session);
        self.persist()
            .map_err(|_| ControlAuthError::PersistenceFailed)?;

        Ok(OAuthStart {
            authorization_url,
            state,
            expires_in: OAUTH_LOGIN_SESSION_TTL_SEC,
        })
    }

    pub async fn github_oauth_callback(
        &self,
        code: &str,
        state: &str,
    ) -> Result<AuthResponse, ControlAuthError> {
        if code.trim().is_empty() || state.trim().is_empty() {
            return Err(ControlAuthError::InvalidInput);
        }
        let config = self
            .github_oauth_config
            .clone()
            .ok_or(ControlAuthError::OAuthNotConfigured)?;
        let session = {
            let mut inner = self.store.write();
            inner
                .oauth_login_sessions
                .remove(&secret_hash(state))
                .ok_or(ControlAuthError::OAuthInvalidState)?
        };
        if session.expires_epoch_sec <= current_epoch_sec() {
            return Err(ControlAuthError::OAuthInvalidState);
        }

        let token = self
            .github_oauth_client
            .exchange_code(code, &session.pkce_verifier, &config)
            .await
            .map_err(oauth_error_to_auth_error)?;
        let profile = self
            .github_oauth_client
            .user_profile(&token.access_token)
            .await
            .map_err(oauth_error_to_auth_error)?;
        let verified_email = self
            .github_oauth_client
            .primary_verified_email(&token.access_token)
            .await
            .map_err(oauth_error_to_auth_error)?;

        self.login_or_create_github_oauth_user(profile, verified_email)
    }

    pub fn start_browser_server_auth(
        &self,
        request: StartServerAuthRequest,
    ) -> Result<BrowserServerAuthStartResponse, ControlPlaneError> {
        if request.device_id.as_str().trim().is_empty()
            || request.device_name.trim().is_empty()
            || request.server_public_key.trim().is_empty()
        {
            return Err(ControlPlaneError::InvalidInput);
        }

        let session_id = format!("srv_auth_{}", uuid::Uuid::new_v4());
        let now = self.server_auth_now_epoch_sec();
        let session = ServerAuthSession {
            session_id: session_id.clone(),
            mode: ServerAuthMode::Browser,
            status: ServerAuthStatus::Pending,
            device_id: request.device_id,
            device_name: request.device_name,
            server_public_key: request.server_public_key,
            user_code_hash: None,
            device_code_hash: None,
            auth_code_hash: None,
            approved_user_id: None,
            poll_interval_sec: 0,
            expires_epoch_sec: now + SERVER_AUTH_SESSION_TTL_SEC,
            created_epoch_sec: now,
            updated_epoch_sec: now,
            last_poll_epoch_sec: None,
        };

        self.store
            .write()
            .server_auth_sessions
            .insert(session_id.clone(), session);
        self.persist()
            .map_err(|_| ControlPlaneError::PersistenceFailed)?;

        Ok(BrowserServerAuthStartResponse {
            auth_url: format!("/server-auth/browser/approve?session_id={session_id}"),
            session_id,
            expires_in: SERVER_AUTH_SESSION_TTL_SEC,
        })
    }

    pub fn approve_browser_server_auth(
        &self,
        session_id: &str,
        user_id: &UserId,
    ) -> Result<BrowserServerAuthApprovalResponse, ControlPlaneError> {
        if session_id.trim().is_empty() {
            return Err(ControlPlaneError::InvalidInput);
        }
        let now = self.server_auth_now_epoch_sec();
        let server_auth_code = format!("srv_code_{}", uuid::Uuid::new_v4());
        {
            let mut inner = self.store.write();
            let session = inner
                .server_auth_sessions
                .get_mut(session_id)
                .ok_or(ControlPlaneError::ServerAuthSessionNotFound)?;
            if session.mode != ServerAuthMode::Browser {
                return Err(ControlPlaneError::InvalidInput);
            }
            if session.expires_epoch_sec <= now {
                session.status = ServerAuthStatus::Expired;
                session.updated_epoch_sec = now;
                return Err(ControlPlaneError::ServerAuthSessionNotReady);
            }
            if !matches!(
                session.status,
                ServerAuthStatus::Pending | ServerAuthStatus::Approved
            ) {
                return Err(ControlPlaneError::ServerAuthSessionNotReady);
            }

            session.status = ServerAuthStatus::Approved;
            session.auth_code_hash = Some(secret_hash(&server_auth_code));
            session.approved_user_id = Some(user_id.clone());
            session.updated_epoch_sec = now;
        }
        self.persist()
            .map_err(|_| ControlPlaneError::PersistenceFailed)?;

        Ok(BrowserServerAuthApprovalResponse {
            session_id: session_id.to_string(),
            server_auth_code,
            status: ServerAuthStatus::Approved,
        })
    }

    pub fn exchange_browser_server_auth(
        &self,
        request: BrowserServerAuthExchangeRequest,
    ) -> Result<ServerCredentialResponse, ControlPlaneError> {
        if request.session_id.trim().is_empty()
            || request.server_auth_code.trim().is_empty()
            || request.server_public_key.trim().is_empty()
        {
            return Err(ControlPlaneError::InvalidInput);
        }

        let now = self.server_auth_now_epoch_sec();
        let auth_code_hash = secret_hash(&request.server_auth_code);
        let (credential, device_id) = {
            let mut inner = self.store.write();
            let session = inner
                .server_auth_sessions
                .get_mut(&request.session_id)
                .ok_or(ControlPlaneError::ServerAuthSessionNotFound)?;
            if session.mode != ServerAuthMode::Browser {
                return Err(ControlPlaneError::InvalidInput);
            }
            if session.expires_epoch_sec <= now {
                session.status = ServerAuthStatus::Expired;
                session.updated_epoch_sec = now;
                return Err(ControlPlaneError::ServerAuthSessionNotReady);
            }
            if session.status != ServerAuthStatus::Approved {
                return Err(ControlPlaneError::ServerAuthSessionNotReady);
            }
            if session.server_public_key != request.server_public_key
                || session.auth_code_hash.as_deref() != Some(auth_code_hash.as_str())
            {
                return Err(ControlPlaneError::ServerAuthInvalidCode);
            }
            let user_id = session
                .approved_user_id
                .clone()
                .ok_or(ControlPlaneError::ServerAuthSessionNotReady)?;
            let credential_id = format!("srv_cred_{}", uuid::Uuid::new_v4());
            let device_id = session.device_id.clone();
            let credential = ServerCredential {
                credential_id: credential_id.clone(),
                user_id,
                device_id: device_id.clone(),
                device_name: session.device_name.clone(),
                server_public_key: session.server_public_key.clone(),
                enabled: true,
                token_version: 1,
                created_epoch_sec: now,
                last_used_epoch_sec: None,
            };
            session.status = ServerAuthStatus::Consumed;
            session.auth_code_hash = None;
            session.updated_epoch_sec = now;
            inner
                .server_credentials
                .insert(credential_id, credential.clone());
            push_audit_log_locked(
                &mut inner,
                credential.user_id.clone(),
                "server-auth",
                ControlRole::User,
                "server_credential.issue",
                "server_credential",
                credential.credential_id.clone(),
                format!("issued server credential for {}", credential.device_id),
            );
            (credential, device_id)
        };
        self.persist()
            .map_err(|_| ControlPlaneError::PersistenceFailed)?;

        let server_token = TokenIssuer::new(self.token_secret())
            .issue_agent_control_token(
                credential.user_id,
                credential.credential_id.clone(),
                credential.token_version,
                CONTROL_TOKEN_EXP,
            )
            .map_err(|_| ControlPlaneError::TokenIssueFailed)?;
        Ok(ServerCredentialResponse {
            credential_id: credential.credential_id,
            device_id,
            server_token,
            token_type: "bearer".to_string(),
        })
    }

    pub fn start_device_server_auth(
        &self,
        request: StartServerAuthRequest,
    ) -> Result<DeviceServerAuthStartResponse, ControlPlaneError> {
        if request.device_id.as_str().trim().is_empty()
            || request.device_name.trim().is_empty()
            || request.server_public_key.trim().is_empty()
        {
            return Err(ControlPlaneError::InvalidInput);
        }

        let session_id = format!("srv_auth_{}", uuid::Uuid::new_v4());
        let device_code = format!("srv_device_{}", uuid::Uuid::new_v4());
        let user_code = new_device_user_code();
        let now = self.server_auth_now_epoch_sec();
        let session = ServerAuthSession {
            session_id,
            mode: ServerAuthMode::DeviceCode,
            status: ServerAuthStatus::Pending,
            device_id: request.device_id,
            device_name: request.device_name,
            server_public_key: request.server_public_key,
            user_code_hash: Some(secret_hash(&normalized_user_code(&user_code))),
            device_code_hash: Some(secret_hash(&device_code)),
            auth_code_hash: None,
            approved_user_id: None,
            poll_interval_sec: SERVER_AUTH_POLL_INTERVAL_SEC,
            expires_epoch_sec: now + SERVER_AUTH_SESSION_TTL_SEC,
            created_epoch_sec: now,
            updated_epoch_sec: now,
            last_poll_epoch_sec: None,
        };

        self.store
            .write()
            .server_auth_sessions
            .insert(session.session_id.clone(), session);
        self.persist()
            .map_err(|_| ControlPlaneError::PersistenceFailed)?;

        Ok(DeviceServerAuthStartResponse {
            device_code,
            user_code: user_code.clone(),
            verification_uri: "/server-auth/device".to_string(),
            verification_uri_complete: format!("/server-auth/device?user_code={user_code}"),
            expires_in: SERVER_AUTH_SESSION_TTL_SEC,
            interval: SERVER_AUTH_POLL_INTERVAL_SEC,
        })
    }

    pub fn approve_device_server_auth(
        &self,
        user_code: &str,
        user_id: &UserId,
        deny: bool,
    ) -> Result<DeviceServerAuthApprovalResponse, ControlPlaneError> {
        let user_code = normalized_user_code(user_code);
        if user_code.is_empty() {
            return Err(ControlPlaneError::InvalidInput);
        }
        let user_code_hash = secret_hash(&user_code);
        let now = self.server_auth_now_epoch_sec();
        let status = if deny {
            ServerAuthStatus::Denied
        } else {
            ServerAuthStatus::Approved
        };

        {
            let mut inner = self.store.write();
            let session = inner
                .server_auth_sessions
                .values_mut()
                .find(|session| {
                    session.mode == ServerAuthMode::DeviceCode
                        && session.user_code_hash.as_deref() == Some(user_code_hash.as_str())
                })
                .ok_or(ControlPlaneError::ServerAuthSessionNotFound)?;
            if session.expires_epoch_sec <= now {
                session.status = ServerAuthStatus::Expired;
                session.updated_epoch_sec = now;
                return Err(ControlPlaneError::ServerAuthSessionNotReady);
            }
            if !matches!(
                session.status,
                ServerAuthStatus::Pending | ServerAuthStatus::Approved
            ) {
                return Err(ControlPlaneError::ServerAuthSessionNotReady);
            }
            session.status = status;
            session.approved_user_id = if deny { None } else { Some(user_id.clone()) };
            session.updated_epoch_sec = now;
        }
        self.persist()
            .map_err(|_| ControlPlaneError::PersistenceFailed)?;

        Ok(DeviceServerAuthApprovalResponse { user_code, status })
    }

    pub fn poll_device_server_auth(
        &self,
        request: PollServerAuthRequest,
    ) -> Result<DeviceServerAuthPollResponse, ControlPlaneError> {
        if request.device_code.trim().is_empty() || request.server_public_key.trim().is_empty() {
            return Err(ControlPlaneError::InvalidInput);
        }

        let now = self.server_auth_now_epoch_sec();
        let device_code_hash = secret_hash(&request.device_code);
        let (status, interval, credential) = {
            let mut inner = self.store.write();
            let mut credential_to_store = None;
            let (status, interval) = {
                let session = inner
                    .server_auth_sessions
                    .values_mut()
                    .find(|session| {
                        session.mode == ServerAuthMode::DeviceCode
                            && session.device_code_hash.as_deref()
                                == Some(device_code_hash.as_str())
                    })
                    .ok_or(ControlPlaneError::ServerAuthSessionNotFound)?;
                if session.expires_epoch_sec <= now {
                    session.status = ServerAuthStatus::Expired;
                    session.updated_epoch_sec = now;
                    (ServerAuthStatus::Expired, session.poll_interval_sec)
                } else {
                    match session.status {
                        ServerAuthStatus::Pending => {
                            if session
                                .last_poll_epoch_sec
                                .map(|last_poll| now < last_poll + session.poll_interval_sec)
                                .unwrap_or(false)
                            {
                                session.poll_interval_sec = session
                                    .poll_interval_sec
                                    .saturating_add(SERVER_AUTH_POLL_INTERVAL_SEC);
                                session.updated_epoch_sec = now;
                                (ServerAuthStatus::SlowDown, session.poll_interval_sec)
                            } else {
                                session.last_poll_epoch_sec = Some(now);
                                session.updated_epoch_sec = now;
                                (
                                    ServerAuthStatus::AuthorizationPending,
                                    session.poll_interval_sec,
                                )
                            }
                        }
                        ServerAuthStatus::Approved => {
                            if session.server_public_key != request.server_public_key {
                                session.status = ServerAuthStatus::Denied;
                                session.updated_epoch_sec = now;
                                (ServerAuthStatus::AccessDenied, session.poll_interval_sec)
                            } else {
                                let user_id = session
                                    .approved_user_id
                                    .clone()
                                    .ok_or(ControlPlaneError::ServerAuthSessionNotReady)?;
                                let credential_id = format!("srv_cred_{}", uuid::Uuid::new_v4());
                                let credential = ServerCredential {
                                    credential_id: credential_id.clone(),
                                    user_id,
                                    device_id: session.device_id.clone(),
                                    device_name: session.device_name.clone(),
                                    server_public_key: session.server_public_key.clone(),
                                    enabled: true,
                                    token_version: 1,
                                    created_epoch_sec: now,
                                    last_used_epoch_sec: None,
                                };
                                session.status = ServerAuthStatus::Consumed;
                                session.updated_epoch_sec = now;
                                credential_to_store = Some(credential);
                                (ServerAuthStatus::Approved, session.poll_interval_sec)
                            }
                        }
                        ServerAuthStatus::Denied | ServerAuthStatus::AccessDenied => {
                            (ServerAuthStatus::AccessDenied, session.poll_interval_sec)
                        }
                        ServerAuthStatus::Expired => {
                            (ServerAuthStatus::Expired, session.poll_interval_sec)
                        }
                        ServerAuthStatus::Consumed => {
                            (ServerAuthStatus::Consumed, session.poll_interval_sec)
                        }
                        ServerAuthStatus::AuthorizationPending | ServerAuthStatus::SlowDown => (
                            ServerAuthStatus::AuthorizationPending,
                            session.poll_interval_sec,
                        ),
                    }
                }
            };

            if let Some(credential) = &credential_to_store {
                inner
                    .server_credentials
                    .insert(credential.credential_id.clone(), credential.clone());
                push_audit_log_locked(
                    &mut inner,
                    credential.user_id.clone(),
                    "server-auth",
                    ControlRole::User,
                    "server_credential.issue",
                    "server_credential",
                    credential.credential_id.clone(),
                    format!("issued server credential for {}", credential.device_id),
                );
            }
            (status, interval, credential_to_store)
        };
        self.persist()
            .map_err(|_| ControlPlaneError::PersistenceFailed)?;

        let credential = match credential {
            Some(credential) => {
                let server_token = TokenIssuer::new(self.token_secret())
                    .issue_agent_control_token(
                        credential.user_id.clone(),
                        credential.credential_id.clone(),
                        credential.token_version,
                        CONTROL_TOKEN_EXP,
                    )
                    .map_err(|_| ControlPlaneError::TokenIssueFailed)?;
                Some(ServerCredentialResponse {
                    credential_id: credential.credential_id,
                    device_id: credential.device_id,
                    server_token,
                    token_type: "bearer".to_string(),
                })
            }
            None => None,
        };

        Ok(DeviceServerAuthPollResponse {
            status,
            interval,
            credential,
        })
    }

    pub fn server_credentials(&self) -> Vec<ServerCredentialSummary> {
        let inner = self.store.read();
        let mut credentials: Vec<_> = inner
            .server_credentials
            .values()
            .map(server_credential_summary)
            .collect();
        credentials.sort_by(|left, right| left.credential_id.cmp(&right.credential_id));
        credentials
    }

    pub fn server_credentials_for_user(&self, user_id: &UserId) -> Vec<ServerCredentialSummary> {
        let inner = self.store.read();
        let mut credentials: Vec<_> = inner
            .server_credentials
            .values()
            .filter(|credential| &credential.user_id == user_id)
            .map(server_credential_summary)
            .collect();
        credentials.sort_by(|left, right| left.credential_id.cmp(&right.credential_id));
        credentials
    }

    pub fn server_credential_for_actor(
        &self,
        actor: &ControlTokenClaims,
        credential_id: &str,
    ) -> Result<ServerCredentialSummary, ControlPlaneError> {
        if credential_id.trim().is_empty() {
            return Err(ControlPlaneError::InvalidInput);
        }
        let inner = self.store.read();
        let credential = inner
            .server_credentials
            .get(credential_id)
            .filter(|credential| {
                actor.role == ControlRole::Admin
                    || (actor.role == ControlRole::User && credential.user_id == actor.user_id)
            })
            .ok_or(ControlPlaneError::ServerCredentialNotFound)?;
        Ok(server_credential_summary(credential))
    }

    pub fn update_server_credential_status(
        &self,
        actor: &ControlTokenClaims,
        credential_id: &str,
        request: UpdateServerCredentialStatusRequest,
    ) -> Result<ServerCredentialSummary, ControlPlaneError> {
        if credential_id.trim().is_empty() {
            return Err(ControlPlaneError::InvalidInput);
        }
        if !matches!(actor.role, ControlRole::Admin | ControlRole::User) {
            return Err(ControlPlaneError::ServerCredentialNotFound);
        }
        let summary = {
            let mut inner = self.store.write();
            let summary = {
                let credential = inner
                    .server_credentials
                    .get_mut(credential_id)
                    .filter(|credential| {
                        actor.role == ControlRole::Admin
                            || (actor.role == ControlRole::User
                                && credential.user_id == actor.user_id)
                    })
                    .ok_or(ControlPlaneError::ServerCredentialNotFound)?;
                if credential.enabled != request.enabled {
                    credential.enabled = request.enabled;
                    credential.token_version = credential.token_version.saturating_add(1).max(1);
                }
                server_credential_summary(credential)
            };
            push_audit_log_locked(
                &mut inner,
                actor.user_id.clone(),
                actor.subject.clone(),
                actor.role,
                "server_credential.status.update",
                "server_credential",
                summary.credential_id.clone(),
                format!("set server credential enabled={}", summary.enabled),
            );
            summary
        };
        self.persist()
            .map_err(|_| ControlPlaneError::PersistenceFailed)?;
        Ok(summary)
    }

    pub fn rotate_server_credential(
        &self,
        actor: &ControlTokenClaims,
        credential_id: &str,
    ) -> Result<ServerCredentialResponse, ControlPlaneError> {
        if credential_id.trim().is_empty() {
            return Err(ControlPlaneError::InvalidInput);
        }
        if !matches!(actor.role, ControlRole::Admin | ControlRole::User) {
            return Err(ControlPlaneError::ServerCredentialNotFound);
        }
        let credential = {
            let mut inner = self.store.write();
            let credential = inner
                .server_credentials
                .get_mut(credential_id)
                .filter(|credential| {
                    actor.role == ControlRole::Admin
                        || (actor.role == ControlRole::User && credential.user_id == actor.user_id)
                })
                .ok_or(ControlPlaneError::ServerCredentialNotFound)?;
            credential.token_version = credential.token_version.saturating_add(1).max(1);
            credential.enabled = true;
            let credential = credential.clone();
            push_audit_log_locked(
                &mut inner,
                actor.user_id.clone(),
                actor.subject.clone(),
                actor.role,
                "server_credential.rotate",
                "server_credential",
                credential.credential_id.clone(),
                format!(
                    "rotated server credential to version {}",
                    credential.token_version
                ),
            );
            credential
        };
        self.persist()
            .map_err(|_| ControlPlaneError::PersistenceFailed)?;

        let server_token = TokenIssuer::new(self.token_secret())
            .issue_agent_control_token(
                credential.user_id,
                credential.credential_id.clone(),
                credential.token_version,
                CONTROL_TOKEN_EXP,
            )
            .map_err(|_| ControlPlaneError::TokenIssueFailed)?;
        Ok(ServerCredentialResponse {
            credential_id: credential.credential_id,
            device_id: credential.device_id,
            server_token,
            token_type: "bearer".to_string(),
        })
    }

    pub fn control_claims_from_bearer(
        &self,
        bearer_token: Option<&str>,
    ) -> Result<ControlTokenClaims, ControlAuthError> {
        let Some(token) = bearer_token else {
            if self.strict_auth {
                return Err(ControlAuthError::InvalidToken);
            }
            return Ok(ControlTokenClaims {
                user_id: self.default_user_id(),
                subject: "anonymous-dev".to_string(),
                role: ControlRole::User,
                exp: CONTROL_TOKEN_EXP,
                relay_token_version: None,
                credential_id: None,
                server_credential_version: None,
            });
        };
        let token = token
            .strip_prefix("Bearer ")
            .or_else(|| token.strip_prefix("bearer "))
            .ok_or(ControlAuthError::InvalidToken)?;
        TokenSigner::new(TokenKey::new(self.token_secret()))
            .verify_control(token, CONTROL_TOKEN_NOW)
            .map_err(|_| ControlAuthError::InvalidToken)
            .and_then(|claims| {
                if self.control_claims_enabled(&claims) {
                    if claims.role == ControlRole::Agent {
                        self.touch_agent_credential_last_used(&claims);
                    }
                    Ok(claims)
                } else {
                    Err(ControlAuthError::InvalidToken)
                }
            })
    }

    pub fn user_from_bearer(&self, bearer_token: Option<&str>) -> Result<UserId, ControlAuthError> {
        Ok(self.control_claims_from_bearer(bearer_token)?.user_id)
    }

    pub fn agent_credential_device_id(&self, claims: &ControlTokenClaims) -> Option<DeviceId> {
        if claims.role != ControlRole::Agent {
            return None;
        }
        let credential_id = claims
            .credential_id
            .as_deref()
            .filter(|credential_id| !credential_id.trim().is_empty())
            .unwrap_or(claims.subject.as_str());
        let token_version = claims.server_credential_version?;
        self.store
            .read()
            .server_credentials
            .get(credential_id)
            .filter(|credential| {
                credential.enabled
                    && credential.token_version == token_version
                    && credential.user_id == claims.user_id
            })
            .map(|credential| credential.device_id.clone())
    }

    fn login_or_create_github_oauth_user(
        &self,
        profile: GitHubUserProfile,
        verified_email: String,
    ) -> Result<AuthResponse, ControlAuthError> {
        let email = normalized_email(&verified_email);
        if email.is_empty() || profile.id.trim().is_empty() {
            return Err(ControlAuthError::InvalidInput);
        }
        let key = oauth_identity_key(OAuthProvider::GitHub, &profile.id);
        let now = current_epoch_sec();
        let (user_id, role) = {
            let mut inner = self.store.write();
            if let Some(existing) = inner.oauth_identities.get_mut(&key) {
                existing.email = email.clone();
                existing.login = profile.login.clone();
                existing.avatar_url = profile.avatar_url.clone();
                existing.updated_epoch_sec = now;
                let user_id = existing.user_id.clone();
                let account = inner
                    .users
                    .get(&user_id)
                    .ok_or(ControlAuthError::InvalidCredentials)?;
                if !account.enabled {
                    return Err(ControlAuthError::InvalidCredentials);
                }
                (user_id, account.role)
            } else if let Some(user_id) = inner.user_ids_by_email.get(&email).cloned() {
                let account = inner
                    .users
                    .get(&user_id)
                    .ok_or(ControlAuthError::InvalidCredentials)?;
                if !account.enabled {
                    return Err(ControlAuthError::InvalidCredentials);
                }
                let role = account.role;
                let identity = OAuthIdentity {
                    provider: OAuthProvider::GitHub,
                    provider_user_id: profile.id,
                    user_id: user_id.clone(),
                    email: email.clone(),
                    login: profile.login,
                    avatar_url: profile.avatar_url,
                    created_epoch_sec: now,
                    updated_epoch_sec: now,
                };
                inner.oauth_identities.insert(key, identity);
                (user_id, role)
            } else {
                let user_id = UserId::new(format!("user_{}", uuid::Uuid::new_v4()));
                let display_name = profile
                    .name
                    .clone()
                    .unwrap_or_else(|| profile.login.clone());
                let account = UserAccount {
                    user_id: user_id.clone(),
                    email: email.clone(),
                    display_name,
                    password_hash: String::new(),
                    role: ControlRole::User,
                    enabled: true,
                };
                let identity = OAuthIdentity {
                    provider: OAuthProvider::GitHub,
                    provider_user_id: profile.id,
                    user_id: user_id.clone(),
                    email: email.clone(),
                    login: profile.login,
                    avatar_url: profile.avatar_url,
                    created_epoch_sec: now,
                    updated_epoch_sec: now,
                };
                inner
                    .user_ids_by_email
                    .insert(email.clone(), user_id.clone());
                inner.users.insert(user_id.clone(), account);
                inner.plans.insert(user_id.clone(), default_plan());
                inner.oauth_identities.insert(key, identity);
                (user_id, ControlRole::User)
            }
        };
        self.persist()
            .map_err(|_| ControlAuthError::PersistenceFailed)?;
        self.auth_response(user_id, email, role)
    }

    fn control_claims_enabled(&self, claims: &ControlTokenClaims) -> bool {
        let inner = self.store.read();
        match claims.role {
            ControlRole::Relay => {
                let token_version = claims.relay_token_version.unwrap_or(1);
                if claims.subject.trim().is_empty() || token_version == 0 {
                    return false;
                }
                inner
                    .relay_credentials
                    .get(&claims.subject)
                    .map(|credential| {
                        credential.enabled && credential.token_version == token_version
                    })
                    .unwrap_or(token_version == 1)
            }
            ControlRole::User | ControlRole::Admin => inner
                .users
                .get(&claims.user_id)
                .map(|account| account.enabled)
                .unwrap_or_else(|| claims.user_id == self.default_user_id()),
            ControlRole::Agent => {
                let Some(credential_id) = claims
                    .credential_id
                    .as_deref()
                    .filter(|credential_id| !credential_id.trim().is_empty())
                    .or_else(|| {
                        (!claims.subject.trim().is_empty()).then_some(claims.subject.as_str())
                    })
                else {
                    return false;
                };
                let Some(token_version) = claims.server_credential_version else {
                    return false;
                };
                inner
                    .server_credentials
                    .get(credential_id)
                    .map(|credential| {
                        credential.enabled
                            && credential.token_version == token_version
                            && credential.user_id == claims.user_id
                            && credential.credential_id == claims.subject
                    })
                    .unwrap_or(false)
            }
        }
    }

    fn touch_agent_credential_last_used(&self, claims: &ControlTokenClaims) {
        let Some(credential_id) = claims
            .credential_id
            .as_deref()
            .filter(|credential_id| !credential_id.trim().is_empty())
            .or_else(|| (!claims.subject.trim().is_empty()).then_some(claims.subject.as_str()))
        else {
            return;
        };
        let Some(token_version) = claims.server_credential_version else {
            return;
        };
        let touched = {
            let mut inner = self.store.write();
            let Some(credential) = inner.server_credentials.get_mut(credential_id) else {
                return;
            };
            if !credential.enabled
                || credential.token_version != token_version
                || credential.user_id != claims.user_id
                || credential.credential_id != claims.subject
            {
                return;
            }
            credential.last_used_epoch_sec = Some(current_epoch_sec());
            true
        };
        if touched {
            let _ = self.persist();
        }
    }

    pub fn users(&self) -> Vec<UserSummary> {
        let inner = self.store.read();
        let mut users: Vec<_> = inner
            .users
            .values()
            .map(|account| self.user_summary_locked(&inner, account))
            .collect();
        users.sort_by(|left, right| left.email.cmp(&right.email));
        users
    }

    pub fn audit_logs(&self) -> Vec<AuditLogEntry> {
        let mut logs = self.store.read().audit_logs.clone();
        logs.reverse();
        logs
    }

    pub fn dashboard_summary(&self) -> DashboardSummary {
        let inner = self.store.read();
        let users = DashboardUserStats {
            total: inner.users.len() as u64,
            enabled: inner
                .users
                .values()
                .filter(|account| account.enabled)
                .count() as u64,
            admins: inner
                .users
                .values()
                .filter(|account| account.role == ControlRole::Admin)
                .count() as u64,
        };
        let controllers = DashboardControllerStats {
            total: inner
                .controllers
                .values()
                .map(|controllers| controllers.len() as u64)
                .sum(),
        };
        let devices = DashboardDeviceStats {
            total: inner.devices.len() as u64,
            online: inner
                .devices
                .values()
                .filter(|device| device.status == DeviceStatus::Online)
                .count() as u64,
        };
        let mut sessions = DashboardSessionStats::default();
        let mut usage = DashboardUsageStats::default();
        for assignment in inner
            .agent_sessions
            .values()
            .flat_map(|assignments| assignments.iter())
        {
            sessions.total += 1;
            match assignment.status {
                AgentSessionStatus::Pending => sessions.pending += 1,
                AgentSessionStatus::Claimed => sessions.claimed += 1,
                AgentSessionStatus::Bound => sessions.bound += 1,
                AgentSessionStatus::Closed => sessions.closed += 1,
                AgentSessionStatus::Expired => sessions.expired += 1,
            }
            if let Some(record) = inner.relay_session_usage.get(&assignment.session_id) {
                usage.actual_uplink_bytes = usage
                    .actual_uplink_bytes
                    .saturating_add(record.stats.uplink_bytes);
                usage.actual_downlink_bytes = usage
                    .actual_downlink_bytes
                    .saturating_add(record.stats.downlink_bytes);
                usage.actual_total_bytes = usage
                    .actual_total_bytes
                    .saturating_add(record.stats.total_bytes);
            }
        }
        let mut relays = DashboardRelayStats {
            total: inner.relays.len() as u64,
            ..DashboardRelayStats::default()
        };
        for relay in inner.relays.values().cloned() {
            if self.relay_with_effective_health(relay).healthy {
                relays.healthy += 1;
            } else {
                relays.unhealthy += 1;
            }
        }
        let recent_audit_logs = inner.audit_logs.iter().rev().take(5).cloned().collect();

        DashboardSummary {
            users,
            devices,
            controllers,
            sessions,
            relays,
            usage,
            recent_audit_logs,
        }
    }

    pub fn admin_session_summaries(&self) -> Vec<AdminSessionSummary> {
        let inner = self.store.read();
        let mut summaries = Vec::new();
        for (device_id, assignments) in &inner.agent_sessions {
            let Some(device) = inner.devices.get(device_id) else {
                continue;
            };
            for assignment in assignments {
                let session_user_id = session_user_id_locked(device, assignment);
                let user_email = inner
                    .users
                    .get(&session_user_id)
                    .map(|account| account.email.clone())
                    .unwrap_or_default();
                let service_name = inner
                    .services
                    .get(device_id)
                    .and_then(|services| {
                        services
                            .iter()
                            .find(|service| service.service_id == assignment.service_id)
                    })
                    .map(|service| service.name.clone())
                    .unwrap_or_default();
                summaries.push(AdminSessionSummary {
                    session_id: assignment.session_id.clone(),
                    user_id: session_user_id,
                    user_email,
                    device_id: device.device_id.clone(),
                    device_name: device.name.clone(),
                    service_id: assignment.service_id.clone(),
                    service_name,
                    client_id: assignment.client_id.clone(),
                    status: assignment.status,
                    relay_addr: assignment.relay_addr.clone(),
                    punch_addr: assignment.punch_addr.clone(),
                    expire_at: assignment.expire_at,
                });
            }
        }
        summaries.sort_by(|left, right| left.session_id.cmp(&right.session_id));
        summaries
    }

    pub fn user_usage_summaries(&self) -> Vec<UserUsageSummary> {
        let inner = self.store.read();
        let signer = TokenSigner::new(TokenKey::new(self.token_secret()));
        let mut usage_by_user: HashMap<UserId, UsageAccumulator> = HashMap::new();

        for (device_id, assignments) in &inner.agent_sessions {
            let Some(device) = inner.devices.get(device_id) else {
                continue;
            };
            for assignment in assignments {
                let session_user_id = session_user_id_locked(device, assignment);
                let usage = usage_by_user.entry(session_user_id).or_default();
                usage.session_count += 1;
                match assignment.status {
                    AgentSessionStatus::Pending => usage.pending_sessions += 1,
                    AgentSessionStatus::Claimed => usage.claimed_sessions += 1,
                    AgentSessionStatus::Bound => usage.bound_sessions += 1,
                    AgentSessionStatus::Closed => usage.closed_sessions += 1,
                    AgentSessionStatus::Expired => usage.expired_sessions += 1,
                }
                if let Ok(claims) = signer.verify_relay(&assignment.relay_token, CONTROL_TOKEN_NOW)
                {
                    usage.relay_quota_granted_bytes = usage
                        .relay_quota_granted_bytes
                        .saturating_add(claims.traffic_quota_bytes);
                }
                if let Some(actual) = inner.relay_session_usage.get(&assignment.session_id) {
                    usage.actual_uplink_bytes = usage
                        .actual_uplink_bytes
                        .saturating_add(actual.stats.uplink_bytes);
                    usage.actual_downlink_bytes = usage
                        .actual_downlink_bytes
                        .saturating_add(actual.stats.downlink_bytes);
                    usage.actual_total_bytes = usage
                        .actual_total_bytes
                        .saturating_add(actual.stats.total_bytes);
                }
            }
        }

        let mut summaries: Vec<_> = inner
            .users
            .values()
            .map(|account| {
                let user = self.user_summary_locked(&inner, account);
                let plan = inner
                    .plans
                    .get(&account.user_id)
                    .cloned()
                    .unwrap_or_else(default_plan);
                let period = inner
                    .user_usage_periods
                    .get(&account.user_id)
                    .cloned()
                    .unwrap_or_else(|| default_user_usage_period(&account.user_id));
                let usage = usage_by_user.remove(&account.user_id).unwrap_or_default();
                UserUsageSummary {
                    user_id: account.user_id.clone(),
                    email: user.email,
                    plan_id: plan.plan_id,
                    current_period_started_epoch_sec: period.current_period_started_epoch_sec,
                    max_controller_devices: plan.max_controller_devices,
                    controller_count: user.controller_count,
                    device_count: user.device_count,
                    session_count: usage.session_count,
                    pending_sessions: usage.pending_sessions,
                    claimed_sessions: usage.claimed_sessions,
                    bound_sessions: usage.bound_sessions,
                    closed_sessions: usage.closed_sessions,
                    expired_sessions: usage.expired_sessions,
                    current_session_quota_bytes: plan.relay_limits.traffic_quota_bytes,
                    relay_quota_granted_bytes: usage.relay_quota_granted_bytes,
                    actual_uplink_bytes: usage.actual_uplink_bytes,
                    actual_downlink_bytes: usage.actual_downlink_bytes,
                    actual_total_bytes: usage.actual_total_bytes,
                }
            })
            .collect();
        summaries.sort_by(|left, right| left.email.cmp(&right.email));
        summaries
    }

    pub fn ensure_user_relay_quota_available(
        &self,
        user_id: &UserId,
    ) -> Result<(), ControlPlaneError> {
        let inner = self.store.read();
        let plan = inner
            .plans
            .get(user_id)
            .cloned()
            .unwrap_or_else(default_plan);
        let actual_total_bytes = user_actual_relay_usage_total_locked(&inner, user_id);
        if actual_total_bytes >= plan.relay_limits.traffic_quota_bytes {
            return Err(ControlPlaneError::RelayTrafficQuotaExceeded);
        }
        Ok(())
    }

    pub fn reset_user_usage_period(
        &self,
        user_id: &UserId,
    ) -> Result<UserUsagePeriod, ControlPlaneError> {
        let period = UserUsagePeriod {
            user_id: user_id.clone(),
            current_period_started_epoch_sec: current_epoch_sec(),
        };

        {
            let mut inner = self.store.write();
            if !inner.users.contains_key(user_id) && user_id != &self.default_user_id() {
                return Err(ControlPlaneError::UserNotFound);
            }

            let mut reset_session_ids = Vec::new();
            for (device_id, assignments) in &inner.agent_sessions {
                let Some(device) = inner.devices.get(device_id) else {
                    continue;
                };
                for assignment in assignments {
                    if session_user_id_locked(device, assignment) == *user_id {
                        reset_session_ids.push(assignment.session_id.clone());
                    }
                }
            }
            for session_id in reset_session_ids {
                inner.relay_session_usage.remove(&session_id);
            }
            inner
                .user_usage_periods
                .insert(user_id.clone(), period.clone());
        }

        self.persist()
            .map_err(|_| ControlPlaneError::PersistenceFailed)?;
        Ok(period)
    }

    pub fn report_relay_session_usage(
        &self,
        request: ReportRelaySessionUsageRequest,
    ) -> Result<(), ControlPlaneError> {
        let relay_id = request.relay_id.trim();
        if relay_id.is_empty() {
            return Err(ControlPlaneError::InvalidInput);
        }
        {
            let mut inner = self.store.write();
            if !inner.relays.contains_key(relay_id) {
                return Err(ControlPlaneError::RelayNotFound);
            }
            for report in &request.sessions {
                if report
                    .stats
                    .session_id
                    .as_ref()
                    .map(|session_id| session_id != &report.session_id)
                    .unwrap_or(false)
                {
                    return Err(ControlPlaneError::InvalidInput);
                }
            }
            for report in request.sessions {
                inner.relay_session_usage.insert(
                    report.session_id,
                    RelaySessionUsageRecord {
                        relay_id: relay_id.to_string(),
                        stats: report.stats,
                    },
                );
            }
        }
        self.persist()
            .map_err(|_| ControlPlaneError::PersistenceFailed)
    }

    pub fn record_audit_log(
        &self,
        actor: &ControlTokenClaims,
        action: impl Into<String>,
        target_type: impl Into<String>,
        target_id: impl Into<String>,
        message: impl Into<String>,
    ) -> Result<AuditLogEntry, ControlPlaneError> {
        let entry = AuditLogEntry {
            audit_id: format!("audit_{}", uuid::Uuid::new_v4()),
            actor_user_id: actor.user_id.clone(),
            actor_subject: actor.subject.clone(),
            actor_role: actor.role,
            action: action.into(),
            target_type: target_type.into(),
            target_id: target_id.into(),
            message: message.into(),
            created_epoch_sec: current_epoch_sec(),
        };
        self.store.write().audit_logs.push(entry.clone());
        self.persist()
            .map_err(|_| ControlPlaneError::PersistenceFailed)?;
        Ok(entry)
    }

    pub fn create_user(
        &self,
        request: CreateUserRequest,
    ) -> Result<UserSummary, ControlPlaneError> {
        validate_email_password(&request.email, &request.password)
            .map_err(|_| ControlPlaneError::InvalidInput)?;
        validate_user_account_role(request.role)?;
        let email = normalized_email(&request.email);
        let summary = {
            let mut inner = self.store.write();
            if inner.user_ids_by_email.contains_key(&email) {
                return Err(ControlPlaneError::EmailAlreadyRegistered);
            }

            let user_id = UserId::new(format!("user_{}", uuid::Uuid::new_v4()));
            let account = UserAccount {
                user_id: user_id.clone(),
                email: email.clone(),
                display_name: request.display_name,
                password_hash: password_hash(&email, &request.password),
                role: request.role,
                enabled: request.enabled,
            };
            inner
                .user_ids_by_email
                .insert(email.clone(), user_id.clone());
            inner.users.insert(user_id.clone(), account.clone());
            inner.plans.insert(user_id, default_plan());
            self.user_summary_locked(&inner, &account)
        };
        self.persist()
            .map_err(|_| ControlPlaneError::PersistenceFailed)?;
        Ok(summary)
    }

    pub fn user_detail(&self, user_id: &UserId) -> Result<UserDetail, ControlPlaneError> {
        let inner = self.store.read();
        let account = inner
            .users
            .get(user_id)
            .ok_or(ControlPlaneError::UserNotFound)?;
        let user = self.user_summary_locked(&inner, account);
        let plan = inner
            .plans
            .get(user_id)
            .cloned()
            .unwrap_or_else(default_plan);
        let controllers = inner.controllers.get(user_id).cloned().unwrap_or_default();
        let mut devices: Vec<_> = inner
            .devices
            .values()
            .filter(|device| &device.user_id == user_id)
            .cloned()
            .collect();
        devices.sort_by(|left, right| left.device_id.cmp(&right.device_id));
        Ok(UserDetail {
            user,
            plan,
            controllers,
            devices,
        })
    }

    pub fn update_user_status(
        &self,
        user_id: &UserId,
        request: UpdateUserStatusRequest,
    ) -> Result<UserSummary, ControlPlaneError> {
        let summary = {
            let mut inner = self.store.write();
            let account = inner
                .users
                .get_mut(user_id)
                .ok_or(ControlPlaneError::UserNotFound)?;
            account.enabled = request.enabled;
            let account = account.clone();
            self.user_summary_locked(&inner, &account)
        };
        self.persist()
            .map_err(|_| ControlPlaneError::PersistenceFailed)?;
        Ok(summary)
    }

    pub fn update_user_role(
        &self,
        user_id: &UserId,
        request: UpdateUserRoleRequest,
    ) -> Result<UserSummary, ControlPlaneError> {
        validate_user_account_role(request.role)?;
        let summary = {
            let mut inner = self.store.write();
            let account = inner
                .users
                .get_mut(user_id)
                .ok_or(ControlPlaneError::UserNotFound)?;
            account.role = request.role;
            let account = account.clone();
            self.user_summary_locked(&inner, &account)
        };
        self.persist()
            .map_err(|_| ControlPlaneError::PersistenceFailed)?;
        Ok(summary)
    }

    fn user_summary_locked(
        &self,
        inner: &crate::store::ControlStore,
        account: &UserAccount,
    ) -> UserSummary {
        let email = if account.email.is_empty() {
            inner
                .user_ids_by_email
                .iter()
                .find(|(_, user_id)| *user_id == &account.user_id)
                .map(|(email, _)| email.clone())
                .unwrap_or_default()
        } else {
            account.email.clone()
        };
        let plan = inner
            .plans
            .get(&account.user_id)
            .cloned()
            .unwrap_or_else(default_plan);
        let controller_count = inner
            .controllers
            .get(&account.user_id)
            .map(|controllers| controllers.len() as u32)
            .unwrap_or(0);
        let device_count = inner
            .devices
            .values()
            .filter(|device| device.user_id == account.user_id)
            .count() as u32;
        UserSummary {
            user_id: account.user_id.clone(),
            email,
            display_name: account.display_name.clone(),
            role: account.role,
            enabled: account.enabled,
            plan_id: plan.plan_id,
            controller_count,
            device_count,
        }
    }

    pub fn plan_for_user(&self, user_id: &UserId) -> Plan {
        self.store
            .read()
            .plans
            .get(user_id)
            .cloned()
            .unwrap_or_else(default_plan)
    }

    pub fn plan_catalog(&self) -> Vec<Plan> {
        let mut plans: Vec<_> = self.store.read().plan_catalog.values().cloned().collect();
        plans.sort_by(|left, right| left.plan_id.cmp(&right.plan_id));
        plans
    }

    pub fn catalog_plan(&self, plan_id: &str) -> Result<Plan, ControlPlaneError> {
        if plan_id.trim().is_empty() {
            return Err(ControlPlaneError::InvalidInput);
        }
        self.store
            .read()
            .plan_catalog
            .get(plan_id)
            .cloned()
            .ok_or(ControlPlaneError::PlanNotFound)
    }

    pub fn update_catalog_plan(
        &self,
        request: UpdatePlanCatalogRequest,
    ) -> Result<Plan, ControlPlaneError> {
        validate_plan(&request.plan)?;
        let plan = request.plan;
        self.store
            .write()
            .plan_catalog
            .insert(plan.plan_id.clone(), plan.clone());
        self.persist()
            .map_err(|_| ControlPlaneError::PersistenceFailed)?;
        Ok(plan)
    }

    pub fn assign_user_plan(
        &self,
        user_id: &UserId,
        request: AssignUserPlanRequest,
    ) -> Result<Plan, ControlPlaneError> {
        if request.plan_id.trim().is_empty() {
            return Err(ControlPlaneError::InvalidInput);
        }
        let mut inner = self.store.write();
        if !inner.users.contains_key(user_id) && user_id != &self.default_user_id() {
            return Err(ControlPlaneError::UserNotFound);
        }
        let plan = inner
            .plan_catalog
            .get(&request.plan_id)
            .cloned()
            .ok_or(ControlPlaneError::PlanNotFound)?;
        inner.plans.insert(user_id.clone(), plan.clone());
        drop(inner);
        self.persist()
            .map_err(|_| ControlPlaneError::PersistenceFailed)?;
        Ok(plan)
    }

    pub fn relay_credentials(&self) -> Vec<RelayCredential> {
        let mut credentials: Vec<_> = self
            .store
            .read()
            .relay_credentials
            .values()
            .cloned()
            .collect();
        credentials.sort_by(|left, right| left.relay_id.cmp(&right.relay_id));
        credentials
    }

    pub fn relay_credential(&self, relay_id: &str) -> Result<RelayCredential, ControlPlaneError> {
        if relay_id.trim().is_empty() {
            return Err(ControlPlaneError::InvalidInput);
        }
        self.store
            .read()
            .relay_credentials
            .get(relay_id)
            .cloned()
            .ok_or(ControlPlaneError::RelayCredentialNotFound)
    }

    pub fn create_relay_credential(
        &self,
        request: CreateRelayCredentialRequest,
    ) -> Result<RelayCredential, ControlPlaneError> {
        if request.relay_id.trim().is_empty() {
            return Err(ControlPlaneError::InvalidInput);
        }
        let credential = RelayCredential {
            relay_id: request.relay_id,
            enabled: request.enabled,
            token_version: 1,
        };
        {
            let mut inner = self.store.write();
            if inner.relay_credentials.contains_key(&credential.relay_id) {
                return Err(ControlPlaneError::RelayCredentialAlreadyExists);
            }
            inner
                .relay_credentials
                .insert(credential.relay_id.clone(), credential.clone());
        }
        self.persist()
            .map_err(|_| ControlPlaneError::PersistenceFailed)?;
        Ok(credential)
    }

    pub fn update_relay_credential_status(
        &self,
        relay_id: &str,
        request: UpdateRelayCredentialStatusRequest,
    ) -> Result<RelayCredential, ControlPlaneError> {
        if relay_id.trim().is_empty() {
            return Err(ControlPlaneError::InvalidInput);
        }
        let credential = {
            let mut inner = self.store.write();
            let credential = inner
                .relay_credentials
                .get_mut(relay_id)
                .ok_or(ControlPlaneError::RelayCredentialNotFound)?;
            credential.enabled = request.enabled;
            credential.clone()
        };
        self.persist()
            .map_err(|_| ControlPlaneError::PersistenceFailed)?;
        Ok(credential)
    }

    pub fn rotate_relay_credential(
        &self,
        relay_id: &str,
    ) -> Result<RelayCredential, ControlPlaneError> {
        if relay_id.trim().is_empty() {
            return Err(ControlPlaneError::InvalidInput);
        }
        let credential = {
            let mut inner = self.store.write();
            let credential = inner
                .relay_credentials
                .get_mut(relay_id)
                .ok_or(ControlPlaneError::RelayCredentialNotFound)?;
            credential.token_version = credential.token_version.saturating_add(1).max(1);
            credential.enabled = true;
            credential.clone()
        };
        self.persist()
            .map_err(|_| ControlPlaneError::PersistenceFailed)?;
        Ok(credential)
    }

    pub fn create_relay_bootstrap(
        &self,
        actor: &ControlTokenClaims,
        request: CreateRelayBootstrapRequest,
    ) -> Result<RelayBootstrapResponse, ControlPlaneError> {
        let relay_id = request.relay_id.trim().to_string();
        let control_url = normalized_relay_control_url(&request.control_url);
        let relay_addr = request.relay_addr.trim().to_string();
        let admin_addr = String::new();
        if relay_id.is_empty()
            || control_url.is_empty()
            || relay_addr.is_empty()
            || request.capacity_streams == 0
            || request.heartbeat_interval_sec == 0
            || request.ttl_sec == 0
        {
            return Err(ControlPlaneError::InvalidInput);
        }

        let ttl_sec = request.ttl_sec.min(RELAY_BOOTSTRAP_MAX_TTL_SEC);
        let now = self.relay_health_now_epoch_sec();
        let bootstrap_id = format!("rb_{}", uuid::Uuid::new_v4().simple());
        let bootstrap_token = format!("rbt_{}", uuid::Uuid::new_v4().simple());
        let expires_epoch_sec = now.saturating_add(ttl_sec);
        let record = RelayBootstrapRecord {
            bootstrap_id: bootstrap_id.clone(),
            control_url: control_url.clone(),
            relay_id: relay_id.clone(),
            relay_addr,
            admin_addr,
            capacity_streams: request.capacity_streams,
            heartbeat_interval_sec: request.heartbeat_interval_sec,
            token_secret: self.token_secret.clone(),
            token_hash: secret_hash(&bootstrap_token),
            created_epoch_sec: now,
            expires_epoch_sec,
            consumed_epoch_sec: None,
            created_by: actor.subject.clone(),
        };

        self.store
            .write()
            .relay_bootstraps
            .insert(bootstrap_id.clone(), record);
        self.persist()
            .map_err(|_| ControlPlaneError::PersistenceFailed)?;

        Ok(RelayBootstrapResponse {
            install_command: relay_bootstrap_install_command(
                &control_url,
                &bootstrap_id,
                &bootstrap_token,
                false,
            ),
            no_service_install_command: relay_bootstrap_install_command(
                &control_url,
                &bootstrap_id,
                &bootstrap_token,
                true,
            ),
            bootstrap_id,
            relay_id,
            control_url,
            expires_epoch_sec,
            bootstrap_token,
        })
    }

    pub fn exchange_relay_bootstrap(
        &self,
        bootstrap_id: &str,
        request: RelayBootstrapExchangeRequest,
    ) -> Result<RelayBootstrapExchangeResponse, ControlPlaneError> {
        if bootstrap_id.trim().is_empty() || request.bootstrap_token.trim().is_empty() {
            return Err(ControlPlaneError::RelayBootstrapUnauthorized);
        }

        let now = self.relay_health_now_epoch_sec();
        let requested_hash = secret_hash(request.bootstrap_token.trim());
        let (
            control_url,
            relay_id,
            relay_addr,
            admin_addr,
            capacity_streams,
            heartbeat_interval_sec,
            token_secret,
            token_version,
        ) = {
            let mut inner = self.store.write();
            let record = inner
                .relay_bootstraps
                .get(bootstrap_id)
                .ok_or(ControlPlaneError::RelayBootstrapUnauthorized)?;
            if record.consumed_epoch_sec.is_some()
                || record.expires_epoch_sec <= now
                || record.token_hash != requested_hash
            {
                return Err(ControlPlaneError::RelayBootstrapUnauthorized);
            }

            let control_url = normalized_relay_control_url(&record.control_url);
            let relay_id = record.relay_id.clone();
            let relay_addr = record.relay_addr.clone();
            let admin_addr = String::new();
            let capacity_streams = record.capacity_streams;
            let heartbeat_interval_sec = record.heartbeat_interval_sec;
            let token_secret = record.token_secret.clone();
            let token_version = {
                let credential = inner
                    .relay_credentials
                    .entry(relay_id.clone())
                    .or_insert_with(|| RelayCredential {
                        relay_id: relay_id.clone(),
                        enabled: true,
                        token_version: 1,
                    });
                credential.enabled = true;
                credential.token_version
            };
            inner
                .relay_bootstraps
                .get_mut(bootstrap_id)
                .expect("relay bootstrap checked above")
                .consumed_epoch_sec = Some(now);

            (
                control_url,
                relay_id,
                relay_addr,
                admin_addr,
                capacity_streams,
                heartbeat_interval_sec,
                token_secret,
                token_version,
            )
        };
        self.persist()
            .map_err(|_| ControlPlaneError::PersistenceFailed)?;
        let control_token = TokenIssuer::new(self.token_secret())
            .issue_relay_control_token(
                self.default_user_id(),
                relay_id.clone(),
                token_version,
                CONTROL_TOKEN_EXP,
            )
            .map_err(|_| ControlPlaneError::TokenIssueFailed)?;

        Ok(RelayBootstrapExchangeResponse {
            control_url,
            control_token,
            relay_id,
            token_secret,
            relay_addr,
            admin_addr,
            capacity_streams,
            heartbeat_interval_sec,
        })
    }

    pub fn managed_plan_for_user(&self, user_id: &UserId) -> Result<Plan, ControlPlaneError> {
        let inner = self.store.read();
        inner
            .plans
            .get(user_id)
            .cloned()
            .filter(|_| inner.users.contains_key(user_id) || user_id == &self.default_user_id())
            .ok_or(ControlPlaneError::UserNotFound)
    }

    pub fn update_user_plan(
        &self,
        user_id: &UserId,
        request: UpdateUserPlanRequest,
    ) -> Result<Plan, ControlPlaneError> {
        validate_plan(&request.plan)?;
        let mut inner = self.store.write();
        if !inner.users.contains_key(user_id) && !inner.plans.contains_key(user_id) {
            return Err(ControlPlaneError::UserNotFound);
        }

        let plan = request.plan;
        inner.plans.insert(user_id.clone(), plan.clone());
        drop(inner);
        self.persist()
            .map_err(|_| ControlPlaneError::PersistenceFailed)?;
        Ok(plan)
    }

    pub fn register_controller(
        &self,
        user_id: &UserId,
        request: RegisterControllerDeviceRequest,
    ) -> Result<ControllerDevice, ControlPlaneError> {
        if request.client_id.trim().is_empty() {
            return Err(ControlPlaneError::InvalidInput);
        }
        let client_id = ClientId::new(request.client_id);
        let mut inner = self.store.write();
        let plan = inner
            .plans
            .get(user_id)
            .cloned()
            .unwrap_or_else(default_plan);
        let controllers = inner.controllers.entry(user_id.clone()).or_default();
        if let Some(existing) = controllers
            .iter_mut()
            .find(|controller| controller.client_id == client_id)
        {
            existing.name = request.name;
            let controller = existing.clone();
            drop(inner);
            self.persist()
                .map_err(|_| ControlPlaneError::PersistenceFailed)?;
            return Ok(controller);
        }
        if controllers.len() >= plan.max_controller_devices as usize {
            return Err(ControlPlaneError::ControllerLimitExceeded);
        }

        let controller = ControllerDevice {
            user_id: user_id.clone(),
            client_id,
            name: request.name,
        };
        controllers.push(controller.clone());
        drop(inner);
        self.persist()
            .map_err(|_| ControlPlaneError::PersistenceFailed)?;
        Ok(controller)
    }

    pub fn controllers_for_user(&self, user_id: &UserId) -> Vec<ControllerDevice> {
        let mut controllers = self
            .store
            .read()
            .controllers
            .get(user_id)
            .cloned()
            .unwrap_or_default();
        controllers.sort_by(|left, right| left.client_id.cmp(&right.client_id));
        controllers
    }

    pub fn remove_controller(
        &self,
        user_id: &UserId,
        client_id: &str,
    ) -> Result<(), ControlPlaneError> {
        if client_id.trim().is_empty() {
            return Err(ControlPlaneError::InvalidInput);
        }

        let client_id = ClientId::new(client_id);
        let mut inner = self.store.write();
        let controllers = inner
            .controllers
            .get_mut(user_id)
            .ok_or(ControlPlaneError::ControllerNotFound)?;
        let original_len = controllers.len();
        controllers.retain(|controller| controller.client_id != client_id);
        if controllers.len() == original_len {
            return Err(ControlPlaneError::ControllerNotFound);
        }
        drop(inner);
        self.persist()
            .map_err(|_| ControlPlaneError::PersistenceFailed)
    }

    pub fn ensure_controller_for_session(
        &self,
        user_id: &UserId,
        client_id: &ClientId,
    ) -> Result<(), ControlPlaneError> {
        if self.controller_exists(user_id, client_id) {
            return Ok(());
        }
        self.register_controller(
            user_id,
            RegisterControllerDeviceRequest {
                client_id: client_id.to_string(),
                name: client_id.to_string(),
            },
        )
        .map(|_| ())
    }

    pub fn register_relay(
        &self,
        request: RegisterRelayRequest,
    ) -> Result<RelayNode, ControlPlaneError> {
        if request.relay_id.trim().is_empty()
            || request.relay_addr.trim().is_empty()
            || request.capacity_streams == 0
        {
            return Err(ControlPlaneError::InvalidInput);
        }
        let now = self.relay_health_now_epoch_sec();
        let relay = RelayNode {
            relay_id: request.relay_id,
            relay_addr: request.relay_addr,
            admin_bound: false,
            admin_addr: String::new(),
            capacity_streams: request.capacity_streams,
            healthy: true,
            last_seen_epoch_sec: now,
            health_status: RelayHealthStatus::Healthy,
            health_reason: String::new(),
            relay_version: String::new(),
            uptime_sec: 0,
            active_sessions: 0,
            active_streams: 0,
            total_uplink_bytes: 0,
            total_downlink_bytes: 0,
            total_bytes: 0,
            data_plane_bound: true,
            last_health_report_epoch_sec: now,
        };
        self.store
            .write()
            .relays
            .insert(relay.relay_id.clone(), relay.clone());
        self.persist()
            .map_err(|_| ControlPlaneError::PersistenceFailed)?;
        Ok(relay)
    }

    pub fn update_relay(
        &self,
        relay_id: &str,
        request: UpdateRelayRequest,
    ) -> Result<RelayNode, ControlPlaneError> {
        if relay_id.trim().is_empty()
            || request.relay_addr.trim().is_empty()
            || request.capacity_streams == 0
        {
            return Err(ControlPlaneError::InvalidInput);
        }

        let mut inner = self.store.write();
        let relay = inner
            .relays
            .get_mut(relay_id)
            .ok_or(ControlPlaneError::RelayNotFound)?;
        relay.relay_addr = request.relay_addr;
        relay.admin_addr.clear();
        relay.capacity_streams = request.capacity_streams;
        apply_legacy_relay_health(relay, request.healthy, self.relay_health_now_epoch_sec());
        let relay = relay.clone();
        drop(inner);
        self.persist()
            .map_err(|_| ControlPlaneError::PersistenceFailed)?;
        Ok(self.relay_with_effective_health(relay))
    }

    pub fn report_relay_health(
        &self,
        relay_id: &str,
        request: ReportRelayHealthRequest,
    ) -> Result<RelayNode, ControlPlaneError> {
        if relay_id.trim().is_empty()
            || request.relay_addr.trim().is_empty()
            || request.capacity_streams == 0
            || request.health.total_bytes
                != request
                    .health
                    .total_uplink_bytes
                    .saturating_add(request.health.total_downlink_bytes)
        {
            return Err(ControlPlaneError::InvalidInput);
        }

        let now = self.relay_health_now_epoch_sec();
        let mut inner = self.store.write();
        let relay = inner
            .relays
            .get_mut(relay_id)
            .ok_or(ControlPlaneError::RelayNotFound)?;
        relay.relay_addr = request.relay_addr;
        relay.admin_addr.clear();
        relay.capacity_streams = request.capacity_streams;
        apply_relay_health_report(relay, request.health, now);
        let relay = relay.clone();
        let snapshots = request
            .sessions
            .into_iter()
            .map(|mut snapshot| {
                if snapshot.last_seen_epoch_sec == 0 {
                    snapshot.last_seen_epoch_sec = now;
                }
                (snapshot.session_id.clone(), snapshot)
            })
            .collect();
        inner
            .relay_session_snapshots
            .insert(relay_id.to_string(), snapshots);
        drop(inner);
        self.persist()
            .map_err(|_| ControlPlaneError::PersistenceFailed)?;
        Ok(self.relay_with_effective_health(relay))
    }

    pub fn remove_relay(&self, relay_id: &str) -> Result<(), ControlPlaneError> {
        if relay_id.trim().is_empty() {
            return Err(ControlPlaneError::InvalidInput);
        }

        let mut inner = self.store.write();
        if inner.relays.remove(relay_id).is_none() {
            return Err(ControlPlaneError::RelayNotFound);
        }
        inner.relay_session_snapshots.remove(relay_id);
        inner
            .relay_commands
            .retain(|_, command| command.relay_id != relay_id);
        drop(inner);
        self.persist()
            .map_err(|_| ControlPlaneError::PersistenceFailed)
    }

    pub fn relays(&self) -> Vec<RelayNode> {
        let mut relays: Vec<_> = self
            .store
            .read()
            .relays
            .values()
            .cloned()
            .map(|relay| self.relay_with_effective_health(relay))
            .collect();
        relays.sort_by(|left, right| left.relay_id.cmp(&right.relay_id));
        relays
    }

    pub fn relay(&self, relay_id: &str) -> Result<RelayNode, ControlPlaneError> {
        self.store
            .read()
            .relays
            .get(relay_id)
            .cloned()
            .map(|relay| self.relay_with_effective_health(relay))
            .ok_or(ControlPlaneError::RelayNotFound)
    }

    pub fn relay_sessions(
        &self,
        relay_id: &str,
    ) -> Result<Vec<RelaySessionSnapshot>, ControlPlaneError> {
        if relay_id.trim().is_empty() {
            return Err(ControlPlaneError::InvalidInput);
        }

        let inner = self.store.read();
        if !inner.relays.contains_key(relay_id) {
            return Err(ControlPlaneError::RelayNotFound);
        }
        let mut sessions: Vec<_> = inner
            .relay_session_snapshots
            .get(relay_id)
            .map(|sessions| sessions.values().cloned().collect())
            .unwrap_or_default();
        sessions.sort_by(|left, right| left.session_id.cmp(&right.session_id));
        Ok(sessions)
    }

    pub fn request_relay_session_disconnect(
        &self,
        relay_id: &str,
        session_id: &SessionId,
    ) -> Result<RelayCommand, ControlPlaneError> {
        if relay_id.trim().is_empty() || session_id.as_str().trim().is_empty() {
            return Err(ControlPlaneError::InvalidInput);
        }

        let now = self.relay_health_now_epoch_sec();
        let command = RelayCommand {
            command_id: format!("rc_{}", uuid::Uuid::new_v4().simple()),
            relay_id: relay_id.to_string(),
            kind: RelayCommandKind::DisconnectSession,
            session_id: Some(session_id.clone()),
            status: RelayCommandStatus::Pending,
            requested_epoch_sec: now,
            updated_epoch_sec: now,
            message: String::new(),
        };
        let mut inner = self.store.write();
        if !inner.relays.contains_key(relay_id) {
            return Err(ControlPlaneError::RelayNotFound);
        }
        inner
            .relay_commands
            .insert(command.command_id.clone(), command.clone());
        drop(inner);
        self.persist()
            .map_err(|_| ControlPlaneError::PersistenceFailed)?;
        Ok(command)
    }

    pub fn pending_relay_commands(
        &self,
        relay_id: &str,
    ) -> Result<Vec<RelayCommand>, ControlPlaneError> {
        if relay_id.trim().is_empty() {
            return Err(ControlPlaneError::InvalidInput);
        }

        let inner = self.store.read();
        if !inner.relays.contains_key(relay_id) {
            return Err(ControlPlaneError::RelayNotFound);
        }
        let mut commands: Vec<_> = inner
            .relay_commands
            .values()
            .filter(|command| {
                command.relay_id == relay_id && command.status == RelayCommandStatus::Pending
            })
            .cloned()
            .collect();
        commands.sort_by(|left, right| {
            left.requested_epoch_sec
                .cmp(&right.requested_epoch_sec)
                .then_with(|| left.command_id.cmp(&right.command_id))
        });
        Ok(commands)
    }

    pub fn report_relay_command_result(
        &self,
        relay_id: &str,
        command_id: &str,
        request: ReportRelayCommandResultRequest,
    ) -> Result<RelayCommand, ControlPlaneError> {
        if relay_id.trim().is_empty()
            || command_id.trim().is_empty()
            || request.status == RelayCommandStatus::Pending
        {
            return Err(ControlPlaneError::InvalidInput);
        }

        let now = self.relay_health_now_epoch_sec();
        let command = {
            let mut inner = self.store.write();
            let command = inner
                .relay_commands
                .get_mut(command_id)
                .filter(|command| command.relay_id == relay_id)
                .ok_or(ControlPlaneError::RelayCommandNotFound)?;
            command.status = request.status;
            command.updated_epoch_sec = now;
            command.message = request.message;
            command.clone()
        };
        self.persist()
            .map_err(|_| ControlPlaneError::PersistenceFailed)?;
        Ok(command)
    }

    pub fn select_relay(&self) -> Result<RelayNode, ControlPlaneError> {
        self.relays()
            .into_iter()
            .filter(|relay| relay.healthy)
            .max_by_key(|relay| relay.capacity_streams)
            .ok_or(ControlPlaneError::NoRelayAvailable)
    }

    fn relay_with_effective_health(&self, mut relay: RelayNode) -> RelayNode {
        if self
            .relay_health_now_epoch_sec()
            .saturating_sub(relay.last_seen_epoch_sec)
            > RELAY_HEARTBEAT_TIMEOUT_SEC
        {
            relay.healthy = false;
            relay.health_status = RelayHealthStatus::Unhealthy;
            relay.health_reason = "heartbeat_stale".to_string();
        }
        relay
    }

    pub fn register_device(&self, device: Device) -> Result<(), ControlPersistenceError> {
        self.register_device_for_user(&self.default_user_id(), device)
    }

    pub fn register_device_for_user(
        &self,
        user_id: &UserId,
        mut device: Device,
    ) -> Result<(), ControlPersistenceError> {
        device.user_id = user_id.clone();
        {
            self.store
                .write()
                .devices
                .insert(device.device_id.clone(), device);
        }
        self.persist()
    }

    pub fn register_services(&self, services: Vec<Service>) -> Result<(), ControlPersistenceError> {
        self.upsert_services(services)
    }

    pub fn register_services_for_user(
        &self,
        user_id: &UserId,
        services: Vec<Service>,
    ) -> Result<(), ControlPersistenceError> {
        {
            let mut inner = self.store.write();
            for service in services {
                if !inner
                    .devices
                    .get(&service.device_id)
                    .map(|device| &device.user_id == user_id)
                    .unwrap_or(false)
                {
                    continue;
                }
                inner
                    .services
                    .entry(service.device_id.clone())
                    .or_default()
                    .retain(|existing| existing.service_id != service.service_id);
                inner
                    .services
                    .entry(service.device_id.clone())
                    .or_default()
                    .push(service);
            }
        }
        self.persist()
    }

    fn upsert_services(&self, services: Vec<Service>) -> Result<(), ControlPersistenceError> {
        {
            let mut inner = self.store.write();
            for service in services {
                inner
                    .services
                    .entry(service.device_id.clone())
                    .or_default()
                    .retain(|existing| existing.service_id != service.service_id);
                inner
                    .services
                    .entry(service.device_id.clone())
                    .or_default()
                    .push(service);
            }
        }
        self.persist()
    }

    pub fn register_p2p_certificate(
        &self,
        device_id: DeviceId,
        certificate_der: Vec<u8>,
    ) -> Result<(), ControlPersistenceError> {
        {
            self.store
                .write()
                .p2p_certificates
                .insert(device_id, certificate_der);
        }
        self.persist()
    }

    pub fn p2p_certificate_for_device(&self, device_id: &DeviceId) -> Option<Vec<u8>> {
        self.store.read().p2p_certificates.get(device_id).cloned()
    }

    pub fn devices(&self) -> Vec<Device> {
        self.devices_for_user(&self.default_user_id())
    }

    pub fn devices_for_user(&self, user_id: &UserId) -> Vec<Device> {
        let inner = self.store.read();
        let mut devices: Vec<_> = inner
            .devices
            .values()
            .filter(|device| user_can_access_device_locked(&inner, user_id, &device.device_id))
            .cloned()
            .collect();
        devices.sort_by(|left, right| left.device_id.cmp(&right.device_id));
        devices
    }

    pub fn device_for_user(
        &self,
        user_id: &UserId,
        device_id: &DeviceId,
    ) -> Result<Device, ControlPlaneError> {
        let inner = self.store.read();
        inner
            .devices
            .get(device_id)
            .filter(|_| user_can_access_device_locked(&inner, user_id, device_id))
            .cloned()
            .ok_or(ControlPlaneError::DeviceNotFound)
    }

    pub fn device_access_grants(
        &self,
        device_id: &DeviceId,
    ) -> Result<Vec<DeviceAccessGrant>, ControlPlaneError> {
        let inner = self.store.read();
        if !inner.devices.contains_key(device_id) {
            return Err(ControlPlaneError::DeviceNotFound);
        }
        let mut grants: Vec<_> = inner
            .device_access_grants
            .get(device_id)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|user_id| DeviceAccessGrant {
                device_id: device_id.clone(),
                user_id,
            })
            .collect();
        grants.sort_by(|left, right| left.user_id.cmp(&right.user_id));
        Ok(grants)
    }

    pub fn grant_device_access(
        &self,
        device_id: &DeviceId,
        request: GrantDeviceAccessRequest,
    ) -> Result<DeviceAccessGrant, ControlPlaneError> {
        if request.user_id.as_str().trim().is_empty() {
            return Err(ControlPlaneError::InvalidInput);
        }

        {
            let mut inner = self.store.write();
            let owner_user_id = inner
                .devices
                .get(device_id)
                .map(|device| device.user_id.clone())
                .ok_or(ControlPlaneError::DeviceNotFound)?;
            if owner_user_id == request.user_id {
                return Err(ControlPlaneError::InvalidInput);
            }
            if !inner.users.contains_key(&request.user_id)
                && request.user_id != self.default_user_id()
            {
                return Err(ControlPlaneError::UserNotFound);
            }
            let grants = inner
                .device_access_grants
                .entry(device_id.clone())
                .or_default();
            if !grants.iter().any(|user_id| user_id == &request.user_id) {
                grants.push(request.user_id.clone());
                grants.sort();
            }
        }

        self.persist()
            .map_err(|_| ControlPlaneError::PersistenceFailed)?;
        Ok(DeviceAccessGrant {
            device_id: device_id.clone(),
            user_id: request.user_id,
        })
    }

    pub fn revoke_device_access(
        &self,
        device_id: &DeviceId,
        user_id: &UserId,
    ) -> Result<(), ControlPlaneError> {
        if user_id.as_str().trim().is_empty() {
            return Err(ControlPlaneError::InvalidInput);
        }

        {
            let mut inner = self.store.write();
            if !inner.devices.contains_key(device_id) {
                return Err(ControlPlaneError::DeviceNotFound);
            }
            let grants = inner
                .device_access_grants
                .get_mut(device_id)
                .ok_or(ControlPlaneError::DeviceAccessGrantNotFound)?;
            let original_len = grants.len();
            grants.retain(|granted_user_id| granted_user_id != user_id);
            if grants.len() == original_len {
                return Err(ControlPlaneError::DeviceAccessGrantNotFound);
            }
            if grants.is_empty() {
                inner.device_access_grants.remove(device_id);
            }
        }

        self.persist()
            .map_err(|_| ControlPlaneError::PersistenceFailed)
    }

    pub fn remove_device_for_user(
        &self,
        user_id: &UserId,
        device_id: &DeviceId,
    ) -> Result<(), ControlPlaneError> {
        let mut inner = self.store.write();
        if !inner
            .devices
            .get(device_id)
            .map(|device| &device.user_id == user_id)
            .unwrap_or(false)
        {
            return Err(ControlPlaneError::DeviceNotFound);
        }

        inner.devices.remove(device_id);
        inner.device_access_grants.remove(device_id);
        inner.services.remove(device_id);
        inner.p2p_certificates.remove(device_id);
        inner.agent_sessions.remove(device_id);
        drop(inner);
        self.persist()
            .map_err(|_| ControlPlaneError::PersistenceFailed)
    }

    pub fn services_for_device(&self, device_id: &DeviceId) -> Vec<Service> {
        self.services_for_device_for_user(&self.default_user_id(), device_id)
    }

    pub fn services_for_device_for_user(
        &self,
        user_id: &UserId,
        device_id: &DeviceId,
    ) -> Vec<Service> {
        let inner = self.store.read();
        if !user_can_access_device_locked(&inner, user_id, device_id) {
            if user_id == &self.default_user_id() && !inner.devices.contains_key(device_id) {
                return inner.services.get(device_id).cloned().unwrap_or_default();
            }
            return Vec::new();
        }
        inner.services.get(device_id).cloned().unwrap_or_default()
    }

    fn controller_exists(&self, user_id: &UserId, client_id: &ClientId) -> bool {
        self.store
            .read()
            .controllers
            .get(user_id)
            .map(|controllers| {
                controllers
                    .iter()
                    .any(|controller| &controller.client_id == client_id)
            })
            .unwrap_or(false)
    }

    pub fn create_user_session(
        &self,
        user_id: &UserId,
        request: CreateSessionRequest,
    ) -> Result<CreateSessionResponse, ControlPlaneError> {
        let client_id = ClientId::new(request.client_id.clone());
        self.ensure_controller_for_session(user_id, &client_id)?;
        self.create_agent_session_for_user(
            user_id.clone(),
            client_id,
            request.device_id,
            request.service_id,
            None,
        )
    }

    fn create_agent_session_for_user(
        &self,
        user_id: UserId,
        client_id: ClientId,
        device_id: DeviceId,
        service_id: ServiceId,
        grant: Option<AgentSessionGrantMetadata>,
    ) -> Result<CreateSessionResponse, ControlPlaneError> {
        if !self.service_exists_for_user(&user_id, &device_id, &service_id) {
            return Err(ControlPlaneError::DeviceNotFound);
        }

        self.ensure_user_relay_quota_available(&user_id)?;
        let plan = self.plan_for_user(&user_id);
        let relay = self.select_relay()?;
        let session_id = SessionId::new(format!("sess_{}", uuid::Uuid::new_v4()));
        let expire_at = CONTROL_TOKEN_EXP;
        let issuer = TokenIssuer::new(self.token_secret());
        let access_token = issuer
            .issue_access_token(
                session_id.clone(),
                client_id.as_str().to_string(),
                expire_at,
            )
            .map_err(|_| ControlPlaneError::TokenIssueFailed)?;
        let relay_token = issuer
            .issue_relay_token(
                user_id.clone(),
                session_id.clone(),
                client_id.clone(),
                device_id.clone(),
                service_id.clone(),
                plan.relay_limits,
                expire_at,
            )
            .map_err(|_| ControlPlaneError::TokenIssueFailed)?;
        let response = CreateSessionResponse {
            session_id: session_id.clone(),
            access_token,
            relay_token: relay_token.clone(),
            relay_addr: relay.relay_addr,
            punch_addr: self.punch_addr().to_string(),
            agent_p2p_cert_der: self.p2p_certificate_for_device(&device_id),
            expire_at,
        };
        self.add_agent_session(AgentSessionAssignment {
            session_id,
            user_id,
            device_id,
            service_id,
            client_id,
            relay_token,
            relay_addr: response.relay_addr.clone(),
            punch_addr: response.punch_addr.clone(),
            expire_at,
            status: AgentSessionStatus::Pending,
            grant_id: grant.as_ref().map(|grant| grant.grant_id.clone()),
            grant_revocation_version: grant.as_ref().map(|grant| grant.revocation_version),
            grant_service_id: grant.map(|grant| grant.service_id),
        })
        .map_err(|_| ControlPlaneError::PersistenceFailed)?;

        Ok(response)
    }

    pub fn start_mobile_pairing(
        &self,
        request: MobilePairingRequest,
    ) -> Result<StartMobilePairingResponse, ControlPlaneError> {
        self.validate_pairing_request(&request)?;
        let now = current_epoch_sec();
        let pending_pairing_id = format!("mp_{}", uuid::Uuid::new_v4());
        let response = StartMobilePairingResponse {
            pending_pairing_id: pending_pairing_id.clone(),
            poll_interval_ms: MOBILE_GRANT_POLL_INTERVAL_MS,
            expires_at: now + MOBILE_GRANT_PENDING_TTL_SEC,
        };
        {
            self.store.write().pending_mobile_pairings.insert(
                pending_pairing_id.clone(),
                PendingMobilePairingRecord {
                    pending_pairing_id,
                    request,
                    expires_at: response.expires_at,
                    status: PendingPairingStatus::Pending,
                    grant: None,
                    denied_reason: None,
                },
            );
        }
        self.persist()
            .map_err(|_| ControlPlaneError::PersistenceFailed)?;
        Ok(response)
    }

    pub fn pending_mobile_pairings_for_device(
        &self,
        device_id: &DeviceId,
    ) -> Result<Vec<PendingMobilePairingRequest>, ControlPlaneError> {
        if !self.device_exists(device_id) {
            return Err(ControlPlaneError::DeviceNotFound);
        }
        let now = current_epoch_sec();
        let mut requests: Vec<_> = self
            .store
            .read()
            .pending_mobile_pairings
            .values()
            .filter(|record| {
                record.request.device_id == *device_id
                    && Self::effective_pairing_status(record, now) == PendingPairingStatus::Pending
            })
            .map(|record| PendingMobilePairingRequest {
                pending_pairing_id: record.pending_pairing_id.clone(),
                request: record.request.clone(),
                expires_at: record.expires_at,
                status: PendingPairingStatus::Pending,
            })
            .collect();
        requests.sort_by(|left, right| left.pending_pairing_id.cmp(&right.pending_pairing_id));
        Ok(requests)
    }

    pub fn mobile_pairing_result(
        &self,
        pending_pairing_id: &str,
    ) -> Result<MobilePairingPollResponse, ControlPlaneError> {
        let now = current_epoch_sec();
        let inner = self.store.read();
        let record = inner
            .pending_mobile_pairings
            .get(pending_pairing_id)
            .ok_or(ControlPlaneError::DeviceNotFound)?;
        Ok(Self::pairing_poll_response(record, now))
    }

    pub fn pending_mobile_pairing_device_id(&self, pending_pairing_id: &str) -> Option<DeviceId> {
        self.store
            .read()
            .pending_mobile_pairings
            .get(pending_pairing_id)
            .map(|record| record.request.device_id.clone())
    }

    pub fn approve_mobile_pairing(
        &self,
        pending_pairing_id: &str,
        request: ApproveMobilePairingRequest,
    ) -> Result<MobilePairingPollResponse, ControlPlaneError> {
        let now = current_epoch_sec();
        let response = {
            let mut inner = self.store.write();
            let record = inner
                .pending_mobile_pairings
                .get_mut(pending_pairing_id)
                .ok_or(ControlPlaneError::DeviceNotFound)?;
            if Self::effective_pairing_status(record, now) != PendingPairingStatus::Pending {
                return Err(ControlPlaneError::InvalidInput);
            }
            Self::validate_pairing_approval(record, &request)?;
            let grant = ApprovedMobileGrantMetadata {
                version: 1,
                device_id: record.request.device_id.clone(),
                grant_id: request.grant_id,
                client_id: record.request.client_id.clone(),
                allowed_services: request.allowed_services,
                revocation_version: request.revocation_version,
            };
            record.status = PendingPairingStatus::Approved;
            record.grant = Some(grant);
            Self::pairing_poll_response(record, now)
        };
        self.persist()
            .map_err(|_| ControlPlaneError::PersistenceFailed)?;
        Ok(response)
    }

    pub fn deny_mobile_pairing(
        &self,
        pending_pairing_id: &str,
        request: DenyMobileGrantRequest,
    ) -> Result<MobilePairingPollResponse, ControlPlaneError> {
        let now = current_epoch_sec();
        let response = {
            let mut inner = self.store.write();
            let record = inner
                .pending_mobile_pairings
                .get_mut(pending_pairing_id)
                .ok_or(ControlPlaneError::DeviceNotFound)?;
            if Self::effective_pairing_status(record, now) != PendingPairingStatus::Pending {
                return Err(ControlPlaneError::InvalidInput);
            }
            record.status = PendingPairingStatus::Denied;
            record.denied_reason = request.reason;
            Self::pairing_poll_response(record, now)
        };
        self.persist()
            .map_err(|_| ControlPlaneError::PersistenceFailed)?;
        Ok(response)
    }

    pub fn start_grant_session(
        &self,
        request: GrantSessionRequest,
    ) -> Result<StartGrantSessionResponse, ControlPlaneError> {
        self.validate_grant_session_request(&request)?;
        let now = current_epoch_sec();
        let pending_session_id = format!("gps_{}", uuid::Uuid::new_v4());
        let response = StartGrantSessionResponse {
            pending_session_id: pending_session_id.clone(),
            poll_interval_ms: MOBILE_GRANT_POLL_INTERVAL_MS,
            expires_at: now + MOBILE_GRANT_PENDING_TTL_SEC,
        };
        {
            self.store.write().pending_grant_sessions.insert(
                pending_session_id.clone(),
                PendingGrantSessionRecord {
                    pending_session_id,
                    request,
                    expires_at: response.expires_at,
                    status: PendingGrantSessionStatus::Pending,
                    session: None,
                    denied_reason: None,
                },
            );
        }
        self.persist()
            .map_err(|_| ControlPlaneError::PersistenceFailed)?;
        Ok(response)
    }

    pub fn pending_grant_sessions_for_device(
        &self,
        device_id: &DeviceId,
    ) -> Result<Vec<PendingGrantSessionRequest>, ControlPlaneError> {
        if !self.device_exists(device_id) {
            return Err(ControlPlaneError::DeviceNotFound);
        }
        let now = current_epoch_sec();
        let mut requests: Vec<_> = self
            .store
            .read()
            .pending_grant_sessions
            .values()
            .filter(|record| {
                record.request.device_id == *device_id
                    && Self::effective_grant_session_status(record, now)
                        == PendingGrantSessionStatus::Pending
            })
            .map(|record| PendingGrantSessionRequest {
                pending_session_id: record.pending_session_id.clone(),
                request: record.request.clone(),
                expires_at: record.expires_at,
                status: PendingGrantSessionStatus::Pending,
            })
            .collect();
        requests.sort_by(|left, right| left.pending_session_id.cmp(&right.pending_session_id));
        Ok(requests)
    }

    pub fn grant_session_result(
        &self,
        pending_session_id: &str,
    ) -> Result<GrantSessionPollResponse, ControlPlaneError> {
        let now = current_epoch_sec();
        let inner = self.store.read();
        let record = inner
            .pending_grant_sessions
            .get(pending_session_id)
            .ok_or(ControlPlaneError::DeviceNotFound)?;
        Ok(Self::grant_session_poll_response(record, now))
    }

    pub fn pending_grant_session_device_id(&self, pending_session_id: &str) -> Option<DeviceId> {
        self.store
            .read()
            .pending_grant_sessions
            .get(pending_session_id)
            .map(|record| record.request.device_id.clone())
    }

    pub fn approve_grant_session(
        &self,
        pending_session_id: &str,
    ) -> Result<GrantSessionPollResponse, ControlPlaneError> {
        let now = current_epoch_sec();
        let request = {
            let inner = self.store.read();
            let record = inner
                .pending_grant_sessions
                .get(pending_session_id)
                .ok_or(ControlPlaneError::DeviceNotFound)?;
            if Self::effective_grant_session_status(record, now)
                != PendingGrantSessionStatus::Pending
            {
                return Err(ControlPlaneError::InvalidInput);
            }
            record.request.clone()
        };
        self.validate_grant_session_request(&request)?;
        let user_id = self
            .store
            .read()
            .devices
            .get(&request.device_id)
            .map(|device| device.user_id.clone())
            .ok_or(ControlPlaneError::DeviceNotFound)?;
        let session = self.create_agent_session_for_user(
            user_id,
            request.client_id.clone(),
            request.device_id.clone(),
            request.service_id.clone(),
            Some(AgentSessionGrantMetadata {
                grant_id: request.grant_id.clone(),
                revocation_version: request.revocation_version,
                service_id: request.service_id.clone(),
            }),
        )?;
        let response = {
            let mut inner = self.store.write();
            let record = inner
                .pending_grant_sessions
                .get_mut(pending_session_id)
                .ok_or(ControlPlaneError::DeviceNotFound)?;
            if Self::effective_grant_session_status(record, now)
                != PendingGrantSessionStatus::Pending
            {
                return Err(ControlPlaneError::InvalidInput);
            }
            record.status = PendingGrantSessionStatus::Approved;
            record.session = Some(session);
            Self::grant_session_poll_response(record, now)
        };
        self.persist()
            .map_err(|_| ControlPlaneError::PersistenceFailed)?;
        Ok(response)
    }

    pub fn deny_grant_session(
        &self,
        pending_session_id: &str,
        request: DenyMobileGrantRequest,
    ) -> Result<GrantSessionPollResponse, ControlPlaneError> {
        let now = current_epoch_sec();
        let response = {
            let mut inner = self.store.write();
            let record = inner
                .pending_grant_sessions
                .get_mut(pending_session_id)
                .ok_or(ControlPlaneError::DeviceNotFound)?;
            if Self::effective_grant_session_status(record, now)
                != PendingGrantSessionStatus::Pending
            {
                return Err(ControlPlaneError::InvalidInput);
            }
            record.status = PendingGrantSessionStatus::Denied;
            record.denied_reason = request.reason;
            Self::grant_session_poll_response(record, now)
        };
        self.persist()
            .map_err(|_| ControlPlaneError::PersistenceFailed)?;
        Ok(response)
    }

    fn validate_pairing_request(
        &self,
        request: &MobilePairingRequest,
    ) -> Result<(), ControlPlaneError> {
        if request.invite_id.trim().is_empty()
            || request.client_id.as_str().trim().is_empty()
            || request.nonce.trim().is_empty()
            || request.proof.trim().is_empty()
            || request.requested_services.is_empty()
            || request
                .requested_services
                .iter()
                .any(|service_id| service_id.as_str().trim().is_empty())
        {
            return Err(ControlPlaneError::InvalidInput);
        }
        if !self.device_exists(&request.device_id) {
            return Err(ControlPlaneError::DeviceNotFound);
        }
        Ok(())
    }

    fn validate_grant_session_request(
        &self,
        request: &GrantSessionRequest,
    ) -> Result<(), ControlPlaneError> {
        if request.client_id.as_str().trim().is_empty()
            || request.grant_id.trim().is_empty()
            || request.nonce.trim().is_empty()
            || request.proof.trim().is_empty()
            || request.service_id.as_str().trim().is_empty()
        {
            return Err(ControlPlaneError::InvalidInput);
        }
        self.validate_device_services(
            &request.device_id,
            std::slice::from_ref(&request.service_id),
        )
    }

    fn validate_device_services(
        &self,
        device_id: &DeviceId,
        service_ids: &[ServiceId],
    ) -> Result<(), ControlPlaneError> {
        let inner = self.store.read();
        if !inner.devices.contains_key(device_id) {
            return Err(ControlPlaneError::DeviceNotFound);
        }
        let services = inner.services.get(device_id).cloned().unwrap_or_default();
        if service_ids.iter().any(|service_id| {
            service_id.as_str().trim().is_empty()
                || !services
                    .iter()
                    .any(|service| service.service_id == *service_id)
        }) {
            return Err(ControlPlaneError::DeviceNotFound);
        }
        Ok(())
    }

    fn validate_pairing_approval(
        record: &PendingMobilePairingRecord,
        request: &ApproveMobilePairingRequest,
    ) -> Result<(), ControlPlaneError> {
        if request.grant_id.trim().is_empty()
            || request.allowed_services.is_empty()
            || request.revocation_version == 0
        {
            return Err(ControlPlaneError::InvalidInput);
        }
        if request.allowed_services.iter().any(|service_id| {
            service_id.as_str().trim().is_empty()
                || !record
                    .request
                    .requested_services
                    .iter()
                    .any(|requested| requested == service_id)
        }) {
            return Err(ControlPlaneError::InvalidInput);
        }
        Ok(())
    }

    fn effective_pairing_status(
        record: &PendingMobilePairingRecord,
        now: u64,
    ) -> PendingPairingStatus {
        if record.status == PendingPairingStatus::Pending && record.expires_at <= now {
            PendingPairingStatus::Expired
        } else {
            record.status.clone()
        }
    }

    fn pairing_poll_response(
        record: &PendingMobilePairingRecord,
        now: u64,
    ) -> MobilePairingPollResponse {
        MobilePairingPollResponse {
            pending_pairing_id: record.pending_pairing_id.clone(),
            status: Self::effective_pairing_status(record, now),
            grant: record.grant.clone(),
            denied_reason: record.denied_reason.clone(),
        }
    }

    fn effective_grant_session_status(
        record: &PendingGrantSessionRecord,
        now: u64,
    ) -> PendingGrantSessionStatus {
        if record.status == PendingGrantSessionStatus::Pending && record.expires_at <= now {
            PendingGrantSessionStatus::Expired
        } else {
            record.status.clone()
        }
    }

    fn grant_session_poll_response(
        record: &PendingGrantSessionRecord,
        now: u64,
    ) -> GrantSessionPollResponse {
        GrantSessionPollResponse {
            pending_session_id: record.pending_session_id.clone(),
            status: Self::effective_grant_session_status(record, now),
            session: record.session.clone(),
            denied_reason: record.denied_reason.clone(),
        }
    }

    pub fn add_agent_session(
        &self,
        assignment: AgentSessionAssignment,
    ) -> Result<(), ControlPersistenceError> {
        {
            self.store
                .write()
                .agent_sessions
                .entry(assignment.device_id.clone())
                .or_default()
                .push(assignment);
        }
        self.persist()
    }

    pub fn agent_sessions_for_device(&self, device_id: &DeviceId) -> Vec<AgentSessionAssignment> {
        self.store
            .read()
            .agent_sessions
            .get(device_id)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter(|assignment| assignment.status == AgentSessionStatus::Pending)
            .collect()
    }

    pub fn device_belongs_to_user(&self, user_id: &UserId, device_id: &DeviceId) -> bool {
        self.store
            .read()
            .devices
            .get(device_id)
            .map(|device| &device.user_id == user_id)
            .unwrap_or(false)
    }

    pub fn device_exists(&self, device_id: &DeviceId) -> bool {
        self.store.read().devices.contains_key(device_id)
    }

    pub fn user_can_access_device(&self, user_id: &UserId, device_id: &DeviceId) -> bool {
        user_can_access_device_locked(&self.store.read(), user_id, device_id)
    }

    pub fn agent_session_device_id(&self, session_id: &SessionId) -> Option<DeviceId> {
        self.store
            .read()
            .agent_sessions
            .iter()
            .find_map(|(device_id, assignments)| {
                assignments
                    .iter()
                    .any(|assignment| &assignment.session_id == session_id)
                    .then(|| device_id.clone())
            })
    }

    pub fn agent_session_user_id(&self, session_id: &SessionId) -> Option<UserId> {
        let inner = self.store.read();
        inner
            .agent_sessions
            .iter()
            .find_map(|(device_id, assignments)| {
                let device = inner.devices.get(device_id)?;
                assignments
                    .iter()
                    .find(|assignment| &assignment.session_id == session_id)
                    .map(|assignment| session_user_id_locked(device, assignment))
            })
    }

    pub fn claim_agent_session(
        &self,
        session_id: &SessionId,
    ) -> Result<AgentSessionAssignment, ControlSessionError> {
        self.update_agent_session_status(
            session_id,
            &[AgentSessionStatus::Pending],
            AgentSessionStatus::Claimed,
        )
    }

    pub fn mark_agent_session_bound(
        &self,
        session_id: &SessionId,
    ) -> Result<AgentSessionAssignment, ControlSessionError> {
        self.update_agent_session_status(
            session_id,
            &[AgentSessionStatus::Claimed],
            AgentSessionStatus::Bound,
        )
    }

    pub fn close_session(
        &self,
        session_id: &SessionId,
    ) -> Result<AgentSessionAssignment, ControlSessionError> {
        self.update_agent_session_status(
            session_id,
            &[
                AgentSessionStatus::Pending,
                AgentSessionStatus::Claimed,
                AgentSessionStatus::Bound,
            ],
            AgentSessionStatus::Closed,
        )
    }

    fn update_agent_session_status(
        &self,
        session_id: &SessionId,
        allowed: &[AgentSessionStatus],
        next: AgentSessionStatus,
    ) -> Result<AgentSessionAssignment, ControlSessionError> {
        let updated = {
            let mut inner = self.store.write();
            let mut updated = None;
            for assignments in inner.agent_sessions.values_mut() {
                if let Some(assignment) = assignments
                    .iter_mut()
                    .find(|assignment| &assignment.session_id == session_id)
                {
                    if !allowed.contains(&assignment.status) {
                        return Err(ControlSessionError::InvalidTransition {
                            session_id: session_id.clone(),
                            status: assignment.status,
                        });
                    }

                    assignment.status = next;
                    updated = Some(assignment.clone());
                    break;
                }
            }
            updated
        };

        let Some(updated) = updated else {
            return Err(ControlSessionError::NotFound {
                session_id: session_id.clone(),
            });
        };
        self.persist()
            .map_err(|_| ControlSessionError::PersistenceFailed)?;
        Ok(updated)
    }

    pub fn service_exists(&self, device_id: &DeviceId, service_id: &ServiceId) -> bool {
        self.service_exists_for_user(&self.default_user_id(), device_id, service_id)
    }

    pub fn service_exists_for_user(
        &self,
        user_id: &UserId,
        device_id: &DeviceId,
        service_id: &ServiceId,
    ) -> bool {
        self.services_for_device_for_user(user_id, device_id)
            .iter()
            .any(|service| &service.service_id == service_id)
    }

    fn auth_response(
        &self,
        user_id: UserId,
        subject: String,
        role: ControlRole,
    ) -> Result<AuthResponse, ControlAuthError> {
        let issuer = TokenIssuer::new(self.token_secret());
        let access_token = issuer
            .issue_control_token(user_id.clone(), subject, role, CONTROL_TOKEN_EXP)
            .map_err(|_| ControlAuthError::TokenIssueFailed)?;
        Ok(AuthResponse {
            user_id,
            access_token,
            expire_at: CONTROL_TOKEN_EXP,
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ControlSessionError {
    #[error("session not found: {session_id}")]
    NotFound { session_id: SessionId },
    #[error("invalid session transition for {session_id} from {status:?}")]
    InvalidTransition {
        session_id: SessionId,
        status: AgentSessionStatus,
    },
    #[error("control state persistence failed")]
    PersistenceFailed,
}

#[derive(Debug, thiserror::Error)]
pub enum ControlAuthError {
    #[error("email is already registered")]
    EmailAlreadyRegistered,
    #[error("invalid credentials")]
    InvalidCredentials,
    #[error("invalid token")]
    InvalidToken,
    #[error("token issue failed")]
    TokenIssueFailed,
    #[error("invalid auth input")]
    InvalidInput,
    #[error("oauth provider is not configured")]
    OAuthNotConfigured,
    #[error("invalid oauth state")]
    OAuthInvalidState,
    #[error("oauth provider failed")]
    OAuthProviderFailed,
    #[error("oauth verified email unavailable")]
    OAuthEmailUnavailable,
    #[error("control state persistence failed")]
    PersistenceFailed,
}

#[derive(Debug, thiserror::Error)]
pub enum ControlPlaneError {
    #[error("controller device limit exceeded")]
    ControllerLimitExceeded,
    #[error("relay traffic quota exceeded")]
    RelayTrafficQuotaExceeded,
    #[error("controller device not found")]
    ControllerNotFound,
    #[error("user not found")]
    UserNotFound,
    #[error("email is already registered")]
    EmailAlreadyRegistered,
    #[error("relay not found")]
    RelayNotFound,
    #[error("relay credential already exists")]
    RelayCredentialAlreadyExists,
    #[error("relay credential not found")]
    RelayCredentialNotFound,
    #[error("relay bootstrap not found")]
    RelayBootstrapNotFound,
    #[error("relay bootstrap token is invalid, expired, or consumed")]
    RelayBootstrapUnauthorized,
    #[error("relay command not found")]
    RelayCommandNotFound,
    #[error("plan not found")]
    PlanNotFound,
    #[error("device not found")]
    DeviceNotFound,
    #[error("device access grant not found")]
    DeviceAccessGrantNotFound,
    #[error("no relay available")]
    NoRelayAvailable,
    #[error("server auth session not found")]
    ServerAuthSessionNotFound,
    #[error("server auth session is not ready")]
    ServerAuthSessionNotReady,
    #[error("server auth code is invalid")]
    ServerAuthInvalidCode,
    #[error("server credential not found")]
    ServerCredentialNotFound,
    #[error("oauth identity not found")]
    OAuthIdentityNotFound,
    #[error("oauth identity is the last login method")]
    OAuthIdentityLastLoginMethod,
    #[error("token issue failed")]
    TokenIssueFailed,
    #[error("invalid input")]
    InvalidInput,
    #[error("control state persistence failed")]
    PersistenceFailed,
}

#[derive(Debug, thiserror::Error)]
pub enum ControlPersistenceError {
    #[error("control state persistence failed: {message}")]
    Store { message: String },
}

impl From<SqliteStoreError> for ControlPersistenceError {
    fn from(error: SqliteStoreError) -> Self {
        Self::Store {
            message: error.to_string(),
        }
    }
}

fn apply_legacy_relay_health(relay: &mut RelayNode, healthy: bool, now_epoch_sec: u64) {
    relay.healthy = healthy;
    relay.health_status = if healthy {
        RelayHealthStatus::Healthy
    } else {
        RelayHealthStatus::Unhealthy
    };
    relay.health_reason = if healthy {
        String::new()
    } else {
        "manual_unhealthy".to_string()
    };
    relay.relay_version.clear();
    relay.uptime_sec = 0;
    relay.active_sessions = 0;
    relay.active_streams = 0;
    relay.total_uplink_bytes = 0;
    relay.total_downlink_bytes = 0;
    relay.total_bytes = 0;
    relay.data_plane_bound = healthy;
    relay.admin_bound = !relay.admin_addr.is_empty();
    relay.last_seen_epoch_sec = now_epoch_sec;
    relay.last_health_report_epoch_sec = now_epoch_sec;
}

fn apply_relay_health_report(relay: &mut RelayNode, report: RelayHealthReport, now_epoch_sec: u64) {
    relay.healthy = report.status == RelayHealthStatus::Healthy;
    relay.health_status = report.status;
    relay.health_reason = report.reason;
    relay.relay_version = report.relay_version;
    relay.uptime_sec = report.uptime_sec;
    relay.active_sessions = report.active_sessions;
    relay.active_streams = report.active_streams;
    relay.total_uplink_bytes = report.total_uplink_bytes;
    relay.total_downlink_bytes = report.total_downlink_bytes;
    relay.total_bytes = report.total_bytes;
    relay.data_plane_bound = report.data_plane_bound;
    relay.admin_bound = report.admin_bound;
    relay.last_seen_epoch_sec = now_epoch_sec;
    relay.last_health_report_epoch_sec = now_epoch_sec;
}

fn default_plan() -> Plan {
    Plan {
        plan_id: "free".to_string(),
        name: "Free".to_string(),
        max_controller_devices: 2,
        relay_limits: RelayLimits {
            max_bps: 1_048_576,
            max_streams: 8,
            max_duration_sec: 3_600,
            traffic_quota_bytes: 104_857_600,
        },
    }
}

fn default_user_usage_period(user_id: &UserId) -> UserUsagePeriod {
    UserUsagePeriod {
        user_id: user_id.clone(),
        current_period_started_epoch_sec: 0,
    }
}

fn server_credential_summary(credential: &ServerCredential) -> ServerCredentialSummary {
    ServerCredentialSummary {
        credential_id: credential.credential_id.clone(),
        user_id: credential.user_id.clone(),
        device_id: credential.device_id.clone(),
        device_name: credential.device_name.clone(),
        enabled: credential.enabled,
        token_version: credential.token_version,
        created_epoch_sec: credential.created_epoch_sec,
        last_used_epoch_sec: credential.last_used_epoch_sec,
    }
}

fn validate_plan(plan: &Plan) -> Result<(), ControlPlaneError> {
    if plan.plan_id.trim().is_empty()
        || plan.name.trim().is_empty()
        || plan.max_controller_devices == 0
        || plan.relay_limits.max_streams == 0
        || plan.relay_limits.max_duration_sec == 0
        || plan.relay_limits.traffic_quota_bytes == 0
    {
        return Err(ControlPlaneError::InvalidInput);
    }
    Ok(())
}

fn validate_user_account_role(role: ControlRole) -> Result<(), ControlPlaneError> {
    match role {
        ControlRole::User | ControlRole::Admin => Ok(()),
        ControlRole::Relay | ControlRole::Agent => Err(ControlPlaneError::InvalidInput),
    }
}

fn validate_email_password(email: &str, password: &str) -> Result<(), ControlAuthError> {
    if normalized_email(email).is_empty() || password.len() < 8 {
        return Err(ControlAuthError::InvalidInput);
    }
    Ok(())
}

fn current_epoch_sec() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time is before unix epoch")
        .as_secs()
}

fn relay_bootstrap_install_command(
    control_url: &str,
    bootstrap_id: &str,
    bootstrap_token: &str,
    no_service: bool,
) -> String {
    let control_url = normalized_relay_control_url(control_url);
    let shell = if no_service { "sh" } else { "sudo sh" };
    let no_service_arg = if no_service { " --no-service" } else { "" };
    format!(
        "curl -fsSL {}/install-relayd.sh | {} -s -- --control-url {} --bootstrap-id {} --bootstrap-token {} --relayd-url {}/relayd{}",
        control_url,
        shell,
        control_url,
        bootstrap_id,
        bootstrap_token,
        control_url,
        no_service_arg
    )
}

fn normalized_relay_control_url(control_url: &str) -> String {
    let trimmed = control_url.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return String::new();
    }
    if trimmed.contains("://") {
        trimmed.to_string()
    } else {
        format!("http://{trimmed}")
    }
}

fn normalized_email(email: &str) -> String {
    email.trim().to_ascii_lowercase()
}

fn oauth_identity_key(provider: OAuthProvider, provider_user_id: &str) -> String {
    format!(
        "{}:{}",
        oauth_provider_key(provider),
        provider_user_id.trim()
    )
}

fn oauth_provider_key(provider: OAuthProvider) -> &'static str {
    match provider {
        OAuthProvider::GitHub => "github",
    }
}

fn normalized_user_code(user_code: &str) -> String {
    user_code.trim().to_ascii_uppercase()
}

fn new_device_user_code() -> String {
    let raw = uuid::Uuid::new_v4()
        .simple()
        .to_string()
        .to_ascii_uppercase();
    format!("{}-{}", &raw[..4], &raw[4..8])
}

fn oauth_error_to_auth_error(error: OAuthError) -> ControlAuthError {
    match error {
        OAuthError::ProviderUnavailable | OAuthError::ProviderRejected => {
            ControlAuthError::OAuthProviderFailed
        }
        OAuthError::VerifiedEmailUnavailable => ControlAuthError::OAuthEmailUnavailable,
    }
}

fn password_hash(email: &str, password: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(normalized_email(email).as_bytes());
    hasher.update(b":");
    hasher.update(password.as_bytes());
    let digest = hasher.finalize();
    digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_auth_clock_defaults_to_wall_time() {
        let state = ControlState::new(
            "dev-secret",
            "relay.example.com:4443",
            "punch.example.com:3478",
        );
        let first = state.server_auth_now_epoch_sec();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while current_epoch_sec() <= first {
            assert!(
                std::time::Instant::now() < deadline,
                "wall clock did not advance while testing server-auth clock"
            );
            std::thread::sleep(std::time::Duration::from_millis(20));
        }

        let second = state.server_auth_now_epoch_sec();

        assert!(
            second > first,
            "default server-auth clock should track wall time"
        );
    }

    #[test]
    fn server_auth_clock_override_stays_fixed_for_tests() {
        let state = ControlState::new(
            "dev-secret",
            "relay.example.com:4443",
            "punch.example.com:3478",
        )
        .with_server_auth_now_epoch_sec(10);

        assert_eq!(state.server_auth_now_epoch_sec(), 10);
        state.set_server_auth_now_epoch_sec(20);
        assert_eq!(state.server_auth_now_epoch_sec(), 20);
    }
}
