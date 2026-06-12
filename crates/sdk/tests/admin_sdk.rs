use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use quic_tunnel_auth::ControlRole;
use quic_tunnel_protocol::{
    ClientId, Device, DeviceId, DeviceStatus, RelayLimits, ServiceId, SessionId, TrafficStats,
    UserId,
};
use quic_tunnel_sdk::{
    admin::{
        AdminApi, AdminListQuery, AdminSdk, AdminSessionSummary, AuditLogEntry, ControllerDevice,
        CreateRelayBootstrapRequest, CreateRelayCredentialRequest, CreateUserRequest,
        DashboardSummary, DeviceAccessGrant, GrantDeviceAccessRequest, Page, Plan,
        RegisterRelayRequest, RelayBootstrapResponse, RelayCredential, RelayHealthReport,
        RelayHealthStatus, RelayNode, RelaySessionUsageReport, ReportRelayHealthRequest,
        ReportRelaySessionUsageRequest, ServerCredentialResponse, ServerCredentialSummary,
        UpdatePlanCatalogRequest, UpdateRelayCredentialStatusRequest, UpdateRelayRequest,
        UpdateServerCredentialStatusRequest, UpdateUserPlanRequest, UpdateUserRoleRequest,
        UpdateUserStatusRequest, UserDetail, UserSummary, UserUsagePeriod, UserUsageSummary,
    },
    store::{MemoryTokenStore, StoredToken, TokenStore},
    SdkError,
};

#[derive(Clone)]
struct FakeAdminApi {
    state: Arc<Mutex<FakeAdminState>>,
}

#[derive(Debug, Default)]
struct FakeAdminState {
    bearer_tokens: Vec<String>,
    list_users_queries: Vec<Option<AdminListQuery>>,
    audit_queries: Vec<Option<AdminListQuery>>,
    usage_queries: Vec<Option<AdminListQuery>>,
    session_queries: Vec<Option<AdminListQuery>>,
    plan_queries: Vec<Option<AdminListQuery>>,
    relay_credential_queries: Vec<Option<AdminListQuery>>,
    relay_queries: Vec<Option<AdminListQuery>>,
    created_users: Vec<CreateUserRequest>,
    fetched_users: Vec<UserId>,
    status_updates: Vec<(UserId, UpdateUserStatusRequest)>,
    role_updates: Vec<(UserId, UpdateUserRoleRequest)>,
    reset_usage_users: Vec<UserId>,
    relay_usage_reports: Vec<ReportRelaySessionUsageRequest>,
    catalog_updates: Vec<UpdatePlanCatalogRequest>,
    assigned_plans: Vec<(UserId, String)>,
    user_plan_updates: Vec<(UserId, UpdateUserPlanRequest)>,
    created_relay_credentials: Vec<CreateRelayCredentialRequest>,
    relay_credential_status_updates: Vec<(String, UpdateRelayCredentialStatusRequest)>,
    rotated_relay_credentials: Vec<String>,
    created_relay_bootstraps: Vec<CreateRelayBootstrapRequest>,
    registered_relays: Vec<RegisterRelayRequest>,
    relay_updates: Vec<(String, UpdateRelayRequest)>,
    relay_health_reports: Vec<(String, ReportRelayHealthRequest)>,
    removed_relays: Vec<String>,
    server_credential_queries: Vec<Option<AdminListQuery>>,
    fetched_server_credentials: Vec<String>,
    server_credential_status_updates: Vec<(String, UpdateServerCredentialStatusRequest)>,
    rotated_server_credentials: Vec<String>,
    controller_queries: Vec<Option<AdminListQuery>>,
    removed_controllers: Vec<ClientId>,
    controlled_device_queries: Vec<Option<AdminListQuery>>,
    fetched_controlled_devices: Vec<DeviceId>,
    device_access_queries: Vec<(DeviceId, Option<AdminListQuery>)>,
    granted_device_access: Vec<(DeviceId, GrantDeviceAccessRequest)>,
    revoked_device_access: Vec<(DeviceId, UserId)>,
    removed_controlled_devices: Vec<DeviceId>,
}

impl FakeAdminApi {
    fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(FakeAdminState::default())),
        }
    }
}

#[async_trait]
impl AdminApi for FakeAdminApi {
    fn set_bearer_token(&mut self, bearer_token: String) {
        self.state
            .lock()
            .expect("fake admin state poisoned")
            .bearer_tokens
            .push(bearer_token);
    }

    async fn dashboard(
        &mut self,
    ) -> Result<DashboardSummary, quic_tunnel_control_client::ControlClientError> {
        Ok(DashboardSummary::default())
    }

