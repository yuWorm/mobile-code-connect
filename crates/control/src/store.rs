use std::{
    collections::HashMap,
    sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard},
};

use mobilecode_connect_auth::ControlRole;
use mobilecode_connect_control_client::{
    AgentSessionAssignment, ApprovedMobileGrantMetadata, AuditLogEntry, ControllerDevice,
    CreateSessionResponse, OAuthIdentity, Plan, RelayCommand, RelayCredential, RelayNode,
    RelaySessionSnapshot, ServerAuthMode, ServerAuthStatus, UserUsagePeriod,
};
use mobilecode_connect_protocol::{
    Device, DeviceId, GrantSessionRequest, MobilePairingRequest, PendingGrantSessionStatus,
    PendingPairingStatus, Service, SessionId, TrafficStats, UserId,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Default)]
pub(crate) struct InMemoryControlStore {
    inner: Arc<RwLock<ControlStore>>,
}

impl InMemoryControlStore {
    pub(crate) fn read(&self) -> RwLockReadGuard<'_, ControlStore> {
        self.inner.read().expect("control state lock poisoned")
    }

    pub(crate) fn write(&self) -> RwLockWriteGuard<'_, ControlStore> {
        self.inner.write().expect("control state lock poisoned")
    }

    pub(crate) fn snapshot(&self) -> ControlStore {
        self.read().clone()
    }

    pub(crate) fn replace(&self, store: ControlStore) {
        *self.write() = store;
    }
}

#[derive(Clone, Default, Deserialize, Serialize)]
pub(crate) struct ControlStore {
    #[serde(default)]
    pub(crate) audit_logs: Vec<AuditLogEntry>,
    pub(crate) users: HashMap<UserId, UserAccount>,
    pub(crate) user_ids_by_email: HashMap<String, UserId>,
    #[serde(default)]
    pub(crate) plan_catalog: HashMap<String, Plan>,
    pub(crate) plans: HashMap<UserId, Plan>,
    pub(crate) controllers: HashMap<UserId, Vec<ControllerDevice>>,
    pub(crate) devices: HashMap<DeviceId, Device>,
    #[serde(default)]
    pub(crate) device_access_grants: HashMap<DeviceId, Vec<UserId>>,
    pub(crate) services: HashMap<DeviceId, Vec<Service>>,
    pub(crate) agent_sessions: HashMap<DeviceId, Vec<AgentSessionAssignment>>,
    #[serde(default)]
    pub(crate) pending_mobile_pairings: HashMap<String, PendingMobilePairingRecord>,
    #[serde(default)]
    pub(crate) pending_grant_sessions: HashMap<String, PendingGrantSessionRecord>,
    pub(crate) p2p_certificates: HashMap<DeviceId, Vec<u8>>,
    #[serde(default)]
    pub(crate) relay_credentials: HashMap<String, RelayCredential>,
    #[serde(default)]
    pub(crate) relay_session_usage: HashMap<SessionId, RelaySessionUsageRecord>,
    #[serde(default)]
    pub(crate) relay_session_snapshots: HashMap<String, HashMap<SessionId, RelaySessionSnapshot>>,
    #[serde(default)]
    pub(crate) relay_commands: HashMap<String, RelayCommand>,
    #[serde(default)]
    pub(crate) relay_bootstraps: HashMap<String, RelayBootstrapRecord>,
    #[serde(default)]
    pub(crate) user_usage_periods: HashMap<UserId, UserUsagePeriod>,
    #[serde(default)]
    pub(crate) oauth_identities: HashMap<String, OAuthIdentity>,
    #[serde(default)]
    pub(crate) oauth_login_sessions: HashMap<String, OAuthLoginSession>,
    #[serde(default)]
    pub(crate) server_auth_sessions: HashMap<String, ServerAuthSession>,
    #[serde(default)]
    pub(crate) server_credentials: HashMap<String, ServerCredential>,
    pub(crate) relays: HashMap<String, RelayNode>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub(crate) struct PendingMobilePairingRecord {
    pub(crate) pending_pairing_id: String,
    pub(crate) request: MobilePairingRequest,
    pub(crate) expires_at: u64,
    pub(crate) status: PendingPairingStatus,
    #[serde(default)]
    pub(crate) grant: Option<ApprovedMobileGrantMetadata>,
    #[serde(default)]
    pub(crate) denied_reason: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub(crate) struct PendingGrantSessionRecord {
    pub(crate) pending_session_id: String,
    pub(crate) request: GrantSessionRequest,
    pub(crate) expires_at: u64,
    pub(crate) status: PendingGrantSessionStatus,
    #[serde(default)]
    pub(crate) session: Option<CreateSessionResponse>,
    #[serde(default)]
    pub(crate) denied_reason: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub(crate) struct RelaySessionUsageRecord {
    pub(crate) relay_id: String,
    pub(crate) stats: TrafficStats,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub(crate) struct RelayBootstrapRecord {
    pub(crate) bootstrap_id: String,
    pub(crate) control_url: String,
    pub(crate) relay_id: String,
    pub(crate) relay_addr: String,
    #[serde(default)]
    pub(crate) admin_addr: String,
    pub(crate) capacity_streams: u32,
    pub(crate) heartbeat_interval_sec: u64,
    pub(crate) token_secret: String,
    pub(crate) token_hash: String,
    pub(crate) created_epoch_sec: u64,
    pub(crate) expires_epoch_sec: u64,
    #[serde(default)]
    pub(crate) consumed_epoch_sec: Option<u64>,
    pub(crate) created_by: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub(crate) struct OAuthLoginSession {
    pub(crate) session_id: String,
    pub(crate) provider: String,
    pub(crate) state_hash: String,
    pub(crate) pkce_verifier: String,
    pub(crate) redirect_uri: Option<String>,
    pub(crate) expires_epoch_sec: u64,
    pub(crate) created_epoch_sec: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub(crate) struct ServerAuthSession {
    pub(crate) session_id: String,
    pub(crate) mode: ServerAuthMode,
    pub(crate) status: ServerAuthStatus,
    pub(crate) device_id: DeviceId,
    pub(crate) device_name: String,
    pub(crate) server_public_key: String,
    pub(crate) user_code_hash: Option<String>,
    pub(crate) device_code_hash: Option<String>,
    pub(crate) auth_code_hash: Option<String>,
    pub(crate) approved_user_id: Option<UserId>,
    pub(crate) poll_interval_sec: u64,
    pub(crate) expires_epoch_sec: u64,
    pub(crate) created_epoch_sec: u64,
    pub(crate) updated_epoch_sec: u64,
    #[serde(default)]
    pub(crate) last_poll_epoch_sec: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub(crate) struct ServerCredential {
    pub(crate) credential_id: String,
    pub(crate) user_id: UserId,
    pub(crate) device_id: DeviceId,
    pub(crate) device_name: String,
    pub(crate) server_public_key: String,
    pub(crate) enabled: bool,
    pub(crate) token_version: u64,
    pub(crate) created_epoch_sec: u64,
    pub(crate) last_used_epoch_sec: Option<u64>,
}

#[derive(Clone, Deserialize, Serialize)]
pub(crate) struct UserAccount {
    pub(crate) user_id: UserId,
    #[serde(default)]
    pub(crate) email: String,
    #[serde(default)]
    pub(crate) display_name: String,
    pub(crate) password_hash: String,
    #[serde(default = "default_user_role")]
    pub(crate) role: ControlRole,
    #[serde(default = "default_enabled")]
    pub(crate) enabled: bool,
}

fn default_user_role() -> ControlRole {
    ControlRole::User
}

fn default_enabled() -> bool {
    true
}
