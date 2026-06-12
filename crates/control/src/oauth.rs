use async_trait::async_trait;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::process::Stdio;
use tokio::{io::AsyncWriteExt, process::Command};

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[cfg(unix)]
    #[tokio::test]
    async fn github_http_client_exchanges_code_and_fetches_verified_email() {
        let dir = unique_temp_dir();
        tokio::fs::create_dir_all(&dir).await.unwrap();
        let curl_path = dir.join("fake-curl.sh");
        let script = format!(
            r#"#!/bin/sh
config="$(cat)"
if printf '%s' "$config" | grep -q '/login/oauth/access_token'; then
  printf '%s' "$config" > "{}/token.config"
  printf '%s\n200' '{{"access_token":"github-access-token","token_type":"bearer","scope":"read:user,user:email"}}'
elif printf '%s' "$config" | grep -q '/user/emails'; then
  printf '%s' "$config" > "{}/emails.config"
  printf '%s\n200' '[{{"email":"secondary@example.com","primary":false,"verified":true}},{{"email":"octocat@example.com","primary":true,"verified":true}}]'
elif printf '%s' "$config" | grep -q '/user'; then
  printf '%s' "$config" > "{}/user.config"
  printf '%s\n200' '{{"id":123456,"login":"octocat","name":"Octo Cat","avatar_url":"https://avatars.githubusercontent.com/u/123456"}}'
else
  exit 2
fi
"#,
            dir.display(),
            dir.display(),
            dir.display()
        );
        tokio::fs::write(&curl_path, script).await.unwrap();
        {
            use std::os::unix::fs::PermissionsExt;
            tokio::fs::set_permissions(&curl_path, std::fs::Permissions::from_mode(0o700))
                .await
                .unwrap();
        }

        let config = GitHubOAuthConfig {
            public_url: "https://control.example.com".to_string(),
            client_id: "client-id".to_string(),
            client_secret: "client-secret".to_string(),
            redirect_url: None,
        };
        let client = GitHubOAuthHttpClient::new()
            .with_token_url("https://github.example.test/login/oauth/access_token")
            .with_api_base_url("https://api.github.example.test")
            .with_curl_command(curl_path.to_string_lossy());

        let token = client
            .exchange_code("github-code", "pkce-verifier", &config)
            .await
            .unwrap();
        let profile = client.user_profile(&token.access_token).await.unwrap();
        let email = client
            .primary_verified_email(&token.access_token)
            .await
            .unwrap();

        assert_eq!(token.access_token, "github-access-token");
        let token_config = tokio::fs::read_to_string(dir.join("token.config"))
            .await
            .unwrap();
        assert!(token_config.contains("header = \"accept: application/json\""));
        assert!(
            token_config.contains("header = \"content-type: application/x-www-form-urlencoded\"")
        );
        assert!(token_config.contains("client_id=client-id"));
        assert!(token_config.contains("client_secret=client-secret"));
        assert!(token_config.contains("code=github-code"));
        assert!(token_config.contains("code_verifier=pkce-verifier"));
        assert!(token_config.contains(
            "redirect_uri=https%3A%2F%2Fcontrol.example.com%2Fauth%2Foauth%2Fgithub%2Fcallback"
        ));
        let user_config = tokio::fs::read_to_string(dir.join("user.config"))
            .await
            .unwrap();
        assert!(user_config.contains("header = \"authorization: Bearer github-access-token\""));
        let emails_config = tokio::fs::read_to_string(dir.join("emails.config"))
            .await
            .unwrap();
        assert!(emails_config.contains("header = \"authorization: Bearer github-access-token\""));
        assert_eq!(
            profile,
            GitHubUserProfile {
                id: "123456".to_string(),
                login: "octocat".to_string(),
                name: Some("Octo Cat".to_string()),
                avatar_url: "https://avatars.githubusercontent.com/u/123456".to_string(),
            }
        );
        assert_eq!(email, "octocat@example.com");

        tokio::fs::remove_dir_all(dir).await.unwrap();
    }

    fn unique_temp_dir() -> PathBuf {
        static NEXT_TEMP_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let id = NEXT_TEMP_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        std::env::temp_dir().join(format!("mobilecode-connect-github-oauth-{suffix}-{id}"))
    }
}

#[derive(Clone, Debug)]
pub struct GitHubOAuthConfig {
    pub public_url: String,
    pub client_id: String,
    pub client_secret: String,
    pub redirect_url: Option<String>,
}

impl GitHubOAuthConfig {
    pub fn callback_url(&self) -> String {
        self.redirect_url.clone().unwrap_or_else(|| {
            format!(
                "{}/auth/oauth/github/callback",
                self.public_url.trim_end_matches('/')
            )
        })
    }