    async fn list_users(
        &mut self,
        query: Option<AdminListQuery>,
    ) -> Result<Page<UserSummary>, quic_tunnel_control_client::ControlClientError> {
        self.state
            .lock()
            .expect("fake admin state poisoned")
            .list_users_queries
            .push(query);
        Ok(page(vec![user_summary()]))
    }

    async fn create_user(
        &mut self,
        request: CreateUserRequest,
    ) -> Result<UserSummary, quic_tunnel_control_client::ControlClientError> {
        self.state
            .lock()
            .expect("fake admin state poisoned")
            .created_users
            .push(request);
        Ok(user_summary())
    }

    async fn user(
        &mut self,
        user_id: &UserId,
    ) -> Result<UserDetail, quic_tunnel_control_client::ControlClientError> {
        self.state
            .lock()
            .expect("fake admin state poisoned")
            .fetched_users
            .push(user_id.clone());
        Ok(user_detail())
    }

    async fn update_user_status(
        &mut self,
        user_id: &UserId,
        request: UpdateUserStatusRequest,
    ) -> Result<UserSummary, quic_tunnel_control_client::ControlClientError> {
        self.state
            .lock()
            .expect("fake admin state poisoned")
            .status_updates
            .push((user_id.clone(), request));
        Ok(user_summary())
    }

    async fn update_user_role(
        &mut self,
        user_id: &UserId,
        request: UpdateUserRoleRequest,
    ) -> Result<UserSummary, quic_tunnel_control_client::ControlClientError> {
        self.state
            .lock()
            .expect("fake admin state poisoned")
            .role_updates
            .push((user_id.clone(), request));
        Ok(user_summary())
    }

    async fn audit_logs(
        &mut self,
        query: Option<AdminListQuery>,
    ) -> Result<Page<AuditLogEntry>, quic_tunnel_control_client::ControlClientError> {
        self.state
            .lock()
            .expect("fake admin state poisoned")
            .audit_queries
            .push(query);
        Ok(page(vec![audit_log()]))
    }

    async fn user_usage_summaries(
        &mut self,
        query: Option<AdminListQuery>,
    ) -> Result<Page<UserUsageSummary>, quic_tunnel_control_client::ControlClientError> {
        self.state
            .lock()
            .expect("fake admin state poisoned")
            .usage_queries
            .push(query);
        Ok(page(vec![usage_summary()]))
    }

    async fn report_relay_session_usage(
        &mut self,
        request: ReportRelaySessionUsageRequest,
    ) -> Result<(), quic_tunnel_control_client::ControlClientError> {
        self.state
            .lock()
            .expect("fake admin state poisoned")
            .relay_usage_reports
            .push(request);
        Ok(())
    }

    async fn reset_user_usage_period(
        &mut self,
        user_id: &UserId,
    ) -> Result<UserUsagePeriod, quic_tunnel_control_client::ControlClientError> {
        self.state
            .lock()
            .expect("fake admin state poisoned")
            .reset_usage_users
            .push(user_id.clone());
        Ok(UserUsagePeriod {
            user_id: user_id.clone(),
            current_period_started_epoch_sec: 100,
        })
    }

    async fn current_plan(
        &mut self,
    ) -> Result<Plan, quic_tunnel_control_client::ControlClientError> {
        Ok(plan("free"))
    }

    async fn plan_catalog(
        &mut self,
        query: Option<AdminListQuery>,
    ) -> Result<Page<Plan>, quic_tunnel_control_client::ControlClientError> {
        self.state
            .lock()
            .expect("fake admin state poisoned")
            .plan_queries
            .push(query);
        Ok(page(vec![plan("team")]))
    }

    async fn update_catalog_plan(
        &mut self,
        request: UpdatePlanCatalogRequest,
    ) -> Result<Plan, quic_tunnel_control_client::ControlClientError> {
        self.state
            .lock()
            .expect("fake admin state poisoned")
            .catalog_updates
            .push(request.clone());
        Ok(request.plan)
    }

    async fn assign_user_plan(
        &mut self,
        user_id: &UserId,
        plan_id: String,
    ) -> Result<Plan, quic_tunnel_control_client::ControlClientError> {
        self.state
            .lock()
            .expect("fake admin state poisoned")
            .assigned_plans
            .push((user_id.clone(), plan_id.clone()));
        Ok(plan(&plan_id))
    }

    async fn update_user_plan(
        &mut self,
        user_id: &UserId,
        request: UpdateUserPlanRequest,
    ) -> Result<Plan, quic_tunnel_control_client::ControlClientError> {
        self.state
            .lock()
            .expect("fake admin state poisoned")
            .user_plan_updates
            .push((user_id.clone(), request.clone()));
        Ok(request.plan)
    }

    async fn list_relay_credentials(
        &mut self,
        query: Option<AdminListQuery>,
    ) -> Result<Page<RelayCredential>, quic_tunnel_control_client::ControlClientError> {
        self.state
            .lock()
            .expect("fake admin state poisoned")
            .relay_credential_queries
            .push(query);
        Ok(page(vec![relay_credential()]))
    }

