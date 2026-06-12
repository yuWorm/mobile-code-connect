use std::path::PathBuf;

use async_trait::async_trait;
pub use mobilecode_connect_auth::ControlRole;
pub use mobilecode_connect_control_client::{
    AdminListQuery, AdminSessionSummary, AuditLogEntry, ControllerDevice,
    CreateRelayBootstrapRequest, CreateRelayCredentialRequest, CreateUserRequest, DashboardSummary,
    DeviceAccessGrant, GrantDeviceAccessRequest, Page, Plan, RegisterRelayRequest,
    RelayBootstrapExchangeRequest, RelayBootstrapExchangeResponse, RelayBootstrapResponse,
    RelayCredential, RelayHealthReport, RelayHealthStatus, RelayNode, RelaySessionUsageReport,
    ReportRelayHealthRequest, ReportRelaySessionUsageRequest, ServerCredentialResponse,
    ServerCredentialSummary, UpdatePlanCatalogRequest, UpdateRelayCredentialStatusRequest,
    UpdateRelayRequest, UpdateServerCredentialStatusRequest, UpdateUserPlanRequest,
    UpdateUserRoleRequest, UpdateUserStatusRequest, UserDetail, UserSummary, UserUsagePeriod,
    UserUsageSummary,
};
use mobilecode_connect_control_client::{
    AssignUserPlanRequest, ControlClientError, HttpControlClient, HttpControlClientOptions,
};
use mobilecode_connect_protocol::{ClientId, Device, DeviceId, UserId};
use tokio::sync::{Mutex, MutexGuard};

use crate::{
    store::{FileTokenStore, MemoryTokenStore, TokenStore},
    SdkError,
};

#[async_trait]
pub trait AdminApi: Send {
    fn set_bearer_token(&mut self, bearer_token: String);

    async fn dashboard(&mut self) -> Result<DashboardSummary, ControlClientError>;

    async fn list_server_credentials(
        &mut self,
        query: Option<AdminListQuery>,
    ) -> Result<Page<ServerCredentialSummary>, ControlClientError>;

    async fn server_credential(
        &mut self,
        credential_id: &str,
    ) -> Result<ServerCredentialSummary, ControlClientError>;

    async fn update_server_credential_status(
        &mut self,
        credential_id: &str,
        request: UpdateServerCredentialStatusRequest,
    ) -> Result<ServerCredentialSummary, ControlClientError>;

    async fn rotate_server_credential(
        &mut self,
        credential_id: &str,
    ) -> Result<ServerCredentialResponse, ControlClientError>;

    async fn list_controllers(
        &mut self,
        query: Option<AdminListQuery>,
    ) -> Result<Page<ControllerDevice>, ControlClientError>;

    async fn remove_controller(&mut self, client_id: &ClientId) -> Result<(), ControlClientError>;

    async fn list_controlled_devices(
        &mut self,
        query: Option<AdminListQuery>,
    ) -> Result<Page<Device>, ControlClientError>;

    async fn controlled_device(
        &mut self,
        device_id: &DeviceId,
    ) -> Result<Device, ControlClientError>;

    async fn device_access_grants(
        &mut self,
        device_id: &DeviceId,
        query: Option<AdminListQuery>,
    ) -> Result<Page<DeviceAccessGrant>, ControlClientError>;

    async fn grant_device_access(
        &mut self,
        device_id: &DeviceId,
        request: GrantDeviceAccessRequest,
    ) -> Result<DeviceAccessGrant, ControlClientError>;

    async fn revoke_device_access(
        &mut self,
        device_id: &DeviceId,
        user_id: &UserId,
    ) -> Result<(), ControlClientError>;

    async fn remove_controlled_device(
        &mut self,
        device_id: &DeviceId,
    ) -> Result<(), ControlClientError>;

    async fn list_users(
        &mut self,
        query: Option<AdminListQuery>,
    ) -> Result<Page<UserSummary>, ControlClientError>;

    async fn create_user(
        &mut self,
        request: CreateUserRequest,
    ) -> Result<UserSummary, ControlClientError>;

    async fn user(&mut self, user_id: &UserId) -> Result<UserDetail, ControlClientError>;

