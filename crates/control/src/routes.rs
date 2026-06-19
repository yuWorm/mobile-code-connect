use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{
        header::{ACCEPT, AUTHORIZATION, CACHE_CONTROL, CONTENT_TYPE, LOCATION},
        HeaderMap, HeaderValue, StatusCode,
    },
    response::{Html, IntoResponse},
    routing::{delete, get, post},
    Json, Router,
};
use mobilecode_connect_auth::{ControlRole, ControlTokenClaims};
use mobilecode_connect_control_client::{
    AdminListQuery, AdminSessionSummary, ApproveMobilePairingRequest, AssignUserPlanRequest,
    AuditLogEntry, AuthResponse, BrowserServerAuthExchangeRequest, BrowserServerAuthStartResponse,
    ControllerDevice, CreateRelayBootstrapRequest, CreateRelayCredentialRequest, DashboardSummary,
    DenyMobileGrantRequest, DeviceAccessGrant, DeviceServerAuthPollResponse,
    DeviceServerAuthStartResponse, GrantDeviceAccessRequest, GrantSessionPollResponse,
    LoginRequest, MobilePairingPollResponse, OAuthIdentity, OAuthProvider, Page,
    PendingGrantSessionRequest, PendingMobilePairingRequest, Plan, PollServerAuthRequest,
    RegisterControllerDeviceRequest, RegisterRelayRequest, RegisterUserRequest,
    RelayBootstrapExchangeRequest, RelayBootstrapExchangeResponse, RelayBootstrapResponse,
    RelayCommand, RelayCredential, RelayNode, RelaySessionSnapshot,
    ReportRelayCommandResultRequest, ReportRelayHealthRequest, ReportRelaySessionUsageRequest,
    ServerAuthSessionDetail, ServerCredentialResponse, ServerCredentialSummary,
    StartGrantSessionResponse, StartMobilePairingResponse, StartServerAuthRequest,
    UpdatePasswordRequest, UpdatePlanCatalogRequest, UpdateRelayCredentialStatusRequest,
    UpdateRelayRequest, UpdateServerCredentialStatusRequest, UpdateUserPlanRequest,
    UpdateUserRoleRequest, UpdateUserStatusRequest, UserDetail, UserSummary, UserUsagePeriod,
    UserUsageSummary,
};
use mobilecode_connect_protocol::{
    Device, DeviceId, GrantSessionRequest, MobilePairingRequest, Service, SessionId, UserId,
};
use serde::Deserialize;
use std::path::PathBuf;

use crate::{
    session::{
        AgentSessionAssignment, CreateSessionRequest, CreateSessionResponse,
        RegisterP2pCertificateRequest,
    },
    state::{ControlAuthError, ControlPlaneError, ControlSessionError, ControlState},
};

const RELAYD_INSTALLER_SCRIPT: &str = include_str!("../../../scripts/install-relayd.sh");

include!(concat!(env!("OUT_DIR"), "/embedded_web.rs"));

pub fn routes(state: ControlState) -> Router {
    Router::new()
        .route("/", get(web_index_page))
        .route("/admin", get(control_admin_page))
        .route("/admin/", get(control_admin_page))
        .route("/admin/{*path}", get(web_spa_page))
        .route("/center", get(web_index_page))
        .route("/center/", get(web_index_page))
        .route("/center/{*path}", get(web_spa_page))
        .route("/login", get(web_index_page))
        .route("/login/", get(web_index_page))
        .route("/login/{*path}", get(web_spa_page))
        .route("/assets/{*path}", get(web_asset))
        .route("/install-relayd.sh", get(relayd_installer_script))
        .route("/relayd", get(relayd_binary))
        .route("/auth/register", post(register_user))
        .route("/auth/login", post(login))
        .route("/auth/password", post(update_password))
        .route("/auth/oauth/github/start", get(start_github_oauth))
        .route("/auth/oauth/github/callback", get(github_oauth_callback))
        .route("/oauth/identities", get(list_oauth_identities))
        .route(
            "/oauth/identities/{provider}/{provider_user_id}",
            get(get_oauth_identity).delete(unlink_oauth_identity),
        )
        .route(
            "/server-auth/browser/start",
            post(start_browser_server_auth),
        )
        .route(
            "/server-auth/browser/session",
            get(browser_server_auth_session_detail),
        )
        .route(
            "/server-auth/browser/approve",
            get(approve_browser_server_auth),
        )
        .route(
            "/server-auth/browser/exchange",
            post(exchange_browser_server_auth),
        )
        .route("/server-auth/device/start", post(start_device_server_auth))
        .route(
            "/server-auth/device/session",
            get(device_server_auth_session_detail),
        )
        .route("/server-auth/device", get(approve_device_server_auth))
        .route("/server-auth/device/poll", post(poll_device_server_auth))
        .route("/server-credentials", get(list_server_credentials))
        .route(
            "/server-credentials/{credential_id}",
            get(get_server_credential),
        )
        .route(
            "/server-credentials/{credential_id}/status",
            post(update_server_credential_status),
        )
        .route(
            "/server-credentials/{credential_id}/rotate",
            post(rotate_server_credential),
        )
        .route("/dashboard", get(dashboard_summary))
        .route("/audit-logs", get(list_audit_logs))
        .route("/usage/users", get(list_user_usage))
        .route(
            "/usage/users/{user_id}/reset",
            post(reset_user_usage_period),
        )
        .route("/usage/relay-sessions", post(report_relay_session_usage))
        .route("/controllers", get(list_controllers))
        .route("/controllers/register", post(register_controller))
        .route("/controllers/{client_id}", delete(remove_controller))
        .route("/users", get(list_users).post(create_user))
        .route("/users/{user_id}", get(get_user))
        .route("/users/{user_id}/status", post(update_user_status))
        .route("/users/{user_id}/role", post(update_user_role))
        .route("/devices", get(list_controlled_devices))
        .route(
            "/devices/{device_id}/access",
            get(list_device_access_grants).post(grant_device_access),
        )
        .route(
            "/devices/{device_id}/access/{user_id}",
            delete(revoke_device_access),
        )
        .route(
            "/devices/{device_id}",
            get(get_controlled_device).delete(remove_controlled_device),
        )
        .route("/plans/current", get(current_plan))
        .route(
            "/plans/catalog",
            get(list_plan_catalog).post(update_plan_catalog),
        )
        .route("/plans/catalog/{plan_id}", get(get_catalog_plan))
        .route(
            "/plans/users/{user_id}",
            get(user_plan).post(update_user_plan),
        )
        .route("/plans/users/{user_id}/assign", post(assign_user_plan))
        .route(
            "/relay-credentials",
            get(list_relay_credentials).post(create_relay_credential),
        )
        .route("/relay-credentials/{relay_id}", get(get_relay_credential))
        .route(
            "/relay-credentials/{relay_id}/status",
            post(update_relay_credential_status),
        )
        .route(
            "/relay-credentials/{relay_id}/rotate",
            post(rotate_relay_credential),
        )
        .route("/relay-bootstraps", post(create_relay_bootstrap))
        .route(
            "/relay-bootstraps/{bootstrap_id}/exchange",
            post(exchange_relay_bootstrap),
        )
        .route("/relays/register", post(register_relay))
        .route("/relays", get(list_relays))
        .route("/relays/{relay_id}/health", post(report_relay_health))
        .route("/relays/{relay_id}/sessions", get(list_relay_sessions))
        .route(
            "/relays/{relay_id}/sessions/{session_id}/disconnect",
            post(disconnect_relay_session),
        )
        .route("/relays/{relay_id}/commands", get(list_relay_commands))
        .route(
            "/relays/{relay_id}/commands/{command_id}/result",
            post(report_relay_command_result),
        )
        .route(
            "/relays/{relay_id}",
            get(get_relay).post(update_relay).delete(remove_relay),
        )
        .route("/agent/register", post(register_device))
        .route("/agent/services", post(register_services))
        .route(
            "/agent/devices/{device_id}/p2p-cert",
            post(register_p2p_certificate),
        )
        .route(
            "/agent/devices/{device_id}/pairing-requests",
            get(list_mobile_pairing_requests),
        )
        .route(
            "/agent/devices/{device_id}/grant-session-requests",
            get(list_grant_session_requests),
        )
        .route(
            "/agent/devices/{device_id}/sessions",
            get(list_agent_sessions),
        )
        .route(
            "/agent/pairing/{pending_pairing_id}/approve",
            post(approve_mobile_pairing),
        )
        .route(
            "/agent/pairing/{pending_pairing_id}/deny",
            post(deny_mobile_pairing),
        )
        .route(
            "/agent/grant-sessions/{pending_session_id}/approve",
            post(approve_grant_session),
        )
        .route(
            "/agent/grant-sessions/{pending_session_id}/deny",
            post(deny_grant_session),
        )
        .route(
            "/agent/sessions/{session_id}/claim",
            post(claim_agent_session),
        )
        .route(
            "/agent/sessions/{session_id}/bound",
            post(mark_agent_session_bound),
        )
        .route("/mobile/devices", get(list_devices))
        .route(
            "/mobile/devices/{device_id}/services",
            get(list_device_services),
        )
        .route("/agent-grants/pairing/start", post(start_mobile_pairing))
        .route(
            "/agent-grants/pairing/{pending_pairing_id}",
            get(mobile_pairing_result),
        )
        .route("/agent-grants/sessions/start", post(start_grant_session))
        .route(
            "/agent-grants/sessions/{pending_session_id}",
            get(grant_session_result),
        )
        .route("/sessions", get(list_admin_sessions).post(create_session))
        .route("/sessions/{session_id}/close", post(close_session))
        .with_state(state)
}

async fn control_admin_page() -> impl IntoResponse {
    web_index_response()
}

async fn web_index_page() -> impl IntoResponse {
    web_index_response()
}