    async fn create_relay_credential(
        &mut self,
        request: CreateRelayCredentialRequest,
    ) -> Result<RelayCredential, quic_tunnel_control_client::ControlClientError> {
        self.state
            .lock()
            .expect("fake admin state poisoned")
            .created_relay_credentials
            .push(request);
        Ok(relay_credential())
    }

    async fn update_relay_credential_status(
        &mut self,
        relay_id: &str,
        request: UpdateRelayCredentialStatusRequest,
    ) -> Result<RelayCredential, quic_tunnel_control_client::ControlClientError> {
        self.state
            .lock()
            .expect("fake admin state poisoned")
            .relay_credential_status_updates
            .push((relay_id.to_string(), request));
        Ok(relay_credential())
    }

    async fn rotate_relay_credential(
        &mut self,
        relay_id: &str,
    ) -> Result<RelayCredential, quic_tunnel_control_client::ControlClientError> {
        self.state
            .lock()
            .expect("fake admin state poisoned")
            .rotated_relay_credentials
            .push(relay_id.to_string());
        Ok(relay_credential())
    }

    async fn create_relay_bootstrap(
        &mut self,
        request: CreateRelayBootstrapRequest,
    ) -> Result<RelayBootstrapResponse, quic_tunnel_control_client::ControlClientError> {
        self.state
            .lock()
            .expect("fake admin state poisoned")
            .created_relay_bootstraps
            .push(request);
        Ok(relay_bootstrap_response())
    }

    async fn register_relay(
        &mut self,
        request: RegisterRelayRequest,
    ) -> Result<RelayNode, quic_tunnel_control_client::ControlClientError> {
        self.state
            .lock()
            .expect("fake admin state poisoned")
            .registered_relays
            .push(request);
        Ok(relay_node())
    }

    async fn update_relay(
        &mut self,
        relay_id: &str,
        request: UpdateRelayRequest,
    ) -> Result<RelayNode, quic_tunnel_control_client::ControlClientError> {
        self.state
            .lock()
            .expect("fake admin state poisoned")
            .relay_updates
            .push((relay_id.to_string(), request));
        Ok(relay_node())
    }

    async fn report_relay_health(
        &mut self,
        relay_id: &str,
        request: ReportRelayHealthRequest,
    ) -> Result<RelayNode, quic_tunnel_control_client::ControlClientError> {
        self.state
            .lock()
            .expect("fake admin state poisoned")
            .relay_health_reports
            .push((relay_id.to_string(), request));
        Ok(relay_node())
    }

    async fn remove_relay(
        &mut self,
        relay_id: &str,
    ) -> Result<(), quic_tunnel_control_client::ControlClientError> {
        self.state
            .lock()
            .expect("fake admin state poisoned")
            .removed_relays
            .push(relay_id.to_string());
        Ok(())
    }

    async fn list_relays(
        &mut self,
        query: Option<AdminListQuery>,
    ) -> Result<Page<RelayNode>, quic_tunnel_control_client::ControlClientError> {
        self.state
            .lock()
            .expect("fake admin state poisoned")
            .relay_queries
            .push(query);
        Ok(page(vec![relay_node()]))
    }

    async fn admin_sessions(
        &mut self,
        query: Option<AdminListQuery>,
    ) -> Result<Page<AdminSessionSummary>, quic_tunnel_control_client::ControlClientError> {
        self.state
            .lock()
            .expect("fake admin state poisoned")
            .session_queries
            .push(query);
        Ok(page(vec![admin_session()]))
    }

    async fn list_server_credentials(
        &mut self,
        query: Option<AdminListQuery>,
    ) -> Result<Page<ServerCredentialSummary>, quic_tunnel_control_client::ControlClientError> {
        self.state
            .lock()
            .expect("fake admin state poisoned")
            .server_credential_queries
            .push(query);
        Ok(page(vec![server_credential_summary()]))
    }

    async fn server_credential(
        &mut self,
        credential_id: &str,
    ) -> Result<ServerCredentialSummary, quic_tunnel_control_client::ControlClientError> {
        self.state
            .lock()
            .expect("fake admin state poisoned")
            .fetched_server_credentials
            .push(credential_id.to_string());
        Ok(server_credential_summary())
    }

    async fn update_server_credential_status(
        &mut self,
        credential_id: &str,
        request: UpdateServerCredentialStatusRequest,
    ) -> Result<ServerCredentialSummary, quic_tunnel_control_client::ControlClientError> {
        self.state
            .lock()
            .expect("fake admin state poisoned")
            .server_credential_status_updates
            .push((credential_id.to_string(), request));
        Ok(server_credential_summary())
    }

