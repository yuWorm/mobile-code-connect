use async_trait::async_trait;
use axum::{
    body::{to_bytes, Body},
    http::{Method, Request, StatusCode},
};
use mobilecode_connect_auth::{ControlRole, TokenKey, TokenSigner};
use mobilecode_connect_control::{
    oauth::{
        GitHubOAuthClient, GitHubOAuthConfig, GitHubOAuthToken, GitHubUserProfile, OAuthError,
    },
    routes::routes,
    state::ControlState,
};
use mobilecode_connect_control_client::{
    AdminSessionSummary, ApproveGrantSessionRequest, ApproveMobilePairingRequest,
    ApprovedMobileGrantMetadata, AssignUserPlanRequest, AuditLogEntry, AuthResponse,
    BrowserServerAuthApprovalResponse, BrowserServerAuthExchangeRequest,
    BrowserServerAuthStartResponse, CreateRelayBootstrapRequest, CreateRelayCredentialRequest,
    CreateSessionResponse, CreateUserRequest, DashboardSummary, DeviceAccessGrant,
    DeviceServerAuthApprovalResponse, DeviceServerAuthPollResponse, DeviceServerAuthStartResponse,
    GrantDeviceAccessRequest, GrantSessionPollResponse, LoginRequest, MobilePairingPollResponse,
    OAuthIdentity, OAuthProvider, Page, PendingGrantSessionRequest, PendingMobilePairingRequest,
    Plan, PollServerAuthRequest, RegisterControllerDeviceRequest, RegisterRelayRequest,
    RegisterUserRequest, RelayBootstrapExchangeRequest, RelayBootstrapExchangeResponse,
    RelayBootstrapResponse, RelayCommand, RelayCommandKind, RelayCommandStatus, RelayCredential,
    RelayHealthReport, RelayHealthStatus, RelayNode, RelaySessionSnapshot, RelaySessionUsageReport,
    ReportRelayCommandResultRequest, ReportRelayHealthRequest, ReportRelaySessionUsageRequest,
    ServerAuthStatus, ServerCredentialResponse, ServerCredentialSummary, StartGrantSessionResponse,
    StartMobilePairingResponse, StartServerAuthRequest, UpdatePasswordRequest,
    UpdatePlanCatalogRequest, UpdateRelayCredentialStatusRequest, UpdateRelayRequest,
    UpdateServerCredentialStatusRequest, UpdateUserPlanRequest, UpdateUserRoleRequest,
    UpdateUserStatusRequest, UserDetail, UserSummary, UserUsagePeriod, UserUsageSummary,
};
use mobilecode_connect_protocol::{
    ClientId, Device, DeviceId, DeviceStatus, GrantSessionRequest, MobilePairingRequest,
    PendingGrantSessionStatus, PendingPairingStatus, RelayLimits, Service, ServiceId,
    ServiceProtocol, SessionId, TrafficStats, UserId,
};
use serde::de::DeserializeOwned;
use std::{
    fs,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use tower::ServiceExt;

#[derive(Clone)]
struct FakeGitHubOAuthClient {
    profile: GitHubUserProfile,
    verified_email: Option<String>,
}

#[async_trait]
impl GitHubOAuthClient for FakeGitHubOAuthClient {
    async fn exchange_code(
        &self,
        code: &str,
        _pkce_verifier: &str,
        _config: &GitHubOAuthConfig,
    ) -> Result<GitHubOAuthToken, OAuthError> {
        Ok(GitHubOAuthToken {
            access_token: format!("fake-token-{code}"),
        })
    }

    async fn user_profile(&self, _access_token: &str) -> Result<GitHubUserProfile, OAuthError> {
        Ok(self.profile.clone())
    }

    async fn primary_verified_email(&self, _access_token: &str) -> Result<String, OAuthError> {
        self.verified_email
            .clone()
            .ok_or(OAuthError::VerifiedEmailUnavailable)
    }
}

fn device(device_id: &str) -> Device {
    Device {
        device_id: DeviceId::new(device_id),
        user_id: UserId::new("wrong_user_should_be_ignored"),
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

async fn request_json<T: serde::Serialize>(
    app: axum::Router,
    method: Method,
    uri: &str,
    token: Option<&str>,
    payload: &T,
) -> axum::response::Response {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json");
    if let Some(token) = token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }

    app.oneshot(
        builder
            .body(Body::from(serde_json::to_vec(payload).unwrap()))
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn get(app: axum::Router, uri: &str, token: &str) -> axum::response::Response {
    app.oneshot(
        Request::builder()
            .method(Method::GET)
            .uri(uri)
            .header("authorization", format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn get_without_token(app: axum::Router, uri: &str) -> axum::response::Response {
    app.oneshot(
        Request::builder()
            .method(Method::GET)
            .uri(uri)
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn delete(app: axum::Router, uri: &str, token: &str) -> axum::response::Response {
    app.oneshot(
        Request::builder()
            .method(Method::DELETE)
            .uri(uri)
            .header("authorization", format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn post_empty(app: axum::Router, uri: &str, token: Option<&str>) -> axum::response::Response {
    let mut builder = Request::builder().method(Method::POST).uri(uri);
    if let Some(token) = token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }

    app.oneshot(builder.body(Body::empty()).unwrap())
        .await
        .unwrap()
}

async fn json<T: DeserializeOwned>(response: axum::response::Response) -> T {
    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    serde_json::from_slice(&body).unwrap()
}

async fn text(response: axum::response::Response) -> String {
    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    String::from_utf8(body.to_vec()).unwrap()
}

async fn register_user(app: axum::Router, email: &str) -> AuthResponse {
    let response = request_json(
        app,
        Method::POST,
        "/auth/register",
        None,
        &RegisterUserRequest {
            email: email.to_string(),
            password: "password-123".to_string(),
            display_name: "Test User".to_string(),
        },
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    json(response).await
}

fn admin_token(state: &ControlState) -> String {
    state.issue_admin_token("admin@example.com").unwrap()
}

fn relay_token(state: &ControlState, relay_id: &str) -> String {
    state.issue_relay_token(relay_id).unwrap()
}

fn github_oauth_config() -> GitHubOAuthConfig {
    GitHubOAuthConfig {
        public_url: "https://control.example.com".to_string(),
        client_id: "github-client-id".to_string(),
        client_secret: "github-client-secret".to_string(),
        redirect_url: None,
    }
}

fn fake_github_client(verified_email: Option<&str>) -> Arc<dyn GitHubOAuthClient> {
    Arc::new(FakeGitHubOAuthClient {
        profile: GitHubUserProfile {
            id: "123456".to_string(),
            login: "octocat".to_string(),
            name: Some("Octo Cat".to_string()),
            avatar_url: "https://avatars.githubusercontent.com/u/123456".to_string(),
        },
        verified_email: verified_email.map(str::to_string),
    })
}

fn query_value(location: &str, key: &str) -> String {
    let query = location
        .split_once('?')
        .map(|(_, query)| query)
        .unwrap_or("");
    for pair in query.split('&') {
        let Some((pair_key, value)) = pair.split_once('=') else {
            continue;
        };
        if pair_key == key {
            return value.to_string();
        }
    }
    panic!("missing query key {key} in {location}");
}

async fn start_device_server_auth(
    app: axum::Router,
    device_id: &str,
) -> DeviceServerAuthStartResponse {
    let response = request_json(
        app,
        Method::POST,
        "/server-auth/device/start",
        None,
        &StartServerAuthRequest {
            device_id: DeviceId::new(device_id),
            device_name: "Device Code Server".to_string(),
            server_public_key: "server-public-key-device".to_string(),
        },
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    json(response).await
}

async fn poll_device_server_auth(
    app: axum::Router,
    start: &DeviceServerAuthStartResponse,
    server_public_key: &str,
) -> DeviceServerAuthPollResponse {
    let response = request_json(
        app,
        Method::POST,
        "/server-auth/device/poll",
        None,
        &PollServerAuthRequest {
            device_code: start.device_code.clone(),
            server_public_key: server_public_key.to_string(),
        },
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    json(response).await
}

async fn issue_browser_server_credential(
    app: axum::Router,
    owner_token: &str,
    device_id: &str,
    device_name: &str,
    server_public_key: &str,
) -> ServerCredentialResponse {
    let start = request_json(
        app.clone(),
        Method::POST,
        "/server-auth/browser/start",
        None,
        &StartServerAuthRequest {
            device_id: DeviceId::new(device_id),
            device_name: device_name.to_string(),
            server_public_key: server_public_key.to_string(),
        },
    )
    .await;
    assert_eq!(start.status(), StatusCode::OK);
    let start: BrowserServerAuthStartResponse = json(start).await;
    let approval = get(
        app.clone(),
        &format!(
            "/server-auth/browser/approve?session_id={}",
            start.session_id
        ),
        owner_token,
    )
    .await;
    assert_eq!(approval.status(), StatusCode::OK);
    let approval: BrowserServerAuthApprovalResponse = json(approval).await;
    let exchange = request_json(
        app,
        Method::POST,
        "/server-auth/browser/exchange",
        None,
        &BrowserServerAuthExchangeRequest {
            session_id: start.session_id,
            server_auth_code: approval.server_auth_code,
            server_public_key: server_public_key.to_string(),
        },
    )
    .await;
    assert_eq!(exchange.status(), StatusCode::OK);
    json(exchange).await
}

#[tokio::test]
async fn github_oauth_start_redirects_to_github_when_configured() {
    let state = ControlState::new(
        "dev-secret",
        "relay.example.com:4443",
        "punch.example.com:3478",
    )
    .with_github_oauth_config(github_oauth_config());
    let app = routes(state);

    let response = get_without_token(app, "/auth/oauth/github/start").await;

    assert_eq!(response.status(), StatusCode::FOUND);
    let location = response
        .headers()
        .get("location")
        .and_then(|value| value.to_str().ok())
        .unwrap();
    assert!(location.starts_with("https://github.com/login/oauth/authorize?"));
    assert!(location.contains("client_id=github-client-id"));
    assert!(location.contains(
        "redirect_uri=https%3A%2F%2Fcontrol.example.com%2Fauth%2Foauth%2Fgithub%2Fcallback"
    ));
    assert!(location.contains("scope=read%3Auser%20user%3Aemail"));
    assert!(!query_value(location, "state").is_empty());
    assert!(!query_value(location, "code_challenge").is_empty());
}

#[tokio::test]
async fn github_oauth_callback_creates_and_reuses_user() {
    let state = ControlState::new(
        "dev-secret",
        "relay.example.com:4443",
        "punch.example.com:3478",
    )
    .with_github_oauth_config(github_oauth_config())
    .with_github_oauth_client(fake_github_client(Some("octocat@example.com")));
    let app = routes(state);

    let first_start = get_without_token(app.clone(), "/auth/oauth/github/start").await;
    assert_eq!(first_start.status(), StatusCode::FOUND);
    let first_location = first_start
        .headers()
        .get("location")
        .unwrap()
        .to_str()
        .unwrap();
    let first_state = query_value(first_location, "state");
    let first_callback = get_without_token(
        app.clone(),
        &format!("/auth/oauth/github/callback?code=first-code&state={first_state}"),
    )
    .await;
    assert_eq!(first_callback.status(), StatusCode::OK);
    let first_auth: AuthResponse = json(first_callback).await;

    let second_start = get_without_token(app.clone(), "/auth/oauth/github/start").await;
    assert_eq!(second_start.status(), StatusCode::FOUND);
    let second_location = second_start
        .headers()
        .get("location")
        .unwrap()
        .to_str()
        .unwrap();
    let second_state = query_value(second_location, "state");
    let second_callback = get_without_token(
        app,
        &format!("/auth/oauth/github/callback?code=second-code&state={second_state}"),
    )
    .await;
    assert_eq!(second_callback.status(), StatusCode::OK);
    let second_auth: AuthResponse = json(second_callback).await;

    assert_eq!(second_auth.user_id, first_auth.user_id);

    let claims = TokenSigner::new(TokenKey::new("dev-secret"))
        .verify_control(&first_auth.access_token, 1_767_000_000)
        .unwrap();
    assert_eq!(claims.role, ControlRole::User);
    assert_eq!(claims.subject, "octocat@example.com");
}

#[tokio::test]
async fn oauth_identities_are_listed_for_owner_and_admin() {
    let state = ControlState::new(
        "dev-secret",
        "relay.example.com:4443",
        "punch.example.com:3478",
    )
    .with_github_oauth_config(github_oauth_config())
    .with_github_oauth_client(fake_github_client(Some("octocat@example.com")));
    let admin_token = admin_token(&state);
    let app = routes(state);

    let start = get_without_token(app.clone(), "/auth/oauth/github/start").await;
    assert_eq!(start.status(), StatusCode::FOUND);
    let location = start.headers().get("location").unwrap().to_str().unwrap();
    let state = query_value(location, "state");
    let callback = get_without_token(
        app.clone(),
        &format!("/auth/oauth/github/callback?code=identity-code&state={state}"),
    )
    .await;
    assert_eq!(callback.status(), StatusCode::OK);
    let owner_auth: AuthResponse = json(callback).await;
    let other_auth = register_user(app.clone(), "oauth-other@example.com").await;

    let owner_list: Page<OAuthIdentity> = json(
        get(
            app.clone(),
            "/oauth/identities?limit=10&q=octocat&sort=provider_user_id",
            &owner_auth.access_token,
        )
        .await,
    )
    .await;
    assert_eq!(owner_list.total, 1);
    assert_eq!(owner_list.items[0].provider, OAuthProvider::GitHub);
    assert_eq!(owner_list.items[0].provider_user_id, "123456");
    assert_eq!(owner_list.items[0].user_id, owner_auth.user_id);
    assert_eq!(owner_list.items[0].email, "octocat@example.com");
    assert_eq!(owner_list.items[0].login, "octocat");

    let other_list: Page<OAuthIdentity> =
        json(get(app.clone(), "/oauth/identities", &other_auth.access_token).await).await;
    assert_eq!(other_list.total, 0);

    let admin_list: Page<OAuthIdentity> = json(
        get(
            app.clone(),
            &format!(
                "/oauth/identities?user_id={}&q=octocat&sort=-updated_epoch_sec",
                owner_auth.user_id
            ),
            &admin_token,
        )
        .await,
    )
    .await;
    assert_eq!(admin_list.total, 1);
    assert_eq!(admin_list.items[0].provider_user_id, "123456");

    let owner_detail: OAuthIdentity = json(
        get(
            app.clone(),
            "/oauth/identities/github/123456",
            &owner_auth.access_token,
        )
        .await,
    )
    .await;
    assert_eq!(owner_detail.provider, OAuthProvider::GitHub);
    assert_eq!(owner_detail.user_id, owner_auth.user_id);

    assert_eq!(
        get(
            app.clone(),
            "/oauth/identities/github/123456",
            &other_auth.access_token,
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        get(app, "/oauth/identities/github/missing", &admin_token)
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn oauth_identity_unlink_rejects_last_login_method() {
    let state = ControlState::new(
        "dev-secret",
        "relay.example.com:4443",
        "punch.example.com:3478",
    )
    .with_github_oauth_config(github_oauth_config())
    .with_github_oauth_client(fake_github_client(Some("octocat@example.com")));
    let app = routes(state);

    let start = get_without_token(app.clone(), "/auth/oauth/github/start").await;
    assert_eq!(start.status(), StatusCode::FOUND);
    let location = start.headers().get("location").unwrap().to_str().unwrap();
    let state = query_value(location, "state");
    let callback = get_without_token(
        app.clone(),
        &format!("/auth/oauth/github/callback?code=oauth-only&state={state}"),
    )
    .await;
    assert_eq!(callback.status(), StatusCode::OK);
    let oauth_only_auth: AuthResponse = json(callback).await;

    let unlink = delete(
        app.clone(),
        "/oauth/identities/github/123456",
        &oauth_only_auth.access_token,
    )
    .await;
    assert_eq!(unlink.status(), StatusCode::CONFLICT);

    let identities: Page<OAuthIdentity> =
        json(get(app, "/oauth/identities", &oauth_only_auth.access_token).await).await;
    assert_eq!(identities.total, 1);
    assert_eq!(identities.items[0].provider_user_id, "123456");
}

#[tokio::test]
async fn oauth_identity_unlink_allows_password_owner_and_admin() {
    let state = ControlState::new(
        "dev-secret",
        "relay.example.com:4443",
        "punch.example.com:3478",
    )
    .with_github_oauth_config(github_oauth_config())
    .with_github_oauth_client(fake_github_client(Some("octocat@example.com")));
    let admin_token = admin_token(&state);
    let app = routes(state.clone());
    let password_owner = register_user(app.clone(), "octocat@example.com").await;

    let start = get_without_token(app.clone(), "/auth/oauth/github/start").await;
    assert_eq!(start.status(), StatusCode::FOUND);
    let location = start.headers().get("location").unwrap().to_str().unwrap();
    let state_value = query_value(location, "state");
    let callback = get_without_token(
        app.clone(),
        &format!("/auth/oauth/github/callback?code=linked&state={state_value}"),
    )
    .await;
    assert_eq!(callback.status(), StatusCode::OK);
    let linked_auth: AuthResponse = json(callback).await;
    assert_eq!(linked_auth.user_id, password_owner.user_id);

    assert_eq!(
        delete(
            app.clone(),
            "/oauth/identities/github/123456",
            &password_owner.access_token,
        )
        .await
        .status(),
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        get(
            app.clone(),
            "/oauth/identities/github/123456",
            &password_owner.access_token,
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );

    state
        .upsert_oauth_identity(OAuthIdentity {
            provider: OAuthProvider::GitHub,
            provider_user_id: "admin-delete".to_string(),
            user_id: password_owner.user_id.clone(),
            email: "octocat@example.com".to_string(),
            login: "octocat-admin".to_string(),
            avatar_url: "https://avatars.githubusercontent.com/u/admin-delete".to_string(),
            created_epoch_sec: 1_767_000_000,
            updated_epoch_sec: 1_767_000_001,
        })
        .unwrap();

    assert_eq!(
        delete(
            app.clone(),
            "/oauth/identities/github/admin-delete",
            &admin_token,
        )
        .await
        .status(),
        StatusCode::NO_CONTENT
    );
    assert!(state.audit_logs().iter().any(|log| {
        log.action == "oauth_identity.unlink"
            && log.target_id == "github:admin-delete"
            && log.actor_role == ControlRole::Admin
    }));
}

#[tokio::test]
async fn oauth_only_user_can_set_password_and_then_unlink_identity() {
    let state = ControlState::new(
        "dev-secret",
        "relay.example.com:4443",
        "punch.example.com:3478",
    )
    .with_github_oauth_config(github_oauth_config())
    .with_github_oauth_client(fake_github_client(Some("octocat@example.com")));
    let app = routes(state.clone());

    let start = get_without_token(app.clone(), "/auth/oauth/github/start").await;
    assert_eq!(start.status(), StatusCode::FOUND);
    let location = start.headers().get("location").unwrap().to_str().unwrap();
    let state_value = query_value(location, "state");
    let callback = get_without_token(
        app.clone(),
        &format!("/auth/oauth/github/callback?code=set-password&state={state_value}"),
    )
    .await;
    assert_eq!(callback.status(), StatusCode::OK);
    let auth: AuthResponse = json(callback).await;

    assert_eq!(
        request_json(
            app.clone(),
            Method::POST,
            "/auth/login",
            None,
            &LoginRequest {
                email: "octocat@example.com".to_string(),
                password: "new-password-123".to_string(),
            },
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );

    let set_password = request_json(
        app.clone(),
        Method::POST,
        "/auth/password",
        Some(&auth.access_token),
        &UpdatePasswordRequest {
            current_password: None,
            new_password: "new-password-123".to_string(),
        },
    )
    .await;
    assert_eq!(set_password.status(), StatusCode::NO_CONTENT);

    let login = request_json(
        app.clone(),
        Method::POST,
        "/auth/login",
        None,
        &LoginRequest {
            email: "octocat@example.com".to_string(),
            password: "new-password-123".to_string(),
        },
    )
    .await;
    assert_eq!(login.status(), StatusCode::OK);
    let login: AuthResponse = json(login).await;
    assert_eq!(login.user_id, auth.user_id);

    assert_eq!(
        delete(
            app.clone(),
            "/oauth/identities/github/123456",
            &auth.access_token,
        )
        .await
        .status(),
        StatusCode::NO_CONTENT
    );
    assert!(state.audit_logs().iter().any(|log| {
        log.action == "auth.password.set"
            && log.target_type == "user"
            && log.target_id == auth.user_id.as_str()
    }));
}

#[tokio::test]
async fn password_user_change_requires_current_password() {
    let state = ControlState::new(
        "dev-secret",
        "relay.example.com:4443",
        "punch.example.com:3478",
    );
    let app = routes(state.clone());
    let auth = register_user(app.clone(), "password-change@example.com").await;

    let wrong_current = request_json(
        app.clone(),
        Method::POST,
        "/auth/password",
        Some(&auth.access_token),
        &UpdatePasswordRequest {
            current_password: Some("wrong-password".to_string()),
            new_password: "updated-password-123".to_string(),
        },
    )
    .await;
    assert_eq!(wrong_current.status(), StatusCode::UNAUTHORIZED);

    let changed = request_json(
        app.clone(),
        Method::POST,
        "/auth/password",
        Some(&auth.access_token),
        &UpdatePasswordRequest {
            current_password: Some("password-123".to_string()),
            new_password: "updated-password-123".to_string(),
        },
    )
    .await;
    assert_eq!(changed.status(), StatusCode::NO_CONTENT);

    assert_eq!(
        request_json(
            app.clone(),
            Method::POST,
            "/auth/login",
            None,
            &LoginRequest {
                email: "password-change@example.com".to_string(),
                password: "password-123".to_string(),
            },
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    let login = request_json(
        app,
        Method::POST,
        "/auth/login",
        None,
        &LoginRequest {
            email: "password-change@example.com".to_string(),
            password: "updated-password-123".to_string(),
        },
    )
    .await;
    assert_eq!(login.status(), StatusCode::OK);
    assert!(state.audit_logs().iter().any(|log| {
        log.action == "auth.password.change"
            && log.target_type == "user"
            && log.target_id == auth.user_id.as_str()
    }));
}

#[tokio::test]
async fn github_oauth_callback_rejects_unverified_email() {
    let state = ControlState::new(
        "dev-secret",
        "relay.example.com:4443",
        "punch.example.com:3478",
    )
    .with_github_oauth_config(github_oauth_config())
    .with_github_oauth_client(fake_github_client(None));
    let app = routes(state);

    let start = get_without_token(app.clone(), "/auth/oauth/github/start").await;
    assert_eq!(start.status(), StatusCode::FOUND);
    let location = start.headers().get("location").unwrap().to_str().unwrap();
    let state = query_value(location, "state");
    let callback = get_without_token(
        app,
        &format!("/auth/oauth/github/callback?code=first-code&state={state}"),
    )
    .await;

    assert_eq!(callback.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn browser_server_auth_issues_agent_credential_once() {
    let state = ControlState::new(
        "dev-secret",
        "relay.example.com:4443",
        "punch.example.com:3478",
    );
    let app = routes(state);
    let user_auth = register_user(app.clone(), "server-owner@example.com").await;

    let start = request_json(
        app.clone(),
        Method::POST,
        "/server-auth/browser/start",
        None,
        &StartServerAuthRequest {
            device_id: DeviceId::new("server_browser_001"),
            device_name: "Browser Login Server".to_string(),
            server_public_key: "server-public-key-001".to_string(),
        },
    )
    .await;
    assert_eq!(start.status(), StatusCode::OK);
    let start: BrowserServerAuthStartResponse = json(start).await;
    assert!(!start.session_id.is_empty());
    assert!(start
        .auth_url
        .contains("/server-auth/browser/approve?session_id="));

    let approval = get(
        app.clone(),
        &format!(
            "/server-auth/browser/approve?session_id={}",
            start.session_id
        ),
        &user_auth.access_token,
    )
    .await;
    assert_eq!(approval.status(), StatusCode::OK);
    let approval: BrowserServerAuthApprovalResponse = json(approval).await;
    assert_eq!(approval.session_id, start.session_id);
    assert_eq!(approval.status, ServerAuthStatus::Approved);
    assert!(!approval.server_auth_code.is_empty());

    let exchange_request = BrowserServerAuthExchangeRequest {
        session_id: start.session_id.clone(),
        server_auth_code: approval.server_auth_code,
        server_public_key: "server-public-key-001".to_string(),
    };
    let exchange = request_json(
        app.clone(),
        Method::POST,
        "/server-auth/browser/exchange",
        None,
        &exchange_request,
    )
    .await;
    assert_eq!(exchange.status(), StatusCode::OK);
    let credential: ServerCredentialResponse = json(exchange).await;
    assert_eq!(credential.device_id, DeviceId::new("server_browser_001"));
    assert_eq!(credential.token_type, "bearer");
    assert!(!credential.credential_id.is_empty());

    let claims = TokenSigner::new(TokenKey::new("dev-secret"))
        .verify_control(&credential.server_token, 1_767_000_000)
        .unwrap();
    assert_eq!(claims.role, ControlRole::Agent);
    assert_eq!(claims.user_id, user_auth.user_id);
    assert_eq!(claims.subject, credential.credential_id);
    assert_eq!(
        claims.credential_id.as_deref(),
        Some(claims.subject.as_str())
    );
    assert_eq!(claims.server_credential_version, Some(1));

    let replay = request_json(
        app,
        Method::POST,
        "/server-auth/browser/exchange",
        None,
        &exchange_request,
    )
    .await;
    assert_eq!(replay.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn device_code_server_auth_handles_polling_approval_denial_and_expiry() {
    let state = ControlState::new(
        "dev-secret",
        "relay.example.com:4443",
        "punch.example.com:3478",
    )
    .with_server_auth_now_epoch_sec(1_000);
    let app = routes(state.clone());
    let user_auth = register_user(app.clone(), "device-code-owner@example.com").await;

    let approved_start = start_device_server_auth(app.clone(), "server_device_approved").await;
    assert_eq!(approved_start.interval, 5);
    assert!(approved_start
        .verification_uri
        .contains("/server-auth/device"));
    assert!(approved_start
        .verification_uri_complete
        .contains(&approved_start.user_code));

    let pending =
        poll_device_server_auth(app.clone(), &approved_start, "server-public-key-device").await;
    assert_eq!(pending.status, ServerAuthStatus::AuthorizationPending);
    assert!(pending.credential.is_none());

    let approval = get(
        app.clone(),
        &format!("/server-auth/device?user_code={}", approved_start.user_code),
        &user_auth.access_token,
    )
    .await;
    assert_eq!(approval.status(), StatusCode::OK);
    let approval: DeviceServerAuthApprovalResponse = json(approval).await;
    assert_eq!(approval.status, ServerAuthStatus::Approved);

    let approved =
        poll_device_server_auth(app.clone(), &approved_start, "server-public-key-device").await;
    assert_eq!(approved.status, ServerAuthStatus::Approved);
    let credential = approved.credential.unwrap();
    assert_eq!(
        credential.device_id,
        DeviceId::new("server_device_approved")
    );
    let claims = TokenSigner::new(TokenKey::new("dev-secret"))
        .verify_control(&credential.server_token, 1_767_000_000)
        .unwrap();
    assert_eq!(claims.role, ControlRole::Agent);
    assert_eq!(claims.user_id, user_auth.user_id);

    let slow_start = start_device_server_auth(app.clone(), "server_device_slow").await;
    let slow_pending =
        poll_device_server_auth(app.clone(), &slow_start, "server-public-key-device").await;
    assert_eq!(slow_pending.status, ServerAuthStatus::AuthorizationPending);
    let slow_down =
        poll_device_server_auth(app.clone(), &slow_start, "server-public-key-device").await;
    assert_eq!(slow_down.status, ServerAuthStatus::SlowDown);
    assert!(slow_down.interval > slow_start.interval);

    let denied_start = start_device_server_auth(app.clone(), "server_device_denied").await;
    let denied = get(
        app.clone(),
        &format!(
            "/server-auth/device?user_code={}&decision=deny",
            denied_start.user_code
        ),
        &user_auth.access_token,
    )
    .await;
    assert_eq!(denied.status(), StatusCode::OK);
    let denied: DeviceServerAuthApprovalResponse = json(denied).await;
    assert_eq!(denied.status, ServerAuthStatus::Denied);
    let denied_poll =
        poll_device_server_auth(app.clone(), &denied_start, "server-public-key-device").await;
    assert_eq!(denied_poll.status, ServerAuthStatus::AccessDenied);
    assert!(denied_poll.credential.is_none());

    let expired_start = start_device_server_auth(app.clone(), "server_device_expired").await;
    state.set_server_auth_now_epoch_sec(1_601);
    let expired_poll =
        poll_device_server_auth(app, &expired_start, "server-public-key-device").await;
    assert_eq!(expired_poll.status, ServerAuthStatus::Expired);
    assert!(expired_poll.credential.is_none());
}

#[tokio::test]
async fn server_credentials_can_be_disabled() {
    let state = ControlState::new(
        "dev-secret",
        "relay.example.com:4443",
        "punch.example.com:3478",
    );
    let app = routes(state.clone());
    let user_auth = register_user(app.clone(), "credential-owner@example.com").await;

    let start = request_json(
        app.clone(),
        Method::POST,
        "/server-auth/browser/start",
        None,
        &StartServerAuthRequest {
            device_id: DeviceId::new("server_credential_001"),
            device_name: "Credential Server".to_string(),
            server_public_key: "server-public-key-credential".to_string(),
        },
    )
    .await;
    assert_eq!(start.status(), StatusCode::OK);
    let start: BrowserServerAuthStartResponse = json(start).await;
    let approval = get(
        app.clone(),
        &format!(
            "/server-auth/browser/approve?session_id={}",
            start.session_id
        ),
        &user_auth.access_token,
    )
    .await;
    assert_eq!(approval.status(), StatusCode::OK);
    let approval: BrowserServerAuthApprovalResponse = json(approval).await;
    let exchange = request_json(
        app.clone(),
        Method::POST,
        "/server-auth/browser/exchange",
        None,
        &BrowserServerAuthExchangeRequest {
            session_id: start.session_id,
            server_auth_code: approval.server_auth_code,
            server_public_key: "server-public-key-credential".to_string(),
        },
    )
    .await;
    assert_eq!(exchange.status(), StatusCode::OK);
    let credential: ServerCredentialResponse = json(exchange).await;
    assert!(state
        .control_claims_from_bearer(Some(&format!("Bearer {}", credential.server_token)))
        .is_ok());

    let listed = get(app.clone(), "/server-credentials", &user_auth.access_token).await;
    assert_eq!(listed.status(), StatusCode::OK);
    let listed: Page<ServerCredentialSummary> = json(listed).await;
    assert_eq!(listed.total, 1);
    assert_eq!(listed.items[0].credential_id, credential.credential_id);
    assert!(listed.items[0].enabled);

    let disabled = request_json(
        app.clone(),
        Method::POST,
        &format!("/server-credentials/{}/status", credential.credential_id),
        Some(&user_auth.access_token),
        &UpdateServerCredentialStatusRequest { enabled: false },
    )
    .await;
    assert_eq!(disabled.status(), StatusCode::OK);
    let disabled: ServerCredentialSummary = json(disabled).await;
    assert!(!disabled.enabled);

    assert!(state
        .control_claims_from_bearer(Some(&format!("Bearer {}", credential.server_token)))
        .is_err());
    let agent_route = request_json(
        app,
        Method::POST,
        "/agent/register",
        Some(&credential.server_token),
        &device("server_credential_001"),
    )
    .await;
    assert_eq!(agent_route.status(), StatusCode::UNAUTHORIZED);

    let audit_logs = state.audit_logs();
    assert!(audit_logs.iter().any(|log| {
        log.action == "server_credential.issue"
            && log.target_id == credential.credential_id
            && log.message.contains("server_credential_001")
    }));
    assert!(audit_logs.iter().any(|log| {
        log.action == "server_credential.status.update"
            && log.target_id == credential.credential_id
            && log.message.contains("enabled=false")
    }));
}

#[tokio::test]
async fn server_credentials_support_detail_rotation_and_admin_queries() {
    let state = ControlState::new(
        "dev-secret",
        "relay.example.com:4443",
        "punch.example.com:3478",
    );
    let admin_token = admin_token(&state);
    let app = routes(state.clone());
    let owner = register_user(app.clone(), "credential-api-owner@example.com").await;
    let other = register_user(app.clone(), "credential-api-other@example.com").await;

    let owner_credential = issue_browser_server_credential(
        app.clone(),
        &owner.access_token,
        "server_credential_api_owner",
        "Owner Credential Server",
        "owner-server-public-key",
    )
    .await;
    let other_credential = issue_browser_server_credential(
        app.clone(),
        &other.access_token,
        "server_credential_api_other",
        "Other Credential Server",
        "other-server-public-key",
    )
    .await;

    let owner_list: Page<ServerCredentialSummary> = json(
        get(
            app.clone(),
            "/server-credentials?limit=10&offset=0&q=Owner&sort=device_name",
            &owner.access_token,
        )
        .await,
    )
    .await;
    assert_eq!(owner_list.total, 1);
    assert_eq!(
        owner_list.items[0].credential_id,
        owner_credential.credential_id
    );
    assert_eq!(owner_list.items[0].user_id, owner.user_id);
    assert_eq!(
        owner_list.items[0].device_id,
        DeviceId::new("server_credential_api_owner")
    );

    let admin_list: Page<ServerCredentialSummary> = json(
        get(
            app.clone(),
            &format!(
                "/server-credentials?user_id={}&enabled=true&q=Credential&sort=-token_version",
                owner.user_id
            ),
            &admin_token,
        )
        .await,
    )
    .await;
    assert_eq!(admin_list.total, 1);
    assert_eq!(
        admin_list.items[0].credential_id,
        owner_credential.credential_id
    );

    let admin_all: Page<ServerCredentialSummary> =
        json(get(app.clone(), "/server-credentials", &admin_token).await).await;
    assert_eq!(admin_all.total, 2);
    assert!(admin_all
        .items
        .iter()
        .any(|credential| credential.credential_id == other_credential.credential_id));

    let owner_detail: ServerCredentialSummary = json(
        get(
            app.clone(),
            &format!("/server-credentials/{}", owner_credential.credential_id),
            &owner.access_token,
        )
        .await,
    )
    .await;
    assert_eq!(owner_detail.credential_id, owner_credential.credential_id);
    assert_eq!(owner_detail.token_version, 1);

    assert_eq!(
        get(
            app.clone(),
            &format!("/server-credentials/{}", owner_credential.credential_id),
            &other.access_token,
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );

    let rotated = post_empty(
        app.clone(),
        &format!(
            "/server-credentials/{}/rotate",
            owner_credential.credential_id
        ),
        Some(&owner.access_token),
    )
    .await;
    assert_eq!(rotated.status(), StatusCode::OK);
    let rotated: ServerCredentialResponse = json(rotated).await;
    assert_eq!(rotated.credential_id, owner_credential.credential_id);
    assert_eq!(
        rotated.device_id,
        DeviceId::new("server_credential_api_owner")
    );
    assert!(state
        .control_claims_from_bearer(Some(&format!("Bearer {}", owner_credential.server_token)))
        .is_err());
    let rotated_claims = TokenSigner::new(TokenKey::new("dev-secret"))
        .verify_control(&rotated.server_token, 1_767_000_000)
        .unwrap();
    assert_eq!(rotated_claims.server_credential_version, Some(2));

    let disabled = request_json(
        app.clone(),
        Method::POST,
        &format!(
            "/server-credentials/{}/status",
            owner_credential.credential_id
        ),
        Some(&admin_token),
        &UpdateServerCredentialStatusRequest { enabled: false },
    )
    .await;
    assert_eq!(disabled.status(), StatusCode::OK);
    let disabled: ServerCredentialSummary = json(disabled).await;
    assert!(!disabled.enabled);

    let audit_logs = state.audit_logs();
    assert!(audit_logs.iter().any(|log| {
        log.action == "server_credential.rotate"
            && log.target_id == owner_credential.credential_id
            && log.message.contains("version 2")
    }));
    assert!(audit_logs.iter().any(|log| {
        log.action == "server_credential.status.update"
            && log.target_id == owner_credential.credential_id
            && log.actor_role == ControlRole::Admin
    }));
}

#[tokio::test]
async fn agent_credential_use_updates_last_used_epoch() {
    let state = ControlState::new(
        "dev-secret",
        "relay.example.com:4443",
        "punch.example.com:3478",
    )
    .with_server_auth_now_epoch_sec(10);
    let app = routes(state.clone());
    let owner = register_user(app.clone(), "credential-last-used@example.com").await;
    let credential = issue_browser_server_credential(
        app.clone(),
        &owner.access_token,
        "server_credential_last_used",
        "Last Used Server",
        "last-used-server-public-key",
    )
    .await;

    let before: ServerCredentialSummary = json(
        get(
            app.clone(),
            &format!("/server-credentials/{}", credential.credential_id),
            &owner.access_token,
        )
        .await,
    )
    .await;
    assert_eq!(before.last_used_epoch_sec, None);

    let agent_register = request_json(
        app.clone(),
        Method::POST,
        "/agent/register",
        Some(&credential.server_token),
        &device("server_credential_last_used"),
    )
    .await;
    assert_eq!(agent_register.status(), StatusCode::OK);

    let after: ServerCredentialSummary = json(
        get(
            app,
            &format!("/server-credentials/{}", credential.credential_id),
            &owner.access_token,
        )
        .await,
    )
    .await;
    let last_used = after.last_used_epoch_sec.unwrap();
    assert!(last_used >= before.created_epoch_sec);
}

#[tokio::test]
async fn agent_credential_authorizes_only_own_device() {
    let state = ControlState::new(
        "dev-secret",
        "relay.example.com:4443",
        "punch.example.com:3478",
    )
    .with_strict_auth(true);
    let admin_token = admin_token(&state);
    let app = routes(state);
    let user_auth = register_user(app.clone(), "agent-owner@example.com").await;

    let start = request_json(
        app.clone(),
        Method::POST,
        "/server-auth/browser/start",
        None,
        &StartServerAuthRequest {
            device_id: DeviceId::new("agent_owned"),
            device_name: "Agent Owned Server".to_string(),
            server_public_key: "agent-owned-public-key".to_string(),
        },
    )
    .await;
    assert_eq!(start.status(), StatusCode::OK);
    let start: BrowserServerAuthStartResponse = json(start).await;
    let approval = get(
        app.clone(),
        &format!(
            "/server-auth/browser/approve?session_id={}",
            start.session_id
        ),
        &user_auth.access_token,
    )
    .await;
    assert_eq!(approval.status(), StatusCode::OK);
    let approval: BrowserServerAuthApprovalResponse = json(approval).await;
    let credential = request_json(
        app.clone(),
        Method::POST,
        "/server-auth/browser/exchange",
        None,
        &BrowserServerAuthExchangeRequest {
            session_id: start.session_id,
            server_auth_code: approval.server_auth_code,
            server_public_key: "agent-owned-public-key".to_string(),
        },
    )
    .await;
    assert_eq!(credential.status(), StatusCode::OK);
    let credential: ServerCredentialResponse = json(credential).await;

    assert_eq!(
        request_json(
            app.clone(),
            Method::POST,
            "/agent/register",
            Some(&credential.server_token),
            &device("agent_owned"),
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        request_json(
            app.clone(),
            Method::POST,
            "/agent/register",
            Some(&credential.server_token),
            &device("agent_other"),
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        request_json(
            app.clone(),
            Method::POST,
            "/agent/services",
            Some(&credential.server_token),
            &vec![service("agent_owned", "svc_agent_owned")],
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        request_json(
            app.clone(),
            Method::POST,
            "/agent/services",
            Some(&credential.server_token),
            &vec![service("agent_other", "svc_agent_other")],
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        request_json(
            app.clone(),
            Method::POST,
            "/agent/devices/agent_owned/p2p-cert",
            Some(&credential.server_token),
            &mobilecode_connect_control_client::RegisterP2pCertificateRequest {
                certificate_der: vec![1, 2, 3],
            },
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        request_json(
            app.clone(),
            Method::POST,
            "/agent/devices/agent_other/p2p-cert",
            Some(&credential.server_token),
            &mobilecode_connect_control_client::RegisterP2pCertificateRequest {
                certificate_der: vec![1, 2, 3],
            },
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        request_json(
            app.clone(),
            Method::POST,
            "/agent/register",
            Some(&user_auth.access_token),
            &device("user_strict_owner_bootstrap"),
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        request_json(
            app,
            Method::POST,
            "/agent/register",
            Some(&admin_token),
            &device("admin_agent_override"),
        )
        .await
        .status(),
        StatusCode::OK
    );
}

#[tokio::test]
async fn agent_grant_pairing_waits_for_agent_approval() {
    let state = ControlState::new(
        "dev-secret",
        "relay.example.com:4443",
        "punch.example.com:3478",
    )
    .with_strict_auth(true);
    let app = routes(state);
    let owner = register_user(app.clone(), "agent-grant-owner@example.com").await;
    let credential = issue_browser_server_credential(
        app.clone(),
        &owner.access_token,
        "agent_grant_pairing",
        "Agent Grant Pairing Server",
        "agent-grant-pairing-public-key",
    )
    .await;
    assert_eq!(
        request_json(
            app.clone(),
            Method::POST,
            "/agent/register",
            Some(&credential.server_token),
            &device("agent_grant_pairing"),
        )
        .await
        .status(),
        StatusCode::OK
    );

    let device_id = DeviceId::new("agent_grant_pairing");
    let service_id = ServiceId::new("svc_agent_grant_web");
    let client_id = ClientId::new("mobile_001");
    let requested_services = vec![service_id.clone()];
    let proof = MobilePairingRequest::proof_for(
        device_id.clone(),
        "inv_pairing_001".to_string(),
        client_id.clone(),
        requested_services.clone(),
        "nonce_pairing_001".to_string(),
        "invite-secret",
    )
    .unwrap();
    let pairing_request = MobilePairingRequest {
        device_id: device_id.clone(),
        invite_id: "inv_pairing_001".to_string(),
        client_id: client_id.clone(),
        requested_services: requested_services.clone(),
        nonce: "nonce_pairing_001".to_string(),
        proof: proof.clone(),
    };

    let started = request_json(
        app.clone(),
        Method::POST,
        "/agent-grants/pairing/start",
        None,
        &pairing_request,
    )
    .await;
    assert_eq!(started.status(), StatusCode::OK);
    let started: StartMobilePairingResponse = json(started).await;
    assert!(!started.pending_pairing_id.is_empty());
    assert!(started.expires_at > 0);

    let listed = get(
        app.clone(),
        "/agent/devices/agent_grant_pairing/pairing-requests",
        &credential.server_token,
    )
    .await;
    assert_eq!(listed.status(), StatusCode::OK);
    let listed: Vec<PendingMobilePairingRequest> = json(listed).await;
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].pending_pairing_id, started.pending_pairing_id);
    assert_eq!(listed[0].request, pairing_request);
    assert_eq!(listed[0].status, PendingPairingStatus::Pending);

    let approved = request_json(
        app.clone(),
        Method::POST,
        &format!("/agent/pairing/{}/approve", started.pending_pairing_id),
        Some(&credential.server_token),
        &ApproveMobilePairingRequest {
            grant_id: "gr_mobile_001".to_string(),
            allowed_services: requested_services.clone(),
            revocation_version: 1,
        },
    )
    .await;
    assert_eq!(approved.status(), StatusCode::OK);
    let approved: MobilePairingPollResponse = json(approved).await;
    assert_eq!(approved.status, PendingPairingStatus::Approved);
    assert_eq!(
        approved.grant,
        Some(ApprovedMobileGrantMetadata {
            version: 1,
            device_id: device_id.clone(),
            grant_id: "gr_mobile_001".to_string(),
            client_id: client_id.clone(),
            allowed_services: requested_services.clone(),
            revocation_version: 1,
        })
    );

    let polled = get_without_token(
        app,
        &format!("/agent-grants/pairing/{}", started.pending_pairing_id),
    )
    .await;
    assert_eq!(polled.status(), StatusCode::OK);
    let polled: MobilePairingPollResponse = json(polled).await;
    assert_eq!(polled.status, PendingPairingStatus::Approved);
    assert_eq!(polled.grant, approved.grant);
}

#[tokio::test]
async fn agent_grant_session_waits_for_agent_approval_before_creating_session() {
    let state = ControlState::new(
        "dev-secret",
        "relay.example.com:4443",
        "punch.example.com:3478",
    )
    .with_strict_auth(true);
    let app = routes(state);
    let owner = register_user(app.clone(), "agent-grant-session-owner@example.com").await;
    let credential = issue_browser_server_credential(
        app.clone(),
        &owner.access_token,
        "agent_grant_session",
        "Agent Grant Session Server",
        "agent-grant-session-public-key",
    )
    .await;
    assert_eq!(
        request_json(
            app.clone(),
            Method::POST,
            "/agent/register",
            Some(&credential.server_token),
            &device("agent_grant_session"),
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        request_json(
            app.clone(),
            Method::POST,
            "/agent/services",
            Some(&credential.server_token),
            &vec![service("agent_grant_session", "svc_agent_grant_web")],
        )
        .await
        .status(),
        StatusCode::OK
    );

    let device_id = DeviceId::new("agent_grant_session");
    let service_id = ServiceId::new("svc_agent_grant_web");
    let client_id = ClientId::new("mobile_001");
    let proof = GrantSessionRequest::proof_for(
        client_id.clone(),
        device_id.clone(),
        service_id.clone(),
        "gr_mobile_session_001".to_string(),
        1,
        "nonce_session_001".to_string(),
        "grant-secret",
    )
    .unwrap();
    let session_request = GrantSessionRequest {
        client_id: client_id.clone(),
        device_id: device_id.clone(),
        service_id: service_id.clone(),
        grant_id: "gr_mobile_session_001".to_string(),
        revocation_version: 1,
        nonce: "nonce_session_001".to_string(),
        proof,
    };

    let started = request_json(
        app.clone(),
        Method::POST,
        "/agent-grants/sessions/start",
        None,
        &session_request,
    )
    .await;
    assert_eq!(started.status(), StatusCode::OK);
    let started: StartGrantSessionResponse = json(started).await;
    assert!(!started.pending_session_id.is_empty());
    assert!(started.expires_at > 0);

    let listed = get(
        app.clone(),
        "/agent/devices/agent_grant_session/grant-session-requests",
        &credential.server_token,
    )
    .await;
    assert_eq!(listed.status(), StatusCode::OK);
    let listed: Vec<PendingGrantSessionRequest> = json(listed).await;
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].pending_session_id, started.pending_session_id);
    assert_eq!(listed[0].request, session_request);
    assert_eq!(listed[0].status, PendingGrantSessionStatus::Pending);

    let approved = request_json(
        app.clone(),
        Method::POST,
        &format!(
            "/agent/grant-sessions/{}/approve",
            started.pending_session_id
        ),
        Some(&credential.server_token),
        &ApproveGrantSessionRequest {},
    )
    .await;
    assert_eq!(approved.status(), StatusCode::OK);
    let approved: GrantSessionPollResponse = json(approved).await;
    assert_eq!(approved.status, PendingGrantSessionStatus::Approved);
    let approved_session: CreateSessionResponse = approved.session.clone().unwrap();
    assert!(!approved_session.access_token.is_empty());
    assert!(!approved_session.relay_token.is_empty());
    assert_eq!(approved_session.relay_addr, "relay.example.com:4443");

    let polled = get_without_token(
        app.clone(),
        &format!("/agent-grants/sessions/{}", started.pending_session_id),
    )
    .await;
    assert_eq!(polled.status(), StatusCode::OK);
    let polled: GrantSessionPollResponse = json(polled).await;
    assert_eq!(polled.status, PendingGrantSessionStatus::Approved);
    assert_eq!(polled.session, approved.session);

    let assignments = get(
        app,
        "/agent/devices/agent_grant_session/sessions",
        &credential.server_token,
    )
    .await;
    assert_eq!(assignments.status(), StatusCode::OK);
    let assignments: Vec<mobilecode_connect_control_client::AgentSessionAssignment> =
        json(assignments).await;
    assert_eq!(assignments.len(), 1);
    assert_eq!(assignments[0].session_id, approved_session.session_id);
    assert_eq!(
        assignments[0].grant_id.as_deref(),
        Some("gr_mobile_session_001")
    );
    assert_eq!(assignments[0].grant_revocation_version, Some(1));
    assert_eq!(assignments[0].grant_service_id, Some(service_id));
}

#[tokio::test]
async fn control_admin_page_is_served_by_control_server() {
    let state = ControlState::new(
        "dev-secret",
        "relay.example.com:4443",
        "punch.example.com:3478",
    );
    let app = routes(state);

    let response = get_without_token(app, "/admin").await;

    assert_eq!(response.status(), StatusCode::OK);
    let body = text(response).await;
    assert!(body.contains("Control Admin"));
    assert!(body.contains("relayPool"));
    assert!(body.contains("planEditor"));
}

#[tokio::test]
async fn admin_dashboard_summary_reports_control_plane_totals() {
    let state = ControlState::new(
        "dev-secret",
        "relay.example.com:4443",
        "punch.example.com:3478",
    );
    let admin_token = admin_token(&state);
    let app = routes(state);
    let auth = register_user(app.clone(), "dashboard-owner@example.com").await;

    assert_eq!(
        get(app.clone(), "/dashboard", &auth.access_token)
            .await
            .status(),
        StatusCode::FORBIDDEN
    );

    let admin_user = request_json(
        app.clone(),
        Method::POST,
        "/users",
        Some(&admin_token),
        &CreateUserRequest {
            email: "dashboard-admin@example.com".to_string(),
            password: "password-123".to_string(),
            display_name: "Dashboard Admin".to_string(),
            role: ControlRole::Admin,
            enabled: true,
        },
    )
    .await;
    assert_eq!(admin_user.status(), StatusCode::OK);

    let relay = request_json(
        app.clone(),
        Method::POST,
        "/relays/register",
        Some(&admin_token),
        &RegisterRelayRequest {
            relay_id: "relay_dashboard".to_string(),
            relay_addr: "relay-dashboard.example.com:4443".to_string(),
            admin_addr: "relay-dashboard.example.com:9090".to_string(),
            capacity_streams: 16,
        },
    )
    .await;
    assert_eq!(relay.status(), StatusCode::OK);

    let controller = request_json(
        app.clone(),
        Method::POST,
        "/controllers/register",
        Some(&auth.access_token),
        &RegisterControllerDeviceRequest {
            client_id: "phone_dashboard".to_string(),
            name: "Dashboard Phone".to_string(),
        },
    )
    .await;
    assert_eq!(controller.status(), StatusCode::OK);

    assert_eq!(
        request_json(
            app.clone(),
            Method::POST,
            "/agent/register",
            Some(&auth.access_token),
            &device("server_dashboard"),
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        request_json(
            app.clone(),
            Method::POST,
            "/agent/services",
            Some(&auth.access_token),
            &vec![service("server_dashboard", "svc_dashboard")],
        )
        .await
        .status(),
        StatusCode::OK
    );

    let session_response = request_json(
        app.clone(),
        Method::POST,
        "/sessions",
        Some(&auth.access_token),
        &mobilecode_connect_control_client::CreateSessionRequest {
            client_id: "phone_dashboard".to_string(),
            device_id: DeviceId::new("server_dashboard"),
            service_id: ServiceId::new("svc_dashboard"),
        },
    )
    .await;
    assert_eq!(session_response.status(), StatusCode::OK);
    let session: mobilecode_connect_control_client::CreateSessionResponse =
        json(session_response).await;

    let report = request_json(
        app.clone(),
        Method::POST,
        "/usage/relay-sessions",
        Some(&admin_token),
        &ReportRelaySessionUsageRequest {
            relay_id: "relay_dashboard".to_string(),
            sessions: vec![RelaySessionUsageReport {
                session_id: session.session_id.clone(),
                stats: TrafficStats {
                    session_id: Some(session.session_id),
                    uplink_bytes: 11,
                    downlink_bytes: 17,
                    total_bytes: 28,
                    active_streams: 1,
                    duration_sec: 2,
                },
            }],
        },
    )
    .await;
    assert_eq!(report.status(), StatusCode::NO_CONTENT);

    let dashboard: DashboardSummary = json(get(app, "/dashboard", &admin_token).await).await;
    assert_eq!(dashboard.users.total, 2);
    assert_eq!(dashboard.users.enabled, 2);
    assert_eq!(dashboard.users.admins, 1);
    assert_eq!(dashboard.controllers.total, 1);
    assert_eq!(dashboard.devices.total, 1);
    assert_eq!(dashboard.devices.online, 1);
    assert_eq!(dashboard.sessions.total, 1);
    assert_eq!(dashboard.sessions.pending, 1);
    assert_eq!(dashboard.sessions.bound, 0);
    assert_eq!(dashboard.relays.total, 2);
    assert_eq!(dashboard.relays.healthy, 2);
    assert_eq!(dashboard.relays.unhealthy, 0);
    assert_eq!(dashboard.usage.actual_uplink_bytes, 11);
    assert_eq!(dashboard.usage.actual_downlink_bytes, 17);
    assert_eq!(dashboard.usage.actual_total_bytes, 28);
    assert!(dashboard
        .recent_audit_logs
        .iter()
        .any(|entry| entry.action == "relay.register" && entry.target_id == "relay_dashboard"));
}

#[tokio::test]
async fn user_registers_logs_in_and_creates_session_with_plan_limits_and_pool_relay() {
    let state = ControlState::new(
        "dev-secret",
        "seed-relay.example.com:4443",
        "punch.example.com:3478",
    );
    let admin_token = admin_token(&state);
    let app = routes(state);
    let auth = register_user(app.clone(), "owner@example.com").await;

    let login = request_json(
        app.clone(),
        Method::POST,
        "/auth/login",
        None,
        &LoginRequest {
            email: "owner@example.com".to_string(),
            password: "password-123".to_string(),
        },
    )
    .await;
    assert_eq!(login.status(), StatusCode::OK);
    let login: AuthResponse = json(login).await;
    assert_eq!(login.user_id, auth.user_id);
    assert!(!login.access_token.is_empty());

    let relay = request_json(
        app.clone(),
        Method::POST,
        "/relays/register",
        Some(&admin_token),
        &RegisterRelayRequest {
            relay_id: "relay_west".to_string(),
            relay_addr: "relay-west.example.com:4443".to_string(),
            admin_addr: "relay-west.example.com:9090".to_string(),
            capacity_streams: 128,
        },
    )
    .await;
    assert_eq!(relay.status(), StatusCode::OK);

    let controller = request_json(
        app.clone(),
        Method::POST,
        "/controllers/register",
        Some(&auth.access_token),
        &RegisterControllerDeviceRequest {
            client_id: "phone_001".to_string(),
            name: "Yux Phone".to_string(),
        },
    )
    .await;
    assert_eq!(controller.status(), StatusCode::OK);

    assert_eq!(
        request_json(
            app.clone(),
            Method::POST,
            "/agent/register",
            Some(&auth.access_token),
            &device("server_001"),
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        request_json(
            app.clone(),
            Method::POST,
            "/agent/services",
            Some(&auth.access_token),
            &vec![service("server_001", "svc_web_3000")],
        )
        .await
        .status(),
        StatusCode::OK
    );

    let session_response = request_json(
        app.clone(),
        Method::POST,
        "/sessions",
        Some(&auth.access_token),
        &mobilecode_connect_control_client::CreateSessionRequest {
            client_id: "phone_001".to_string(),
            device_id: DeviceId::new("server_001"),
            service_id: ServiceId::new("svc_web_3000"),
        },
    )
    .await;
    assert_eq!(session_response.status(), StatusCode::OK);
    let session: mobilecode_connect_control_client::CreateSessionResponse =
        json(session_response).await;
    assert_eq!(session.relay_addr, "relay-west.example.com:4443");

    let claims = TokenSigner::new(TokenKey::new("dev-secret"))
        .verify_relay(&session.relay_token, 1_767_000_000)
        .unwrap();
    assert_eq!(claims.user_id, auth.user_id);
    assert_eq!(claims.max_streams, 8);
    assert_eq!(claims.max_bps, 1_048_576);
    assert_eq!(claims.traffic_quota_bytes, 104_857_600);
}

#[tokio::test]
async fn session_state_routes_require_owner_or_admin_token() {
    let state = ControlState::new(
        "dev-secret",
        "relay.example.com:4443",
        "punch.example.com:3478",
    )
    .with_strict_auth(true);
    let admin_token = admin_token(&state);
    let app = routes(state);
    let owner = register_user(app.clone(), "session-owner@example.com").await;
    let other = register_user(app.clone(), "session-other@example.com").await;

    assert_eq!(
        request_json(
            app.clone(),
            Method::POST,
            "/agent/register",
            Some(&owner.access_token),
            &device("server_session_auth"),
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        request_json(
            app.clone(),
            Method::POST,
            "/agent/services",
            Some(&owner.access_token),
            &vec![service("server_session_auth", "svc_session_auth")],
        )
        .await
        .status(),
        StatusCode::OK
    );

    let session_response = request_json(
        app.clone(),
        Method::POST,
        "/sessions",
        Some(&owner.access_token),
        &mobilecode_connect_control_client::CreateSessionRequest {
            client_id: "phone_session_auth".to_string(),
            device_id: DeviceId::new("server_session_auth"),
            service_id: ServiceId::new("svc_session_auth"),
        },
    )
    .await;
    assert_eq!(session_response.status(), StatusCode::OK);
    let session: mobilecode_connect_control_client::CreateSessionResponse =
        json(session_response).await;

    assert_eq!(
        get(
            app.clone(),
            "/agent/devices/server_session_auth/sessions",
            &other.access_token,
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        get(
            app.clone(),
            "/agent/devices/server_session_auth/sessions",
            &owner.access_token,
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        post_empty(
            app.clone(),
            &format!("/agent/sessions/{}/claim", session.session_id),
            None,
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        post_empty(
            app.clone(),
            &format!("/agent/sessions/{}/claim", session.session_id),
            Some(&other.access_token),
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );

    let claimed = post_empty(
        app.clone(),
        &format!("/agent/sessions/{}/claim", session.session_id),
        Some(&owner.access_token),
    )
    .await;
    assert_eq!(claimed.status(), StatusCode::OK);

    assert_eq!(
        post_empty(
            app.clone(),
            &format!("/agent/sessions/{}/bound", session.session_id),
            Some(&other.access_token),
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        post_empty(
            app.clone(),
            &format!("/agent/sessions/{}/bound", session.session_id),
            Some(&owner.access_token),
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        post_empty(
            app.clone(),
            &format!("/sessions/{}/close", session.session_id),
            Some(&other.access_token),
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        post_empty(
            app,
            &format!("/sessions/{}/close", session.session_id),
            Some(&admin_token),
        )
        .await
        .status(),
        StatusCode::OK
    );
}

#[tokio::test]
async fn admin_grants_user_access_to_controlled_device_without_agent_privileges() {
    let state = ControlState::new(
        "dev-secret",
        "relay.example.com:4443",
        "punch.example.com:3478",
    )
    .with_strict_auth(true);
    let admin_token = admin_token(&state);
    let app = routes(state);
    let owner = register_user(app.clone(), "device-owner@example.com").await;
    let grantee = register_user(app.clone(), "device-grantee@example.com").await;

    assert_eq!(
        request_json(
            app.clone(),
            Method::POST,
            "/agent/register",
            Some(&owner.access_token),
            &device("server_shared"),
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        request_json(
            app.clone(),
            Method::POST,
            "/agent/services",
            Some(&owner.access_token),
            &vec![service("server_shared", "svc_shared")],
        )
        .await
        .status(),
        StatusCode::OK
    );

    let grantee_devices: Vec<Device> =
        json(get(app.clone(), "/mobile/devices", &grantee.access_token).await).await;
    assert!(grantee_devices.is_empty());
    assert_eq!(
        get(
            app.clone(),
            "/mobile/devices/server_shared/services",
            &grantee.access_token,
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        request_json(
            app.clone(),
            Method::POST,
            "/sessions",
            Some(&grantee.access_token),
            &mobilecode_connect_control_client::CreateSessionRequest {
                client_id: "phone_grantee".to_string(),
                device_id: DeviceId::new("server_shared"),
                service_id: ServiceId::new("svc_shared"),
            },
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );

    assert_eq!(
        request_json(
            app.clone(),
            Method::POST,
            "/devices/server_shared/access",
            Some(&grantee.access_token),
            &GrantDeviceAccessRequest {
                user_id: grantee.user_id.clone(),
            },
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );

    let grant = request_json(
        app.clone(),
        Method::POST,
        "/devices/server_shared/access",
        Some(&admin_token),
        &GrantDeviceAccessRequest {
            user_id: grantee.user_id.clone(),
        },
    )
    .await;
    assert_eq!(grant.status(), StatusCode::OK);
    let grant: DeviceAccessGrant = json(grant).await;
    assert_eq!(grant.device_id, DeviceId::new("server_shared"));
    assert_eq!(grant.user_id, grantee.user_id);

    let grants: Page<DeviceAccessGrant> =
        json(get(app.clone(), "/devices/server_shared/access", &admin_token).await).await;
    assert_eq!(grants.total, 1);
    assert_eq!(grants.items, vec![grant]);

    let grantee_devices: Vec<Device> =
        json(get(app.clone(), "/mobile/devices", &grantee.access_token).await).await;
    assert_eq!(grantee_devices.len(), 1);
    assert_eq!(grantee_devices[0].device_id, DeviceId::new("server_shared"));

    let grantee_services: Vec<Service> = json(
        get(
            app.clone(),
            "/mobile/devices/server_shared/services",
            &grantee.access_token,
        )
        .await,
    )
    .await;
    assert_eq!(
        grantee_services,
        vec![service("server_shared", "svc_shared")]
    );

    let session_response = request_json(
        app.clone(),
        Method::POST,
        "/sessions",
        Some(&grantee.access_token),
        &mobilecode_connect_control_client::CreateSessionRequest {
            client_id: "phone_grantee".to_string(),
            device_id: DeviceId::new("server_shared"),
            service_id: ServiceId::new("svc_shared"),
        },
    )
    .await;
    assert_eq!(session_response.status(), StatusCode::OK);
    let session: mobilecode_connect_control_client::CreateSessionResponse =
        json(session_response).await;
    let claims = TokenSigner::new(TokenKey::new("dev-secret"))
        .verify_relay(&session.relay_token, 1_767_000_000)
        .unwrap();
    assert_eq!(claims.user_id, grantee.user_id);

    assert_eq!(
        get(
            app.clone(),
            "/agent/devices/server_shared/sessions",
            &grantee.access_token,
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        get(
            app.clone(),
            "/agent/devices/server_shared/sessions",
            &owner.access_token,
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        post_empty(
            app.clone(),
            &format!("/sessions/{}/close", session.session_id),
            Some(&grantee.access_token),
        )
        .await
        .status(),
        StatusCode::OK
    );

    assert_eq!(
        delete(
            app.clone(),
            &format!("/devices/server_shared/access/{}", grantee.user_id),
            &admin_token,
        )
        .await
        .status(),
        StatusCode::NO_CONTENT
    );
    let grantee_devices: Vec<Device> =
        json(get(app, "/mobile/devices", &grantee.access_token).await).await;
    assert!(grantee_devices.is_empty());
}

#[tokio::test]
async fn admin_lists_and_closes_control_sessions() {
    let state = ControlState::new(
        "dev-secret",
        "relay.example.com:4443",
        "punch.example.com:3478",
    );
    let admin_token = admin_token(&state);
    let app = routes(state);
    let owner = register_user(app.clone(), "admin-session-owner@example.com").await;

    assert_eq!(
        request_json(
            app.clone(),
            Method::POST,
            "/agent/register",
            Some(&owner.access_token),
            &device("server_admin_session"),
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        request_json(
            app.clone(),
            Method::POST,
            "/agent/services",
            Some(&owner.access_token),
            &vec![service("server_admin_session", "svc_admin_session")],
        )
        .await
        .status(),
        StatusCode::OK
    );

    let session_response = request_json(
        app.clone(),
        Method::POST,
        "/sessions",
        Some(&owner.access_token),
        &mobilecode_connect_control_client::CreateSessionRequest {
            client_id: "phone_admin_session".to_string(),
            device_id: DeviceId::new("server_admin_session"),
            service_id: ServiceId::new("svc_admin_session"),
        },
    )
    .await;
    assert_eq!(session_response.status(), StatusCode::OK);
    let session: mobilecode_connect_control_client::CreateSessionResponse =
        json(session_response).await;

    assert_eq!(
        get(app.clone(), "/sessions", &owner.access_token)
            .await
            .status(),
        StatusCode::FORBIDDEN
    );

    let sessions: Page<AdminSessionSummary> =
        json(get(app.clone(), "/sessions", &admin_token).await).await;
    let summary = sessions
        .items
        .iter()
        .find(|summary| summary.session_id == session.session_id)
        .unwrap();

    assert_eq!(summary.user_id, owner.user_id);
    assert_eq!(summary.user_email, "admin-session-owner@example.com");
    assert_eq!(summary.device_id, DeviceId::new("server_admin_session"));
    assert_eq!(summary.device_name, "Office PC");
    assert_eq!(summary.service_id, ServiceId::new("svc_admin_session"));
    assert_eq!(summary.service_name, "Dev Web");
    assert_eq!(summary.client_id.as_str(), "phone_admin_session");
    assert_eq!(
        summary.status,
        mobilecode_connect_control_client::AgentSessionStatus::Pending
    );
    assert_eq!(summary.relay_addr, "relay.example.com:4443");
    assert_eq!(summary.expire_at, session.expire_at);

    let close = post_empty(
        app.clone(),
        &format!("/sessions/{}/close", session.session_id),
        Some(&admin_token),
    )
    .await;
    assert_eq!(close.status(), StatusCode::OK);

    let sessions: Page<AdminSessionSummary> = json(get(app, "/sessions", &admin_token).await).await;
    let summary = sessions
        .items
        .iter()
        .find(|summary| summary.session_id == session.session_id)
        .unwrap();
    assert_eq!(
        summary.status,
        mobilecode_connect_control_client::AgentSessionStatus::Closed
    );
}

#[tokio::test]
async fn strict_auth_rejects_missing_bearer_token() {
    let state = ControlState::new(
        "dev-secret",
        "relay.example.com:4443",
        "punch.example.com:3478",
    )
    .with_strict_auth(true);
    let app = routes(state);

    let response = get_without_token(app, "/mobile/devices").await;

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn admin_only_routes_reject_user_tokens_and_accept_admin_tokens() {
    let state = ControlState::new(
        "dev-secret",
        "relay.example.com:4443",
        "punch.example.com:3478",
    );
    let admin_token = admin_token(&state);
    let app = routes(state);
    let auth = register_user(app.clone(), "ordinary@example.com").await;

    let plan_response = request_json(
        app.clone(),
        Method::POST,
        &format!("/plans/users/{}", auth.user_id),
        Some(&auth.access_token),
        &UpdateUserPlanRequest {
            plan: Plan {
                plan_id: "team".to_string(),
                name: "Team".to_string(),
                max_controller_devices: 4,
                relay_limits: RelayLimits {
                    max_bps: 8_192,
                    max_streams: 12,
                    max_duration_sec: 3_600,
                    traffic_quota_bytes: 2_097_152,
                },
            },
        },
    )
    .await;
    assert_eq!(plan_response.status(), StatusCode::FORBIDDEN);

    let relay_response = request_json(
        app.clone(),
        Method::POST,
        "/relays/register",
        Some(&auth.access_token),
        &RegisterRelayRequest {
            relay_id: "relay_forbidden".to_string(),
            relay_addr: "relay-forbidden.example.com:4443".to_string(),
            admin_addr: "relay-forbidden.example.com:9090".to_string(),
            capacity_streams: 64,
        },
    )
    .await;
    assert_eq!(relay_response.status(), StatusCode::FORBIDDEN);

    let relay_response = request_json(
        app,
        Method::POST,
        "/relays/register",
        Some(&admin_token),
        &RegisterRelayRequest {
            relay_id: "relay_admin".to_string(),
            relay_addr: "relay-admin.example.com:4443".to_string(),
            admin_addr: "relay-admin.example.com:9090".to_string(),
            capacity_streams: 64,
        },
    )
    .await;
    assert_eq!(relay_response.status(), StatusCode::OK);
}

#[tokio::test]
async fn admin_user_management_lists_details_and_disables_users() {
    let state = ControlState::new(
        "dev-secret",
        "relay.example.com:4443",
        "punch.example.com:3478",
    );
    let admin_auth = state
        .bootstrap_admin_user(RegisterUserRequest {
            email: "admin@example.com".to_string(),
            password: "admin-password-123".to_string(),
            display_name: "Admin".to_string(),
        })
        .unwrap();
    let app = routes(state);
    let auth = register_user(app.clone(), "managed@example.com").await;

    let forbidden = get(app.clone(), "/users", &auth.access_token).await;
    assert_eq!(forbidden.status(), StatusCode::FORBIDDEN);

    let users: Page<UserSummary> =
        json(get(app.clone(), "/users", &admin_auth.access_token).await).await;
    let managed = users
        .items
        .iter()
        .find(|user| user.user_id == auth.user_id)
        .unwrap();
    assert_eq!(managed.email, "managed@example.com");
    assert_eq!(managed.display_name, "Test User");
    assert_eq!(managed.role, ControlRole::User);
    assert!(managed.enabled);
    assert_eq!(managed.plan_id, "free");

    let detail: UserDetail = json(
        get(
            app.clone(),
            &format!("/users/{}", auth.user_id),
            &admin_auth.access_token,
        )
        .await,
    )
    .await;
    assert_eq!(detail.user.user_id, auth.user_id);
    assert_eq!(detail.plan.plan_id, "free");
    assert!(detail.controllers.is_empty());
    assert!(detail.devices.is_empty());

    let disabled = request_json(
        app.clone(),
        Method::POST,
        &format!("/users/{}/status", auth.user_id),
        Some(&admin_auth.access_token),
        &UpdateUserStatusRequest { enabled: false },
    )
    .await;
    assert_eq!(disabled.status(), StatusCode::OK);
    let disabled: UserSummary = json(disabled).await;
    assert!(!disabled.enabled);

    let login = request_json(
        app.clone(),
        Method::POST,
        "/auth/login",
        None,
        &LoginRequest {
            email: "managed@example.com".to_string(),
            password: "password-123".to_string(),
        },
    )
    .await;
    assert_eq!(login.status(), StatusCode::UNAUTHORIZED);

    let stale_token_response = get(app, "/controllers", &auth.access_token).await;
    assert_eq!(stale_token_response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn admin_user_management_creates_users_and_updates_roles() {
    let state = ControlState::new(
        "dev-secret",
        "relay.example.com:4443",
        "punch.example.com:3478",
    );
    let admin_auth = state
        .bootstrap_admin_user(RegisterUserRequest {
            email: "admin@example.com".to_string(),
            password: "admin-password-123".to_string(),
            display_name: "Admin".to_string(),
        })
        .unwrap();
    let app = routes(state);
    let ordinary = register_user(app.clone(), "ordinary@example.com").await;
    let request = CreateUserRequest {
        email: "managed-admin@example.com".to_string(),
        password: "managed-password-123".to_string(),
        display_name: "Managed Admin".to_string(),
        role: ControlRole::Admin,
        enabled: true,
    };

    let forbidden = request_json(
        app.clone(),
        Method::POST,
        "/users",
        Some(&ordinary.access_token),
        &request,
    )
    .await;
    assert_eq!(forbidden.status(), StatusCode::FORBIDDEN);

    let created = request_json(
        app.clone(),
        Method::POST,
        "/users",
        Some(&admin_auth.access_token),
        &request,
    )
    .await;
    assert_eq!(created.status(), StatusCode::OK);
    let created: UserSummary = json(created).await;
    assert_eq!(created.email, "managed-admin@example.com");
    assert_eq!(created.display_name, "Managed Admin");
    assert_eq!(created.role, ControlRole::Admin);
    assert!(created.enabled);
    assert_eq!(created.plan_id, "free");

    let duplicate = request_json(
        app.clone(),
        Method::POST,
        "/users",
        Some(&admin_auth.access_token),
        &request,
    )
    .await;
    assert_eq!(duplicate.status(), StatusCode::CONFLICT);

    let login = request_json(
        app.clone(),
        Method::POST,
        "/auth/login",
        None,
        &LoginRequest {
            email: "managed-admin@example.com".to_string(),
            password: "managed-password-123".to_string(),
        },
    )
    .await;
    assert_eq!(login.status(), StatusCode::OK);
    let login: AuthResponse = json(login).await;
    let claims = TokenSigner::new(TokenKey::new("dev-secret"))
        .verify_control(&login.access_token, 1_767_000_000)
        .unwrap();
    assert_eq!(claims.role, ControlRole::Admin);

    let invalid_role = request_json(
        app.clone(),
        Method::POST,
        "/users",
        Some(&admin_auth.access_token),
        &CreateUserRequest {
            role: ControlRole::Relay,
            ..request.clone()
        },
    )
    .await;
    assert_eq!(invalid_role.status(), StatusCode::BAD_REQUEST);

    let updated = request_json(
        app.clone(),
        Method::POST,
        &format!("/users/{}/role", created.user_id),
        Some(&admin_auth.access_token),
        &UpdateUserRoleRequest {
            role: ControlRole::User,
        },
    )
    .await;
    assert_eq!(updated.status(), StatusCode::OK);
    let updated: UserSummary = json(updated).await;
    assert_eq!(updated.role, ControlRole::User);

    let invalid_update = request_json(
        app.clone(),
        Method::POST,
        &format!("/users/{}/role", created.user_id),
        Some(&admin_auth.access_token),
        &UpdateUserRoleRequest {
            role: ControlRole::Relay,
        },
    )
    .await;
    assert_eq!(invalid_update.status(), StatusCode::BAD_REQUEST);

    let missing = request_json(
        app,
        Method::POST,
        "/users/user_missing/role",
        Some(&admin_auth.access_token),
        &UpdateUserRoleRequest {
            role: ControlRole::User,
        },
    )
    .await;
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn admin_user_list_returns_paginated_filtered_page() {
    let state = ControlState::new(
        "dev-secret",
        "relay.example.com:4443",
        "punch.example.com:3478",
    );
    let admin_auth = state
        .bootstrap_admin_user(RegisterUserRequest {
            email: "root@example.com".to_string(),
            password: "admin-password-123".to_string(),
            display_name: "Root".to_string(),
        })
        .unwrap();
    let app = routes(state);

    for (email, role) in [
        ("ops-alpha@example.com", ControlRole::Admin),
        ("ops-beta@example.com", ControlRole::Admin),
        ("user-alpha@example.com", ControlRole::User),
    ] {
        let response = request_json(
            app.clone(),
            Method::POST,
            "/users",
            Some(&admin_auth.access_token),
            &CreateUserRequest {
                email: email.to_string(),
                password: "password-123".to_string(),
                display_name: email.to_string(),
                role,
                enabled: true,
            },
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
    }

    let page: Page<UserSummary> = json(
        get(
            app,
            "/users?q=ops&role=admin&sort=email&limit=1&offset=1",
            &admin_auth.access_token,
        )
        .await,
    )
    .await;

    assert_eq!(page.total, 2);
    assert_eq!(page.limit, 1);
    assert_eq!(page.offset, 1);
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].email, "ops-beta@example.com");
    assert_eq!(page.items[0].role, ControlRole::Admin);
}

#[tokio::test]
async fn admin_audit_logs_record_management_actions() {
    let state = ControlState::new(
        "dev-secret",
        "relay.example.com:4443",
        "punch.example.com:3478",
    );
    let admin_auth = state
        .bootstrap_admin_user(RegisterUserRequest {
            email: "admin@example.com".to_string(),
            password: "admin-password-123".to_string(),
            display_name: "Admin".to_string(),
        })
        .unwrap();
    let app = routes(state);
    let ordinary = register_user(app.clone(), "audit-user@example.com").await;

    assert_eq!(
        get(app.clone(), "/audit-logs", &ordinary.access_token)
            .await
            .status(),
        StatusCode::FORBIDDEN
    );

    let created_user = request_json(
        app.clone(),
        Method::POST,
        "/users",
        Some(&admin_auth.access_token),
        &CreateUserRequest {
            email: "audited-member@example.com".to_string(),
            password: "audited-password-123".to_string(),
            display_name: "Audited Member".to_string(),
            role: ControlRole::User,
            enabled: true,
        },
    )
    .await;
    assert_eq!(created_user.status(), StatusCode::OK);
    let created_user: UserSummary = json(created_user).await;

    let status = request_json(
        app.clone(),
        Method::POST,
        &format!("/users/{}/status", created_user.user_id),
        Some(&admin_auth.access_token),
        &UpdateUserStatusRequest { enabled: false },
    )
    .await;
    assert_eq!(status.status(), StatusCode::OK);

    let plan = request_json(
        app.clone(),
        Method::POST,
        &format!("/plans/users/{}", created_user.user_id),
        Some(&admin_auth.access_token),
        &UpdateUserPlanRequest {
            plan: Plan {
                plan_id: "audited".to_string(),
                name: "Audited".to_string(),
                max_controller_devices: 3,
                relay_limits: RelayLimits {
                    max_bps: 4_096,
                    max_streams: 6,
                    max_duration_sec: 3_600,
                    traffic_quota_bytes: 1_048_576,
                },
            },
        },
    )
    .await;
    assert_eq!(plan.status(), StatusCode::OK);

    let credential = request_json(
        app.clone(),
        Method::POST,
        "/relay-credentials",
        Some(&admin_auth.access_token),
        &CreateRelayCredentialRequest {
            relay_id: "relay_audit".to_string(),
            enabled: true,
        },
    )
    .await;
    assert_eq!(credential.status(), StatusCode::OK);

    let rotate = request_json(
        app.clone(),
        Method::POST,
        "/relay-credentials/relay_audit/rotate",
        Some(&admin_auth.access_token),
        &serde_json::json!({}),
    )
    .await;
    assert_eq!(rotate.status(), StatusCode::OK);

    let relay = request_json(
        app.clone(),
        Method::POST,
        "/relays/register",
        Some(&admin_auth.access_token),
        &RegisterRelayRequest {
            relay_id: "relay_audit".to_string(),
            relay_addr: "relay-audit.example.com:4443".to_string(),
            admin_addr: "relay-audit.example.com:9090".to_string(),
            capacity_streams: 16,
        },
    )
    .await;
    assert_eq!(relay.status(), StatusCode::OK);

    let logs: Page<AuditLogEntry> =
        json(get(app, "/audit-logs", &admin_auth.access_token).await).await;
    let actions: Vec<_> = logs
        .items
        .iter()
        .map(|entry| entry.action.as_str())
        .collect();

    for expected in [
        "user.create",
        "user.status.update",
        "plan.user.update",
        "relay_credential.create",
        "relay_credential.rotate",
        "relay.register",
    ] {
        assert!(
            actions.contains(&expected),
            "missing audit action {expected}"
        );
    }

    let user_create = logs
        .items
        .iter()
        .find(|entry| {
            entry.action == "user.create" && entry.target_id == created_user.user_id.to_string()
        })
        .unwrap();
    assert_eq!(user_create.actor_subject, "admin@example.com");
    assert_eq!(user_create.actor_role, ControlRole::Admin);
    assert_eq!(user_create.target_type, "user");
    assert!(!user_create.audit_id.is_empty());
    assert!(user_create.created_epoch_sec > 0);
}

#[tokio::test]
async fn admin_usage_summary_reports_user_sessions_and_granted_relay_quota() {
    let state = ControlState::new(
        "dev-secret",
        "relay.example.com:4443",
        "punch.example.com:3478",
    );
    let admin_token = admin_token(&state);
    let app = routes(state);
    let auth = register_user(app.clone(), "usage@example.com").await;

    assert_eq!(
        get(app.clone(), "/usage/users", &auth.access_token)
            .await
            .status(),
        StatusCode::FORBIDDEN
    );

    let usage_plan = Plan {
        plan_id: "usage".to_string(),
        name: "Usage".to_string(),
        max_controller_devices: 3,
        relay_limits: RelayLimits {
            max_bps: 9_999,
            max_streams: 7,
            max_duration_sec: 3_600,
            traffic_quota_bytes: 12_345,
        },
    };
    let plan_response = request_json(
        app.clone(),
        Method::POST,
        &format!("/plans/users/{}", auth.user_id),
        Some(&admin_token),
        &UpdateUserPlanRequest {
            plan: usage_plan.clone(),
        },
    )
    .await;
    assert_eq!(plan_response.status(), StatusCode::OK);

    assert_eq!(
        request_json(
            app.clone(),
            Method::POST,
            "/agent/register",
            Some(&auth.access_token),
            &device("server_usage"),
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        request_json(
            app.clone(),
            Method::POST,
            "/agent/services",
            Some(&auth.access_token),
            &vec![service("server_usage", "svc_usage")],
        )
        .await
        .status(),
        StatusCode::OK
    );

    let session = request_json(
        app.clone(),
        Method::POST,
        "/sessions",
        Some(&auth.access_token),
        &mobilecode_connect_control_client::CreateSessionRequest {
            client_id: "phone_usage".to_string(),
            device_id: DeviceId::new("server_usage"),
            service_id: ServiceId::new("svc_usage"),
        },
    )
    .await;
    assert_eq!(session.status(), StatusCode::OK);

    let summaries: Page<UserUsageSummary> =
        json(get(app, "/usage/users", &admin_token).await).await;
    let summary = summaries
        .items
        .iter()
        .find(|summary| summary.user_id == auth.user_id)
        .unwrap();

    assert_eq!(summary.email, "usage@example.com");
    assert_eq!(summary.plan_id, "usage");
    assert_eq!(summary.max_controller_devices, 3);
    assert_eq!(summary.controller_count, 1);
    assert_eq!(summary.device_count, 1);
    assert_eq!(summary.session_count, 1);
    assert_eq!(summary.pending_sessions, 1);
    assert_eq!(summary.claimed_sessions, 0);
    assert_eq!(summary.bound_sessions, 0);
    assert_eq!(summary.closed_sessions, 0);
    assert_eq!(summary.expired_sessions, 0);
    assert_eq!(summary.current_session_quota_bytes, 12_345);
    assert_eq!(summary.relay_quota_granted_bytes, 12_345);
}

#[tokio::test]
async fn relay_reports_actual_session_usage_to_control_summary() {
    let state = ControlState::new(
        "dev-secret",
        "relay.example.com:4443",
        "punch.example.com:3478",
    );
    let admin_token = admin_token(&state);
    let relay_control_token = relay_token(&state, "relay_usage");
    let wrong_relay_control_token = relay_token(&state, "relay_other");
    let app = routes(state);
    let auth = register_user(app.clone(), "actual-usage@example.com").await;

    assert_eq!(
        request_json(
            app.clone(),
            Method::POST,
            "/relays/register",
            Some(&relay_control_token),
            &RegisterRelayRequest {
                relay_id: "relay_usage".to_string(),
                relay_addr: "relay.example.com:4443".to_string(),
                admin_addr: "relay.example.com:9090".to_string(),
                capacity_streams: 16,
            },
        )
        .await
        .status(),
        StatusCode::OK
    );

    assert_eq!(
        request_json(
            app.clone(),
            Method::POST,
            "/agent/register",
            Some(&auth.access_token),
            &device("server_actual_usage"),
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        request_json(
            app.clone(),
            Method::POST,
            "/agent/services",
            Some(&auth.access_token),
            &vec![service("server_actual_usage", "svc_actual_usage")],
        )
        .await
        .status(),
        StatusCode::OK
    );

    let session_response = request_json(
        app.clone(),
        Method::POST,
        "/sessions",
        Some(&auth.access_token),
        &mobilecode_connect_control_client::CreateSessionRequest {
            client_id: "phone_actual_usage".to_string(),
            device_id: DeviceId::new("server_actual_usage"),
            service_id: ServiceId::new("svc_actual_usage"),
        },
    )
    .await;
    assert_eq!(session_response.status(), StatusCode::OK);
    let session: mobilecode_connect_control_client::CreateSessionResponse =
        json(session_response).await;

    let report = ReportRelaySessionUsageRequest {
        relay_id: "relay_usage".to_string(),
        sessions: vec![RelaySessionUsageReport {
            session_id: session.session_id.clone(),
            stats: TrafficStats {
                session_id: Some(session.session_id.clone()),
                uplink_bytes: 11,
                downlink_bytes: 17,
                total_bytes: 28,
                active_streams: 1,
                duration_sec: 2,
            },
        }],
    };

    assert_eq!(
        request_json(
            app.clone(),
            Method::POST,
            "/usage/relay-sessions",
            Some(&auth.access_token),
            &report,
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        request_json(
            app.clone(),
            Method::POST,
            "/usage/relay-sessions",
            Some(&wrong_relay_control_token),
            &report,
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        request_json(
            app.clone(),
            Method::POST,
            "/usage/relay-sessions",
            Some(&relay_control_token),
            &report,
        )
        .await
        .status(),
        StatusCode::NO_CONTENT
    );

    let summaries: Page<UserUsageSummary> =
        json(get(app, "/usage/users", &admin_token).await).await;
    let summary = summaries
        .items
        .iter()
        .find(|summary| summary.user_id == auth.user_id)
        .unwrap();
    assert_eq!(summary.actual_uplink_bytes, 11);
    assert_eq!(summary.actual_downlink_bytes, 17);
    assert_eq!(summary.actual_total_bytes, 28);
}

#[tokio::test]
async fn plan_relay_traffic_quota_blocks_new_sessions_after_actual_usage_exhausts_quota() {
    let state = ControlState::new(
        "dev-secret",
        "relay.example.com:4443",
        "punch.example.com:3478",
    );
    let admin_token = admin_token(&state);
    let app = routes(state);
    let auth = register_user(app.clone(), "quota-limit@example.com").await;

    let plan = request_json(
        app.clone(),
        Method::POST,
        &format!("/plans/users/{}", auth.user_id),
        Some(&admin_token),
        &UpdateUserPlanRequest {
            plan: Plan {
                plan_id: "quota-limit".to_string(),
                name: "Quota Limit".to_string(),
                max_controller_devices: 3,
                relay_limits: RelayLimits {
                    max_bps: 9_999,
                    max_streams: 7,
                    max_duration_sec: 3_600,
                    traffic_quota_bytes: 28,
                },
            },
        },
    )
    .await;
    assert_eq!(plan.status(), StatusCode::OK);

    assert_eq!(
        request_json(
            app.clone(),
            Method::POST,
            "/agent/register",
            Some(&auth.access_token),
            &device("server_quota_limit"),
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        request_json(
            app.clone(),
            Method::POST,
            "/agent/services",
            Some(&auth.access_token),
            &vec![service("server_quota_limit", "svc_quota_limit")],
        )
        .await
        .status(),
        StatusCode::OK
    );

    let first_session = request_json(
        app.clone(),
        Method::POST,
        "/sessions",
        Some(&auth.access_token),
        &mobilecode_connect_control_client::CreateSessionRequest {
            client_id: "phone_quota_limit".to_string(),
            device_id: DeviceId::new("server_quota_limit"),
            service_id: ServiceId::new("svc_quota_limit"),
        },
    )
    .await;
    assert_eq!(first_session.status(), StatusCode::OK);
    let first_session: mobilecode_connect_control_client::CreateSessionResponse =
        json(first_session).await;

    let report = request_json(
        app.clone(),
        Method::POST,
        "/usage/relay-sessions",
        Some(&admin_token),
        &ReportRelaySessionUsageRequest {
            relay_id: "relay_default".to_string(),
            sessions: vec![RelaySessionUsageReport {
                session_id: first_session.session_id,
                stats: TrafficStats {
                    session_id: None,
                    uplink_bytes: 11,
                    downlink_bytes: 17,
                    total_bytes: 28,
                    active_streams: 0,
                    duration_sec: 4,
                },
            }],
        },
    )
    .await;
    assert_eq!(report.status(), StatusCode::NO_CONTENT);

    let blocked_session = request_json(
        app,
        Method::POST,
        "/sessions",
        Some(&auth.access_token),
        &mobilecode_connect_control_client::CreateSessionRequest {
            client_id: "phone_quota_limit".to_string(),
            device_id: DeviceId::new("server_quota_limit"),
            service_id: ServiceId::new("svc_quota_limit"),
        },
    )
    .await;
    assert_eq!(blocked_session.status(), StatusCode::PAYMENT_REQUIRED);
}

#[tokio::test]
async fn admin_resets_user_usage_period_and_allows_new_sessions() {
    let state = ControlState::new(
        "dev-secret",
        "relay.example.com:4443",
        "punch.example.com:3478",
    );
    let admin_token = admin_token(&state);
    let app = routes(state);
    let auth = register_user(app.clone(), "quota-reset@example.com").await;

    let plan = request_json(
        app.clone(),
        Method::POST,
        &format!("/plans/users/{}", auth.user_id),
        Some(&admin_token),
        &UpdateUserPlanRequest {
            plan: Plan {
                plan_id: "quota-reset".to_string(),
                name: "Quota Reset".to_string(),
                max_controller_devices: 3,
                relay_limits: RelayLimits {
                    max_bps: 9_999,
                    max_streams: 7,
                    max_duration_sec: 3_600,
                    traffic_quota_bytes: 28,
                },
            },
        },
    )
    .await;
    assert_eq!(plan.status(), StatusCode::OK);
    assert_eq!(
        request_json(
            app.clone(),
            Method::POST,
            "/agent/register",
            Some(&auth.access_token),
            &device("server_quota_reset"),
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        request_json(
            app.clone(),
            Method::POST,
            "/agent/services",
            Some(&auth.access_token),
            &vec![service("server_quota_reset", "svc_quota_reset")],
        )
        .await
        .status(),
        StatusCode::OK
    );

    let first_session = request_json(
        app.clone(),
        Method::POST,
        "/sessions",
        Some(&auth.access_token),
        &mobilecode_connect_control_client::CreateSessionRequest {
            client_id: "phone_quota_reset".to_string(),
            device_id: DeviceId::new("server_quota_reset"),
            service_id: ServiceId::new("svc_quota_reset"),
        },
    )
    .await;
    assert_eq!(first_session.status(), StatusCode::OK);
    let first_session: mobilecode_connect_control_client::CreateSessionResponse =
        json(first_session).await;

    let report = request_json(
        app.clone(),
        Method::POST,
        "/usage/relay-sessions",
        Some(&admin_token),
        &ReportRelaySessionUsageRequest {
            relay_id: "relay_default".to_string(),
            sessions: vec![RelaySessionUsageReport {
                session_id: first_session.session_id,
                stats: TrafficStats {
                    session_id: None,
                    uplink_bytes: 11,
                    downlink_bytes: 17,
                    total_bytes: 28,
                    active_streams: 0,
                    duration_sec: 4,
                },
            }],
        },
    )
    .await;
    assert_eq!(report.status(), StatusCode::NO_CONTENT);

    let blocked_session = request_json(
        app.clone(),
        Method::POST,
        "/sessions",
        Some(&auth.access_token),
        &mobilecode_connect_control_client::CreateSessionRequest {
            client_id: "phone_quota_reset".to_string(),
            device_id: DeviceId::new("server_quota_reset"),
            service_id: ServiceId::new("svc_quota_reset"),
        },
    )
    .await;
    assert_eq!(blocked_session.status(), StatusCode::PAYMENT_REQUIRED);

    assert_eq!(
        post_empty(
            app.clone(),
            &format!("/usage/users/{}/reset", auth.user_id),
            Some(&auth.access_token),
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );

    let reset = post_empty(
        app.clone(),
        &format!("/usage/users/{}/reset", auth.user_id),
        Some(&admin_token),
    )
    .await;
    assert_eq!(reset.status(), StatusCode::OK);
    let reset: UserUsagePeriod = json(reset).await;
    assert_eq!(reset.user_id, auth.user_id);
    assert!(reset.current_period_started_epoch_sec > 0);

    let summaries: Page<UserUsageSummary> =
        json(get(app.clone(), "/usage/users", &admin_token).await).await;
    let summary = summaries
        .items
        .iter()
        .find(|summary| summary.user_id == auth.user_id)
        .unwrap();
    assert_eq!(summary.actual_total_bytes, 0);
    assert_eq!(
        summary.current_period_started_epoch_sec,
        reset.current_period_started_epoch_sec
    );

    let next_session = request_json(
        app,
        Method::POST,
        "/sessions",
        Some(&auth.access_token),
        &mobilecode_connect_control_client::CreateSessionRequest {
            client_id: "phone_quota_reset".to_string(),
            device_id: DeviceId::new("server_quota_reset"),
            service_id: ServiceId::new("svc_quota_reset"),
        },
    )
    .await;
    assert_eq!(next_session.status(), StatusCode::OK);
}

#[tokio::test]
async fn admin_usage_summary_supports_sort_limit_and_offset() {
    let state = ControlState::new(
        "dev-secret",
        "relay.example.com:4443",
        "punch.example.com:3478",
    );
    let admin_token = admin_token(&state);
    let relay_control_token = relay_token(&state, "relay_usage_query");
    let app = routes(state);

    let register_relay = request_json(
        app.clone(),
        Method::POST,
        "/relays/register",
        Some(&relay_control_token),
        &RegisterRelayRequest {
            relay_id: "relay_usage_query".to_string(),
            relay_addr: "relay.example.com:4443".to_string(),
            admin_addr: "relay.example.com:9090".to_string(),
            capacity_streams: 16,
        },
    )
    .await;
    assert_eq!(register_relay.status(), StatusCode::OK);

    let mut reports = Vec::new();
    for (email, device_id, service_id, client_id, uplink, downlink) in [
        (
            "usage-a@example.com",
            "server_query_a",
            "svc_query_a",
            "phone_query_a",
            3,
            7,
        ),
        (
            "usage-b@example.com",
            "server_query_b",
            "svc_query_b",
            "phone_query_b",
            11,
            19,
        ),
        (
            "usage-c@example.com",
            "server_query_c",
            "svc_query_c",
            "phone_query_c",
            5,
            15,
        ),
    ] {
        let auth = register_user(app.clone(), email).await;
        assert_eq!(
            request_json(
                app.clone(),
                Method::POST,
                "/agent/register",
                Some(&auth.access_token),
                &device(device_id),
            )
            .await
            .status(),
            StatusCode::OK
        );
        assert_eq!(
            request_json(
                app.clone(),
                Method::POST,
                "/agent/services",
                Some(&auth.access_token),
                &vec![service(device_id, service_id)],
            )
            .await
            .status(),
            StatusCode::OK
        );
        let session_response = request_json(
            app.clone(),
            Method::POST,
            "/sessions",
            Some(&auth.access_token),
            &mobilecode_connect_control_client::CreateSessionRequest {
                client_id: client_id.to_string(),
                device_id: DeviceId::new(device_id),
                service_id: ServiceId::new(service_id),
            },
        )
        .await;
        assert_eq!(session_response.status(), StatusCode::OK);
        let session: mobilecode_connect_control_client::CreateSessionResponse =
            json(session_response).await;
        reports.push(RelaySessionUsageReport {
            session_id: session.session_id.clone(),
            stats: TrafficStats {
                session_id: Some(session.session_id),
                uplink_bytes: uplink,
                downlink_bytes: downlink,
                total_bytes: uplink + downlink,
                duration_sec: 1,
                active_streams: 0,
            },
        });
    }

    let report = request_json(
        app.clone(),
        Method::POST,
        "/usage/relay-sessions",
        Some(&relay_control_token),
        &ReportRelaySessionUsageRequest {
            relay_id: "relay_usage_query".to_string(),
            sessions: reports,
        },
    )
    .await;
    assert_eq!(report.status(), StatusCode::NO_CONTENT);

    let ordinary = register_user(app.clone(), "usage-query-ordinary@example.com").await;
    assert_eq!(
        get(
            app.clone(),
            "/usage/users?sort=actual_total_bytes&limit=1",
            &ordinary.access_token,
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );

    let default_summaries: Page<UserUsageSummary> =
        json(get(app.clone(), "/usage/users", &admin_token).await).await;
    assert!(default_summaries
        .items
        .iter()
        .any(|summary| summary.email == "usage-a@example.com"));
    assert!(default_summaries
        .items
        .iter()
        .any(|summary| summary.email == "usage-b@example.com"));
    assert!(default_summaries
        .items
        .iter()
        .any(|summary| summary.email == "usage-c@example.com"));

    let summaries: Page<UserUsageSummary> = json(
        get(
            app,
            "/usage/users?sort=actual_total_bytes&limit=2&offset=1",
            &admin_token,
        )
        .await,
    )
    .await;

    assert_eq!(summaries.total, 4);
    assert_eq!(summaries.limit, 2);
    assert_eq!(summaries.offset, 1);
    assert_eq!(summaries.items.len(), 2);
    assert_eq!(summaries.items[0].email, "usage-c@example.com");
    assert_eq!(summaries.items[0].actual_total_bytes, 20);
    assert_eq!(summaries.items[1].email, "usage-a@example.com");
    assert_eq!(summaries.items[1].actual_total_bytes, 10);
}

#[tokio::test]
async fn plan_enforces_controller_device_limit() {
    let state = ControlState::new(
        "dev-secret",
        "relay.example.com:4443",
        "punch.example.com:3478",
    );
    let app = routes(state);
    let auth = register_user(app.clone(), "limited@example.com").await;

    for client_id in ["phone_001", "laptop_001"] {
        let response = request_json(
            app.clone(),
            Method::POST,
            "/controllers/register",
            Some(&auth.access_token),
            &RegisterControllerDeviceRequest {
                client_id: client_id.to_string(),
                name: client_id.to_string(),
            },
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
    }

    let response = request_json(
        app,
        Method::POST,
        "/controllers/register",
        Some(&auth.access_token),
        &RegisterControllerDeviceRequest {
            client_id: "tablet_001".to_string(),
            name: "Tablet".to_string(),
        },
    )
    .await;
    assert_eq!(response.status(), StatusCode::PAYMENT_REQUIRED);
}

#[tokio::test]
async fn controller_management_lists_and_removes_controller_devices() {
    let state = ControlState::new(
        "dev-secret",
        "relay.example.com:4443",
        "punch.example.com:3478",
    );
    let app = routes(state);
    let auth = register_user(app.clone(), "controllers@example.com").await;

    for client_id in ["phone_001", "laptop_001"] {
        let response = request_json(
            app.clone(),
            Method::POST,
            "/controllers/register",
            Some(&auth.access_token),
            &RegisterControllerDeviceRequest {
                client_id: client_id.to_string(),
                name: client_id.to_string(),
            },
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
    }

    let controllers: Page<mobilecode_connect_control_client::ControllerDevice> =
        json(get(app.clone(), "/controllers", &auth.access_token).await).await;
    assert_eq!(controllers.items.len(), 2);
    assert!(controllers
        .items
        .iter()
        .any(|controller| controller.client_id.as_str() == "phone_001"));

    let delete_response = delete(app.clone(), "/controllers/phone_001", &auth.access_token).await;
    assert_eq!(delete_response.status(), StatusCode::NO_CONTENT);

    let controllers: Page<mobilecode_connect_control_client::ControllerDevice> =
        json(get(app.clone(), "/controllers", &auth.access_token).await).await;
    assert_eq!(controllers.items.len(), 1);
    assert!(!controllers
        .items
        .iter()
        .any(|controller| controller.client_id.as_str() == "phone_001"));

    let response = request_json(
        app,
        Method::POST,
        "/controllers/register",
        Some(&auth.access_token),
        &RegisterControllerDeviceRequest {
            client_id: "tablet_001".to_string(),
            name: "Tablet".to_string(),
        },
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn controlled_device_management_lists_gets_and_removes_devices() {
    let state = ControlState::new(
        "dev-secret",
        "relay.example.com:4443",
        "punch.example.com:3478",
    );
    let app = routes(state);
    let auth = register_user(app.clone(), "devices@example.com").await;

    assert_eq!(
        request_json(
            app.clone(),
            Method::POST,
            "/agent/register",
            Some(&auth.access_token),
            &device("server_001"),
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        request_json(
            app.clone(),
            Method::POST,
            "/agent/services",
            Some(&auth.access_token),
            &vec![service("server_001", "svc_web")],
        )
        .await
        .status(),
        StatusCode::OK
    );

    let devices: Page<Device> = json(get(app.clone(), "/devices", &auth.access_token).await).await;
    assert_eq!(devices.items.len(), 1);
    assert_eq!(devices.items[0].device_id.as_str(), "server_001");

    let fetched: Device =
        json(get(app.clone(), "/devices/server_001", &auth.access_token).await).await;
    assert_eq!(fetched.device_id.as_str(), "server_001");

    let delete_response = delete(app.clone(), "/devices/server_001", &auth.access_token).await;
    assert_eq!(delete_response.status(), StatusCode::NO_CONTENT);

    let devices: Page<Device> = json(get(app.clone(), "/devices", &auth.access_token).await).await;
    assert!(devices.items.is_empty());
    let services: Vec<Service> = json(
        get(
            app.clone(),
            "/mobile/devices/server_001/services",
            &auth.access_token,
        )
        .await,
    )
    .await;
    assert!(services.is_empty());

    let session_response = request_json(
        app,
        Method::POST,
        "/sessions",
        Some(&auth.access_token),
        &mobilecode_connect_control_client::CreateSessionRequest {
            client_id: "phone_001".to_string(),
            device_id: DeviceId::new("server_001"),
            service_id: ServiceId::new("svc_web"),
        },
    )
    .await;
    assert_eq!(session_response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn relay_pool_lists_registered_relays() {
    let state = ControlState::new(
        "dev-secret",
        "relay.example.com:4443",
        "punch.example.com:3478",
    );
    let admin_token = admin_token(&state);
    let app = routes(state);

    let response = request_json(
        app.clone(),
        Method::POST,
        "/relays/register",
        Some(&admin_token),
        &RegisterRelayRequest {
            relay_id: "relay_cn_001".to_string(),
            relay_addr: "127.0.0.1:4443".to_string(),
            admin_addr: "127.0.0.1:9090".to_string(),
            capacity_streams: 64,
        },
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);

    let relays: Page<RelayNode> = json(get(app, "/relays", &admin_token).await).await;
    assert!(relays
        .items
        .iter()
        .any(|relay| relay.relay_id == "relay_cn_001"
            && relay.relay_addr == "127.0.0.1:4443"
            && relay.healthy));
}

#[tokio::test]
async fn relay_role_token_registers_and_heartbeats_only_its_own_relay() {
    let state = ControlState::new(
        "dev-secret",
        "seed-relay.example.com:4443",
        "punch.example.com:3478",
    );
    let admin_token = admin_token(&state);
    let relay_token = relay_token(&state, "relay_self");
    let app = routes(state);

    let register = request_json(
        app.clone(),
        Method::POST,
        "/relays/register",
        Some(&relay_token),
        &RegisterRelayRequest {
            relay_id: "relay_self".to_string(),
            relay_addr: "relay-self.example.com:4443".to_string(),
            admin_addr: "relay-self.example.com:9090".to_string(),
            capacity_streams: 64,
        },
    )
    .await;
    assert_eq!(register.status(), StatusCode::OK);

    let heartbeat = request_json(
        app.clone(),
        Method::POST,
        "/relays/relay_self",
        Some(&relay_token),
        &UpdateRelayRequest {
            relay_addr: "relay-self-new.example.com:4443".to_string(),
            admin_addr: "relay-self-new.example.com:9090".to_string(),
            capacity_streams: 128,
            healthy: true,
        },
    )
    .await;
    assert_eq!(heartbeat.status(), StatusCode::OK);
    let heartbeat: RelayNode = json(heartbeat).await;
    assert_eq!(heartbeat.relay_addr, "relay-self-new.example.com:4443");
    assert_eq!(heartbeat.capacity_streams, 128);
    assert!(heartbeat.healthy);

    let wrong_register = request_json(
        app.clone(),
        Method::POST,
        "/relays/register",
        Some(&relay_token),
        &RegisterRelayRequest {
            relay_id: "relay_other".to_string(),
            relay_addr: "relay-other.example.com:4443".to_string(),
            admin_addr: "relay-other.example.com:9090".to_string(),
            capacity_streams: 64,
        },
    )
    .await;
    assert_eq!(wrong_register.status(), StatusCode::FORBIDDEN);

    let wrong_heartbeat = request_json(
        app.clone(),
        Method::POST,
        "/relays/relay_other",
        Some(&relay_token),
        &UpdateRelayRequest {
            relay_addr: "relay-other.example.com:4443".to_string(),
            admin_addr: "relay-other.example.com:9090".to_string(),
            capacity_streams: 64,
            healthy: true,
        },
    )
    .await;
    assert_eq!(wrong_heartbeat.status(), StatusCode::FORBIDDEN);

    assert_eq!(
        get(app.clone(), "/users", &relay_token).await.status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        get(app.clone(), "/relays", &relay_token).await.status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        get(app.clone(), "/controllers", &relay_token)
            .await
            .status(),
        StatusCode::FORBIDDEN
    );

    let relays: Page<RelayNode> = json(get(app, "/relays", &admin_token).await).await;
    assert!(relays
        .items
        .iter()
        .any(|relay| relay.relay_id == "relay_self"
            && relay.relay_addr == "relay-self-new.example.com:4443"
            && relay.capacity_streams == 128));
    assert!(!relays
        .items
        .iter()
        .any(|relay| relay.relay_id == "relay_other"));
}

#[tokio::test]
async fn admin_rotates_and_disables_relay_credentials() {
    let state = ControlState::new(
        "dev-secret",
        "seed-relay.example.com:4443",
        "punch.example.com:3478",
    );
    let admin_token = admin_token(&state);
    let app = routes(state.clone());
    let ordinary = register_user(app.clone(), "relay-credential-user@example.com").await;

    assert_eq!(
        get(app.clone(), "/relay-credentials", &ordinary.access_token)
            .await
            .status(),
        StatusCode::FORBIDDEN
    );

    let forbidden_create = request_json(
        app.clone(),
        Method::POST,
        "/relay-credentials",
        Some(&ordinary.access_token),
        &CreateRelayCredentialRequest {
            relay_id: "relay_rotate".to_string(),
            enabled: true,
        },
    )
    .await;
    assert_eq!(forbidden_create.status(), StatusCode::FORBIDDEN);

    let created = request_json(
        app.clone(),
        Method::POST,
        "/relay-credentials",
        Some(&admin_token),
        &CreateRelayCredentialRequest {
            relay_id: "relay_rotate".to_string(),
            enabled: true,
        },
    )
    .await;
    assert_eq!(created.status(), StatusCode::OK);
    let created: RelayCredential = json(created).await;
    assert_eq!(created.relay_id, "relay_rotate");
    assert!(created.enabled);
    assert_eq!(created.token_version, 1);

    let credentials: Page<RelayCredential> =
        json(get(app.clone(), "/relay-credentials", &admin_token).await).await;
    assert!(credentials
        .items
        .iter()
        .any(|credential| credential.relay_id == "relay_rotate"));

    let fetched: RelayCredential =
        json(get(app.clone(), "/relay-credentials/relay_rotate", &admin_token).await).await;
    assert_eq!(fetched, created);

    let token_v1 = relay_token(&state, "relay_rotate");
    let register = request_json(
        app.clone(),
        Method::POST,
        "/relays/register",
        Some(&token_v1),
        &RegisterRelayRequest {
            relay_id: "relay_rotate".to_string(),
            relay_addr: "relay-rotate.example.com:4443".to_string(),
            admin_addr: "relay-rotate.example.com:9090".to_string(),
            capacity_streams: 32,
        },
    )
    .await;
    assert_eq!(register.status(), StatusCode::OK);

    let rotated = request_json(
        app.clone(),
        Method::POST,
        "/relay-credentials/relay_rotate/rotate",
        Some(&admin_token),
        &serde_json::json!({}),
    )
    .await;
    assert_eq!(rotated.status(), StatusCode::OK);
    let rotated: RelayCredential = json(rotated).await;
    assert_eq!(rotated.token_version, 2);
    assert!(rotated.enabled);

    let old_token_heartbeat = request_json(
        app.clone(),
        Method::POST,
        "/relays/relay_rotate",
        Some(&token_v1),
        &UpdateRelayRequest {
            relay_addr: "old-token.example.com:4443".to_string(),
            admin_addr: "old-token.example.com:9090".to_string(),
            capacity_streams: 32,
            healthy: true,
        },
    )
    .await;
    assert_eq!(old_token_heartbeat.status(), StatusCode::UNAUTHORIZED);

    let token_v2 = relay_token(&state, "relay_rotate");
    let new_token_heartbeat = request_json(
        app.clone(),
        Method::POST,
        "/relays/relay_rotate",
        Some(&token_v2),
        &UpdateRelayRequest {
            relay_addr: "new-token.example.com:4443".to_string(),
            admin_addr: "new-token.example.com:9090".to_string(),
            capacity_streams: 64,
            healthy: true,
        },
    )
    .await;
    assert_eq!(new_token_heartbeat.status(), StatusCode::OK);

    let disabled = request_json(
        app.clone(),
        Method::POST,
        "/relay-credentials/relay_rotate/status",
        Some(&admin_token),
        &UpdateRelayCredentialStatusRequest { enabled: false },
    )
    .await;
    assert_eq!(disabled.status(), StatusCode::OK);
    let disabled: RelayCredential = json(disabled).await;
    assert!(!disabled.enabled);
    assert_eq!(disabled.token_version, 2);
    assert!(state.issue_relay_token("relay_rotate").is_err());

    let disabled_heartbeat = request_json(
        app.clone(),
        Method::POST,
        "/relays/relay_rotate",
        Some(&token_v2),
        &UpdateRelayRequest {
            relay_addr: "disabled-token.example.com:4443".to_string(),
            admin_addr: "disabled-token.example.com:9090".to_string(),
            capacity_streams: 64,
            healthy: true,
        },
    )
    .await;
    assert_eq!(disabled_heartbeat.status(), StatusCode::UNAUTHORIZED);

    let missing = request_json(
        app,
        Method::POST,
        "/relay-credentials/relay_missing/rotate",
        Some(&admin_token),
        &serde_json::json!({}),
    )
    .await;
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn admin_creates_single_use_relay_bootstrap_for_relayd_install() {
    let state = ControlState::new(
        "dev-secret",
        "seed-relay.example.com:4443",
        "punch.example.com:3478",
    );
    let admin_token = admin_token(&state);
    let app = routes(state.clone());
    let ordinary = register_user(app.clone(), "relay-bootstrap-user@example.com").await;
    let request = CreateRelayBootstrapRequest {
        relay_id: "relay_bootstrap".to_string(),
        control_url: "https://control.example.com".to_string(),
        relay_addr: "relay-bootstrap.example.com:4443".to_string(),
        admin_addr: "should-be-ignored.example.com:9090".to_string(),
        capacity_streams: 128,
        heartbeat_interval_sec: 30,
        ttl_sec: 900,
    };

    let forbidden = request_json(
        app.clone(),
        Method::POST,
        "/relay-bootstraps",
        Some(&ordinary.access_token),
        &request,
    )
    .await;
    assert_eq!(forbidden.status(), StatusCode::FORBIDDEN);

    let created = request_json(
        app.clone(),
        Method::POST,
        "/relay-bootstraps",
        Some(&admin_token),
        &request,
    )
    .await;
    assert_eq!(created.status(), StatusCode::OK);
    let created: RelayBootstrapResponse = json(created).await;
    assert_eq!(created.relay_id, "relay_bootstrap");
    assert_eq!(created.control_url, "https://control.example.com");
    assert!(created.bootstrap_id.starts_with("rb_"));
    assert!(!created.bootstrap_token.is_empty());
    assert!(created.install_command.contains(&created.bootstrap_id));
    assert!(created.install_command.contains(&created.bootstrap_token));
    assert!(created
        .install_command
        .contains("--control-url https://control.example.com"));
    assert!(created
        .install_command
        .contains("--relayd-url https://control.example.com/relayd"));
    assert!(!created.install_command.contains("9090"));
    assert!(!created.install_command.contains("admin-listen"));
    assert!(created.install_command.contains("| sudo sh -s --"));
    assert!(created
        .no_service_install_command
        .contains("--control-url https://control.example.com"));
    assert!(created
        .no_service_install_command
        .contains("--relayd-url https://control.example.com/relayd"));
    assert!(created.no_service_install_command.contains("| sh -s --"));
    assert!(!created.no_service_install_command.contains("| sudo sh"));
    assert!(created.no_service_install_command.contains("--no-service"));
    assert!(!created.no_service_install_command.contains("9090"));
    assert!(!created.no_service_install_command.contains("admin-listen"));
    assert!(created
        .no_service_install_command
        .contains(&created.bootstrap_id));
    assert!(created
        .no_service_install_command
        .contains(&created.bootstrap_token));

    let exchange = request_json(
        app.clone(),
        Method::POST,
        &format!("/relay-bootstraps/{}/exchange", created.bootstrap_id),
        None,
        &RelayBootstrapExchangeRequest {
            bootstrap_token: created.bootstrap_token.clone(),
        },
    )
    .await;
    assert_eq!(exchange.status(), StatusCode::OK);
    let exchange: RelayBootstrapExchangeResponse = json(exchange).await;
    assert_eq!(exchange.relay_id, "relay_bootstrap");
    assert_eq!(exchange.control_url, "https://control.example.com");
    assert_eq!(exchange.relay_addr, "relay-bootstrap.example.com:4443");
    assert_eq!(exchange.admin_addr, "");
    assert_eq!(exchange.capacity_streams, 128);
    assert_eq!(exchange.heartbeat_interval_sec, 30);
    assert!(!exchange.control_token.is_empty());
    assert!(!exchange.token_secret.is_empty());

    let replay = request_json(
        app.clone(),
        Method::POST,
        &format!("/relay-bootstraps/{}/exchange", created.bootstrap_id),
        None,
        &RelayBootstrapExchangeRequest {
            bootstrap_token: created.bootstrap_token,
        },
    )
    .await;
    assert_eq!(replay.status(), StatusCode::UNAUTHORIZED);

    let register = request_json(
        app,
        Method::POST,
        "/relays/register",
        Some(&exchange.control_token),
        &RegisterRelayRequest {
            relay_id: exchange.relay_id,
            relay_addr: exchange.relay_addr,
            admin_addr: exchange.admin_addr,
            capacity_streams: exchange.capacity_streams,
        },
    )
    .await;
    assert_eq!(register.status(), StatusCode::OK);
    let relay: RelayNode = json(register).await;
    assert_eq!(relay.relay_id, "relay_bootstrap");
    assert_eq!(relay.admin_addr, "");
    assert!(!relay.admin_bound);
}

#[tokio::test]
async fn relayd_installer_script_is_downloadable_from_control() {
    let state = ControlState::new(
        "dev-secret",
        "seed-relay.example.com:4443",
        "punch.example.com:3478",
    );
    let app = routes(state);

    let response = get_without_token(app, "/install-relayd.sh").await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok()),
        Some("text/x-shellscript; charset=utf-8")
    );
    let body = text(response).await;
    assert!(body.starts_with("#!/usr/bin/env bash"));
    assert!(body.contains("--no-service"));
    assert!(body.contains("relay-bootstraps/$bootstrap_id/exchange"));
}

#[tokio::test]
async fn relayd_binary_is_downloadable_from_configured_path() {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let relayd_path = std::env::temp_dir().join(format!(
        "quic-test-relayd-download-{}-{suffix}",
        std::process::id()
    ));
    fs::write(&relayd_path, b"dummy-relayd-binary").unwrap();
    std::env::set_var("QUIC_TUNNEL_RELAYD_BINARY", &relayd_path);

    let state = ControlState::new(
        "dev-secret",
        "seed-relay.example.com:4443",
        "punch.example.com:3478",
    );
    let app = routes(state);
    let response = get_without_token(app, "/relayd").await;
    std::env::remove_var("QUIC_TUNNEL_RELAYD_BINARY");
    let _ = fs::remove_file(&relayd_path);

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok()),
        Some("application/octet-stream")
    );
    assert_eq!(
        response
            .headers()
            .get("content-disposition")
            .and_then(|value| value.to_str().ok()),
        Some("attachment; filename=\"relayd\"")
    );
    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    assert_eq!(&body[..], b"dummy-relayd-binary");
}

#[tokio::test]
async fn relay_bootstrap_normalizes_bare_control_host_for_local_install() {
    let state = ControlState::new(
        "dev-secret",
        "seed-relay.example.com:4443",
        "punch.example.com:3478",
    );
    let admin_token = admin_token(&state);
    let app = routes(state);

    let created = request_json(
        app.clone(),
        Method::POST,
        "/relay-bootstraps",
        Some(&admin_token),
        &CreateRelayBootstrapRequest {
            relay_id: "relay_local_bootstrap".to_string(),
            control_url: "127.0.0.1:4242".to_string(),
            relay_addr: "127.0.0.1:4433".to_string(),
            admin_addr: "127.0.0.1:9090".to_string(),
            capacity_streams: 32,
            heartbeat_interval_sec: 30,
            ttl_sec: 900,
        },
    )
    .await;
    assert_eq!(created.status(), StatusCode::OK);
    let created: RelayBootstrapResponse = json(created).await;
    assert_eq!(created.control_url, "http://127.0.0.1:4242");
    assert!(created
        .install_command
        .contains("curl -fsSL http://127.0.0.1:4242/install-relayd.sh"));
    assert!(created
        .install_command
        .contains("--control-url http://127.0.0.1:4242"));
    assert!(created
        .install_command
        .contains("--relayd-url http://127.0.0.1:4242/relayd"));
    assert!(created
        .no_service_install_command
        .contains("--control-url http://127.0.0.1:4242"));
    assert!(created
        .no_service_install_command
        .contains("--relayd-url http://127.0.0.1:4242/relayd"));

    let exchange = request_json(
        app,
        Method::POST,
        &format!("/relay-bootstraps/{}/exchange", created.bootstrap_id),
        None,
        &RelayBootstrapExchangeRequest {
            bootstrap_token: created.bootstrap_token,
        },
    )
    .await;
    assert_eq!(exchange.status(), StatusCode::OK);
    let exchange: RelayBootstrapExchangeResponse = json(exchange).await;
    assert_eq!(exchange.control_url, "http://127.0.0.1:4242");
}

#[tokio::test]
async fn relay_bootstrap_exchange_rejects_invalid_token() {
    let state = ControlState::new(
        "dev-secret",
        "seed-relay.example.com:4443",
        "punch.example.com:3478",
    );
    let admin_token = admin_token(&state);
    let app = routes(state);
    let created = request_json(
        app.clone(),
        Method::POST,
        "/relay-bootstraps",
        Some(&admin_token),
        &CreateRelayBootstrapRequest {
            relay_id: "relay_bad_bootstrap".to_string(),
            control_url: "https://control.example.com".to_string(),
            relay_addr: "relay-bad.example.com:4443".to_string(),
            admin_addr: String::new(),
            capacity_streams: 32,
            heartbeat_interval_sec: 30,
            ttl_sec: 900,
        },
    )
    .await;
    assert_eq!(created.status(), StatusCode::OK);
    let created: RelayBootstrapResponse = json(created).await;

    let invalid = request_json(
        app,
        Method::POST,
        &format!("/relay-bootstraps/{}/exchange", created.bootstrap_id),
        None,
        &RelayBootstrapExchangeRequest {
            bootstrap_token: "wrong-token".to_string(),
        },
    )
    .await;
    assert_eq!(invalid.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn relay_pool_updates_health_capacity_and_removes_relays() {
    let state = ControlState::new(
        "dev-secret",
        "seed-relay.example.com:4443",
        "punch.example.com:3478",
    );
    let admin_token = admin_token(&state);
    let app = routes(state);
    let auth = register_user(app.clone(), "relay-ops@example.com").await;

    for (relay_id, relay_addr, capacity_streams) in [
        ("relay_a", "relay-a.example.com:4443", 16),
        ("relay_b", "relay-b.example.com:4443", 64),
    ] {
        let response = request_json(
            app.clone(),
            Method::POST,
            "/relays/register",
            Some(&admin_token),
            &RegisterRelayRequest {
                relay_id: relay_id.to_string(),
                relay_addr: relay_addr.to_string(),
                admin_addr: format!("{relay_id}.example.com:9090"),
                capacity_streams,
            },
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
    }

    let update = request_json(
        app.clone(),
        Method::POST,
        "/relays/relay_b",
        Some(&admin_token),
        &UpdateRelayRequest {
            relay_addr: "relay-b-new.example.com:4443".to_string(),
            admin_addr: "relay-b-new.example.com:9090".to_string(),
            capacity_streams: 128,
            healthy: false,
        },
    )
    .await;
    assert_eq!(update.status(), StatusCode::OK);
    let updated: RelayNode = json(update).await;
    assert_eq!(updated.relay_addr, "relay-b-new.example.com:4443");
    assert_eq!(updated.capacity_streams, 128);
    assert!(!updated.healthy);

    assert_eq!(
        request_json(
            app.clone(),
            Method::POST,
            "/agent/register",
            Some(&auth.access_token),
            &device("server_relay_ops"),
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        request_json(
            app.clone(),
            Method::POST,
            "/agent/services",
            Some(&auth.access_token),
            &vec![service("server_relay_ops", "svc_relay_ops")],
        )
        .await
        .status(),
        StatusCode::OK
    );

    let session_response = request_json(
        app.clone(),
        Method::POST,
        "/sessions",
        Some(&auth.access_token),
        &mobilecode_connect_control_client::CreateSessionRequest {
            client_id: "phone_ops".to_string(),
            device_id: DeviceId::new("server_relay_ops"),
            service_id: ServiceId::new("svc_relay_ops"),
        },
    )
    .await;
    assert_eq!(session_response.status(), StatusCode::OK);
    let session: mobilecode_connect_control_client::CreateSessionResponse =
        json(session_response).await;
    assert_eq!(session.relay_addr, "relay-a.example.com:4443");

    let delete_response = delete(app.clone(), "/relays/relay_b", &admin_token).await;
    assert_eq!(delete_response.status(), StatusCode::NO_CONTENT);

    let relays: Page<RelayNode> = json(get(app, "/relays", &admin_token).await).await;
    assert!(!relays.items.iter().any(|relay| relay.relay_id == "relay_b"));
}

#[tokio::test]
async fn relay_health_report_updates_operational_snapshot() {
    let state = ControlState::new(
        "dev-secret",
        "seed-relay.example.com:4443",
        "punch.example.com:3478",
    )
    .with_relay_health_now_epoch_sec(1_000);
    let admin_token = admin_token(&state);
    let relay_token = relay_token(&state, "relay_health");
    let app = routes(state.clone());

    let register = request_json(
        app.clone(),
        Method::POST,
        "/relays/register",
        Some(&relay_token),
        &RegisterRelayRequest {
            relay_id: "relay_health".to_string(),
            relay_addr: "relay-health.example.com:4443".to_string(),
            admin_addr: "relay-health.example.com:9090".to_string(),
            capacity_streams: 128,
        },
    )
    .await;
    assert_eq!(register.status(), StatusCode::OK);
    let registered: RelayNode = json(register).await;
    assert_eq!(registered.admin_addr, "");
    assert!(!registered.admin_bound);

    state.set_relay_health_now_epoch_sec(1_010);
    let report = request_json(
        app.clone(),
        Method::POST,
        "/relays/relay_health/health",
        Some(&relay_token),
        &ReportRelayHealthRequest {
            relay_addr: "relay-health-new.example.com:4443".to_string(),
            admin_addr: "relay-health-new.example.com:9090".to_string(),
            capacity_streams: 256,
            health: RelayHealthReport {
                status: RelayHealthStatus::Degraded,
                reason: "stream_pressure".to_string(),
                relay_version: "1.2.3".to_string(),
                uptime_sec: 42,
                active_sessions: 3,
                active_streams: 7,
                total_uplink_bytes: 1_024,
                total_downlink_bytes: 2_048,
                total_bytes: 3_072,
                data_plane_bound: true,
                admin_bound: true,
            },
            sessions: Vec::new(),
        },
    )
    .await;
    assert_eq!(report.status(), StatusCode::OK);
    let relay: RelayNode = json(report).await;
    assert_eq!(relay.relay_addr, "relay-health-new.example.com:4443");
    assert_eq!(relay.admin_addr, "");
    assert_eq!(relay.capacity_streams, 256);
    assert!(!relay.healthy);
    assert_eq!(relay.health_status, RelayHealthStatus::Degraded);
    assert_eq!(relay.health_reason, "stream_pressure");
    assert_eq!(relay.relay_version, "1.2.3");
    assert_eq!(relay.uptime_sec, 42);
    assert_eq!(relay.active_sessions, 3);
    assert_eq!(relay.active_streams, 7);
    assert_eq!(relay.total_uplink_bytes, 1_024);
    assert_eq!(relay.total_downlink_bytes, 2_048);
    assert_eq!(relay.total_bytes, 3_072);
    assert!(relay.data_plane_bound);
    assert!(relay.admin_bound);
    assert_eq!(relay.last_seen_epoch_sec, 1_010);
    assert_eq!(relay.last_health_report_epoch_sec, 1_010);

    let relays: Page<RelayNode> = json(get(app, "/relays", &admin_token).await).await;
    let listed = relays
        .items
        .iter()
        .find(|relay| relay.relay_id == "relay_health")
        .unwrap();
    assert_eq!(listed.health_status, RelayHealthStatus::Degraded);
    assert_eq!(listed.health_reason, "stream_pressure");
    assert_eq!(listed.active_streams, 7);
}

#[tokio::test]
async fn relay_live_ops_reports_sessions_and_processes_disconnect_commands() {
    let state = ControlState::new(
        "dev-secret",
        "seed-relay.example.com:4443",
        "punch.example.com:3478",
    )
    .with_relay_health_now_epoch_sec(2_000);
    let admin_token = admin_token(&state);
    let live_relay_token = relay_token(&state, "relay_live_ops");
    let wrong_relay_token = relay_token(&state, "relay_other_live_ops");
    let app = routes(state.clone());
    let user = register_user(app.clone(), "relay-live-ops@example.com").await;

    let register = request_json(
        app.clone(),
        Method::POST,
        "/relays/register",
        Some(&live_relay_token),
        &RegisterRelayRequest {
            relay_id: "relay_live_ops".to_string(),
            relay_addr: "relay-live.example.com:4443".to_string(),
            admin_addr: String::new(),
            capacity_streams: 128,
        },
    )
    .await;
    assert_eq!(register.status(), StatusCode::OK);

    state.set_relay_health_now_epoch_sec(2_010);
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
            uplink_bytes: 1_024,
            downlink_bytes: 2_048,
            total_bytes: 3_072,
            duration_sec: 30,
            active_streams: 2,
        },
        last_seen_epoch_sec: 0,
    };
    let report = request_json(
        app.clone(),
        Method::POST,
        "/relays/relay_live_ops/health",
        Some(&live_relay_token),
        &ReportRelayHealthRequest {
            relay_addr: "relay-live.example.com:4443".to_string(),
            admin_addr: String::new(),
            capacity_streams: 128,
            health: RelayHealthReport {
                status: RelayHealthStatus::Healthy,
                reason: String::new(),
                relay_version: "1.2.3".to_string(),
                uptime_sec: 90,
                active_sessions: 1,
                active_streams: 2,
                total_uplink_bytes: 1_024,
                total_downlink_bytes: 2_048,
                total_bytes: 3_072,
                data_plane_bound: true,
                admin_bound: false,
            },
            sessions: vec![snapshot.clone()],
        },
    )
    .await;
    assert_eq!(report.status(), StatusCode::OK);

    let forbidden = get(
        app.clone(),
        "/relays/relay_live_ops/sessions",
        &user.access_token,
    )
    .await;
    assert_eq!(forbidden.status(), StatusCode::FORBIDDEN);

    let sessions: Page<RelaySessionSnapshot> =
        json(get(app.clone(), "/relays/relay_live_ops/sessions", &admin_token).await).await;
    assert_eq!(sessions.total, 1);
    assert_eq!(sessions.items[0].session_id, session_id);
    assert_eq!(sessions.items[0].state, "ready");
    assert!(sessions.items[0].mobile_bound);
    assert!(sessions.items[0].agent_bound);
    assert_eq!(sessions.items[0].stats.total_bytes, 3_072);
    assert_eq!(sessions.items[0].last_seen_epoch_sec, 2_010);

    let command_response = post_empty(
        app.clone(),
        "/relays/relay_live_ops/sessions/sess_live_ops_001/disconnect",
        Some(&admin_token),
    )
    .await;
    assert_eq!(command_response.status(), StatusCode::OK);
    let command: RelayCommand = json(command_response).await;
    assert_eq!(command.relay_id, "relay_live_ops");
    assert_eq!(command.kind, RelayCommandKind::DisconnectSession);
    assert_eq!(
        command.session_id,
        Some(SessionId::new("sess_live_ops_001"))
    );
    assert_eq!(command.status, RelayCommandStatus::Pending);
    assert_eq!(command.requested_epoch_sec, 2_010);

    let wrong_commands = get(
        app.clone(),
        "/relays/relay_live_ops/commands",
        &wrong_relay_token,
    )
    .await;
    assert_eq!(wrong_commands.status(), StatusCode::FORBIDDEN);

    let pending: Vec<RelayCommand> = json(
        get(
            app.clone(),
            "/relays/relay_live_ops/commands",
            &live_relay_token,
        )
        .await,
    )
    .await;
    assert_eq!(pending, vec![command.clone()]);

    state.set_relay_health_now_epoch_sec(2_020);
    let result = request_json(
        app.clone(),
        Method::POST,
        &format!(
            "/relays/relay_live_ops/commands/{}/result",
            command.command_id
        ),
        Some(&live_relay_token),
        &ReportRelayCommandResultRequest {
            status: RelayCommandStatus::Succeeded,
            message: "session closed locally".to_string(),
        },
    )
    .await;
    assert_eq!(result.status(), StatusCode::OK);
    let completed: RelayCommand = json(result).await;
    assert_eq!(completed.status, RelayCommandStatus::Succeeded);
    assert_eq!(completed.updated_epoch_sec, 2_020);
    assert_eq!(completed.message, "session closed locally");

    let pending_after: Vec<RelayCommand> =
        json(get(app, "/relays/relay_live_ops/commands", &live_relay_token).await).await;
    assert!(pending_after.is_empty());
}

#[tokio::test]
async fn relay_health_report_rejects_inconsistent_byte_totals() {
    let state = ControlState::new(
        "dev-secret",
        "seed-relay.example.com:4443",
        "punch.example.com:3478",
    );
    let relay_token = relay_token(&state, "relay_bad_health");
    let app = routes(state);

    let register = request_json(
        app.clone(),
        Method::POST,
        "/relays/register",
        Some(&relay_token),
        &RegisterRelayRequest {
            relay_id: "relay_bad_health".to_string(),
            relay_addr: "relay-bad-health.example.com:4443".to_string(),
            admin_addr: "relay-bad-health.example.com:9090".to_string(),
            capacity_streams: 128,
        },
    )
    .await;
    assert_eq!(register.status(), StatusCode::OK);

    let report = request_json(
        app,
        Method::POST,
        "/relays/relay_bad_health/health",
        Some(&relay_token),
        &ReportRelayHealthRequest {
            relay_addr: "relay-bad-health.example.com:4443".to_string(),
            admin_addr: "relay-bad-health.example.com:9090".to_string(),
            capacity_streams: 128,
            health: RelayHealthReport {
                status: RelayHealthStatus::Healthy,
                reason: String::new(),
                relay_version: "1.2.3".to_string(),
                uptime_sec: 1,
                active_sessions: 0,
                active_streams: 0,
                total_uplink_bytes: 10,
                total_downlink_bytes: 20,
                total_bytes: 99,
                data_plane_bound: true,
                admin_bound: true,
            },
            sessions: Vec::new(),
        },
    )
    .await;
    assert_eq!(report.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn stale_relay_heartbeat_marks_relay_unavailable_for_sessions() {
    let state = ControlState::new(
        "dev-secret",
        "seed-relay.example.com:4443",
        "punch.example.com:3478",
    )
    .with_relay_health_now_epoch_sec(1_000);
    let admin_token = admin_token(&state);
    let app = routes(state.clone());
    let auth = register_user(app.clone(), "relay-stale@example.com").await;

    let relay = request_json(
        app.clone(),
        Method::POST,
        "/relays/register",
        Some(&admin_token),
        &RegisterRelayRequest {
            relay_id: "relay_stale".to_string(),
            relay_addr: "relay-stale.example.com:4443".to_string(),
            admin_addr: "relay-stale.example.com:9090".to_string(),
            capacity_streams: 128,
        },
    )
    .await;
    assert_eq!(relay.status(), StatusCode::OK);
    let relay: RelayNode = json(relay).await;
    assert!(relay.healthy);
    assert_eq!(relay.last_seen_epoch_sec, 1_000);

    state.set_relay_health_now_epoch_sec(1_091);
    let relays: Page<RelayNode> = json(get(app.clone(), "/relays", &admin_token).await).await;
    let stale = relays
        .items
        .iter()
        .find(|relay| relay.relay_id == "relay_stale")
        .unwrap();
    assert!(!stale.healthy);
    assert_eq!(stale.health_status, RelayHealthStatus::Unhealthy);
    assert_eq!(stale.health_reason, "heartbeat_stale");
    assert_eq!(stale.last_seen_epoch_sec, 1_000);

    assert_eq!(
        request_json(
            app.clone(),
            Method::POST,
            "/agent/register",
            Some(&auth.access_token),
            &device("server_stale"),
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        request_json(
            app.clone(),
            Method::POST,
            "/agent/services",
            Some(&auth.access_token),
            &vec![service("server_stale", "svc_stale")],
        )
        .await
        .status(),
        StatusCode::OK
    );

    let session_response = request_json(
        app,
        Method::POST,
        "/sessions",
        Some(&auth.access_token),
        &mobilecode_connect_control_client::CreateSessionRequest {
            client_id: "phone_stale".to_string(),
            device_id: DeviceId::new("server_stale"),
            service_id: ServiceId::new("svc_stale"),
        },
    )
    .await;
    assert_eq!(session_response.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn plan_management_updates_controller_limit_and_session_relay_limits() {
    let state = ControlState::new(
        "dev-secret",
        "seed-relay.example.com:4443",
        "punch.example.com:3478",
    );
    let admin_token = admin_token(&state);
    let app = routes(state);
    let auth = register_user(app.clone(), "plan-owner@example.com").await;
    let upgraded = Plan {
        plan_id: "team".to_string(),
        name: "Team".to_string(),
        max_controller_devices: 1,
        relay_limits: RelayLimits {
            max_bps: 2_048,
            max_streams: 3,
            max_duration_sec: 60,
            traffic_quota_bytes: 4_096,
        },
    };

    let response = request_json(
        app.clone(),
        Method::POST,
        &format!("/plans/users/{}", auth.user_id),
        Some(&admin_token),
        &UpdateUserPlanRequest {
            plan: upgraded.clone(),
        },
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(json::<Plan>(response).await, upgraded);

    let current_plan: Plan =
        json(get(app.clone(), "/plans/current", &auth.access_token).await).await;
    assert_eq!(current_plan, upgraded);

    let missing_plan = get(app.clone(), "/plans/users/user_missing", &admin_token).await;
    assert_eq!(missing_plan.status(), StatusCode::NOT_FOUND);

    assert_eq!(
        request_json(
            app.clone(),
            Method::POST,
            "/relays/register",
            Some(&admin_token),
            &RegisterRelayRequest {
                relay_id: "relay_team".to_string(),
                relay_addr: "relay-team.example.com:4443".to_string(),
                admin_addr: "relay-team.example.com:9090".to_string(),
                capacity_streams: 128,
            },
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        request_json(
            app.clone(),
            Method::POST,
            "/agent/register",
            Some(&auth.access_token),
            &device("server_team"),
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        request_json(
            app.clone(),
            Method::POST,
            "/agent/services",
            Some(&auth.access_token),
            &vec![service("server_team", "svc_team")],
        )
        .await
        .status(),
        StatusCode::OK
    );

    let session_response = request_json(
        app.clone(),
        Method::POST,
        "/sessions",
        Some(&auth.access_token),
        &mobilecode_connect_control_client::CreateSessionRequest {
            client_id: "phone_001".to_string(),
            device_id: DeviceId::new("server_team"),
            service_id: ServiceId::new("svc_team"),
        },
    )
    .await;
    assert_eq!(session_response.status(), StatusCode::OK);
    let session: mobilecode_connect_control_client::CreateSessionResponse =
        json(session_response).await;
    let claims = TokenSigner::new(TokenKey::new("dev-secret"))
        .verify_relay(&session.relay_token, 1_767_000_000)
        .unwrap();
    assert_eq!(claims.max_streams, upgraded.relay_limits.max_streams);
    assert_eq!(claims.max_bps, upgraded.relay_limits.max_bps);
    assert_eq!(
        claims.traffic_quota_bytes,
        upgraded.relay_limits.traffic_quota_bytes
    );

    let second_controller = request_json(
        app,
        Method::POST,
        "/controllers/register",
        Some(&auth.access_token),
        &RegisterControllerDeviceRequest {
            client_id: "laptop_001".to_string(),
            name: "Laptop".to_string(),
        },
    )
    .await;
    assert_eq!(second_controller.status(), StatusCode::PAYMENT_REQUIRED);
}

#[tokio::test]
async fn admin_plan_catalog_creates_templates_and_assigns_users() {
    let state = ControlState::new(
        "dev-secret",
        "seed-relay.example.com:4443",
        "punch.example.com:3478",
    );
    let admin_token = admin_token(&state);
    let app = routes(state);
    let auth = register_user(app.clone(), "catalog-owner@example.com").await;
    let team = Plan {
        plan_id: "team".to_string(),
        name: "Team".to_string(),
        max_controller_devices: 6,
        relay_limits: RelayLimits {
            max_bps: 8_192,
            max_streams: 12,
            max_duration_sec: 7_200,
            traffic_quota_bytes: 2_097_152,
        },
    };

    let forbidden = get(app.clone(), "/plans/catalog", &auth.access_token).await;
    assert_eq!(forbidden.status(), StatusCode::FORBIDDEN);

    let created = request_json(
        app.clone(),
        Method::POST,
        "/plans/catalog",
        Some(&admin_token),
        &UpdatePlanCatalogRequest { plan: team.clone() },
    )
    .await;
    assert_eq!(created.status(), StatusCode::OK);
    assert_eq!(json::<Plan>(created).await, team);

    let catalog: Page<Plan> = json(get(app.clone(), "/plans/catalog", &admin_token).await).await;
    assert!(catalog.items.iter().any(|plan| plan.plan_id == "free"));
    assert!(catalog.items.iter().any(|plan| plan.plan_id == "team"));

    let fetched: Plan = json(get(app.clone(), "/plans/catalog/team", &admin_token).await).await;
    assert_eq!(fetched, team);

    let assign_forbidden = request_json(
        app.clone(),
        Method::POST,
        &format!("/plans/users/{}/assign", auth.user_id),
        Some(&auth.access_token),
        &AssignUserPlanRequest {
            plan_id: "team".to_string(),
        },
    )
    .await;
    assert_eq!(assign_forbidden.status(), StatusCode::FORBIDDEN);

    let assigned = request_json(
        app.clone(),
        Method::POST,
        &format!("/plans/users/{}/assign", auth.user_id),
        Some(&admin_token),
        &AssignUserPlanRequest {
            plan_id: "team".to_string(),
        },
    )
    .await;
    assert_eq!(assigned.status(), StatusCode::OK);
    assert_eq!(json::<Plan>(assigned).await, team);

    let current_plan: Plan =
        json(get(app.clone(), "/plans/current", &auth.access_token).await).await;
    assert_eq!(current_plan, team);

    let missing = request_json(
        app,
        Method::POST,
        &format!("/plans/users/{}/assign", auth.user_id),
        Some(&admin_token),
        &AssignUserPlanRequest {
            plan_id: "missing".to_string(),
        },
    )
    .await;
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
}