    async fn update_user_status(
        &mut self,
        user_id: &UserId,
        request: UpdateUserStatusRequest,
    ) -> Result<UserSummary, ControlClientError>;

    async fn update_user_role(
        &mut self,
        user_id: &UserId,
        request: UpdateUserRoleRequest,
    ) -> Result<UserSummary, ControlClientError>;

    async fn audit_logs(
        &mut self,
        query: Option<AdminListQuery>,
    ) -> Result<Page<AuditLogEntry>, ControlClientError>;

    async fn user_usage_summaries(
        &mut self,
        query: Option<AdminListQuery>,
    ) -> Result<Page<UserUsageSummary>, ControlClientError>;

    async fn report_relay_session_usage(
        &mut self,
        request: ReportRelaySessionUsageRequest,
    ) -> Result<(), ControlClientError>;

    async fn reset_user_usage_period(
        &mut self,
        user_id: &UserId,
    ) -> Result<UserUsagePeriod, ControlClientError>;

    async fn current_plan(&mut self) -> Result<Plan, ControlClientError>;

    async fn plan_catalog(
        &mut self,
        query: Option<AdminListQuery>,
    ) -> Result<Page<Plan>, ControlClientError>;

    async fn update_catalog_plan(
        &mut self,
        request: UpdatePlanCatalogRequest,
    ) -> Result<Plan, ControlClientError>;

    async fn assign_user_plan(
        &mut self,
        user_id: &UserId,
        plan_id: String,
    ) -> Result<Plan, ControlClientError>;

    async fn update_user_plan(
        &mut self,
        user_id: &UserId,
        request: UpdateUserPlanRequest,
    ) -> Result<Plan, ControlClientError>;

    async fn list_relay_credentials(
        &mut self,
        query: Option<AdminListQuery>,
    ) -> Result<Page<RelayCredential>, ControlClientError>;

    async fn create_relay_credential(
        &mut self,
        request: CreateRelayCredentialRequest,
    ) -> Result<RelayCredential, ControlClientError>;

    async fn update_relay_credential_status(
        &mut self,
        relay_id: &str,
        request: UpdateRelayCredentialStatusRequest,
    ) -> Result<RelayCredential, ControlClientError>;

    async fn rotate_relay_credential(
        &mut self,
        relay_id: &str,
    ) -> Result<RelayCredential, ControlClientError>;

    async fn create_relay_bootstrap(
        &mut self,
        request: CreateRelayBootstrapRequest,
    ) -> Result<RelayBootstrapResponse, ControlClientError>;

    async fn register_relay(
        &mut self,
        request: RegisterRelayRequest,
    ) -> Result<RelayNode, ControlClientError>;

    async fn update_relay(
        &mut self,
        relay_id: &str,
        request: UpdateRelayRequest,
    ) -> Result<RelayNode, ControlClientError>;

    async fn report_relay_health(
        &mut self,
        relay_id: &str,
        request: ReportRelayHealthRequest,
    ) -> Result<RelayNode, ControlClientError>;

    async fn remove_relay(&mut self, relay_id: &str) -> Result<(), ControlClientError>;

    async fn list_relays(
        &mut self,
        query: Option<AdminListQuery>,
    ) -> Result<Page<RelayNode>, ControlClientError>;

    async fn admin_sessions(
        &mut self,
        query: Option<AdminListQuery>,
    ) -> Result<Page<AdminSessionSummary>, ControlClientError>;
}

#[async_trait]
impl AdminApi for HttpControlClient {
    fn set_bearer_token(&mut self, bearer_token: String) {
        HttpControlClient::set_bearer_token(self, bearer_token);
    }

    async fn dashboard(&mut self) -> Result<DashboardSummary, ControlClientError> {
        HttpControlClient::dashboard(self).await
    }

    async fn list_server_credentials(
        &mut self,
        query: Option<AdminListQuery>,
    ) -> Result<Page<ServerCredentialSummary>, ControlClientError> {
        match query {
            Some(query) => HttpControlClient::list_server_credentials_with_query(self, query).await,
            None => HttpControlClient::list_server_credentials(self).await,
        }
    }