    async fn rotate_server_credential(
        &mut self,
        credential_id: &str,
    ) -> Result<ServerCredentialResponse, quic_tunnel_control_client::ControlClientError> {
        self.state
            .lock()
            .expect("fake admin state poisoned")
            .rotated_server_credentials
            .push(credential_id.to_string());
        Ok(server_credential_response())
    }

    async fn list_controllers(
        &mut self,
        query: Option<AdminListQuery>,
    ) -> Result<Page<ControllerDevice>, quic_tunnel_control_client::ControlClientError> {
        self.state
            .lock()
            .expect("fake admin state poisoned")
            .controller_queries
            .push(query);
        Ok(page(vec![controller()]))
    }

    async fn remove_controller(
        &mut self,
        client_id: &ClientId,
    ) -> Result<(), quic_tunnel_control_client::ControlClientError> {
        self.state
            .lock()
            .expect("fake admin state poisoned")
            .removed_controllers
            .push(client_id.clone());
        Ok(())
    }

    async fn list_controlled_devices(
        &mut self,
        query: Option<AdminListQuery>,
    ) -> Result<Page<Device>, quic_tunnel_control_client::ControlClientError> {
        self.state
            .lock()
            .expect("fake admin state poisoned")
            .controlled_device_queries
            .push(query);
        Ok(page(vec![device()]))
    }

    async fn controlled_device(
        &mut self,
        device_id: &DeviceId,
    ) -> Result<Device, quic_tunnel_control_client::ControlClientError> {
        self.state
            .lock()
            .expect("fake admin state poisoned")
            .fetched_controlled_devices
            .push(device_id.clone());
        Ok(device())
    }

    async fn device_access_grants(
        &mut self,
        device_id: &DeviceId,
        query: Option<AdminListQuery>,
    ) -> Result<Page<DeviceAccessGrant>, quic_tunnel_control_client::ControlClientError> {
        self.state
            .lock()
            .expect("fake admin state poisoned")
            .device_access_queries
            .push((device_id.clone(), query));
        Ok(page(vec![device_access_grant()]))
    }

    async fn grant_device_access(
        &mut self,
        device_id: &DeviceId,
        request: GrantDeviceAccessRequest,
    ) -> Result<DeviceAccessGrant, quic_tunnel_control_client::ControlClientError> {
        self.state
            .lock()
            .expect("fake admin state poisoned")
            .granted_device_access
            .push((device_id.clone(), request));
        Ok(device_access_grant())
    }

    async fn revoke_device_access(
        &mut self,
        device_id: &DeviceId,
        user_id: &UserId,
    ) -> Result<(), quic_tunnel_control_client::ControlClientError> {
        self.state
            .lock()
            .expect("fake admin state poisoned")
            .revoked_device_access
            .push((device_id.clone(), user_id.clone()));
        Ok(())
    }

    async fn remove_controlled_device(
        &mut self,
        device_id: &DeviceId,
    ) -> Result<(), quic_tunnel_control_client::ControlClientError> {
        self.state
            .lock()
            .expect("fake admin state poisoned")
            .removed_controlled_devices
            .push(device_id.clone());
        Ok(())
    }
}

#[tokio::test]
async fn admin_sdk_reads_dashboard_audit_usage_and_sessions_with_saved_token() {
    let api = FakeAdminApi::new();
    let sdk = AdminSdk::new(api.clone(), token_store().await);
    let query = AdminListQuery {
        limit: Some(10),
        offset: Some(20),
        q: Some("alice".to_string()),
        ..AdminListQuery::default()
    };

    assert_eq!(sdk.dashboard().await.unwrap(), DashboardSummary::default());
    assert_eq!(
        sdk.audit_logs(Some(query.clone())).await.unwrap(),
        page(vec![audit_log()])
    );
    assert_eq!(
        sdk.user_usage_summaries(Some(query.clone())).await.unwrap(),
        page(vec![usage_summary()])
    );
    assert_eq!(
        sdk.admin_sessions(Some(query.clone())).await.unwrap(),
        page(vec![admin_session()])
    );

    let state = api.state.lock().expect("fake admin state poisoned");
    assert_eq!(state.audit_queries, vec![Some(query.clone())]);
    assert_eq!(state.usage_queries, vec![Some(query.clone())]);
    assert_eq!(state.session_queries, vec![Some(query)]);
    assert_bearer_count(&state, 4);
}

