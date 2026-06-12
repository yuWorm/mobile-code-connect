use std::{
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use mobilecode_connect_auth::{ControlRole, TokenKey, TokenSigner};
use mobilecode_connect_control::state::ControlState;
use mobilecode_connect_control_client::{
    AgentSessionAssignment, AgentSessionStatus, AssignUserPlanRequest, CreateRelayBootstrapRequest,
    CreateRelayCredentialRequest, CreateUserRequest, GrantDeviceAccessRequest, LoginRequest,
    OAuthIdentity, OAuthProvider, RegisterControllerDeviceRequest, RegisterRelayRequest,
    RegisterUserRequest, RelayBootstrapExchangeRequest, RelaySessionUsageReport,
    ReportRelaySessionUsageRequest, UpdatePlanCatalogRequest, UpdateRelayCredentialStatusRequest,
    UpdateRelayRequest, UpdateUserRoleRequest,
};
use mobilecode_connect_control_client::{Plan, UpdateUserPlanRequest};
use mobilecode_connect_protocol::{
    ClientId, Device, DeviceId, DeviceStatus, RelayLimits, Service, ServiceId, ServiceProtocol,
    SessionId, TrafficStats, UserId,
};

fn temp_db_path(test_name: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    path.push(format!("mobilecode-connect-{test_name}-{nanos}.sqlite"));
    path
}

fn device(device_id: &str) -> Device {
    Device {
        device_id: DeviceId::new(device_id),
        user_id: UserId::new("ignored"),
        name: "Office PC".to_string(),
        status: DeviceStatus::Online,
        agent_version: "0.1.0".to_string(),
    }
}

fn service(device_id: &str, service_id: &str) -> Service {
    Service {
        service_id: ServiceId::new(service_id),
        device_id: DeviceId::new(device_id),
        name: "Dev Web".to_string(),
        protocol: ServiceProtocol::Tcp,
        target_host: "127.0.0.1".to_string(),
        target_port: 3000,
    }
}

#[test]
fn sqlite_store_restores_users_and_relay_pool_after_restart() {
    let db_path = temp_db_path("control-persistence");
    let token_secret = "dev-secret";
    let relay_addr = "seed-relay.example.com:4443";
    let punch_addr = "punch.example.com:3478";

    let state = ControlState::new_sqlite(token_secret, relay_addr, punch_addr, &db_path).unwrap();
    let auth = state
        .register_user(RegisterUserRequest {
            email: "owner@example.com".to_string(),
            password: "password-123".to_string(),
            display_name: "Owner".to_string(),
        })
        .unwrap();
    state
        .register_relay(RegisterRelayRequest {
            relay_id: "relay_west".to_string(),
            relay_addr: "relay-west.example.com:4443".to_string(),
            admin_addr: "relay-west.example.com:9090".to_string(),
            capacity_streams: 128,
        })
        .unwrap();
    drop(state);

    let restored =
        ControlState::new_sqlite(token_secret, relay_addr, punch_addr, &db_path).unwrap();
    let login = restored
        .login(LoginRequest {
            email: "owner@example.com".to_string(),
            password: "password-123".to_string(),
        })
        .unwrap();
    let relays = restored.relays();

    assert_eq!(login.user_id, auth.user_id);
    assert!(relays.iter().any(|relay| relay.relay_id == "relay_west"
        && relay.relay_addr == "relay-west.example.com:4443"
        && relay.capacity_streams == 128
        && relay.healthy));

    let _ = std::fs::remove_file(db_path);
}

#[test]
fn oauth_identity_persists_across_restart() {
    let db_path = temp_db_path("control-oauth-identity-persistence");
    let token_secret = "dev-secret";
    let relay_addr = "seed-relay.example.com:4443";
    let punch_addr = "punch.example.com:3478";

    let state = ControlState::new_sqlite(token_secret, relay_addr, punch_addr, &db_path).unwrap();
    let auth = state
        .register_user(RegisterUserRequest {
            email: "oauth-owner@example.com".to_string(),
            password: "password-123".to_string(),
            display_name: "OAuth Owner".to_string(),
        })
        .unwrap();
    let identity = OAuthIdentity {
        provider: OAuthProvider::GitHub,
        provider_user_id: "123456".to_string(),
        user_id: auth.user_id,
        email: "oauth-owner@example.com".to_string(),
        login: "octocat".to_string(),
        avatar_url: "https://avatars.githubusercontent.com/u/123456".to_string(),
        created_epoch_sec: 1_767_000_000,
        updated_epoch_sec: 1_767_000_001,
    };
    state.upsert_oauth_identity(identity.clone()).unwrap();
    drop(state);

    let restored =
        ControlState::new_sqlite(token_secret, relay_addr, punch_addr, &db_path).unwrap();
    let restored_identity = restored
        .oauth_identity(OAuthProvider::GitHub, "123456")
        .unwrap();

    assert_eq!(restored_identity, identity);

    let _ = std::fs::remove_file(db_path);
}

#[test]
fn sqlite_store_restores_bootstrapped_admin_user_role_after_restart() {
    let db_path = temp_db_path("control-admin-persistence");
    let token_secret = "dev-secret";
    let relay_addr = "seed-relay.example.com:4443";
    let punch_addr = "punch.example.com:3478";

    let state = ControlState::new_sqlite(token_secret, relay_addr, punch_addr, &db_path).unwrap();
    let admin = state
        .bootstrap_admin_user(RegisterUserRequest {
            email: "admin@example.com".to_string(),
            password: "admin-password-123".to_string(),
            display_name: "Admin".to_string(),
        })
        .unwrap();
    let claims = TokenSigner::new(TokenKey::new(token_secret))
        .verify_control(&admin.access_token, 1_767_000_000)
        .unwrap();
    assert_eq!(claims.role, ControlRole::Admin);
    drop(state);

    let restored =
        ControlState::new_sqlite(token_secret, relay_addr, punch_addr, &db_path).unwrap();
    let login = restored
        .login(LoginRequest {
            email: "admin@example.com".to_string(),
            password: "admin-password-123".to_string(),
        })
        .unwrap();
    let claims = TokenSigner::new(TokenKey::new(token_secret))
        .verify_control(&login.access_token, 1_767_000_000)
        .unwrap();

    assert_eq!(login.user_id, admin.user_id);
    assert_eq!(claims.user_id, admin.user_id);
    assert_eq!(claims.role, ControlRole::Admin);

    let _ = std::fs::remove_file(db_path);
}

#[test]
fn sqlite_store_restores_admin_created_user_and_role_update_after_restart() {
    let db_path = temp_db_path("control-admin-created-user-persistence");
    let token_secret = "dev-secret";
    let relay_addr = "seed-relay.example.com:4443";
    let punch_addr = "punch.example.com:3478";

    let state = ControlState::new_sqlite(token_secret, relay_addr, punch_addr, &db_path).unwrap();
    let created = state
        .create_user(CreateUserRequest {
            email: "created-admin@example.com".to_string(),
            password: "created-password-123".to_string(),
            display_name: "Created Admin".to_string(),
            role: ControlRole::Admin,
            enabled: true,
        })
        .unwrap();
    state
        .update_user_role(
            &created.user_id,
            UpdateUserRoleRequest {
                role: ControlRole::User,
            },
        )
        .unwrap();
    drop(state);

    let restored =
        ControlState::new_sqlite(token_secret, relay_addr, punch_addr, &db_path).unwrap();
    let login = restored
        .login(LoginRequest {
            email: "created-admin@example.com".to_string(),
            password: "created-password-123".to_string(),
        })
        .unwrap();
    let claims = TokenSigner::new(TokenKey::new(token_secret))
        .verify_control(&login.access_token, 1_767_000_000)
        .unwrap();
    let user = restored.user_detail(&created.user_id).unwrap();

    assert_eq!(login.user_id, created.user_id);
    assert_eq!(claims.role, ControlRole::User);
    assert_eq!(user.user.email, "created-admin@example.com");
    assert_eq!(user.user.display_name, "Created Admin");
    assert_eq!(user.user.role, ControlRole::User);

    let _ = std::fs::remove_file(db_path);
}

#[test]
fn registered_user_login_keeps_user_role() {
    let state = ControlState::new(
        "dev-secret",
        "relay.example.com:4443",
        "punch.example.com:3478",
    );
    let auth = state
        .register_user(RegisterUserRequest {
            email: "owner@example.com".to_string(),
            password: "password-123".to_string(),
            display_name: "Owner".to_string(),
        })
        .unwrap();

    let claims = TokenSigner::new(TokenKey::new("dev-secret"))
        .verify_control(&auth.access_token, 1_767_000_000)
        .unwrap();

    assert_eq!(claims.role, ControlRole::User);
}

#[test]
fn sqlite_store_restores_updated_user_plan_after_restart() {
    let db_path = temp_db_path("control-plan-persistence");
    let token_secret = "dev-secret";
    let relay_addr = "seed-relay.example.com:4443";
    let punch_addr = "punch.example.com:3478";

    let state = ControlState::new_sqlite(token_secret, relay_addr, punch_addr, &db_path).unwrap();
    let auth = state
        .register_user(RegisterUserRequest {
            email: "plan-owner@example.com".to_string(),
            password: "password-123".to_string(),
            display_name: "Plan Owner".to_string(),
        })
        .unwrap();
    let upgraded = Plan {
        plan_id: "team".to_string(),
        name: "Team".to_string(),
        max_controller_devices: 5,
        relay_limits: RelayLimits {
            max_bps: 4_096,
            max_streams: 16,
            max_duration_sec: 7_200,
            traffic_quota_bytes: 1_048_576,
        },
    };
    state
        .update_user_plan(
            &auth.user_id,
            UpdateUserPlanRequest {
                plan: upgraded.clone(),
            },
        )
        .unwrap();
    drop(state);

    let restored =
        ControlState::new_sqlite(token_secret, relay_addr, punch_addr, &db_path).unwrap();

    assert_eq!(restored.plan_for_user(&auth.user_id), upgraded);

    let _ = std::fs::remove_file(db_path);
}

#[test]
fn sqlite_store_restores_plan_catalog_after_restart() {
    let db_path = temp_db_path("control-plan-catalog-persistence");
    let token_secret = "dev-secret";
    let relay_addr = "seed-relay.example.com:4443";
    let punch_addr = "punch.example.com:3478";

    let state = ControlState::new_sqlite(token_secret, relay_addr, punch_addr, &db_path).unwrap();
    let auth = state
        .register_user(RegisterUserRequest {
            email: "catalog-owner@example.com".to_string(),
            password: "password-123".to_string(),
            display_name: "Catalog Owner".to_string(),
        })
        .unwrap();
    let team = Plan {
        plan_id: "team".to_string(),
        name: "Team".to_string(),
        max_controller_devices: 8,
        relay_limits: RelayLimits {
            max_bps: 16_384,
            max_streams: 32,
            max_duration_sec: 7_200,
            traffic_quota_bytes: 8_388_608,
        },
    };
    state
        .update_catalog_plan(UpdatePlanCatalogRequest { plan: team.clone() })
        .unwrap();
    state
        .assign_user_plan(
            &auth.user_id,
            AssignUserPlanRequest {
                plan_id: "team".to_string(),
            },
        )
        .unwrap();
    drop(state);

    let restored =
        ControlState::new_sqlite(token_secret, relay_addr, punch_addr, &db_path).unwrap();
    let catalog = restored.plan_catalog();

    assert!(catalog.iter().any(|plan| plan.plan_id == "free"));
    assert_eq!(restored.catalog_plan("team").unwrap(), team);
    assert_eq!(restored.plan_for_user(&auth.user_id), team);

    let _ = std::fs::remove_file(db_path);
}

#[test]
fn sqlite_store_restores_relay_pool_updates_after_restart() {
    let db_path = temp_db_path("control-relay-persistence");
    let token_secret = "dev-secret";
    let relay_addr = "seed-relay.example.com:4443";
    let punch_addr = "punch.example.com:3478";

    let state = ControlState::new_sqlite(token_secret, relay_addr, punch_addr, &db_path).unwrap();
    state
        .register_relay(RegisterRelayRequest {
            relay_id: "relay_a".to_string(),
            relay_addr: "relay-a.example.com:4443".to_string(),
            admin_addr: "relay-a.example.com:9090".to_string(),
            capacity_streams: 16,
        })
        .unwrap();
    state
        .register_relay(RegisterRelayRequest {
            relay_id: "relay_b".to_string(),
            relay_addr: "relay-b.example.com:4443".to_string(),
            admin_addr: "relay-b.example.com:9090".to_string(),
            capacity_streams: 64,
        })
        .unwrap();
    state
        .update_relay(
            "relay_b",
            UpdateRelayRequest {
                relay_addr: "relay-b-new.example.com:4443".to_string(),
                admin_addr: "relay-b-new.example.com:9090".to_string(),
                capacity_streams: 128,
                healthy: false,
            },
        )
        .unwrap();
    state.remove_relay("relay_a").unwrap();
    drop(state);

    let restored =
        ControlState::new_sqlite(token_secret, relay_addr, punch_addr, &db_path).unwrap();
    let relays = restored.relays();

    assert!(!relays.iter().any(|relay| relay.relay_id == "relay_a"));
    let relay_b = relays
        .iter()
        .find(|relay| relay.relay_id == "relay_b")
        .unwrap();
    assert_eq!(relay_b.relay_addr, "relay-b-new.example.com:4443");
    assert_eq!(relay_b.capacity_streams, 128);
    assert!(!relay_b.healthy);

    let _ = std::fs::remove_file(db_path);
}

#[test]
fn sqlite_store_restores_relay_credentials_after_restart() {
    let db_path = temp_db_path("control-relay-credential-persistence");
    let token_secret = "dev-secret";
    let relay_addr = "seed-relay.example.com:4443";
    let punch_addr = "punch.example.com:3478";

    let state = ControlState::new_sqlite(token_secret, relay_addr, punch_addr, &db_path).unwrap();
    state
        .create_relay_credential(CreateRelayCredentialRequest {
            relay_id: "relay_managed".to_string(),
            enabled: true,
        })
        .unwrap();
    state.rotate_relay_credential("relay_managed").unwrap();
    state
        .update_relay_credential_status(
            "relay_managed",
            UpdateRelayCredentialStatusRequest { enabled: false },
        )
        .unwrap();
    drop(state);

    let restored =
        ControlState::new_sqlite(token_secret, relay_addr, punch_addr, &db_path).unwrap();
    let credential = restored.relay_credential("relay_managed").unwrap();

    assert_eq!(credential.relay_id, "relay_managed");
    assert_eq!(credential.token_version, 2);
    assert!(!credential.enabled);
    assert!(restored.issue_relay_token("relay_managed").is_err());

    let enabled = restored
        .update_relay_credential_status(
            "relay_managed",
            UpdateRelayCredentialStatusRequest { enabled: true },
        )
        .unwrap();
    assert!(enabled.enabled);
    assert_eq!(enabled.token_version, 2);
    let token = restored.issue_relay_token("relay_managed").unwrap();
    let claims = TokenSigner::new(TokenKey::new(token_secret))
        .verify_control(&token, 1_767_000_000)
        .unwrap();
    assert_eq!(claims.role, ControlRole::Relay);
    assert_eq!(claims.subject, "relay_managed");
    assert_eq!(claims.relay_token_version, Some(2));

    let _ = std::fs::remove_file(db_path);
}

#[test]
fn sqlite_store_restores_relay_bootstrap_for_single_exchange_after_restart() {
    let db_path = temp_db_path("control-relay-bootstrap-persistence");
    let token_secret = "dev-secret";
    let relay_addr = "seed-relay.example.com:4443";
    let punch_addr = "punch.example.com:3478";

    let state = ControlState::new_sqlite(token_secret, relay_addr, punch_addr, &db_path)
        .unwrap()
        .with_relay_health_now_epoch_sec(1_767_000_000);
    let admin_token = state.issue_admin_token("admin@example.com").unwrap();
    let actor = TokenSigner::new(TokenKey::new(token_secret))
        .verify_control(&admin_token, 1_767_000_000)
        .unwrap();
    let bootstrap = state
        .create_relay_bootstrap(
            &actor,
            CreateRelayBootstrapRequest {
                relay_id: "relay_bootstrap_persisted".to_string(),
                control_url: "https://control.example.com".to_string(),
                relay_addr: "relay-persisted.example.com:4443".to_string(),
                admin_addr: "127.0.0.1:9090".to_string(),
                capacity_streams: 64,
                heartbeat_interval_sec: 30,
                ttl_sec: 900,
            },
        )
        .unwrap();
    drop(state);

    let restored = ControlState::new_sqlite(token_secret, relay_addr, punch_addr, &db_path)
        .unwrap()
        .with_relay_health_now_epoch_sec(1_767_000_001);
    let exchange = restored
        .exchange_relay_bootstrap(
            &bootstrap.bootstrap_id,
            RelayBootstrapExchangeRequest {
                bootstrap_token: bootstrap.bootstrap_token.clone(),
            },
        )
        .unwrap();

    assert_eq!(exchange.relay_id, "relay_bootstrap_persisted");
    assert_eq!(exchange.control_url, "https://control.example.com");
    assert_eq!(exchange.relay_addr, "relay-persisted.example.com:4443");
    assert_eq!(exchange.capacity_streams, 64);
    assert_eq!(exchange.token_secret, token_secret);
    let claims = TokenSigner::new(TokenKey::new(token_secret))
        .verify_control(&exchange.control_token, 1_767_000_000)
        .unwrap();
    assert_eq!(claims.role, ControlRole::Relay);
    assert_eq!(claims.subject, "relay_bootstrap_persisted");
    assert_eq!(claims.relay_token_version, Some(1));

    assert!(restored
        .exchange_relay_bootstrap(
            &bootstrap.bootstrap_id,
            RelayBootstrapExchangeRequest {
                bootstrap_token: bootstrap.bootstrap_token,
            },
        )
        .is_err());

    let _ = std::fs::remove_file(db_path);
}

#[test]
fn sqlite_store_restores_reported_relay_session_usage_after_restart() {
    let db_path = temp_db_path("control-relay-usage-persistence");
    let token_secret = "dev-secret";
    let relay_addr = "seed-relay.example.com:4443";
    let punch_addr = "punch.example.com:3478";
    let session_id = SessionId::new("sess_reported_usage");

    let state = ControlState::new_sqlite(token_secret, relay_addr, punch_addr, &db_path).unwrap();
    let auth = state
        .register_user(RegisterUserRequest {
            email: "reported-usage@example.com".to_string(),
            password: "password-123".to_string(),
            display_name: "Reported Usage".to_string(),
        })
        .unwrap();
    state
        .register_relay(RegisterRelayRequest {
            relay_id: "relay_usage".to_string(),
            relay_addr: "relay-usage.example.com:4443".to_string(),
            admin_addr: "relay-usage.example.com:9090".to_string(),
            capacity_streams: 32,
        })
        .unwrap();
    state
        .register_device_for_user(&auth.user_id, device("server_usage"))
        .unwrap();
    state
        .register_services_for_user(&auth.user_id, vec![service("server_usage", "svc_usage")])
        .unwrap();
    state
        .add_agent_session(AgentSessionAssignment {
            session_id: session_id.clone(),
            user_id: auth.user_id.clone(),
            device_id: DeviceId::new("server_usage"),
            service_id: ServiceId::new("svc_usage"),
            client_id: ClientId::new("phone_usage"),
            relay_token: String::new(),
            relay_addr: "relay-usage.example.com:4443".to_string(),
            punch_addr: punch_addr.to_string(),
            expire_at: 4_102_444_800,
            status: AgentSessionStatus::Bound,
            grant_id: None,
            grant_revocation_version: None,
            grant_service_id: None,
        })
        .unwrap();
    state
        .report_relay_session_usage(ReportRelaySessionUsageRequest {
            relay_id: "relay_usage".to_string(),
            sessions: vec![RelaySessionUsageReport {
                session_id: session_id.clone(),
                stats: TrafficStats {
                    session_id: Some(session_id),
                    uplink_bytes: 21,
                    downlink_bytes: 34,
                    total_bytes: 55,
                    duration_sec: 8,
                    active_streams: 0,
                },
            }],
        })
        .unwrap();
    drop(state);

    let restored =
        ControlState::new_sqlite(token_secret, relay_addr, punch_addr, &db_path).unwrap();
    let summaries = restored.user_usage_summaries();
    let summary = summaries
        .iter()
        .find(|summary| summary.user_id == auth.user_id)
        .unwrap();

    assert_eq!(summary.actual_uplink_bytes, 21);
    assert_eq!(summary.actual_downlink_bytes, 34);
    assert_eq!(summary.actual_total_bytes, 55);

    let _ = std::fs::remove_file(db_path);
}

#[test]
fn sqlite_store_restores_usage_period_reset_after_restart() {
    let db_path = temp_db_path("control-usage-reset-persistence");
    let token_secret = "dev-secret";
    let relay_addr = "seed-relay.example.com:4443";
    let punch_addr = "punch.example.com:3478";
    let session_id = SessionId::new("sess_reset_usage");

    let state = ControlState::new_sqlite(token_secret, relay_addr, punch_addr, &db_path).unwrap();
    let auth = state
        .register_user(RegisterUserRequest {
            email: "reset-usage@example.com".to_string(),
            password: "password-123".to_string(),
            display_name: "Reset Usage".to_string(),
        })
        .unwrap();
    state
        .register_relay(RegisterRelayRequest {
            relay_id: "relay_reset_usage".to_string(),
            relay_addr: "relay-reset-usage.example.com:4443".to_string(),
            admin_addr: "relay-reset-usage.example.com:9090".to_string(),
            capacity_streams: 32,
        })
        .unwrap();
    state
        .register_device_for_user(&auth.user_id, device("server_reset_usage"))
        .unwrap();
    state
        .register_services_for_user(
            &auth.user_id,
            vec![service("server_reset_usage", "svc_reset_usage")],
        )
        .unwrap();
    state
        .add_agent_session(AgentSessionAssignment {
            session_id: session_id.clone(),
            user_id: auth.user_id.clone(),
            device_id: DeviceId::new("server_reset_usage"),
            service_id: ServiceId::new("svc_reset_usage"),
            client_id: ClientId::new("phone_reset_usage"),
            relay_token: String::new(),
            relay_addr: "relay-reset-usage.example.com:4443".to_string(),
            punch_addr: punch_addr.to_string(),
            expire_at: 4_102_444_800,
            status: AgentSessionStatus::Bound,
            grant_id: None,
            grant_revocation_version: None,
            grant_service_id: None,
        })
        .unwrap();
    state
        .report_relay_session_usage(ReportRelaySessionUsageRequest {
            relay_id: "relay_reset_usage".to_string(),
            sessions: vec![RelaySessionUsageReport {
                session_id,
                stats: TrafficStats {
                    session_id: None,
                    uplink_bytes: 21,
                    downlink_bytes: 34,
                    total_bytes: 55,
                    duration_sec: 8,
                    active_streams: 0,
                },
            }],
        })
        .unwrap();
    let period = state.reset_user_usage_period(&auth.user_id).unwrap();
    drop(state);

    let restored =
        ControlState::new_sqlite(token_secret, relay_addr, punch_addr, &db_path).unwrap();
    let summary = restored
        .user_usage_summaries()
        .into_iter()
        .find(|summary| summary.user_id == auth.user_id)
        .unwrap();

    assert_eq!(
        summary.current_period_started_epoch_sec,
        period.current_period_started_epoch_sec
    );
    assert_eq!(summary.actual_uplink_bytes, 0);
    assert_eq!(summary.actual_downlink_bytes, 0);
    assert_eq!(summary.actual_total_bytes, 0);

    let _ = std::fs::remove_file(db_path);
}

#[test]
fn sqlite_store_restores_audit_logs_after_restart() {
    let db_path = temp_db_path("control-audit-persistence");
    let token_secret = "dev-secret";
    let relay_addr = "seed-relay.example.com:4443";
    let punch_addr = "punch.example.com:3478";

    let state = ControlState::new_sqlite(token_secret, relay_addr, punch_addr, &db_path).unwrap();
    let admin_token = state.issue_admin_token("admin@example.com").unwrap();
    let claims = TokenSigner::new(TokenKey::new(token_secret))
        .verify_control(&admin_token, 1_767_000_000)
        .unwrap();
    let first = state
        .record_audit_log(
            &claims,
            "user.create",
            "user",
            "user_audit",
            "created user user_audit",
        )
        .unwrap();
    let second = state
        .record_audit_log(
            &claims,
            "relay_credential.rotate",
            "relay",
            "relay_audit",
            "rotated relay credential",
        )
        .unwrap();
    drop(state);

    let restored =
        ControlState::new_sqlite(token_secret, relay_addr, punch_addr, &db_path).unwrap();
    let logs = restored.audit_logs();

    assert_eq!(logs.len(), 2);
    assert_eq!(logs[0].audit_id, second.audit_id);
    assert_eq!(logs[0].actor_subject, "admin@example.com");
    assert_eq!(logs[0].action, "relay_credential.rotate");
    assert_eq!(logs[0].target_type, "relay");
    assert_eq!(logs[0].target_id, "relay_audit");
    assert_eq!(logs[1].audit_id, first.audit_id);

    let _ = std::fs::remove_file(db_path);
}

#[test]
fn sqlite_store_restores_controller_removal_after_restart() {
    let db_path = temp_db_path("control-controller-persistence");
    let token_secret = "dev-secret";
    let relay_addr = "seed-relay.example.com:4443";
    let punch_addr = "punch.example.com:3478";

    let state = ControlState::new_sqlite(token_secret, relay_addr, punch_addr, &db_path).unwrap();
    let auth = state
        .register_user(RegisterUserRequest {
            email: "controllers@example.com".to_string(),
            password: "password-123".to_string(),
            display_name: "Controller Owner".to_string(),
        })
        .unwrap();
    for client_id in ["phone_001", "laptop_001"] {
        state
            .register_controller(
                &auth.user_id,
                RegisterControllerDeviceRequest {
                    client_id: client_id.to_string(),
                    name: client_id.to_string(),
                },
            )
            .unwrap();
    }
    state.remove_controller(&auth.user_id, "phone_001").unwrap();
    drop(state);

    let restored =
        ControlState::new_sqlite(token_secret, relay_addr, punch_addr, &db_path).unwrap();
    let controllers = restored.controllers_for_user(&auth.user_id);

    assert_eq!(controllers.len(), 1);
    assert_eq!(controllers[0].client_id.as_str(), "laptop_001");

    let _ = std::fs::remove_file(db_path);
}

#[test]
fn sqlite_store_restores_controlled_device_removal_after_restart() {
    let db_path = temp_db_path("control-device-persistence");
    let token_secret = "dev-secret";
    let relay_addr = "seed-relay.example.com:4443";
    let punch_addr = "punch.example.com:3478";

    let state = ControlState::new_sqlite(token_secret, relay_addr, punch_addr, &db_path).unwrap();
    let auth = state
        .register_user(RegisterUserRequest {
            email: "devices@example.com".to_string(),
            password: "password-123".to_string(),
            display_name: "Device Owner".to_string(),
        })
        .unwrap();
    state
        .register_device_for_user(&auth.user_id, device("server_001"))
        .unwrap();
    state
        .register_services_for_user(&auth.user_id, vec![service("server_001", "svc_web")])
        .unwrap();
    state
        .register_p2p_certificate(DeviceId::new("server_001"), vec![1, 2, 3])
        .unwrap();
    state
        .remove_device_for_user(&auth.user_id, &DeviceId::new("server_001"))
        .unwrap();
    drop(state);

    let restored =
        ControlState::new_sqlite(token_secret, relay_addr, punch_addr, &db_path).unwrap();

    assert!(restored.devices_for_user(&auth.user_id).is_empty());
    assert!(restored
        .services_for_device_for_user(&auth.user_id, &DeviceId::new("server_001"))
        .is_empty());
    assert!(restored
        .p2p_certificate_for_device(&DeviceId::new("server_001"))
        .is_none());

    let _ = std::fs::remove_file(db_path);
}

#[test]
fn sqlite_store_restores_device_access_grants_after_restart() {
    let db_path = temp_db_path("control-device-access-persistence");
    let token_secret = "dev-secret";
    let relay_addr = "seed-relay.example.com:4443";
    let punch_addr = "punch.example.com:3478";

    let state = ControlState::new_sqlite(token_secret, relay_addr, punch_addr, &db_path).unwrap();
    let owner = state
        .register_user(RegisterUserRequest {
            email: "device-owner@example.com".to_string(),
            password: "password-123".to_string(),
            display_name: "Device Owner".to_string(),
        })
        .unwrap();
    let grantee = state
        .register_user(RegisterUserRequest {
            email: "device-grantee@example.com".to_string(),
            password: "password-123".to_string(),
            display_name: "Device Grantee".to_string(),
        })
        .unwrap();
    let device_id = DeviceId::new("server_granted");
    state
        .register_device_for_user(&owner.user_id, device(device_id.as_str()))
        .unwrap();
    state
        .register_services_for_user(&owner.user_id, vec![service(device_id.as_str(), "svc_web")])
        .unwrap();
    state
        .grant_device_access(
            &device_id,
            GrantDeviceAccessRequest {
                user_id: grantee.user_id.clone(),
            },
        )
        .unwrap();
    drop(state);

    let restored =
        ControlState::new_sqlite(token_secret, relay_addr, punch_addr, &db_path).unwrap();
    let grants = restored.device_access_grants(&device_id).unwrap();
    assert_eq!(grants.len(), 1);
    assert_eq!(grants[0].user_id, grantee.user_id);
    let devices = restored.devices_for_user(&grantee.user_id);
    assert_eq!(devices.len(), 1);
    assert_eq!(devices[0].device_id, device_id);
    assert_eq!(
        restored.services_for_device_for_user(&grantee.user_id, &device_id),
        vec![service(device_id.as_str(), "svc_web")]
    );

    restored
        .remove_device_for_user(&owner.user_id, &device_id)
        .unwrap();
    assert!(restored.devices_for_user(&grantee.user_id).is_empty());
    assert!(restored.device_access_grants(&device_id).is_err());

    let _ = std::fs::remove_file(db_path);
}