    async fn server_credential(
        &mut self,
        credential_id: &str,
    ) -> Result<ServerCredentialSummary, ControlClientError> {
        HttpControlClient::server_credential(self, credential_id).await
    }

    async fn update_server_credential_status(
        &mut self,
        credential_id: &str,
        request: UpdateServerCredentialStatusRequest,
    ) -> Result<ServerCredentialSummary, ControlClientError> {
        HttpControlClient::update_server_credential_status(self, credential_id, request).await
    }

    async fn rotate_server_credential(
        &mut self,
        credential_id: &str,
    ) -> Result<ServerCredentialResponse, ControlClientError> {
        HttpControlClient::rotate_server_credential(self, credential_id).await
    }

    async fn list_controllers(
        &mut self,
        query: Option<AdminListQuery>,
    ) -> Result<Page<ControllerDevice>, ControlClientError> {
        match query {
            Some(query) => HttpControlClient::list_controllers_with_query(self, query).await,
            None => HttpControlClient::list_controllers(self).await,
        }
    }

    async fn remove_controller(&mut self, client_id: &ClientId) -> Result<(), ControlClientError> {
        HttpControlClient::remove_controller(self, client_id).await
    }

    async fn list_controlled_devices(
        &mut self,
        query: Option<AdminListQuery>,
    ) -> Result<Page<Device>, ControlClientError> {
        match query {
            Some(query) => HttpControlClient::list_controlled_devices_with_query(self, query).await,
            None => HttpControlClient::list_controlled_devices(self).await,
        }
    }

    async fn controlled_device(
        &mut self,
        device_id: &DeviceId,
    ) -> Result<Device, ControlClientError> {
        HttpControlClient::controlled_device(self, device_id).await
    }

    async fn device_access_grants(
        &mut self,
        device_id: &DeviceId,
        query: Option<AdminListQuery>,
    ) -> Result<Page<DeviceAccessGrant>, ControlClientError> {
        match query {
            Some(query) => {
                HttpControlClient::device_access_grants_with_query(self, device_id, query).await
            }
            None => HttpControlClient::device_access_grants(self, device_id).await,
        }
    }

    async fn grant_device_access(
        &mut self,
        device_id: &DeviceId,
        request: GrantDeviceAccessRequest,
    ) -> Result<DeviceAccessGrant, ControlClientError> {
        HttpControlClient::grant_device_access(self, device_id, request).await
    }

    async fn revoke_device_access(
        &mut self,
        device_id: &DeviceId,
        user_id: &UserId,
    ) -> Result<(), ControlClientError> {
        HttpControlClient::revoke_device_access(self, device_id, user_id).await
    }

    async fn remove_controlled_device(
        &mut self,
        device_id: &DeviceId,
    ) -> Result<(), ControlClientError> {
        HttpControlClient::remove_controlled_device(self, device_id).await
    }

    async fn list_users(
        &mut self,
        query: Option<AdminListQuery>,
    ) -> Result<Page<UserSummary>, ControlClientError> {
        match query {
            Some(query) => HttpControlClient::list_users_with_query(self, query).await,
            None => HttpControlClient::list_users(self).await,
        }
    }

    async fn create_user(
        &mut self,
        request: CreateUserRequest,
    ) -> Result<UserSummary, ControlClientError> {
        HttpControlClient::create_user(self, request).await
    }

    async fn user(&mut self, user_id: &UserId) -> Result<UserDetail, ControlClientError> {
        HttpControlClient::user(self, user_id).await
    }

    async fn update_user_status(
        &mut self,
        user_id: &UserId,
        request: UpdateUserStatusRequest,
    ) -> Result<UserSummary, ControlClientError> {
        HttpControlClient::update_user_status(self, user_id, request).await
    }

    async fn update_user_role(
        &mut self,
        user_id: &UserId,
        request: UpdateUserRoleRequest,
    ) -> Result<UserSummary, ControlClientError> {
        HttpControlClient::update_user_role(self, user_id, request).await
    }

    async fn audit_logs(
        &mut self,
        query: Option<AdminListQuery>,
    ) -> Result<Page<AuditLogEntry>, ControlClientError> {
        match query {
            Some(query) => HttpControlClient::audit_logs_with_query(self, query).await,
            None => HttpControlClient::audit_logs(self).await,
        }
    }