#[tokio::test]
async fn admin_sdk_manages_users_usage_and_plans_with_saved_token() {
    let api = FakeAdminApi::new();
    let sdk = AdminSdk::new(api.clone(), token_store().await);
    let user_id = UserId::new("user_001");
    let query = AdminListQuery {
        role: Some("admin".to_string()),
        enabled: Some(true),
        ..AdminListQuery::default()
    };

    assert_eq!(
        sdk.list_users(Some(query.clone())).await.unwrap(),
        page(vec![user_summary()])
    );
    assert_eq!(
        sdk.create_user(create_user_request()).await.unwrap(),
        user_summary()
    );
    assert_eq!(sdk.user(&user_id).await.unwrap(), user_detail());
    assert_eq!(
        sdk.update_user_status(&user_id, UpdateUserStatusRequest { enabled: false })
            .await
            .unwrap(),
        user_summary()
    );
    assert_eq!(
        sdk.update_user_role(
            &user_id,
            UpdateUserRoleRequest {
                role: ControlRole::Admin,
            },
        )
        .await
        .unwrap(),
        user_summary()
    );
    assert_eq!(
        sdk.reset_user_usage_period(&user_id).await.unwrap(),
        UserUsagePeriod {
            user_id: user_id.clone(),
            current_period_started_epoch_sec: 100,
        }
    );
    sdk.report_relay_session_usage(report_usage_request())
        .await
        .unwrap();
    assert_eq!(sdk.current_plan().await.unwrap(), plan("free"));
    assert_eq!(
        sdk.plan_catalog(None).await.unwrap(),
        page(vec![plan("team")])
    );
    assert_eq!(
        sdk.update_catalog_plan(UpdatePlanCatalogRequest { plan: plan("pro") })
            .await
            .unwrap(),
        plan("pro")
    );
    assert_eq!(
        sdk.assign_user_plan(&user_id, "team").await.unwrap(),
        plan("team")
    );
    assert_eq!(
        sdk.update_user_plan(
            &user_id,
            UpdateUserPlanRequest {
                plan: plan("enterprise"),
            },
        )
        .await
        .unwrap(),
        plan("enterprise")
    );

    let state = api.state.lock().expect("fake admin state poisoned");
    assert_eq!(state.list_users_queries, vec![Some(query)]);
    assert_eq!(state.created_users, vec![create_user_request()]);
    assert_eq!(state.fetched_users, vec![user_id.clone()]);
    assert_eq!(
        state.status_updates,
        vec![(user_id.clone(), UpdateUserStatusRequest { enabled: false })]
    );
    assert_eq!(
        state.role_updates,
        vec![(
            user_id.clone(),
            UpdateUserRoleRequest {
                role: ControlRole::Admin,
            },
        )]
    );
    assert_eq!(state.reset_usage_users, vec![user_id.clone()]);
    assert_eq!(state.relay_usage_reports, vec![report_usage_request()]);
    assert_eq!(
        state.catalog_updates,
        vec![UpdatePlanCatalogRequest { plan: plan("pro") }]
    );
    assert_eq!(
        state.assigned_plans,
        vec![(user_id.clone(), "team".to_string())]
    );
    assert_eq!(
        state.user_plan_updates,
        vec![(
            user_id,
            UpdateUserPlanRequest {
                plan: plan("enterprise"),
            },
        )]
    );
    assert_bearer_count(&state, 12);
}

#[tokio::test]
async fn admin_sdk_manages_relay_pool_and_credentials_with_saved_token() {
    let api = FakeAdminApi::new();
    let sdk = AdminSdk::new(api.clone(), token_store().await);
    let query = AdminListQuery {
        healthy: Some(true),
        ..AdminListQuery::default()
    };

    assert_eq!(
        sdk.list_relay_credentials(None).await.unwrap(),
        page(vec![relay_credential()])
    );
    assert_eq!(
        sdk.create_relay_credential(CreateRelayCredentialRequest {
            relay_id: "relay_001".to_string(),
            enabled: true,
        })
        .await
        .unwrap(),
        relay_credential()
    );
    assert_eq!(
        sdk.update_relay_credential_status(
            "relay_001",
            UpdateRelayCredentialStatusRequest { enabled: false },
        )
        .await
        .unwrap(),
        relay_credential()
    );
    assert_eq!(
        sdk.rotate_relay_credential("relay_001").await.unwrap(),
        relay_credential()
    );
    assert_eq!(
        sdk.create_relay_bootstrap(create_relay_bootstrap_request())
            .await
            .unwrap(),
        relay_bootstrap_response()
    );
    assert_eq!(
        sdk.register_relay(register_relay_request()).await.unwrap(),
        relay_node()
    );
    assert_eq!(
        sdk.update_relay(
            "relay_001",
            UpdateRelayRequest {
                relay_addr: "127.0.0.1:4444".to_string(),
                admin_addr: "127.0.0.1:9091".to_string(),
                capacity_streams: 200,
                healthy: true,
            },
        )
        .await
        .unwrap(),
        relay_node()
    );
    assert_eq!(
        sdk.report_relay_health("relay_001", report_relay_health_request())
            .await
            .unwrap(),
        relay_node()
    );
    sdk.remove_relay("relay_001").await.unwrap();
    assert_eq!(
        sdk.list_relays(Some(query.clone())).await.unwrap(),
        page(vec![relay_node()])
    );

    let state = api.state.lock().expect("fake admin state poisoned");
    assert_eq!(state.relay_credential_queries, vec![None]);
    assert_eq!(
        state.created_relay_credentials,
        vec![CreateRelayCredentialRequest {
            relay_id: "relay_001".to_string(),
            enabled: true,
        }]
    );
    assert_eq!(
        state.relay_credential_status_updates,
        vec![(
            "relay_001".to_string(),
            UpdateRelayCredentialStatusRequest { enabled: false },
        )]
    );
    assert_eq!(
        state.rotated_relay_credentials,
        vec!["relay_001".to_string()]
    );
    assert_eq!(
        state.created_relay_bootstraps,
        vec![create_relay_bootstrap_request()]
    );
    assert_eq!(state.registered_relays, vec![register_relay_request()]);
    assert_eq!(
        state.relay_health_reports,
        vec![("relay_001".to_string(), report_relay_health_request())]
    );
    assert_eq!(state.removed_relays, vec!["relay_001".to_string()]);
    assert_eq!(state.relay_queries, vec![Some(query)]);
    assert_bearer_count(&state, 10);
}