    pub fn authorize_url(&self, state: &str, code_challenge: &str) -> String {
        let callback_url = self.callback_url();
        format!(
            "https://github.com/login/oauth/authorize?client_id={}&redirect_uri={}&scope={}&state={}&code_challenge={}&code_challenge_method=S256",
            percent_encode(&self.client_id),
            percent_encode(&callback_url),
            percent_encode("read:user user:email"),
            percent_encode(state),
            percent_encode(code_challenge),
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GitHubOAuthToken {
    pub access_token: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GitHubUserProfile {
    pub id: String,
    pub login: String,
    pub name: Option<String>,
    pub avatar_url: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OAuthStart {
    pub authorization_url: String,
    pub state: String,
    pub expires_in: u64,
}

#[async_trait]
pub trait GitHubOAuthClient: Send + Sync {
    async fn exchange_code(
        &self,
        code: &str,
        pkce_verifier: &str,
        config: &GitHubOAuthConfig,
    ) -> Result<GitHubOAuthToken, OAuthError>;

    async fn user_profile(&self, access_token: &str) -> Result<GitHubUserProfile, OAuthError>;

    async fn primary_verified_email(&self, access_token: &str) -> Result<String, OAuthError>;
}

const GITHUB_TOKEN_URL: &str = "https://github.com/login/oauth/access_token";
const GITHUB_API_BASE_URL: &str = "https://api.github.com";
const USER_AGENT: &str = "mobilecode-connect-control";

#[derive(Clone, Debug)]
pub struct GitHubOAuthHttpClient {
    token_url: String,
    api_base_url: String,
    curl_command: String,
}

impl GitHubOAuthHttpClient {
    pub fn new() -> Self {
        Self {
            token_url: GITHUB_TOKEN_URL.to_string(),
            api_base_url: GITHUB_API_BASE_URL.to_string(),
            curl_command: "curl".to_string(),
        }
    }

    pub fn with_token_url(mut self, token_url: impl Into<String>) -> Self {
        self.token_url = token_url.into();
        self
    }

    pub fn with_api_base_url(mut self, api_base_url: impl Into<String>) -> Self {
        self.api_base_url = api_base_url.into();
        self
    }

    pub fn with_curl_command(mut self, curl_command: impl Into<String>) -> Self {
        self.curl_command = curl_command.into();
        self
    }

    fn api_url(&self, path: &str) -> String {
        format!(
            "{}/{}",
            self.api_base_url.trim_end_matches('/'),
            path.trim_start_matches('/')
        )
    }

    async fn request_json(
        &self,
        method: &str,
        url: &str,
        headers: &[(&str, String)],
        body: Option<String>,
    ) -> Result<CurlResponse, OAuthError> {
        let mut config = String::new();
        config.push_str("silent\n");
        config.push_str("show-error\n");
        config.push_str("location\n");
        config.push_str(&format!(
            "request = \"{}\"\n",
            escape_curl_config_value(method)
        ));
        config.push_str(&format!("url = \"{}\"\n", escape_curl_config_value(url)));
        config.push_str("write-out = \"\\n%{http_code}\"\n");
        for (name, value) in headers {
            config.push_str(&format!(
                "header = \"{}: {}\"\n",
                escape_curl_config_value(name),
                escape_curl_config_value(value)
            ));
        }
        if let Some(body) = body {
            config.push_str(&format!("data = \"{}\"\n", escape_curl_config_value(&body)));
        }

        let mut child = Command::new(&self.curl_command)
            .arg("-K")
            .arg("-")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|_| OAuthError::ProviderUnavailable)?;
        let mut stdin = child.stdin.take().ok_or(OAuthError::ProviderUnavailable)?;
        stdin
            .write_all(config.as_bytes())
            .await
            .map_err(|_| OAuthError::ProviderUnavailable)?;
        drop(stdin);

        let output = child
            .wait_with_output()
            .await
            .map_err(|_| OAuthError::ProviderUnavailable)?;
        if !output.status.success() {
            return Err(OAuthError::ProviderUnavailable);
        }
        let stdout = String::from_utf8(output.stdout).map_err(|_| OAuthError::ProviderRejected)?;
        let (body, status_code) = stdout
            .rsplit_once('\n')
            .ok_or(OAuthError::ProviderRejected)?;
        let status_code = status_code
            .trim()
            .parse::<u16>()
            .map_err(|_| OAuthError::ProviderRejected)?;
        Ok(CurlResponse {
            status_code,
            body: body.to_string(),
        })
    }
}

impl Default for GitHubOAuthHttpClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl GitHubOAuthClient for GitHubOAuthHttpClient {
    async fn exchange_code(
        &self,
        code: &str,
        pkce_verifier: &str,
        config: &GitHubOAuthConfig,
    ) -> Result<GitHubOAuthToken, OAuthError> {
        let body = format!(
            "client_id={}&client_secret={}&code={}&redirect_uri={}&code_verifier={}",
            percent_encode(&config.client_id),
            percent_encode(&config.client_secret),
            percent_encode(code),
            percent_encode(&config.callback_url()),
            percent_encode(pkce_verifier),
        );
        let response = self
            .request_json(
                "POST",
                &self.token_url,
                &[
                    ("accept", "application/json".to_string()),
                    (
                        "content-type",
                        "application/x-www-form-urlencoded".to_string(),
                    ),
                    ("user-agent", USER_AGENT.to_string()),
                ],
                Some(body),
            )
            .await?;
        if !response.success() {
            return Err(OAuthError::ProviderRejected);
        }
        let token = serde_json::from_str::<GitHubTokenResponse>(&response.body)
            .map_err(|_| OAuthError::ProviderRejected)?;
        if token.error.is_some() {
            return Err(OAuthError::ProviderRejected);
        }
        let access_token = token
            .access_token
            .filter(|access_token| !access_token.trim().is_empty())
            .ok_or(OAuthError::ProviderRejected)?;
        Ok(GitHubOAuthToken { access_token })
    }

    async fn user_profile(&self, access_token: &str) -> Result<GitHubUserProfile, OAuthError> {
        let response = self
            .request_json(
                "GET",
                &self.api_url("/user"),
                &[
                    ("accept", "application/vnd.github+json".to_string()),
                    ("authorization", format!("Bearer {access_token}")),
                    ("user-agent", USER_AGENT.to_string()),
                ],
                None,
            )
            .await?;
        if !response.success() {
            return Err(OAuthError::ProviderRejected);
        }
        let profile = serde_json::from_str::<GitHubUserResponse>(&response.body)
            .map_err(|_| OAuthError::ProviderRejected)?;
        Ok(GitHubUserProfile {
            id: profile.id_as_string()?,
            login: profile.login,
            name: profile.name,
            avatar_url: profile.avatar_url.unwrap_or_default(),
        })
    }

    async fn primary_verified_email(&self, access_token: &str) -> Result<String, OAuthError> {
        let response = self
            .request_json(
                "GET",
                &self.api_url("/user/emails"),
                &[
                    ("accept", "application/vnd.github+json".to_string()),
                    ("authorization", format!("Bearer {access_token}")),
                    ("user-agent", USER_AGENT.to_string()),
                ],
                None,
            )
            .await?;
        if !response.success() {
            return Err(OAuthError::VerifiedEmailUnavailable);
        }
        let emails = serde_json::from_str::<Vec<GitHubEmailResponse>>(&response.body)
            .map_err(|_| OAuthError::VerifiedEmailUnavailable)?;
        emails
            .into_iter()
            .find(|email| email.primary && email.verified && !email.email.trim().is_empty())
            .map(|email| email.email)
            .ok_or(OAuthError::VerifiedEmailUnavailable)
    }
}

#[derive(Debug, Deserialize)]
struct GitHubTokenResponse {
    access_token: Option<String>,
    #[serde(default)]
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GitHubUserResponse {
    id: serde_json::Value,
    login: String,
    name: Option<String>,
    avatar_url: Option<String>,
}

impl GitHubUserResponse {
    fn id_as_string(&self) -> Result<String, OAuthError> {
        match &self.id {
            serde_json::Value::String(id) if !id.trim().is_empty() => Ok(id.clone()),
            serde_json::Value::Number(id) => Ok(id.to_string()),
            _ => Err(OAuthError::ProviderRejected),
        }
    }
}

#[derive(Debug, Deserialize)]
struct GitHubEmailResponse {
    email: String,
    primary: bool,
    verified: bool,
}

struct CurlResponse {
    status_code: u16,
    body: String,
}

impl CurlResponse {
    fn success(&self) -> bool {
        (200..300).contains(&self.status_code)
    }
}

#[derive(Clone, Debug, Default)]
pub struct UnavailableGitHubOAuthClient;

#[async_trait]
impl GitHubOAuthClient for UnavailableGitHubOAuthClient {
    async fn exchange_code(
        &self,
        _code: &str,
        _pkce_verifier: &str,
        _config: &GitHubOAuthConfig,
    ) -> Result<GitHubOAuthToken, OAuthError> {
        Err(OAuthError::ProviderUnavailable)
    }

    async fn user_profile(&self, _access_token: &str) -> Result<GitHubUserProfile, OAuthError> {
        Err(OAuthError::ProviderUnavailable)
    }

    async fn primary_verified_email(&self, _access_token: &str) -> Result<String, OAuthError> {
        Err(OAuthError::ProviderUnavailable)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum OAuthError {
    #[error("oauth provider unavailable")]
    ProviderUnavailable,
    #[error("oauth provider rejected request")]
    ProviderRejected,
    #[error("oauth verified email unavailable")]
    VerifiedEmailUnavailable,
}

pub fn pkce_challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

pub fn secret_hash(value: &str) -> String {
    let digest = Sha256::digest(value.as_bytes());
    digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>()
}

fn percent_encode(value: &str) -> String {
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

fn escape_curl_config_value(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