    async fn user_usage_summaries(
        &mut self,
        query: Option<AdminListQuery>,
    ) -> Result<Page<UserUsageSummary>, ControlClientError> {
        match query {
            Some(query) => HttpControlClient::user_usage_summaries_with_query(self, query).await,
            None => HttpControlClient::user_usage_summaries(self).await,
        }
    }

    async fn report_relay_session_usage(
        &mut self,
        request: ReportRelaySessionUsageRequest,
    ) -> Result<(), ControlClientError> {
        HttpControlClient::report_relay_session_usage(self, request).await
    }

    async fn reset_user_usage_period(
        &mut self,
        user_id: &UserId,
    ) -> Result<UserUsagePeriod, ControlClientError> {
        HttpControlClient::reset_user_usage_period(self, user_id).await
    }

    async fn current_plan(&mut self) -> Result<Plan, ControlClientError> {
        HttpControlClient::current_plan(self).await
    }

    async fn plan_catalog(
        &mut self,
        query: Option<AdminListQuery>,
    ) -> Result<Page<Plan>, ControlClientError> {
        match query {
            Some(query) => HttpControlClient::plan_catalog_with_query(self, query).await,
            None => HttpControlClient::plan_catalog(self).await,
        }
    }

    async fn update_catalog_plan(
        &mut self,
        request: UpdatePlanCatalogRequest,
    ) -> Result<Plan, ControlClientError> {
        HttpControlClient::update_catalog_plan(self, request).await
    }

    async fn assign_user_plan(
        &mut self,
        user_id: &UserId,
        plan_id: String,
    ) -> Result<Plan, ControlClientError> {
        HttpControlClient::assign_user_plan(self, user_id, AssignUserPlanRequest { plan_id }).await
    }

    async fn update_user_plan(
        &mut self,
        user_id: &UserId,
        request: UpdateUserPlanRequest,
    ) -> Result<Plan, ControlClientError> {
        HttpControlClient::update_user_plan(self, user_id, request).await
    }

    async fn list_relay_credentials(
        &mut self,
        query: Option<AdminListQuery>,
    ) -> Result<Page<RelayCredential>, ControlClientError> {
        match query {
            Some(query) => HttpControlClient::list_relay_credentials_with_query(self, query).await,
            None => HttpControlClient::list_relay_credentials(self).await,
        }
    }

    async fn create_relay_credential(
        &mut self,
        request: CreateRelayCredentialRequest,
    ) -> Result<RelayCredential, ControlClientError> {
        HttpControlClient::create_relay_credential(self, request).await
    }

    async fn update_relay_credential_status(
        &mut self,
        relay_id: &str,
        request: UpdateRelayCredentialStatusRequest,
    ) -> Result<RelayCredential, ControlClientError> {
        HttpControlClient::update_relay_credential_status(self, relay_id, request).await
    }

    async fn rotate_relay_credential(
        &mut self,
        relay_id: &str,
    ) -> Result<RelayCredential, ControlClientError> {
        HttpControlClient::rotate_relay_credential(self, relay_id).await
    }

    async fn create_relay_bootstrap(
        &mut self,
        request: CreateRelayBootstrapRequest,
    ) -> Result<RelayBootstrapResponse, ControlClientError> {
        HttpControlClient::create_relay_bootstrap(self, request).await
    }

    async fn register_relay(
        &mut self,
        request: RegisterRelayRequest,
    ) -> Result<RelayNode, ControlClientError> {
        HttpControlClient::register_relay(self, request).await
    }

    async fn update_relay(
        &mut self,
        relay_id: &str,
        request: UpdateRelayRequest,
    ) -> Result<RelayNode, ControlClientError> {
        HttpControlClient::update_relay(self, relay_id, request).await
    }

    async fn report_relay_health(
        &mut self,
        relay_id: &str,
        request: ReportRelayHealthRequest,
    ) -> Result<RelayNode, ControlClientError> {
        HttpControlClient::report_relay_health(self, relay_id, request).await
    }