#[tokio::test]
async fn admin_sdk_requires_saved_token() {
    let api = FakeAdminApi::new();
    let sdk = AdminSdk::new(api.clone(), MemoryTokenStore::default());

    let err = sdk.dashboard().await.unwrap_err();

    assert!(matches!(err, SdkError::NotAuthenticated));
    let state = api.state.lock().expect("fake admin state poisoned");
    assert!(state.bearer_tokens.is_empty());
}

#[tokio::test]
async fn admin_sdk_manages_server_credentials_with_saved_token() {
    let api = FakeAdminApi::new();
    let sdk = AdminSdk::new(api.clone(), token_store().await);
    let query = AdminListQuery {
        enabled: Some(true),
        ..AdminListQuery::default()
    };

    assert_eq!(
        sdk.list_server_credentials(Some(query.clone()))
            .await
            .unwrap(),
        page(vec![server_credential_summary()])
    );
    assert_eq!(
        sdk.server_credential("srv_cred_001").await.unwrap(),
        server_credential_summary()
    );
    assert_eq!(
        sdk.update_server_credential_status(
            "srv_cred_001",
            UpdateServerCredentialStatusRequest { enabled: false },
        )
        .await
        .unwrap(),
        server_credential_summary()
    );
    assert_eq!(
        sdk.rotate_server_credential("srv_cred_001").await.unwrap(),
        server_credential_response()
    );

    let state = api.state.lock().expect("fake admin state poisoned");
    assert_eq!(state.server_credential_queries, vec![Some(query)]);
    assert_eq!(
        state.fetched_server_credentials,
        vec!["srv_cred_001".to_string()]
    );
    assert_eq!(
        state.server_credential_status_updates,
        vec![(
            "srv_cred_001".to_string(),
            UpdateServerCredentialStatusRequest { enabled: false },
        )]
    );
    assert_eq!(
        state.rotated_server_credentials,
        vec!["srv_cred_001".to_string()]
    );
    assert_bearer_count(&state, 4);
}

#[tokio::test]
async fn admin_sdk_manages_controllers_with_saved_token() {
    let api = FakeAdminApi::new();
    let sdk = AdminSdk::new(api.clone(), token_store().await);
    let query = AdminListQuery {
        q: Some("phone".to_string()),
        ..AdminListQuery::default()
    };

    assert_eq!(
        sdk.list_controllers(Some(query.clone())).await.unwrap(),
        page(vec![controller()])
    );
    sdk.remove_controller(&ClientId::new("phone_001"))
        .await
        .unwrap();

    let state = api.state.lock().expect("fake admin state poisoned");
    assert_eq!(state.controller_queries, vec![Some(query)]);
    assert_eq!(state.removed_controllers, vec![ClientId::new("phone_001")]);
    assert_bearer_count(&state, 2);
}