async fn web_spa_page(Path(path): Path<String>) -> impl IntoResponse {
    if let Some(asset) = embedded_web_asset(&path) {
        return embedded_web_response(asset);
    }
    web_index_response()
}

async fn web_asset(Path(path): Path<String>) -> impl IntoResponse {
    let asset_path = format!("assets/{path}");
    match embedded_web_asset(&asset_path) {
        Some(asset) => embedded_web_response(asset),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

fn web_index_response() -> axum::response::Response {
    if embedded_web_available() {
        if let Some(asset) = embedded_web_asset("index.html") {
            return embedded_web_response(asset);
        }
    }
    Html(include_str!("../../../docs/control-admin.html")).into_response()
}

fn is_html_navigation_without_auth(headers: &HeaderMap) -> bool {
    !headers.contains_key(AUTHORIZATION) && accepts_html(headers)
}

fn accepts_html(headers: &HeaderMap) -> bool {
    headers
        .get(ACCEPT)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.contains("text/html"))
        .unwrap_or(false)
}

fn embedded_web_response(asset: EmbeddedWebAsset) -> axum::response::Response {
    let mut response = Body::from(asset.bytes).into_response();
    let headers = response.headers_mut();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static(asset.content_type));
    headers.insert(CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    response
}

async fn relayd_installer_script() -> impl IntoResponse {
    (
        [("content-type", "text/x-shellscript; charset=utf-8")],
        RELAYD_INSTALLER_SCRIPT,
    )
}

async fn relayd_binary() -> impl IntoResponse {
    let Some(path) = relayd_binary_path() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    match tokio::fs::read(path).await {
        Ok(bytes) => (
            [
                ("content-type", "application/octet-stream"),
                ("content-disposition", "attachment; filename=\"relayd\""),
            ],
            bytes,
        )
            .into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

fn relayd_binary_path() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("MOBILECODE_CONNECT_RELAYD_BINARY")
        .or_else(|_| std::env::var("QUIC_TUNNEL_RELAYD_BINARY"))
    {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Some(path);
        }
    }

    let exe_name = if cfg!(windows) {
        "relayd.exe"
    } else {
        "relayd"
    };
    let current_exe = std::env::current_exe().ok()?;
    let current_dir = current_exe.parent()?;
    for dir in [Some(current_dir), current_dir.parent()]
        .into_iter()
        .flatten()
    {
        let candidate = dir.join(exe_name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

#[derive(Deserialize)]
struct OAuthStartQuery {
    redirect_uri: Option<String>,
}

#[derive(Deserialize)]
struct OAuthCallbackQuery {
    code: String,
    state: String,
}

#[derive(Deserialize)]
struct BrowserServerAuthApproveQuery {
    session_id: String,
}

#[derive(Deserialize)]
struct DeviceServerAuthApproveQuery {
    user_code: String,
    decision: Option<String>,
}

async fn dashboard_summary(
    State(state): State<ControlState>,
    headers: HeaderMap,
) -> Result<Json<DashboardSummary>, StatusCode> {
    admin_from_headers(&state, &headers)?;
    Ok(Json(state.dashboard_summary()))
}

async fn register_user(
    State(state): State<ControlState>,
    Json(request): Json<RegisterUserRequest>,
) -> Result<Json<AuthResponse>, StatusCode> {
    state
        .register_user(request)
        .map(Json)
        .map_err(auth_error_status)
}

async fn login(
    State(state): State<ControlState>,
    Json(request): Json<LoginRequest>,
) -> Result<Json<AuthResponse>, StatusCode> {
    state.login(request).map(Json).map_err(auth_error_status)
}

async fn update_password(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Json(request): Json<UpdatePasswordRequest>,
) -> Result<StatusCode, StatusCode> {
    let actor = logged_in_human_claims_from_headers(&state, &headers)?;
    state
        .update_password(&actor, request)
        .map_err(auth_error_status)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn start_github_oauth(
    State(state): State<ControlState>,
    Query(query): Query<OAuthStartQuery>,
) -> Result<impl IntoResponse, StatusCode> {
    let start = state
        .start_github_oauth(query.redirect_uri)
        .map_err(auth_error_status)?;
    let mut headers = HeaderMap::new();
    headers.insert(
        LOCATION,
        HeaderValue::from_str(&start.authorization_url)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
    );
    Ok((StatusCode::FOUND, headers))
}

async fn github_oauth_callback(
    State(state): State<ControlState>,
    Query(query): Query<OAuthCallbackQuery>,
) -> Result<Json<AuthResponse>, StatusCode> {
    state
        .github_oauth_callback(&query.code, &query.state)
        .await
        .map(Json)
        .map_err(auth_error_status)
}

async fn list_oauth_identities(
    State(state): State<ControlState>,
    Query(query): Query<AdminListQuery>,
    headers: HeaderMap,
) -> Result<Json<Page<OAuthIdentity>>, StatusCode> {
    let actor = logged_in_human_claims_from_headers(&state, &headers)?;
    let identities = if actor.role == ControlRole::Admin {
        state.oauth_identities()
    } else {
        state.oauth_identities_for_user(&actor.user_id)
    };
    Ok(Json(page_vec(
        filter_sort_oauth_identities(identities, &query),
        &query,
    )))
}

async fn get_oauth_identity(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Path((provider, provider_user_id)): Path<(String, String)>,
) -> Result<Json<OAuthIdentity>, StatusCode> {
    let actor = logged_in_human_claims_from_headers(&state, &headers)?;
    let provider = parse_oauth_provider_path(&provider)?;
    state
        .oauth_identity_for_actor(&actor, provider, &provider_user_id)
        .map(Json)
        .map_err(control_plane_error_status)
}

async fn unlink_oauth_identity(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Path((provider, provider_user_id)): Path<(String, String)>,
) -> Result<StatusCode, StatusCode> {
    let actor = logged_in_human_claims_from_headers(&state, &headers)?;
    let provider = parse_oauth_provider_path(&provider)?;
    state
        .unlink_oauth_identity(&actor, provider, &provider_user_id)
        .map_err(control_plane_error_status)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn start_browser_server_auth(
    State(state): State<ControlState>,
    Json(request): Json<StartServerAuthRequest>,
) -> Result<Json<BrowserServerAuthStartResponse>, StatusCode> {
    state
        .start_browser_server_auth(request)
        .map(Json)
        .map_err(control_plane_error_status)
}

async fn browser_server_auth_session_detail(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Query(query): Query<BrowserServerAuthApproveQuery>,
) -> Result<Json<ServerAuthSessionDetail>, StatusCode> {
    let _user_id = logged_in_user_id_from_headers(&state, &headers)?;
    state
        .browser_server_auth_session_detail(&query.session_id)
        .map(Json)
        .map_err(control_plane_error_status)
}

async fn approve_browser_server_auth(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Query(query): Query<BrowserServerAuthApproveQuery>,
) -> axum::response::Response {
    if is_html_navigation_without_auth(&headers) {
        return web_index_response();
    }
    let user_id = match logged_in_user_id_from_headers(&state, &headers) {
        Ok(user_id) => user_id,
        Err(status) => return status.into_response(),
    };
    match state.approve_browser_server_auth(&query.session_id, &user_id) {
        Ok(response) => Json(response).into_response(),
        Err(error) => control_plane_error_status(error).into_response(),
    }
}

async fn exchange_browser_server_auth(
    State(state): State<ControlState>,
    Json(request): Json<BrowserServerAuthExchangeRequest>,
) -> Result<Json<ServerCredentialResponse>, StatusCode> {
    state
        .exchange_browser_server_auth(request)
        .map(Json)
        .map_err(control_plane_error_status)
}

async fn start_device_server_auth(
    State(state): State<ControlState>,
    Json(request): Json<StartServerAuthRequest>,
) -> Result<Json<DeviceServerAuthStartResponse>, StatusCode> {
    state
        .start_device_server_auth(request)
        .map(Json)
        .map_err(control_plane_error_status)
}

async fn device_server_auth_session_detail(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Query(query): Query<DeviceServerAuthApproveQuery>,
) -> Result<Json<ServerAuthSessionDetail>, StatusCode> {
    let _user_id = logged_in_user_id_from_headers(&state, &headers)?;
    state
        .device_server_auth_session_detail(&query.user_code)
        .map(Json)
        .map_err(control_plane_error_status)
}

async fn approve_device_server_auth(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Query(query): Query<DeviceServerAuthApproveQuery>,
) -> axum::response::Response {
    if is_html_navigation_without_auth(&headers) {
        return web_index_response();
    }
    let user_id = match logged_in_user_id_from_headers(&state, &headers) {
        Ok(user_id) => user_id,
        Err(status) => return status.into_response(),
    };
    let deny = query
        .decision
        .as_deref()
        .map(|decision| matches!(decision.trim(), "deny" | "denied"))
        .unwrap_or(false);
    match state.approve_device_server_auth(&query.user_code, &user_id, deny) {
        Ok(response) => Json(response).into_response(),
        Err(error) => control_plane_error_status(error).into_response(),
    }
}

async fn poll_device_server_auth(
    State(state): State<ControlState>,
    Json(request): Json<PollServerAuthRequest>,
) -> Result<Json<DeviceServerAuthPollResponse>, StatusCode> {
    state
        .poll_device_server_auth(request)
        .map(Json)
        .map_err(control_plane_error_status)
}

async fn list_server_credentials(
    State(state): State<ControlState>,
    Query(query): Query<AdminListQuery>,
    headers: HeaderMap,
) -> Result<Json<Page<ServerCredentialSummary>>, StatusCode> {
    let actor = logged_in_human_claims_from_headers(&state, &headers)?;
    let credentials = if actor.role == ControlRole::Admin {
        state.server_credentials()
    } else {
        state.server_credentials_for_user(&actor.user_id)
    };
    Ok(Json(page_vec(
        filter_sort_server_credentials(credentials, &query),
        &query,
    )))
}

async fn get_server_credential(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Path(credential_id): Path<String>,
) -> Result<Json<ServerCredentialSummary>, StatusCode> {
    let actor = logged_in_human_claims_from_headers(&state, &headers)?;
    state
        .server_credential_for_actor(&actor, &credential_id)
        .map(Json)
        .map_err(control_plane_error_status)
}

async fn update_server_credential_status(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Path(credential_id): Path<String>,
    Json(request): Json<UpdateServerCredentialStatusRequest>,
) -> Result<Json<ServerCredentialSummary>, StatusCode> {
    let actor = logged_in_human_claims_from_headers(&state, &headers)?;
    state
        .update_server_credential_status(&actor, &credential_id, request)
        .map(Json)
        .map_err(control_plane_error_status)
}

async fn rotate_server_credential(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Path(credential_id): Path<String>,
) -> Result<Json<ServerCredentialResponse>, StatusCode> {
    let actor = logged_in_human_claims_from_headers(&state, &headers)?;
    state
        .rotate_server_credential(&actor, &credential_id)
        .map(Json)
        .map_err(control_plane_error_status)
}

async fn list_audit_logs(
    State(state): State<ControlState>,
    Query(query): Query<AdminListQuery>,
    headers: HeaderMap,
) -> Result<Json<Page<AuditLogEntry>>, StatusCode> {
    admin_from_headers(&state, &headers)?;
    Ok(Json(page_vec(
        filter_sort_audit_logs(state.audit_logs(), &query),
        &query,
    )))
}

async fn list_user_usage(
    State(state): State<ControlState>,
    Query(query): Query<AdminListQuery>,
    headers: HeaderMap,
) -> Result<Json<Page<UserUsageSummary>>, StatusCode> {
    admin_from_headers(&state, &headers)?;
    Ok(Json(page_vec(
        filter_sort_usage(state.user_usage_summaries(), &query),
        &query,
    )))
}

async fn reset_user_usage_period(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Path(user_id): Path<String>,
) -> Result<Json<UserUsagePeriod>, StatusCode> {
    let actor = admin_claims_from_headers(&state, &headers)?;
    let user_id = UserId::new(user_id);
    let period = state
        .reset_user_usage_period(&user_id)
        .map_err(control_plane_error_status)?;
    state
        .record_audit_log(
            &actor,
            "usage.user.reset",
            "user",
            user_id.to_string(),
            format!(
                "reset usage period to {}",
                period.current_period_started_epoch_sec
            ),
        )
        .map_err(control_plane_error_status)?;
    Ok(Json(period))
}

async fn list_admin_sessions(
    State(state): State<ControlState>,
    Query(query): Query<AdminListQuery>,
    headers: HeaderMap,
) -> Result<Json<Page<AdminSessionSummary>>, StatusCode> {
    admin_from_headers(&state, &headers)?;
    Ok(Json(page_vec(
        filter_sort_sessions(state.admin_session_summaries(), &query),
        &query,
    )))
}

async fn report_relay_session_usage(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Json(request): Json<ReportRelaySessionUsageRequest>,
) -> Result<StatusCode, StatusCode> {
    relay_writer_from_headers(&state, &headers, &request.relay_id)?;
    state
        .report_relay_session_usage(request)
        .map_err(control_plane_error_status)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn register_controller(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Json(request): Json<RegisterControllerDeviceRequest>,
) -> Result<Json<ControllerDevice>, StatusCode> {
    let user_id = user_id_from_headers(&state, &headers)?;
    state
        .register_controller(&user_id, request)
        .map(Json)
        .map_err(control_plane_error_status)
}

async fn list_controllers(
    State(state): State<ControlState>,
    Query(query): Query<AdminListQuery>,
    headers: HeaderMap,
) -> Result<Json<Page<ControllerDevice>>, StatusCode> {
    let user_id = user_id_from_headers(&state, &headers)?;
    Ok(Json(page_vec(
        filter_sort_controllers(state.controllers_for_user(&user_id), &query),
        &query,
    )))
}

async fn remove_controller(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Path(client_id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    let user_id = user_id_from_headers(&state, &headers)?;
    state
        .remove_controller(&user_id, &client_id)
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(control_plane_error_status)
}

async fn list_users(
    State(state): State<ControlState>,
    Query(query): Query<AdminListQuery>,
    headers: HeaderMap,
) -> Result<Json<Page<UserSummary>>, StatusCode> {
    admin_from_headers(&state, &headers)?;
    Ok(Json(page_vec(
        filter_sort_users(state.users(), &query),
        &query,
    )))
}

async fn create_user(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Json(request): Json<mobilecode_connect_control_client::CreateUserRequest>,
) -> Result<Json<UserSummary>, StatusCode> {
    let actor = admin_claims_from_headers(&state, &headers)?;
    let user = state
        .create_user(request)
        .map_err(control_plane_error_status)?;
    state
        .record_audit_log(
            &actor,
            "user.create",
            "user",
            user.user_id.to_string(),
            format!("created user {}", user.email),
        )
        .map_err(control_plane_error_status)?;
    Ok(Json(user))
}

async fn get_user(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Path(user_id): Path<String>,
) -> Result<Json<UserDetail>, StatusCode> {
    admin_from_headers(&state, &headers)?;
    state
        .user_detail(&UserId::new(user_id))
        .map(Json)
        .map_err(control_plane_error_status)
}

async fn update_user_status(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Path(user_id): Path<String>,
    Json(request): Json<UpdateUserStatusRequest>,
) -> Result<Json<UserSummary>, StatusCode> {
    let actor = admin_claims_from_headers(&state, &headers)?;
    let user = state
        .update_user_status(&UserId::new(user_id), request)
        .map_err(control_plane_error_status)?;
    state
        .record_audit_log(
            &actor,
            "user.status.update",
            "user",
            user.user_id.to_string(),
            format!("set user enabled={}", user.enabled),
        )
        .map_err(control_plane_error_status)?;
    Ok(Json(user))
}

async fn update_user_role(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Path(user_id): Path<String>,
    Json(request): Json<UpdateUserRoleRequest>,
) -> Result<Json<UserSummary>, StatusCode> {
    let actor = admin_claims_from_headers(&state, &headers)?;
    let user = state
        .update_user_role(&UserId::new(user_id), request)
        .map_err(control_plane_error_status)?;
    state
        .record_audit_log(
            &actor,
            "user.role.update",
            "user",
            user.user_id.to_string(),
            format!("set user role={:?}", user.role),
        )
        .map_err(control_plane_error_status)?;
    Ok(Json(user))
}

async fn list_controlled_devices(
    State(state): State<ControlState>,
    Query(query): Query<AdminListQuery>,
    headers: HeaderMap,
) -> Result<Json<Page<Device>>, StatusCode> {
    let user_id = user_id_from_headers(&state, &headers)?;
    Ok(Json(page_vec(
        filter_sort_devices(state.devices_for_user(&user_id), &query),
        &query,
    )))
}

async fn get_controlled_device(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Path(device_id): Path<String>,
) -> Result<Json<Device>, StatusCode> {
    let user_id = user_id_from_headers(&state, &headers)?;
    state
        .device_for_user(&user_id, &DeviceId::new(device_id))
        .map(Json)
        .map_err(control_plane_error_status)
}

async fn list_device_access_grants(
    State(state): State<ControlState>,
    Query(query): Query<AdminListQuery>,
    headers: HeaderMap,
    Path(device_id): Path<String>,
) -> Result<Json<Page<DeviceAccessGrant>>, StatusCode> {
    admin_from_headers(&state, &headers)?;
    let grants = state
        .device_access_grants(&DeviceId::new(device_id))
        .map_err(control_plane_error_status)?;
    Ok(Json(page_vec(
        filter_sort_device_access_grants(grants, &query),
        &query,
    )))
}

async fn grant_device_access(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Path(device_id): Path<String>,
    Json(request): Json<GrantDeviceAccessRequest>,
) -> Result<Json<DeviceAccessGrant>, StatusCode> {
    let actor = admin_claims_from_headers(&state, &headers)?;
    let grant = state
        .grant_device_access(&DeviceId::new(device_id), request)
        .map_err(control_plane_error_status)?;
    state
        .record_audit_log(
            &actor,
            "device.access.grant",
            "device",
            grant.device_id.to_string(),
            format!("granted device access to {}", grant.user_id),
        )
        .map_err(control_plane_error_status)?;
    Ok(Json(grant))
}

async fn revoke_device_access(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Path((device_id, user_id)): Path<(String, String)>,
) -> Result<StatusCode, StatusCode> {
    let actor = admin_claims_from_headers(&state, &headers)?;
    let device_id = DeviceId::new(device_id);
    let user_id = UserId::new(user_id);
    state
        .revoke_device_access(&device_id, &user_id)
        .map_err(control_plane_error_status)?;
    state
        .record_audit_log(
            &actor,
            "device.access.revoke",
            "device",
            device_id.to_string(),
            format!("revoked device access from {user_id}"),
        )
        .map_err(control_plane_error_status)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn remove_controlled_device(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Path(device_id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    let user_id = user_id_from_headers(&state, &headers)?;
    state
        .remove_device_for_user(&user_id, &DeviceId::new(device_id))
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(control_plane_error_status)
}

async fn current_plan(
    State(state): State<ControlState>,
    headers: HeaderMap,
) -> Result<Json<Plan>, StatusCode> {
    let user_id = user_id_from_headers(&state, &headers)?;
    Ok(Json(state.plan_for_user(&user_id)))
}

async fn list_plan_catalog(
    State(state): State<ControlState>,
    Query(query): Query<AdminListQuery>,
    headers: HeaderMap,
) -> Result<Json<Page<Plan>>, StatusCode> {
    admin_from_headers(&state, &headers)?;
    Ok(Json(page_vec(
        filter_sort_plans(state.plan_catalog(), &query),
        &query,
    )))
}

async fn get_catalog_plan(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Path(plan_id): Path<String>,
) -> Result<Json<Plan>, StatusCode> {
    admin_from_headers(&state, &headers)?;
    state
        .catalog_plan(&plan_id)
        .map(Json)
        .map_err(control_plane_error_status)
}

async fn update_plan_catalog(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Json(request): Json<UpdatePlanCatalogRequest>,
) -> Result<Json<Plan>, StatusCode> {
    let actor = admin_claims_from_headers(&state, &headers)?;
    let plan = state
        .update_catalog_plan(request)
        .map_err(control_plane_error_status)?;
    state
        .record_audit_log(
            &actor,
            "plan.catalog.update",
            "plan",
            plan.plan_id.clone(),
            format!("updated plan template {}", plan.name),
        )
        .map_err(control_plane_error_status)?;
    Ok(Json(plan))
}

async fn user_plan(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Path(user_id): Path<String>,
) -> Result<Json<Plan>, StatusCode> {
    admin_from_headers(&state, &headers)?;
    state
        .managed_plan_for_user(&UserId::new(user_id))
        .map(Json)
        .map_err(control_plane_error_status)
}

async fn update_user_plan(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Path(user_id): Path<String>,
    Json(request): Json<UpdateUserPlanRequest>,
) -> Result<Json<Plan>, StatusCode> {
    let actor = admin_claims_from_headers(&state, &headers)?;
    let user_id = UserId::new(user_id);
    let plan = state
        .update_user_plan(&user_id, request)
        .map_err(control_plane_error_status)?;
    state
        .record_audit_log(
            &actor,
            "plan.user.update",
            "user",
            user_id.to_string(),
            format!("updated user plan {}", plan.plan_id),
        )
        .map_err(control_plane_error_status)?;
    Ok(Json(plan))
}

async fn assign_user_plan(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Path(user_id): Path<String>,
    Json(request): Json<AssignUserPlanRequest>,
) -> Result<Json<Plan>, StatusCode> {
    let actor = admin_claims_from_headers(&state, &headers)?;
    let user_id = UserId::new(user_id);
    let plan = state
        .assign_user_plan(&user_id, request)
        .map_err(control_plane_error_status)?;
    state
        .record_audit_log(
            &actor,
            "plan.user.assign",
            "user",
            user_id.to_string(),
            format!("assigned user plan {}", plan.plan_id),
        )
        .map_err(control_plane_error_status)?;
    Ok(Json(plan))
}

async fn list_relay_credentials(
    State(state): State<ControlState>,
    Query(query): Query<AdminListQuery>,
    headers: HeaderMap,
) -> Result<Json<Page<RelayCredential>>, StatusCode> {
    admin_from_headers(&state, &headers)?;
    Ok(Json(page_vec(
        filter_sort_relay_credentials(state.relay_credentials(), &query),
        &query,
    )))
}

async fn create_relay_credential(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Json(request): Json<CreateRelayCredentialRequest>,
) -> Result<Json<RelayCredential>, StatusCode> {
    let actor = admin_claims_from_headers(&state, &headers)?;
    let credential = state
        .create_relay_credential(request)
        .map_err(control_plane_error_status)?;
    state
        .record_audit_log(
            &actor,
            "relay_credential.create",
            "relay",
            credential.relay_id.clone(),
            format!(
                "created relay credential enabled={} version={}",
                credential.enabled, credential.token_version
            ),
        )
        .map_err(control_plane_error_status)?;
    Ok(Json(credential))
}

async fn get_relay_credential(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Path(relay_id): Path<String>,
) -> Result<Json<RelayCredential>, StatusCode> {
    admin_from_headers(&state, &headers)?;
    state
        .relay_credential(&relay_id)
        .map(Json)
        .map_err(control_plane_error_status)
}

async fn update_relay_credential_status(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Path(relay_id): Path<String>,
    Json(request): Json<UpdateRelayCredentialStatusRequest>,
) -> Result<Json<RelayCredential>, StatusCode> {
    let actor = admin_claims_from_headers(&state, &headers)?;
    let credential = state
        .update_relay_credential_status(&relay_id, request)
        .map_err(control_plane_error_status)?;
    state
        .record_audit_log(
            &actor,
            "relay_credential.status.update",
            "relay",
            credential.relay_id.clone(),
            format!("set relay credential enabled={}", credential.enabled),
        )
        .map_err(control_plane_error_status)?;
    Ok(Json(credential))
}

async fn rotate_relay_credential(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Path(relay_id): Path<String>,
) -> Result<Json<RelayCredential>, StatusCode> {
    let actor = admin_claims_from_headers(&state, &headers)?;
    let credential = state
        .rotate_relay_credential(&relay_id)
        .map_err(control_plane_error_status)?;
    state
        .record_audit_log(
            &actor,
            "relay_credential.rotate",
            "relay",
            credential.relay_id.clone(),
            format!(
                "rotated relay credential to version {}",
                credential.token_version
            ),
        )
        .map_err(control_plane_error_status)?;
    Ok(Json(credential))
}

async fn create_relay_bootstrap(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Json(request): Json<CreateRelayBootstrapRequest>,
) -> Result<Json<RelayBootstrapResponse>, StatusCode> {
    let actor = admin_claims_from_headers(&state, &headers)?;
    let bootstrap = state
        .create_relay_bootstrap(&actor, request)
        .map_err(control_plane_error_status)?;
    state
        .record_audit_log(
            &actor,
            "relay_bootstrap.create",
            "relay",
            bootstrap.relay_id.clone(),
            format!(
                "created relay bootstrap {} exp={}",
                bootstrap.bootstrap_id, bootstrap.expires_epoch_sec
            ),
        )
        .map_err(control_plane_error_status)?;
    Ok(Json(bootstrap))
}

async fn exchange_relay_bootstrap(
    State(state): State<ControlState>,
    Path(bootstrap_id): Path<String>,
    Json(request): Json<RelayBootstrapExchangeRequest>,
) -> Result<Json<RelayBootstrapExchangeResponse>, StatusCode> {
    state
        .exchange_relay_bootstrap(&bootstrap_id, request)
        .map(Json)
        .map_err(control_plane_error_status)
}

async fn register_relay(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Json(request): Json<RegisterRelayRequest>,
) -> Result<Json<RelayNode>, StatusCode> {
    let actor = relay_writer_from_headers(&state, &headers, &request.relay_id)?;
    let relay = state
        .register_relay(request)
        .map_err(control_plane_error_status)?;
    if actor.role == ControlRole::Admin {
        state
            .record_audit_log(
                &actor,
                "relay.register",
                "relay",
                relay.relay_id.clone(),
                format!("registered relay {}", relay.relay_addr),
            )
            .map_err(control_plane_error_status)?;
    }
    Ok(Json(relay))
}

async fn list_relays(
    State(state): State<ControlState>,
    Query(query): Query<AdminListQuery>,
    headers: HeaderMap,
) -> Result<Json<Page<RelayNode>>, StatusCode> {
    admin_from_headers(&state, &headers)?;
    Ok(Json(page_vec(
        filter_sort_relays(state.relays(), &query),
        &query,
    )))
}

async fn get_relay(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Path(relay_id): Path<String>,
) -> Result<Json<RelayNode>, StatusCode> {
    admin_from_headers(&state, &headers)?;
    state
        .relay(&relay_id)
        .map(Json)
        .map_err(control_plane_error_status)
}

async fn update_relay(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Path(relay_id): Path<String>,
    Json(request): Json<UpdateRelayRequest>,
) -> Result<Json<RelayNode>, StatusCode> {
    let actor = relay_writer_from_headers(&state, &headers, &relay_id)?;
    let relay = state
        .update_relay(&relay_id, request)
        .map_err(control_plane_error_status)?;
    if actor.role == ControlRole::Admin {
        state
            .record_audit_log(
                &actor,
                "relay.update",
                "relay",
                relay.relay_id.clone(),
                format!("updated relay healthy={}", relay.healthy),
            )
            .map_err(control_plane_error_status)?;
    }
    Ok(Json(relay))
}

async fn report_relay_health(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Path(relay_id): Path<String>,
    Json(request): Json<ReportRelayHealthRequest>,
) -> Result<Json<RelayNode>, StatusCode> {
    let actor = relay_writer_from_headers(&state, &headers, &relay_id)?;
    let relay = state
        .report_relay_health(&relay_id, request)
        .map_err(control_plane_error_status)?;
    if actor.role == ControlRole::Admin {
        state
            .record_audit_log(
                &actor,
                "relay.health",
                "relay",
                relay.relay_id.clone(),
                format!("reported relay health={:?}", relay.health_status),
            )
            .map_err(control_plane_error_status)?;
    }
    Ok(Json(relay))
}

async fn list_relay_sessions(
    State(state): State<ControlState>,
    Query(query): Query<AdminListQuery>,
    headers: HeaderMap,
    Path(relay_id): Path<String>,
) -> Result<Json<Page<RelaySessionSnapshot>>, StatusCode> {
    admin_from_headers(&state, &headers)?;
    Ok(Json(page_vec(
        filter_sort_relay_sessions(
            state
                .relay_sessions(&relay_id)
                .map_err(control_plane_error_status)?,
            &query,
        ),
        &query,
    )))
}

async fn disconnect_relay_session(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Path((relay_id, session_id)): Path<(String, String)>,
) -> Result<Json<RelayCommand>, StatusCode> {
    let actor = admin_claims_from_headers(&state, &headers)?;
    let session_id = SessionId::new(session_id);
    let command = state
        .request_relay_session_disconnect(&relay_id, &session_id)
        .map_err(control_plane_error_status)?;
    state
        .record_audit_log(
            &actor,
            "relay.command.disconnect_session",
            "relay",
            relay_id,
            format!("queued disconnect for relay session {session_id}"),
        )
        .map_err(control_plane_error_status)?;
    Ok(Json(command))
}

async fn list_relay_commands(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Path(relay_id): Path<String>,
) -> Result<Json<Vec<RelayCommand>>, StatusCode> {
    relay_writer_from_headers(&state, &headers, &relay_id)?;
    state
        .pending_relay_commands(&relay_id)
        .map(Json)
        .map_err(control_plane_error_status)
}

async fn report_relay_command_result(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Path((relay_id, command_id)): Path<(String, String)>,
    Json(request): Json<ReportRelayCommandResultRequest>,
) -> Result<Json<RelayCommand>, StatusCode> {
    relay_writer_from_headers(&state, &headers, &relay_id)?;
    state
        .report_relay_command_result(&relay_id, &command_id, request)
        .map(Json)
        .map_err(control_plane_error_status)
}

async fn remove_relay(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Path(relay_id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    let actor = admin_claims_from_headers(&state, &headers)?;
    state
        .remove_relay(&relay_id)
        .map_err(control_plane_error_status)?;
    state
        .record_audit_log(&actor, "relay.delete", "relay", relay_id, "deleted relay")
        .map_err(control_plane_error_status)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn register_device(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Json(device): Json<Device>,
) -> Result<(), StatusCode> {
    let user_id = agent_route_user_for_device_from_headers(&state, &headers, &device.device_id)?;
    state
        .register_device_for_user(&user_id, device)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(())
}

async fn register_services(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Json(services): Json<Vec<Service>>,
) -> Result<(), StatusCode> {
    let claims = control_claims_from_headers(&state, &headers)?;
    if claims.role == ControlRole::Relay {
        return Err(StatusCode::FORBIDDEN);
    }
    if claims.role == ControlRole::Agent {
        let device_id = state
            .agent_credential_device_id(&claims)
            .ok_or(StatusCode::FORBIDDEN)?;
        if services
            .iter()
            .any(|service| service.device_id != device_id)
        {
            return Err(StatusCode::FORBIDDEN);
        }
    }
    let user_id = claims.user_id;
    state
        .register_services_for_user(&user_id, services)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(())
}

async fn register_p2p_certificate(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Path(device_id): Path<String>,
    Json(request): Json<RegisterP2pCertificateRequest>,
) -> Result<(), StatusCode> {
    let device_id = DeviceId::new(device_id);
    let _ = agent_route_user_for_device_from_headers(&state, &headers, &device_id)?;
    state
        .register_p2p_certificate(device_id, request.certificate_der)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(())
}

async fn start_mobile_pairing(
    State(state): State<ControlState>,
    Json(request): Json<MobilePairingRequest>,
) -> Result<Json<StartMobilePairingResponse>, StatusCode> {
    state
        .start_mobile_pairing(request)
        .map(Json)
        .map_err(control_plane_error_status)
}

async fn mobile_pairing_result(
    State(state): State<ControlState>,
    Path(pending_pairing_id): Path<String>,
) -> Result<Json<MobilePairingPollResponse>, StatusCode> {
    state
        .mobile_pairing_result(&pending_pairing_id)
        .map(Json)
        .map_err(control_plane_error_status)
}

async fn list_mobile_pairing_requests(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Path(device_id): Path<String>,
) -> Result<Json<Vec<PendingMobilePairingRequest>>, StatusCode> {
    let claims = control_claims_from_headers(&state, &headers)?;
    let device_id = DeviceId::new(device_id);
    authorize_agent_device_access(&state, &claims, &device_id)?;
    state
        .pending_mobile_pairings_for_device(&device_id)
        .map(Json)
        .map_err(control_plane_error_status)
}

async fn approve_mobile_pairing(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Path(pending_pairing_id): Path<String>,
    Json(request): Json<ApproveMobilePairingRequest>,
) -> Result<Json<MobilePairingPollResponse>, StatusCode> {
    let claims = control_claims_from_headers(&state, &headers)?;
    let device_id = state
        .pending_mobile_pairing_device_id(&pending_pairing_id)
        .ok_or(StatusCode::NOT_FOUND)?;
    authorize_agent_device_access(&state, &claims, &device_id)?;
    state
        .approve_mobile_pairing(&pending_pairing_id, request)
        .map(Json)
        .map_err(control_plane_error_status)
}

async fn deny_mobile_pairing(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Path(pending_pairing_id): Path<String>,
    Json(request): Json<DenyMobileGrantRequest>,
) -> Result<Json<MobilePairingPollResponse>, StatusCode> {
    let claims = control_claims_from_headers(&state, &headers)?;
    let device_id = state
        .pending_mobile_pairing_device_id(&pending_pairing_id)
        .ok_or(StatusCode::NOT_FOUND)?;
    authorize_agent_device_access(&state, &claims, &device_id)?;
    state
        .deny_mobile_pairing(&pending_pairing_id, request)
        .map(Json)
        .map_err(control_plane_error_status)
}

async fn start_grant_session(
    State(state): State<ControlState>,
    Json(request): Json<GrantSessionRequest>,
) -> Result<Json<StartGrantSessionResponse>, StatusCode> {
    state
        .start_grant_session(request)
        .map(Json)
        .map_err(control_plane_error_status)
}

async fn grant_session_result(
    State(state): State<ControlState>,
    Path(pending_session_id): Path<String>,
) -> Result<Json<GrantSessionPollResponse>, StatusCode> {
    state
        .grant_session_result(&pending_session_id)
        .map(Json)
        .map_err(control_plane_error_status)
}

async fn list_grant_session_requests(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Path(device_id): Path<String>,
) -> Result<Json<Vec<PendingGrantSessionRequest>>, StatusCode> {
    let claims = control_claims_from_headers(&state, &headers)?;
    let device_id = DeviceId::new(device_id);
    authorize_agent_device_access(&state, &claims, &device_id)?;
    state
        .pending_grant_sessions_for_device(&device_id)
        .map(Json)
        .map_err(control_plane_error_status)
}

async fn approve_grant_session(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Path(pending_session_id): Path<String>,
    Json(_request): Json<mobilecode_connect_control_client::ApproveGrantSessionRequest>,
) -> Result<Json<GrantSessionPollResponse>, StatusCode> {
    let claims = control_claims_from_headers(&state, &headers)?;
    let device_id = state
        .pending_grant_session_device_id(&pending_session_id)
        .ok_or(StatusCode::NOT_FOUND)?;
    authorize_agent_device_access(&state, &claims, &device_id)?;
    state
        .approve_grant_session(&pending_session_id)
        .map(Json)
        .map_err(control_plane_error_status)
}

async fn deny_grant_session(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Path(pending_session_id): Path<String>,
    Json(request): Json<DenyMobileGrantRequest>,
) -> Result<Json<GrantSessionPollResponse>, StatusCode> {
    let claims = control_claims_from_headers(&state, &headers)?;
    let device_id = state
        .pending_grant_session_device_id(&pending_session_id)
        .ok_or(StatusCode::NOT_FOUND)?;
    authorize_agent_device_access(&state, &claims, &device_id)?;
    state
        .deny_grant_session(&pending_session_id, request)
        .map(Json)
        .map_err(control_plane_error_status)
}

async fn list_agent_sessions(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Path(device_id): Path<String>,
) -> Result<Json<Vec<AgentSessionAssignment>>, StatusCode> {
    let claims = control_claims_from_headers(&state, &headers)?;
    let device_id = DeviceId::new(device_id);
    authorize_agent_device_access(&state, &claims, &device_id)?;
    Ok(Json(state.agent_sessions_for_device(&device_id)))
}

async fn claim_agent_session(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Path(session_id): Path<String>,
) -> Result<Json<AgentSessionAssignment>, StatusCode> {
    let session_id = SessionId::new(session_id);
    let claims = control_claims_from_headers(&state, &headers)?;
    authorize_agent_session_access(&state, &claims, &session_id)?;
    state
        .claim_agent_session(&session_id)
        .map(Json)
        .map_err(session_error_status)
}

async fn mark_agent_session_bound(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Path(session_id): Path<String>,
) -> Result<Json<AgentSessionAssignment>, StatusCode> {
    let session_id = SessionId::new(session_id);
    let claims = control_claims_from_headers(&state, &headers)?;
    authorize_agent_session_access(&state, &claims, &session_id)?;
    state
        .mark_agent_session_bound(&session_id)
        .map(Json)
        .map_err(session_error_status)
}

async fn close_session(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Path(session_id): Path<String>,
) -> Result<Json<AgentSessionAssignment>, StatusCode> {
    let session_id = SessionId::new(session_id);
    let claims = control_claims_from_headers(&state, &headers)?;
    authorize_session_close_access(&state, &claims, &session_id)?;
    state
        .close_session(&session_id)
        .map(Json)
        .map_err(session_error_status)
}

async fn list_devices(
    State(state): State<ControlState>,
    headers: HeaderMap,
) -> Result<Json<Vec<Device>>, StatusCode> {
    let user_id = user_id_from_headers(&state, &headers)?;
    Ok(Json(state.devices_for_user(&user_id)))
}

async fn list_device_services(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Path(device_id): Path<String>,
) -> Result<Json<Vec<Service>>, StatusCode> {
    let user_id = user_id_from_headers(&state, &headers)?;
    let device_id = DeviceId::new(device_id);
    if !state.user_can_access_device(&user_id, &device_id) {
        if !state.device_exists(&device_id) {
            return Ok(Json(Vec::new()));
        }
        return Err(StatusCode::FORBIDDEN);
    }
    Ok(Json(
        state.services_for_device_for_user(&user_id, &device_id),
    ))
}

async fn create_session(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Json(request): Json<CreateSessionRequest>,
) -> Result<Json<CreateSessionResponse>, StatusCode> {
    let user_id = user_id_from_headers(&state, &headers)?;
    state
        .create_user_session(&user_id, request)
        .map(Json)
        .map_err(control_plane_error_status)
}

fn user_id_from_headers(
    state: &ControlState,
    headers: &HeaderMap,
) -> Result<mobilecode_connect_protocol::UserId, StatusCode> {
    let claims = control_claims_from_headers(state, headers)?;
    if matches!(claims.role, ControlRole::Relay | ControlRole::Agent) {
        return Err(StatusCode::FORBIDDEN);
    }
    Ok(claims.user_id)
}

fn agent_route_user_for_device_from_headers(
    state: &ControlState,
    headers: &HeaderMap,
    device_id: &DeviceId,
) -> Result<mobilecode_connect_protocol::UserId, StatusCode> {
    let claims = control_claims_from_headers(state, headers)?;
    match claims.role {
        ControlRole::Admin => Ok(claims.user_id),
        ControlRole::Agent
            if state
                .agent_credential_device_id(&claims)
                .map(|credential_device_id| credential_device_id == *device_id)
                .unwrap_or(false) =>
        {
            Ok(claims.user_id)
        }
        ControlRole::User => Ok(claims.user_id),
        ControlRole::Relay | ControlRole::Agent => Err(StatusCode::FORBIDDEN),
    }
}

fn logged_in_user_id_from_headers(
    state: &ControlState,
    headers: &HeaderMap,
) -> Result<mobilecode_connect_protocol::UserId, StatusCode> {
    if !headers.contains_key("authorization") {
        return Err(StatusCode::UNAUTHORIZED);
    }
    user_id_from_headers(state, headers)
}

fn logged_in_human_claims_from_headers(
    state: &ControlState,
    headers: &HeaderMap,
) -> Result<ControlTokenClaims, StatusCode> {
    if !headers.contains_key("authorization") {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let claims = control_claims_from_headers(state, headers)?;
    if matches!(claims.role, ControlRole::Relay | ControlRole::Agent) {
        return Err(StatusCode::FORBIDDEN);
    }
    Ok(claims)
}

fn parse_oauth_provider_path(provider: &str) -> Result<OAuthProvider, StatusCode> {
    match provider.trim().to_ascii_lowercase().as_str() {
        "github" => Ok(OAuthProvider::GitHub),
        _ => Err(StatusCode::BAD_REQUEST),
    }
}

fn oauth_provider_path(provider: OAuthProvider) -> &'static str {
    match provider {
        OAuthProvider::GitHub => "github",
    }
}

fn control_claims_from_headers(
    state: &ControlState,
    headers: &HeaderMap,
) -> Result<ControlTokenClaims, StatusCode> {
    let bearer = headers
        .get("authorization")
        .and_then(|value| value.to_str().ok());
    state
        .control_claims_from_bearer(bearer)
        .map_err(auth_error_status)
}

fn admin_from_headers(state: &ControlState, headers: &HeaderMap) -> Result<(), StatusCode> {
    admin_claims_from_headers(state, headers).map(|_| ())
}

fn admin_claims_from_headers(
    state: &ControlState,
    headers: &HeaderMap,
) -> Result<ControlTokenClaims, StatusCode> {
    let claims = control_claims_from_headers(state, headers)?;
    if claims.role != ControlRole::Admin {
        return Err(StatusCode::FORBIDDEN);
    }
    Ok(claims)
}

fn relay_writer_from_headers(
    state: &ControlState,
    headers: &HeaderMap,
    relay_id: &str,
) -> Result<ControlTokenClaims, StatusCode> {
    let claims = control_claims_from_headers(state, headers)?;
    match claims.role {
        ControlRole::Admin => Ok(claims),
        ControlRole::Relay if claims.subject == relay_id && !relay_id.trim().is_empty() => {
            Ok(claims)
        }
        ControlRole::Relay | ControlRole::User | ControlRole::Agent => Err(StatusCode::FORBIDDEN),
    }
}

fn page_vec<T>(items: Vec<T>, query: &AdminListQuery) -> Page<T> {
    let total = items.len() as u64;
    let offset = query.offset.unwrap_or(0);
    let limit = query
        .limit
        .unwrap_or_else(|| total.min(u32::MAX as u64) as u32);
    let items = items
        .into_iter()
        .skip(offset as usize)
        .take(limit as usize)
        .collect();
    Page {
        items,
        total,
        limit,
        offset,
    }
}

fn query_matches(query: &AdminListQuery, fields: &[&str]) -> bool {
    let Some(q) = normalized_optional(query.q.as_deref()) else {
        return true;
    };
    fields
        .iter()
        .any(|field| field.to_ascii_lowercase().contains(&q))
}

fn normalized_optional(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_ascii_lowercase)
}

fn sort_key(query: &AdminListQuery) -> &str {
    query.sort.as_deref().map(str::trim).unwrap_or("")
}

fn filter_sort_users(mut users: Vec<UserSummary>, query: &AdminListQuery) -> Vec<UserSummary> {
    if let Some(role) = normalized_optional(query.role.as_deref()) {
        users.retain(|user| format!("{:?}", user.role).to_ascii_lowercase() == role);
    }
    if let Some(enabled) = query.enabled {
        users.retain(|user| user.enabled == enabled);
    }
    users.retain(|user| {
        query_matches(
            query,
            &[
                user.user_id.as_str(),
                &user.email,
                &user.display_name,
                &user.plan_id,
            ],
        )
    });
    match sort_key(query) {
        "role" => users.sort_by(|left, right| {
            format!("{:?}", left.role)
                .cmp(&format!("{:?}", right.role))
                .then_with(|| left.email.cmp(&right.email))
        }),
        "enabled" => users.sort_by(|left, right| {
            right
                .enabled
                .cmp(&left.enabled)
                .then_with(|| left.email.cmp(&right.email))
        }),
        "controller_count" => users.sort_by(|left, right| {
            right
                .controller_count
                .cmp(&left.controller_count)
                .then_with(|| left.email.cmp(&right.email))
        }),
        "device_count" => users.sort_by(|left, right| {
            right
                .device_count
                .cmp(&left.device_count)
                .then_with(|| left.email.cmp(&right.email))
        }),
        "email_desc" | "-email" => users.sort_by(|left, right| right.email.cmp(&left.email)),
        "email" | _ => users.sort_by(|left, right| left.email.cmp(&right.email)),
    }
    users
}

fn filter_sort_sessions(
    mut sessions: Vec<AdminSessionSummary>,
    query: &AdminListQuery,
) -> Vec<AdminSessionSummary> {
    if let Some(status) = normalized_optional(query.status.as_deref()) {
        sessions.retain(|session| format!("{:?}", session.status).to_ascii_lowercase() == status);
    }
    if let Some(user_id) = &query.user_id {
        sessions.retain(|session| &session.user_id == user_id);
    }
    if let Some(device_id) = &query.device_id {
        sessions.retain(|session| &session.device_id == device_id);
    }
    sessions.retain(|session| {
        query_matches(
            query,
            &[
                session.session_id.as_str(),
                session.user_id.as_str(),
                &session.user_email,
                session.device_id.as_str(),
                &session.device_name,
                session.service_id.as_str(),
                &session.service_name,
                session.client_id.as_str(),
                &session.relay_addr,
            ],
        )
    });
    match sort_key(query) {
        "status" => sessions.sort_by(|left, right| {
            format!("{:?}", left.status)
                .cmp(&format!("{:?}", right.status))
                .then_with(|| left.session_id.cmp(&right.session_id))
        }),
        "user_email" => sessions.sort_by(|left, right| {
            left.user_email
                .cmp(&right.user_email)
                .then_with(|| left.session_id.cmp(&right.session_id))
        }),
        "expire_at" | "-expire_at" => sessions.sort_by(|left, right| {
            right
                .expire_at
                .cmp(&left.expire_at)
                .then_with(|| left.session_id.cmp(&right.session_id))
        }),
        "session_id_desc" | "-session_id" => {
            sessions.sort_by(|left, right| right.session_id.cmp(&left.session_id))
        }
        "session_id" | _ => sessions.sort_by(|left, right| left.session_id.cmp(&right.session_id)),
    }
    sessions
}

fn filter_sort_usage(
    mut summaries: Vec<UserUsageSummary>,
    query: &AdminListQuery,
) -> Vec<UserUsageSummary> {
    if let Some(user_id) = &query.user_id {
        summaries.retain(|summary| &summary.user_id == user_id);
    }
    summaries.retain(|summary| {
        query_matches(
            query,
            &[summary.user_id.as_str(), &summary.email, &summary.plan_id],
        )
    });
    match sort_key(query) {
        "actual_total_bytes" | "-actual_total_bytes" => summaries.sort_by(|left, right| {
            right
                .actual_total_bytes
                .cmp(&left.actual_total_bytes)
                .then_with(|| left.email.cmp(&right.email))
        }),
        "relay_quota_granted_bytes" | "-relay_quota_granted_bytes" => {
            summaries.sort_by(|left, right| {
                right
                    .relay_quota_granted_bytes
                    .cmp(&left.relay_quota_granted_bytes)
                    .then_with(|| left.email.cmp(&right.email))
            })
        }
        "session_count" | "-session_count" => summaries.sort_by(|left, right| {
            right
                .session_count
                .cmp(&left.session_count)
                .then_with(|| left.email.cmp(&right.email))
        }),
        "email_desc" | "-email" => summaries.sort_by(|left, right| right.email.cmp(&left.email)),
        "email" | _ => summaries.sort_by(|left, right| left.email.cmp(&right.email)),
    }
    summaries
}

fn filter_sort_audit_logs(
    mut logs: Vec<AuditLogEntry>,
    query: &AdminListQuery,
) -> Vec<AuditLogEntry> {
    if let Some(action) = normalized_optional(query.action.as_deref()) {
        logs.retain(|log| log.action.to_ascii_lowercase() == action);
    }
    if let Some(target_type) = normalized_optional(query.target_type.as_deref()) {
        logs.retain(|log| log.target_type.to_ascii_lowercase() == target_type);
    }
    logs.retain(|log| {
        query_matches(
            query,
            &[
                &log.audit_id,
                log.actor_user_id.as_str(),
                &log.actor_subject,
                &log.action,
                &log.target_type,
                &log.target_id,
                &log.message,
            ],
        )
    });
    match sort_key(query) {
        "created_epoch_sec_asc" | "created_at_asc" => logs.sort_by(|left, right| {
            left.created_epoch_sec
                .cmp(&right.created_epoch_sec)
                .then_with(|| left.audit_id.cmp(&right.audit_id))
        }),
        "action" => logs.sort_by(|left, right| {
            left.action
                .cmp(&right.action)
                .then_with(|| right.created_epoch_sec.cmp(&left.created_epoch_sec))
        }),
        "created_epoch_sec" | "-created_epoch_sec" | _ => logs.sort_by(|left, right| {
            right
                .created_epoch_sec
                .cmp(&left.created_epoch_sec)
                .then_with(|| right.audit_id.cmp(&left.audit_id))
        }),
    }
    logs
}

fn filter_sort_relays(mut relays: Vec<RelayNode>, query: &AdminListQuery) -> Vec<RelayNode> {
    if let Some(healthy) = query.healthy {
        relays.retain(|relay| relay.healthy == healthy);
    }
    relays.retain(|relay| {
        query_matches(
            query,
            &[&relay.relay_id, &relay.relay_addr, &relay.admin_addr],
        )
    });
    match sort_key(query) {
        "healthy" => relays.sort_by(|left, right| {
            right
                .healthy
                .cmp(&left.healthy)
                .then_with(|| left.relay_id.cmp(&right.relay_id))
        }),
        "capacity_streams" | "-capacity_streams" => relays.sort_by(|left, right| {
            right
                .capacity_streams
                .cmp(&left.capacity_streams)
                .then_with(|| left.relay_id.cmp(&right.relay_id))
        }),
        "last_seen_epoch_sec" | "-last_seen_epoch_sec" => relays.sort_by(|left, right| {
            right
                .last_seen_epoch_sec
                .cmp(&left.last_seen_epoch_sec)
                .then_with(|| left.relay_id.cmp(&right.relay_id))
        }),
        "relay_id_desc" | "-relay_id" => {
            relays.sort_by(|left, right| right.relay_id.cmp(&left.relay_id))
        }
        "relay_id" | _ => relays.sort_by(|left, right| left.relay_id.cmp(&right.relay_id)),
    }
    relays
}

fn filter_sort_relay_sessions(
    mut sessions: Vec<RelaySessionSnapshot>,
    query: &AdminListQuery,
) -> Vec<RelaySessionSnapshot> {
    if let Some(status) = normalized_optional(query.status.as_deref()) {
        sessions.retain(|session| session.state.to_ascii_lowercase() == status);
    }
    sessions.retain(|session| {
        query_matches(
            query,
            &[
                session.session_id.as_str(),
                &session.state,
                session
                    .stats
                    .session_id
                    .as_ref()
                    .map(SessionId::as_str)
                    .unwrap_or(""),
            ],
        )
    });
    match sort_key(query) {
        "state" => sessions.sort_by(|left, right| {
            left.state
                .cmp(&right.state)
                .then_with(|| left.session_id.cmp(&right.session_id))
        }),
        "total_bytes" | "-total_bytes" => sessions.sort_by(|left, right| {
            right
                .stats
                .total_bytes
                .cmp(&left.stats.total_bytes)
                .then_with(|| left.session_id.cmp(&right.session_id))
        }),
        "last_seen_epoch_sec" | "-last_seen_epoch_sec" => sessions.sort_by(|left, right| {
            right
                .last_seen_epoch_sec
                .cmp(&left.last_seen_epoch_sec)
                .then_with(|| left.session_id.cmp(&right.session_id))
        }),
        "session_id_desc" | "-session_id" => {
            sessions.sort_by(|left, right| right.session_id.cmp(&left.session_id))
        }
        "session_id" | _ => sessions.sort_by(|left, right| left.session_id.cmp(&right.session_id)),
    }
    sessions
}

fn filter_sort_relay_credentials(
    mut credentials: Vec<RelayCredential>,
    query: &AdminListQuery,
) -> Vec<RelayCredential> {
    if let Some(enabled) = query.enabled {
        credentials.retain(|credential| credential.enabled == enabled);
    }
    credentials.retain(|credential| query_matches(query, &[&credential.relay_id]));
    match sort_key(query) {
        "enabled" => credentials.sort_by(|left, right| {
            right
                .enabled
                .cmp(&left.enabled)
                .then_with(|| left.relay_id.cmp(&right.relay_id))
        }),
        "token_version" | "-token_version" => credentials.sort_by(|left, right| {
            right
                .token_version
                .cmp(&left.token_version)
                .then_with(|| left.relay_id.cmp(&right.relay_id))
        }),
        "relay_id_desc" | "-relay_id" => {
            credentials.sort_by(|left, right| right.relay_id.cmp(&left.relay_id))
        }
        "relay_id" | _ => credentials.sort_by(|left, right| left.relay_id.cmp(&right.relay_id)),
    }
    credentials
}

fn filter_sort_server_credentials(
    mut credentials: Vec<ServerCredentialSummary>,
    query: &AdminListQuery,
) -> Vec<ServerCredentialSummary> {
    if let Some(user_id) = &query.user_id {
        credentials.retain(|credential| &credential.user_id == user_id);
    }
    if let Some(device_id) = &query.device_id {
        credentials.retain(|credential| &credential.device_id == device_id);
    }
    if let Some(enabled) = query.enabled {
        credentials.retain(|credential| credential.enabled == enabled);
    }
    credentials.retain(|credential| {
        query_matches(
            query,
            &[
                &credential.credential_id,
                credential.user_id.as_str(),
                credential.device_id.as_str(),
                &credential.device_name,
            ],
        )
    });
    match sort_key(query) {
        "device_name" => credentials.sort_by(|left, right| {
            left.device_name
                .cmp(&right.device_name)
                .then_with(|| left.credential_id.cmp(&right.credential_id))
        }),
        "device_id_desc" | "-device_id" => {
            credentials.sort_by(|left, right| right.device_id.cmp(&left.device_id))
        }
        "device_id" => credentials.sort_by(|left, right| {
            left.device_id
                .cmp(&right.device_id)
                .then_with(|| left.credential_id.cmp(&right.credential_id))
        }),
        "enabled" => credentials.sort_by(|left, right| {
            right
                .enabled
                .cmp(&left.enabled)
                .then_with(|| left.credential_id.cmp(&right.credential_id))
        }),
        "token_version" | "-token_version" => credentials.sort_by(|left, right| {
            right
                .token_version
                .cmp(&left.token_version)
                .then_with(|| left.credential_id.cmp(&right.credential_id))
        }),
        "last_used_epoch_sec" | "-last_used_epoch_sec" => credentials.sort_by(|left, right| {
            right
                .last_used_epoch_sec
                .unwrap_or(0)
                .cmp(&left.last_used_epoch_sec.unwrap_or(0))
                .then_with(|| left.credential_id.cmp(&right.credential_id))
        }),
        "created_epoch_sec" | "-created_epoch_sec" => credentials.sort_by(|left, right| {
            right
                .created_epoch_sec
                .cmp(&left.created_epoch_sec)
                .then_with(|| left.credential_id.cmp(&right.credential_id))
        }),
        "credential_id_desc" | "-credential_id" => {
            credentials.sort_by(|left, right| right.credential_id.cmp(&left.credential_id))
        }
        "credential_id" | _ => {
            credentials.sort_by(|left, right| left.credential_id.cmp(&right.credential_id))
        }
    }
    credentials
}

fn filter_sort_oauth_identities(
    mut identities: Vec<OAuthIdentity>,
    query: &AdminListQuery,
) -> Vec<OAuthIdentity> {
    if let Some(user_id) = &query.user_id {
        identities.retain(|identity| &identity.user_id == user_id);
    }
    identities.retain(|identity| {
        query_matches(
            query,
            &[
                oauth_provider_path(identity.provider),
                &identity.provider_user_id,
                identity.user_id.as_str(),
                &identity.email,
                &identity.login,
            ],
        )
    });
    match sort_key(query) {
        "email" => identities.sort_by(|left, right| {
            left.email
                .cmp(&right.email)
                .then_with(|| left.provider_user_id.cmp(&right.provider_user_id))
        }),
        "login" => identities.sort_by(|left, right| {
            left.login
                .cmp(&right.login)
                .then_with(|| left.provider_user_id.cmp(&right.provider_user_id))
        }),
        "user_id" => identities.sort_by(|left, right| {
            left.user_id
                .cmp(&right.user_id)
                .then_with(|| left.provider_user_id.cmp(&right.provider_user_id))
        }),
        "updated_epoch_sec" | "-updated_epoch_sec" => identities.sort_by(|left, right| {
            right
                .updated_epoch_sec
                .cmp(&left.updated_epoch_sec)
                .then_with(|| left.provider_user_id.cmp(&right.provider_user_id))
        }),
        "created_epoch_sec" | "-created_epoch_sec" => identities.sort_by(|left, right| {
            right
                .created_epoch_sec
                .cmp(&left.created_epoch_sec)
                .then_with(|| left.provider_user_id.cmp(&right.provider_user_id))
        }),
        "provider_user_id_desc" | "-provider_user_id" => {
            identities.sort_by(|left, right| right.provider_user_id.cmp(&left.provider_user_id))
        }
        "provider_user_id" | _ => {
            identities.sort_by(|left, right| left.provider_user_id.cmp(&right.provider_user_id))
        }
    }
    identities
}

fn filter_sort_controllers(
    mut controllers: Vec<ControllerDevice>,
    query: &AdminListQuery,
) -> Vec<ControllerDevice> {
    if let Some(user_id) = &query.user_id {
        controllers.retain(|controller| &controller.user_id == user_id);
    }
    controllers.retain(|controller| {
        query_matches(
            query,
            &[
                controller.user_id.as_str(),
                controller.client_id.as_str(),
                &controller.name,
            ],
        )
    });
    match sort_key(query) {
        "name" => controllers.sort_by(|left, right| {
            left.name
                .cmp(&right.name)
                .then_with(|| left.client_id.cmp(&right.client_id))
        }),
        "client_id_desc" | "-client_id" => {
            controllers.sort_by(|left, right| right.client_id.cmp(&left.client_id))
        }
        "client_id" | _ => controllers.sort_by(|left, right| left.client_id.cmp(&right.client_id)),
    }
    controllers
}

fn filter_sort_devices(mut devices: Vec<Device>, query: &AdminListQuery) -> Vec<Device> {
    if let Some(user_id) = &query.user_id {
        devices.retain(|device| &device.user_id == user_id);
    }
    if let Some(status) = normalized_optional(query.status.as_deref()) {
        devices.retain(|device| format!("{:?}", device.status).to_ascii_lowercase() == status);
    }
    devices.retain(|device| {
        query_matches(
            query,
            &[
                device.user_id.as_str(),
                device.device_id.as_str(),
                &device.name,
                &device.agent_version,
            ],
        )
    });
    match sort_key(query) {
        "name" => devices.sort_by(|left, right| {
            left.name
                .cmp(&right.name)
                .then_with(|| left.device_id.cmp(&right.device_id))
        }),
        "status" => devices.sort_by(|left, right| {
            format!("{:?}", left.status)
                .cmp(&format!("{:?}", right.status))
                .then_with(|| left.device_id.cmp(&right.device_id))
        }),
        "device_id_desc" | "-device_id" => {
            devices.sort_by(|left, right| right.device_id.cmp(&left.device_id))
        }
        "device_id" | _ => devices.sort_by(|left, right| left.device_id.cmp(&right.device_id)),
    }
    devices
}

fn filter_sort_device_access_grants(
    mut grants: Vec<DeviceAccessGrant>,
    query: &AdminListQuery,
) -> Vec<DeviceAccessGrant> {
    if let Some(user_id) = &query.user_id {
        grants.retain(|grant| &grant.user_id == user_id);
    }
    if let Some(device_id) = &query.device_id {
        grants.retain(|grant| &grant.device_id == device_id);
    }
    grants
        .retain(|grant| query_matches(query, &[grant.device_id.as_str(), grant.user_id.as_str()]));
    match sort_key(query) {
        "user_id_desc" | "-user_id" => {
            grants.sort_by(|left, right| right.user_id.cmp(&left.user_id))
        }
        "device_id" => grants.sort_by(|left, right| {
            left.device_id
                .cmp(&right.device_id)
                .then_with(|| left.user_id.cmp(&right.user_id))
        }),
        "device_id_desc" | "-device_id" => grants.sort_by(|left, right| {
            right
                .device_id
                .cmp(&left.device_id)
                .then_with(|| left.user_id.cmp(&right.user_id))
        }),
        "user_id" | _ => grants.sort_by(|left, right| left.user_id.cmp(&right.user_id)),
    }
    grants
}

fn filter_sort_plans(mut plans: Vec<Plan>, query: &AdminListQuery) -> Vec<Plan> {
    plans.retain(|plan| query_matches(query, &[&plan.plan_id, &plan.name]));
    match sort_key(query) {
        "name" => plans.sort_by(|left, right| {
            left.name
                .cmp(&right.name)
                .then_with(|| left.plan_id.cmp(&right.plan_id))
        }),
        "max_controller_devices" | "-max_controller_devices" => plans.sort_by(|left, right| {
            right
                .max_controller_devices
                .cmp(&left.max_controller_devices)
                .then_with(|| left.plan_id.cmp(&right.plan_id))
        }),
        "plan_id_desc" | "-plan_id" => {
            plans.sort_by(|left, right| right.plan_id.cmp(&left.plan_id))
        }
        "plan_id" | _ => plans.sort_by(|left, right| left.plan_id.cmp(&right.plan_id)),
    }
    plans
}

fn authorize_agent_device_access(
    state: &ControlState,
    claims: &ControlTokenClaims,
    device_id: &DeviceId,
) -> Result<(), StatusCode> {
    match claims.role {
        ControlRole::Admin => Ok(()),
        ControlRole::Agent
            if state
                .agent_credential_device_id(claims)
                .map(|credential_device_id| credential_device_id == *device_id)
                .unwrap_or(false) =>
        {
            Ok(())
        }
        ControlRole::User if state.device_belongs_to_user(&claims.user_id, device_id) => Ok(()),
        ControlRole::User | ControlRole::Relay | ControlRole::Agent => Err(StatusCode::FORBIDDEN),
    }
}

fn authorize_agent_session_access(
    state: &ControlState,
    claims: &ControlTokenClaims,
    session_id: &SessionId,
) -> Result<(), StatusCode> {
    let device_id = state
        .agent_session_device_id(session_id)
        .ok_or(StatusCode::NOT_FOUND)?;
    authorize_agent_device_access(state, claims, &device_id)
}

fn authorize_session_close_access(
    state: &ControlState,
    claims: &ControlTokenClaims,
    session_id: &SessionId,
) -> Result<(), StatusCode> {
    let device_id = state
        .agent_session_device_id(session_id)
        .ok_or(StatusCode::NOT_FOUND)?;
    match claims.role {
        ControlRole::Admin => Ok(()),
        ControlRole::Agent
            if state
                .agent_credential_device_id(claims)
                .map(|credential_device_id| credential_device_id == device_id)
                .unwrap_or(false) =>
        {
            Ok(())
        }
        ControlRole::User if state.device_belongs_to_user(&claims.user_id, &device_id) => Ok(()),
        ControlRole::User
            if state
                .agent_session_user_id(session_id)
                .map(|user_id| user_id == claims.user_id)
                .unwrap_or(false) =>
        {
            Ok(())
        }
        ControlRole::User | ControlRole::Relay | ControlRole::Agent => Err(StatusCode::FORBIDDEN),
    }
}

fn auth_error_status(error: ControlAuthError) -> StatusCode {
    match error {
        ControlAuthError::EmailAlreadyRegistered => StatusCode::CONFLICT,
        ControlAuthError::InvalidCredentials | ControlAuthError::InvalidToken => {
            StatusCode::UNAUTHORIZED
        }
        ControlAuthError::OAuthInvalidState => StatusCode::UNAUTHORIZED,
        ControlAuthError::OAuthEmailUnavailable => StatusCode::FORBIDDEN,
        ControlAuthError::OAuthNotConfigured => StatusCode::SERVICE_UNAVAILABLE,
        ControlAuthError::OAuthProviderFailed => StatusCode::BAD_GATEWAY,
        ControlAuthError::TokenIssueFailed | ControlAuthError::PersistenceFailed => {
            StatusCode::INTERNAL_SERVER_ERROR
        }
        ControlAuthError::InvalidInput => StatusCode::BAD_REQUEST,
    }
}

fn control_plane_error_status(error: ControlPlaneError) -> StatusCode {
    match error {
        ControlPlaneError::ControllerLimitExceeded => StatusCode::PAYMENT_REQUIRED,
        ControlPlaneError::RelayTrafficQuotaExceeded => StatusCode::PAYMENT_REQUIRED,
        ControlPlaneError::ControllerNotFound => StatusCode::NOT_FOUND,
        ControlPlaneError::UserNotFound => StatusCode::NOT_FOUND,
        ControlPlaneError::EmailAlreadyRegistered => StatusCode::CONFLICT,
        ControlPlaneError::PlanNotFound => StatusCode::NOT_FOUND,
        ControlPlaneError::RelayNotFound => StatusCode::NOT_FOUND,
        ControlPlaneError::RelayCredentialAlreadyExists => StatusCode::CONFLICT,
        ControlPlaneError::RelayCredentialNotFound => StatusCode::NOT_FOUND,
        ControlPlaneError::RelayBootstrapNotFound => StatusCode::NOT_FOUND,
        ControlPlaneError::RelayCommandNotFound => StatusCode::NOT_FOUND,
        ControlPlaneError::RelayBootstrapUnauthorized => StatusCode::UNAUTHORIZED,
        ControlPlaneError::DeviceNotFound => StatusCode::NOT_FOUND,
        ControlPlaneError::DeviceAccessGrantNotFound => StatusCode::NOT_FOUND,
        ControlPlaneError::NoRelayAvailable => StatusCode::SERVICE_UNAVAILABLE,
        ControlPlaneError::ServerAuthSessionNotFound => StatusCode::NOT_FOUND,
        ControlPlaneError::ServerAuthSessionNotReady => StatusCode::CONFLICT,
        ControlPlaneError::ServerAuthInvalidCode => StatusCode::UNAUTHORIZED,
        ControlPlaneError::ServerCredentialNotFound => StatusCode::NOT_FOUND,
        ControlPlaneError::OAuthIdentityNotFound => StatusCode::NOT_FOUND,
        ControlPlaneError::OAuthIdentityLastLoginMethod => StatusCode::CONFLICT,
        ControlPlaneError::InvalidInput => StatusCode::BAD_REQUEST,
        ControlPlaneError::TokenIssueFailed | ControlPlaneError::PersistenceFailed => {
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

fn session_error_status(error: ControlSessionError) -> StatusCode {
    match error {
        ControlSessionError::NotFound { .. } => StatusCode::NOT_FOUND,
        ControlSessionError::InvalidTransition { .. } => StatusCode::CONFLICT,
        ControlSessionError::PersistenceFailed => StatusCode::INTERNAL_SERVER_ERROR,
    }
}