    async fn remove_relay(&mut self, relay_id: &str) -> Result<(), ControlClientError> {
        HttpControlClient::remove_relay(self, relay_id).await
    }

    async fn list_relays(
        &mut self,
        query: Option<AdminListQuery>,
    ) -> Result<Page<RelayNode>, ControlClientError> {
        match query {
            Some(query) => HttpControlClient::list_relays_with_query(self, query).await,
            None => HttpControlClient::list_relays(self).await,
        }
    }

    async fn admin_sessions(
        &mut self,
        query: Option<AdminListQuery>,
    ) -> Result<Page<AdminSessionSummary>, ControlClientError> {
        match query {
            Some(query) => HttpControlClient::admin_sessions_with_query(self, query).await,
            None => HttpControlClient::admin_sessions(self).await,
        }
    }
}

pub struct AdminSdk<A, S> {
    api: Mutex<A>,
    token_store: S,
}

impl<A, S> AdminSdk<A, S>
where
    A: AdminApi,
    S: TokenStore,
{
    pub fn new(api: A, token_store: S) -> Self {
        Self {
            api: Mutex::new(api),
            token_store,
        }
    }

    pub async fn current_token(&self) -> Result<Option<crate::store::StoredToken>, SdkError> {
        Ok(self.token_store.load_token().await?)
    }

    pub async fn dashboard(&self) -> Result<DashboardSummary, SdkError> {
        let mut api = self.authorized_api().await?;
        Ok(api.dashboard().await?)
    }

    pub async fn list_server_credentials(
        &self,
        query: Option<AdminListQuery>,
    ) -> Result<Page<ServerCredentialSummary>, SdkError> {
        let mut api = self.authorized_api().await?;
        Ok(api.list_server_credentials(query).await?)
    }

    pub async fn server_credential(
        &self,
        credential_id: &str,
    ) -> Result<ServerCredentialSummary, SdkError> {
        let mut api = self.authorized_api().await?;
        Ok(api.server_credential(credential_id).await?)
    }

    pub async fn update_server_credential_status(
        &self,
        credential_id: &str,
        request: UpdateServerCredentialStatusRequest,
    ) -> Result<ServerCredentialSummary, SdkError> {
        let mut api = self.authorized_api().await?;
        Ok(api
            .update_server_credential_status(credential_id, request)
            .await?)
    }

    pub async fn rotate_server_credential(
        &self,
        credential_id: &str,
    ) -> Result<ServerCredentialResponse, SdkError> {
        let mut api = self.authorized_api().await?;
        Ok(api.rotate_server_credential(credential_id).await?)
    }

    pub async fn list_controllers(
        &self,
        query: Option<AdminListQuery>,
    ) -> Result<Page<ControllerDevice>, SdkError> {
        let mut api = self.authorized_api().await?;
        Ok(api.list_controllers(query).await?)
    }

    pub async fn remove_controller(&self, client_id: &ClientId) -> Result<(), SdkError> {
        let mut api = self.authorized_api().await?;
        api.remove_controller(client_id).await?;
        Ok(())
    }

    pub async fn list_controlled_devices(
        &self,
        query: Option<AdminListQuery>,
    ) -> Result<Page<Device>, SdkError> {
        let mut api = self.authorized_api().await?;
        Ok(api.list_controlled_devices(query).await?)
    }

    pub async fn controlled_device(&self, device_id: &DeviceId) -> Result<Device, SdkError> {
        let mut api = self.authorized_api().await?;
        Ok(api.controlled_device(device_id).await?)
    }

    pub async fn device_access_grants(
        &self,
        device_id: &DeviceId,
        query: Option<AdminListQuery>,
    ) -> Result<Page<DeviceAccessGrant>, SdkError> {
        let mut api = self.authorized_api().await?;
        Ok(api.device_access_grants(device_id, query).await?)
    }

    pub async fn grant_device_access(
        &self,
        device_id: &DeviceId,
        request: GrantDeviceAccessRequest,
    ) -> Result<DeviceAccessGrant, SdkError> {
        let mut api = self.authorized_api().await?;
        Ok(api.grant_device_access(device_id, request).await?)
    }

    pub async fn revoke_device_access(
        &self,
        device_id: &DeviceId,
        user_id: &UserId,
    ) -> Result<(), SdkError> {
        let mut api = self.authorized_api().await?;
        api.revoke_device_access(device_id, user_id).await?;
        Ok(())
    }

    pub async fn remove_controlled_device(&self, device_id: &DeviceId) -> Result<(), SdkError> {
        let mut api = self.authorized_api().await?;
        api.remove_controlled_device(device_id).await?;
        Ok(())
    }

    pub async fn list_users(
        &self,
        query: Option<AdminListQuery>,
    ) -> Result<Page<UserSummary>, SdkError> {
        let mut api = self.authorized_api().await?;
        Ok(api.list_users(query).await?)
    }

    pub async fn create_user(&self, request: CreateUserRequest) -> Result<UserSummary, SdkError> {
        let mut api = self.authorized_api().await?;
        Ok(api.create_user(request).await?)
    }

    pub async fn user(&self, user_id: &UserId) -> Result<UserDetail, SdkError> {
        let mut api = self.authorized_api().await?;
        Ok(api.user(user_id).await?)
    }

    pub async fn update_user_status(
        &self,
        user_id: &UserId,
        request: UpdateUserStatusRequest,
    ) -> Result<UserSummary, SdkError> {
        let mut api = self.authorized_api().await?;
        Ok(api.update_user_status(user_id, request).await?)
    }

    pub async fn update_user_role(
        &self,
        user_id: &UserId,
        request: UpdateUserRoleRequest,
    ) -> Result<UserSummary, SdkError> {
        let mut api = self.authorized_api().await?;
        Ok(api.update_user_role(user_id, request).await?)
    }

    pub async fn audit_logs(
        &self,
        query: Option<AdminListQuery>,
    ) -> Result<Page<AuditLogEntry>, SdkError> {
        let mut api = self.authorized_api().await?;
        Ok(api.audit_logs(query).await?)
    }

    pub async fn user_usage_summaries(
        &self,
        query: Option<AdminListQuery>,
    ) -> Result<Page<UserUsageSummary>, SdkError> {
        let mut api = self.authorized_api().await?;
        Ok(api.user_usage_summaries(query).await?)
    }

    pub async fn report_relay_session_usage(
        &self,
        request: ReportRelaySessionUsageRequest,
    ) -> Result<(), SdkError> {
        let mut api = self.authorized_api().await?;
        api.report_relay_session_usage(request).await?;
        Ok(())
    }

    pub async fn reset_user_usage_period(
        &self,
        user_id: &UserId,
    ) -> Result<UserUsagePeriod, SdkError> {
        let mut api = self.authorized_api().await?;
        Ok(api.reset_user_usage_period(user_id).await?)
    }

    pub async fn current_plan(&self) -> Result<Plan, SdkError> {
        let mut api = self.authorized_api().await?;
        Ok(api.current_plan().await?)
    }

    pub async fn plan_catalog(
        &self,
        query: Option<AdminListQuery>,
    ) -> Result<Page<Plan>, SdkError> {
        let mut api = self.authorized_api().await?;
        Ok(api.plan_catalog(query).await?)
    }

    pub async fn update_catalog_plan(
        &self,
        request: UpdatePlanCatalogRequest,
    ) -> Result<Plan, SdkError> {
        let mut api = self.authorized_api().await?;
        Ok(api.update_catalog_plan(request).await?)
    }

    pub async fn assign_user_plan(
        &self,
        user_id: &UserId,
        plan_id: impl Into<String>,
    ) -> Result<Plan, SdkError> {
        let mut api = self.authorized_api().await?;
        Ok(api.assign_user_plan(user_id, plan_id.into()).await?)
    }

    pub async fn update_user_plan(
        &self,
        user_id: &UserId,
        request: UpdateUserPlanRequest,
    ) -> Result<Plan, SdkError> {
        let mut api = self.authorized_api().await?;
        Ok(api.update_user_plan(user_id, request).await?)
    }

    pub async fn list_relay_credentials(
        &self,
        query: Option<AdminListQuery>,
    ) -> Result<Page<RelayCredential>, SdkError> {
        let mut api = self.authorized_api().await?;
        Ok(api.list_relay_credentials(query).await?)
    }

    pub async fn create_relay_credential(
        &self,
        request: CreateRelayCredentialRequest,
    ) -> Result<RelayCredential, SdkError> {
        let mut api = self.authorized_api().await?;
        Ok(api.create_relay_credential(request).await?)
    }

    pub async fn update_relay_credential_status(
        &self,
        relay_id: &str,
        request: UpdateRelayCredentialStatusRequest,
    ) -> Result<RelayCredential, SdkError> {
        let mut api = self.authorized_api().await?;
        Ok(api
            .update_relay_credential_status(relay_id, request)
            .await?)
    }

    pub async fn rotate_relay_credential(
        &self,
        relay_id: &str,
    ) -> Result<RelayCredential, SdkError> {
        let mut api = self.authorized_api().await?;
        Ok(api.rotate_relay_credential(relay_id).await?)
    }

    pub async fn create_relay_bootstrap(
        &self,
        request: CreateRelayBootstrapRequest,
    ) -> Result<RelayBootstrapResponse, SdkError> {
        let mut api = self.authorized_api().await?;
        Ok(api.create_relay_bootstrap(request).await?)
    }

    pub async fn register_relay(
        &self,
        request: RegisterRelayRequest,
    ) -> Result<RelayNode, SdkError> {
        let mut api = self.authorized_api().await?;
        Ok(api.register_relay(request).await?)
    }

    pub async fn update_relay(
        &self,
        relay_id: &str,
        request: UpdateRelayRequest,
    ) -> Result<RelayNode, SdkError> {
        let mut api = self.authorized_api().await?;
        Ok(api.update_relay(relay_id, request).await?)
    }

    pub async fn report_relay_health(
        &self,
        relay_id: &str,
        request: ReportRelayHealthRequest,
    ) -> Result<RelayNode, SdkError> {
        let mut api = self.authorized_api().await?;
        Ok(api.report_relay_health(relay_id, request).await?)
    }

    pub async fn remove_relay(&self, relay_id: &str) -> Result<(), SdkError> {
        let mut api = self.authorized_api().await?;
        api.remove_relay(relay_id).await?;
        Ok(())
    }

    pub async fn list_relays(
        &self,
        query: Option<AdminListQuery>,
    ) -> Result<Page<RelayNode>, SdkError> {
        let mut api = self.authorized_api().await?;
        Ok(api.list_relays(query).await?)
    }

    pub async fn admin_sessions(
        &self,
        query: Option<AdminListQuery>,
    ) -> Result<Page<AdminSessionSummary>, SdkError> {
        let mut api = self.authorized_api().await?;
        Ok(api.admin_sessions(query).await?)
    }

    async fn authorized_api(&self) -> Result<MutexGuard<'_, A>, SdkError> {
        let token = self
            .token_store
            .load_token()
            .await?
            .ok_or(SdkError::NotAuthenticated)?;
        let mut api = self.api.lock().await;
        api.set_bearer_token(token.access_token);
        Ok(api)
    }
}

impl<S> AdminSdk<HttpControlClient, S>
where
    S: TokenStore,
{
    pub fn with_http_client(base_url: impl AsRef<str>, token_store: S) -> Result<Self, SdkError> {
        Self::with_http_client_options(base_url, token_store, HttpControlClientOptions::default())
    }

    pub fn with_http_client_options(
        base_url: impl AsRef<str>,
        token_store: S,
        options: HttpControlClientOptions,
    ) -> Result<Self, SdkError> {
        Ok(Self::new(
            HttpControlClient::with_options(base_url, options)?,
            token_store,
        ))
    }
}

impl AdminSdk<HttpControlClient, MemoryTokenStore> {
    pub fn in_memory(base_url: impl AsRef<str>) -> Result<Self, SdkError> {
        Self::with_http_client(base_url, MemoryTokenStore::default())
    }
}

impl AdminSdk<HttpControlClient, FileTokenStore> {
    pub fn with_file_token_store(
        base_url: impl AsRef<str>,
        token_path: impl Into<PathBuf>,
    ) -> Result<Self, SdkError> {
        Self::with_http_client(base_url, FileTokenStore::new(token_path))
    }
}