#[tokio::test]
async fn admin_sdk_manages_controlled_devices_and_access_grants_with_saved_token() {
    let api = FakeAdminApi::new();
    let sdk = AdminSdk::new(api.clone(), token_store().await);
    let query = AdminListQuery {
        user_id: Some(UserId::new("user_001")),
        ..AdminListQuery::default()
    };
    let device_id = DeviceId::new("pc_001");
    let grantee = UserId::new("user_002");

    assert_eq!(
        sdk.list_controlled_devices(Some(query.clone()))
            .await
            .unwrap(),
        page(vec![device()])
    );
    assert_eq!(sdk.controlled_device(&device_id).await.unwrap(), device());
    assert_eq!(
        sdk.device_access_grants(&device_id, Some(query.clone()))
            .await
            .unwrap(),
        page(vec![device_access_grant()])
    );
    assert_eq!(
        sdk.grant_device_access(
            &device_id,
            GrantDeviceAccessRequest {
                user_id: grantee.clone(),
            },
        )
        .await
        .unwrap(),
        device_access_grant()
    );
    sdk.revoke_device_access(&device_id, &grantee)
        .await
        .unwrap();
    sdk.remove_controlled_device(&device_id).await.unwrap();

    let state = api.state.lock().expect("fake admin state poisoned");
    assert_eq!(state.controlled_device_queries, vec![Some(query.clone())]);
    assert_eq!(state.fetched_controlled_devices, vec![device_id.clone()]);
    assert_eq!(
        state.device_access_queries,
        vec![(device_id.clone(), Some(query))]
    );
    assert_eq!(
        state.granted_device_access,
        vec![(
            device_id.clone(),
            GrantDeviceAccessRequest {
                user_id: grantee.clone(),
            },
        )]
    );
    assert_eq!(
        state.revoked_device_access,
        vec![(device_id.clone(), grantee)]
    );
    assert_eq!(state.removed_controlled_devices, vec![device_id]);
    assert_bearer_count(&state, 6);
}

async fn token_store() -> MemoryTokenStore {
    let store = MemoryTokenStore::default();
    store
        .save_token(StoredToken {
            user_id: UserId::new("admin_001"),
            access_token: "admin-token".to_string(),
            expire_at: 100,
        })
        .await
        .unwrap();
    store
}

fn assert_bearer_count(state: &FakeAdminState, count: usize) {
    assert_eq!(state.bearer_tokens, vec!["admin-token".to_string(); count]);
}

fn page<T>(items: Vec<T>) -> Page<T> {
    Page {
        total: items.len() as u64,
        limit: 50,
        offset: 0,
        items,
    }
}

fn user_summary() -> UserSummary {
    UserSummary {
        user_id: UserId::new("user_001"),
        email: "alice@example.com".to_string(),
        display_name: "Alice".to_string(),
        role: ControlRole::User,
        enabled: true,
        plan_id: "free".to_string(),
        controller_count: 1,
        device_count: 2,
    }
}

fn user_detail() -> UserDetail {
    UserDetail {
        user: user_summary(),
        plan: plan("free"),
        controllers: Vec::new(),
        devices: vec![device()],
    }
}

fn create_user_request() -> CreateUserRequest {
    CreateUserRequest {
        email: "alice@example.com".to_string(),
        password: "password-123".to_string(),
        display_name: "Alice".to_string(),
        role: ControlRole::User,
        enabled: true,
    }
}

fn audit_log() -> AuditLogEntry {
    AuditLogEntry {
        audit_id: "audit_001".to_string(),
        actor_user_id: UserId::new("admin_001"),
        actor_subject: "admin@example.com".to_string(),
        actor_role: ControlRole::Admin,
        action: "user.create".to_string(),
        target_type: "user".to_string(),
        target_id: "user_001".to_string(),
        message: "created user".to_string(),
        created_epoch_sec: 100,
    }
}

fn usage_summary() -> UserUsageSummary {
    UserUsageSummary {
        user_id: UserId::new("user_001"),
        email: "alice@example.com".to_string(),
        plan_id: "free".to_string(),
        current_period_started_epoch_sec: 100,
        max_controller_devices: 2,
        controller_count: 1,
        device_count: 2,
        session_count: 3,
        pending_sessions: 0,
        claimed_sessions: 1,
        bound_sessions: 1,
        closed_sessions: 1,
        expired_sessions: 0,
        current_session_quota_bytes: 1024,
        relay_quota_granted_bytes: 2048,
        actual_uplink_bytes: 10,
        actual_downlink_bytes: 20,
        actual_total_bytes: 30,
    }
}

fn report_usage_request() -> ReportRelaySessionUsageRequest {
    ReportRelaySessionUsageRequest {
        relay_id: "relay_001".to_string(),
        sessions: vec![RelaySessionUsageReport {
            session_id: SessionId::new("sess_001"),
            stats: TrafficStats {
                session_id: Some(SessionId::new("sess_001")),
                uplink_bytes: 10,
                downlink_bytes: 20,
                total_bytes: 30,
                duration_sec: 40,
                active_streams: 1,
            },
        }],
    }
}

fn plan(plan_id: &str) -> Plan {
    Plan {
        plan_id: plan_id.to_string(),
        name: plan_id.to_string(),
        max_controller_devices: 2,
        relay_limits: RelayLimits {
            max_bps: 1024,
            max_streams: 8,
            max_duration_sec: 3600,
            traffic_quota_bytes: 1_048_576,
        },
    }
}

fn relay_credential() -> RelayCredential {
    RelayCredential {
        relay_id: "relay_001".to_string(),
        enabled: true,
        token_version: 1,
    }
}

fn register_relay_request() -> RegisterRelayRequest {
    RegisterRelayRequest {
        relay_id: "relay_001".to_string(),
        relay_addr: "127.0.0.1:4443".to_string(),
        admin_addr: "127.0.0.1:9090".to_string(),
        capacity_streams: 100,
    }
}

fn report_relay_health_request() -> ReportRelayHealthRequest {
    ReportRelayHealthRequest {
        relay_addr: "127.0.0.1:4443".to_string(),
        admin_addr: "127.0.0.1:9090".to_string(),
        capacity_streams: 100,
        health: RelayHealthReport {
            status: RelayHealthStatus::Healthy,
            reason: String::new(),
            relay_version: "0.1.0".to_string(),
            uptime_sec: 10,
            active_sessions: 1,
            active_streams: 2,
            total_uplink_bytes: 3,
            total_downlink_bytes: 4,
            total_bytes: 7,
            data_plane_bound: true,
            admin_bound: true,
        },
        sessions: Vec::new(),
    }
}

fn create_relay_bootstrap_request() -> CreateRelayBootstrapRequest {
    CreateRelayBootstrapRequest {
        relay_id: "relay_001".to_string(),
        control_url: "https://control.example.com".to_string(),
        relay_addr: "127.0.0.1:4443".to_string(),
        admin_addr: "127.0.0.1:9090".to_string(),
        capacity_streams: 100,
        heartbeat_interval_sec: 30,
        ttl_sec: 900,
    }
}

fn relay_bootstrap_response() -> RelayBootstrapResponse {
    RelayBootstrapResponse {
        bootstrap_id: "rb_001".to_string(),
        relay_id: "relay_001".to_string(),
        control_url: "https://control.example.com".to_string(),
        expires_epoch_sec: 1000,
        install_command: "curl -fsSL https://control.example.com/install-relayd.sh | sudo sh"
            .to_string(),
        no_service_install_command:
            "curl -fsSL https://control.example.com/install-relayd.sh | sudo sh -s -- --no-service"
                .to_string(),
        bootstrap_token: "shown-once".to_string(),
    }
}

fn relay_node() -> RelayNode {
    RelayNode {
        relay_id: "relay_001".to_string(),
        relay_addr: "127.0.0.1:4443".to_string(),
        admin_addr: "127.0.0.1:9090".to_string(),
        capacity_streams: 100,
        healthy: true,
        last_seen_epoch_sec: 100,
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
        admin_bound: true,
        last_health_report_epoch_sec: 100,
    }
}

fn server_credential_summary() -> ServerCredentialSummary {
    ServerCredentialSummary {
        credential_id: "srv_cred_001".to_string(),
        user_id: UserId::new("user_001"),
        device_id: DeviceId::new("pc_001"),
        device_name: "Office PC".to_string(),
        enabled: true,
        token_version: 1,
        created_epoch_sec: 100,
        last_used_epoch_sec: Some(120),
    }
}

fn server_credential_response() -> ServerCredentialResponse {
    ServerCredentialResponse {
        credential_id: "srv_cred_001".to_string(),
        device_id: DeviceId::new("pc_001"),
        server_token: "server-token".to_string(),
        token_type: "Bearer".to_string(),
    }
}

fn controller() -> ControllerDevice {
    ControllerDevice {
        user_id: UserId::new("user_001"),
        client_id: ClientId::new("phone_001"),
        name: "Phone".to_string(),
    }
}

fn device() -> Device {
    Device {
        device_id: DeviceId::new("pc_001"),
        user_id: UserId::new("user_001"),
        name: "Office PC".to_string(),
        status: DeviceStatus::Online,
        agent_version: "0.1.0".to_string(),
    }
}

fn device_access_grant() -> DeviceAccessGrant {
    DeviceAccessGrant {
        device_id: DeviceId::new("pc_001"),
        user_id: UserId::new("user_002"),
    }
}

fn admin_session() -> AdminSessionSummary {
    AdminSessionSummary {
        session_id: SessionId::new("sess_001"),
        user_id: UserId::new("user_001"),
        user_email: "alice@example.com".to_string(),
        device_id: DeviceId::new("pc_001"),
        device_name: "Office PC".to_string(),
        service_id: ServiceId::new("svc_web"),
        service_name: "Web".to_string(),
        client_id: ClientId::new("phone_001"),
        status: quic_tunnel_control_client::AgentSessionStatus::Bound,
        relay_addr: "127.0.0.1:4443".to_string(),
        punch_addr: "127.0.0.1:3478".to_string(),
        expire_at: 1000,
    }
}
