use std::{path::PathBuf, sync::Arc};

use anyhow::{bail, Context, Result};
use clap::Parser;
use mobilecode_connect_control::{
    oauth::{GitHubOAuthConfig, GitHubOAuthHttpClient},
    routes::routes,
    state::ControlState,
};
use mobilecode_connect_control_client::RegisterUserRequest;

#[derive(Debug, Parser)]
#[command(name = "control-server")]
#[command(about = "Control plane API for MobileCode Connect")]
struct Cli {
    #[arg(long, default_value = "127.0.0.1:4242")]
    listen: String,
    #[arg(long, env = "MOBILECODE_CONNECT_TOKEN_SECRET")]
    token_secret: Option<String>,
    #[arg(long)]
    relay_addr: String,
    #[arg(long)]
    punch_addr: String,
    #[arg(long, env = "MOBILECODE_CONNECT_CONTROL_STATE_DB")]
    state_db: Option<PathBuf>,
    #[arg(long, env = "MOBILECODE_CONNECT_STRICT_AUTH", default_value_t = false)]
    strict_auth: bool,
    #[arg(long, env = "MOBILECODE_CONNECT_PUBLIC_URL")]
    public_url: Option<String>,
    #[arg(long, env = "MOBILECODE_CONNECT_GITHUB_CLIENT_ID")]
    github_client_id: Option<String>,
    #[arg(long, env = "MOBILECODE_CONNECT_GITHUB_CLIENT_SECRET")]
    github_client_secret: Option<String>,
    #[arg(long, env = "MOBILECODE_CONNECT_GITHUB_REDIRECT_URL")]
    github_redirect_url: Option<String>,
    #[arg(long, hide = true)]
    github_curl_command: Option<String>,
    #[arg(long)]
    print_admin_token: Option<String>,
    #[arg(long)]
    print_relay_token: Option<String>,
    #[arg(long, env = "MOBILECODE_CONNECT_ADMIN_EMAIL")]
    bootstrap_admin_email: Option<String>,
    #[arg(long, env = "MOBILECODE_CONNECT_ADMIN_PASSWORD")]
    bootstrap_admin_password: Option<String>,
    #[arg(long, env = "MOBILECODE_CONNECT_ADMIN_DISPLAY_NAME")]
    bootstrap_admin_display_name: Option<String>,
}

impl Cli {
    fn token_secret(&self) -> Result<String> {
        self.token_secret
            .clone()
            .or_else(|| {
                env_alias(
                    "MOBILECODE_CONNECT_TOKEN_SECRET",
                    "QUIC_TUNNEL_TOKEN_SECRET",
                )
            })
            .context("--token-secret is required")
    }

    fn state_db(&self) -> Option<PathBuf> {
        self.state_db.clone().or_else(|| {
            env_alias(
                "MOBILECODE_CONNECT_CONTROL_STATE_DB",
                "QUIC_TUNNEL_CONTROL_STATE_DB",
            )
            .map(PathBuf::from)
        })
    }

    fn strict_auth(&self) -> bool {
        if self.strict_auth || std::env::var_os("MOBILECODE_CONNECT_STRICT_AUTH").is_some() {
            return self.strict_auth;
        }
        bool_env_alias("QUIC_TUNNEL_STRICT_AUTH").unwrap_or(false)
    }

    fn public_url(&self) -> Option<String> {
        self.public_url
            .clone()
            .or_else(|| env_alias("MOBILECODE_CONNECT_PUBLIC_URL", "QUIC_TUNNEL_PUBLIC_URL"))
    }

    fn github_client_id(&self) -> Option<String> {
        self.github_client_id.clone().or_else(|| {
            env_alias(
                "MOBILECODE_CONNECT_GITHUB_CLIENT_ID",
                "QUIC_TUNNEL_GITHUB_CLIENT_ID",
            )
        })
    }

    fn github_client_secret(&self) -> Option<String> {
        self.github_client_secret.clone().or_else(|| {
            env_alias(
                "MOBILECODE_CONNECT_GITHUB_CLIENT_SECRET",
                "QUIC_TUNNEL_GITHUB_CLIENT_SECRET",
            )
        })
    }

    fn github_redirect_url(&self) -> Option<String> {
        self.github_redirect_url.clone().or_else(|| {
            env_alias(
                "MOBILECODE_CONNECT_GITHUB_REDIRECT_URL",
                "QUIC_TUNNEL_GITHUB_REDIRECT_URL",
            )
        })
    }

    fn bootstrap_admin_email(&self) -> Option<String> {
        self.bootstrap_admin_email
            .clone()
            .or_else(|| env_alias("MOBILECODE_CONNECT_ADMIN_EMAIL", "QUIC_TUNNEL_ADMIN_EMAIL"))
    }

    fn bootstrap_admin_password(&self) -> Option<String> {
        self.bootstrap_admin_password.clone().or_else(|| {
            env_alias(
                "MOBILECODE_CONNECT_ADMIN_PASSWORD",
                "QUIC_TUNNEL_ADMIN_PASSWORD",
            )
        })
    }

    fn bootstrap_admin_display_name(&self) -> String {
        self.bootstrap_admin_display_name
            .clone()
            .or_else(|| {
                env_alias(
                    "MOBILECODE_CONNECT_ADMIN_DISPLAY_NAME",
                    "QUIC_TUNNEL_ADMIN_DISPLAY_NAME",
                )
            })
            .unwrap_or_else(|| "Admin".to_string())
    }

    fn bootstrap_admin_request(&self) -> Result<Option<RegisterUserRequest>> {
        match (
            self.bootstrap_admin_email(),
            self.bootstrap_admin_password(),
        ) {
            (None, None) => Ok(None),
            (Some(email), Some(password)) => Ok(Some(RegisterUserRequest {
                email,
                password,
                display_name: self.bootstrap_admin_display_name(),
            })),
            _ => {
                bail!("--bootstrap-admin-email and --bootstrap-admin-password must be provided together")
            }
        }
    }

    fn github_oauth_config(&self) -> Result<Option<GitHubOAuthConfig>> {
        match (
            self.public_url(),
            self.github_client_id(),
            self.github_client_secret(),
        ) {
            (None, None, None) => Ok(None),
            (Some(public_url), Some(client_id), Some(client_secret)) => {
                Ok(Some(GitHubOAuthConfig {
                    public_url,
                    client_id,
                    client_secret,
                    redirect_url: self.github_redirect_url(),
                }))
            }
            _ => bail!(
                "--public-url, --github-client-id, and --github-client-secret must be provided together"
            ),
        }
    }

    fn control_state(&self) -> Result<ControlState> {
        let token_secret = self.token_secret()?;
        let mut state = if let Some(state_db) = self.state_db() {
            ControlState::new_sqlite(
                token_secret,
                self.relay_addr.clone(),
                self.punch_addr.clone(),
                &state_db,
            )
            .context("open control state database")?
        } else {
            ControlState::new(
                token_secret,
                self.relay_addr.clone(),
                self.punch_addr.clone(),
            )
        }
        .with_strict_auth(self.strict_auth());
        if let Some(config) = self.github_oauth_config()? {
            let mut client = GitHubOAuthHttpClient::new();
            if let Some(curl_command) = &self.github_curl_command {
                client = client.with_curl_command(curl_command.clone());
            }
            state = state
                .with_github_oauth_config(config)
                .with_github_oauth_client(Arc::new(client));
        }
        Ok(state)
    }
}

fn env_alias(canonical: &str, legacy: &str) -> Option<String> {
    std::env::var(canonical)
        .ok()
        .filter(|value| !value.is_empty())
        .or_else(|| std::env::var(legacy).ok().filter(|value| !value.is_empty()))
}

fn bool_env_alias(name: &str) -> Option<bool> {
    let value = std::env::var(name).ok()?;
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    if cli.print_admin_token.is_some() && cli.print_relay_token.is_some() {
        bail!("--print-admin-token and --print-relay-token cannot be used together");
    }
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let bootstrap_admin = cli.bootstrap_admin_request()?;
    let state = cli.control_state()?;

    if let Some(request) = bootstrap_admin {
        let auth = state
            .bootstrap_admin_user(request)
            .context("bootstrap admin user")?;
        tracing::info!(user_id = %auth.user_id, "bootstrapped admin user");
    }

    if let Some(subject) = cli.print_admin_token {
        println!(
            "{}",
            state
                .issue_admin_token(subject)
                .context("issue admin control token")?
        );
        return Ok(());
    }

    if let Some(relay_id) = cli.print_relay_token {
        println!(
            "{}",
            state
                .issue_relay_token(relay_id)
                .context("issue relay control token")?
        );
        return Ok(());
    }

    let listener = tokio::net::TcpListener::bind(&cli.listen).await?;
    axum::serve(listener, routes(state)).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn parses_control_server_args() {
        let cli = Cli::try_parse_from([
            "control-server",
            "--listen",
            "127.0.0.1:4242",
            "--token-secret",
            "dev-secret",
            "--relay-addr",
            "relay.example.com:4433",
            "--punch-addr",
            "punch.example.com:3478",
            "--state-db",
            "/tmp/control-state.sqlite",
            "--strict-auth",
            "--print-admin-token",
            "admin@example.com",
            "--bootstrap-admin-email",
            "root@example.com",
            "--bootstrap-admin-password",
            "admin-password-123",
            "--bootstrap-admin-display-name",
            "Root Admin",
        ])
        .unwrap();

        assert_eq!(cli.listen, "127.0.0.1:4242");
        assert_eq!(cli.token_secret, Some("dev-secret".to_string()));
        assert_eq!(cli.relay_addr, "relay.example.com:4433");
        assert_eq!(cli.punch_addr, "punch.example.com:3478");
        assert_eq!(
            cli.state_db,
            Some(PathBuf::from("/tmp/control-state.sqlite"))
        );
        assert!(cli.strict_auth);
        assert_eq!(cli.print_admin_token, Some("admin@example.com".to_string()));
        assert_eq!(cli.print_relay_token, None);
        assert_eq!(
            cli.bootstrap_admin_email,
            Some("root@example.com".to_string())
        );
        assert_eq!(
            cli.bootstrap_admin_password,
            Some("admin-password-123".to_string())
        );
        assert_eq!(
            cli.bootstrap_admin_display_name,
            Some("Root Admin".to_string())
        );
        assert!(cli.bootstrap_admin_request().unwrap().is_some());
    }

    #[test]
    fn control_server_accepts_legacy_env_aliases() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::remove_var("MOBILECODE_CONNECT_TOKEN_SECRET");
        std::env::set_var("QUIC_TUNNEL_TOKEN_SECRET", "legacy-secret");

        let cli = Cli::try_parse_from([
            "control-server",
            "--relay-addr",
            "relay.example.com:4433",
            "--punch-addr",
            "punch.example.com:3478",
        ])
        .unwrap();

        assert_eq!(cli.token_secret().unwrap(), "legacy-secret");

        std::env::remove_var("QUIC_TUNNEL_TOKEN_SECRET");
    }

    #[test]
    fn parses_control_server_relay_token_args() {
        let cli = Cli::try_parse_from([
            "control-server",
            "--token-secret",
            "dev-secret",
            "--relay-addr",
            "relay.example.com:4433",
            "--punch-addr",
            "punch.example.com:3478",
            "--print-relay-token",
            "relay_dev_001",
        ])
        .unwrap();

        assert_eq!(cli.print_admin_token, None);
        assert_eq!(cli.print_relay_token, Some("relay_dev_001".to_string()));
    }

    #[test]
    fn oauth_config_args_build_github_config() {
        let cli = Cli::try_parse_from([
            "control-server",
            "--token-secret",
            "dev-secret",
            "--relay-addr",
            "relay.example.com:4433",
            "--punch-addr",
            "punch.example.com:3478",
            "--public-url",
            "https://control.example.com",
            "--github-client-id",
            "github-client-id",
            "--github-client-secret",
            "github-client-secret",
            "--github-redirect-url",
            "https://control.example.com/oauth/github/callback",
        ])
        .unwrap();

        let config = cli.github_oauth_config().unwrap().unwrap();
        assert_eq!(config.public_url, "https://control.example.com");
        assert_eq!(config.client_id, "github-client-id");
        assert_eq!(config.client_secret, "github-client-secret");
        assert_eq!(
            config.redirect_url.as_deref(),
            Some("https://control.example.com/oauth/github/callback")
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn oauth_config_builds_state_with_github_http_client() {
        let dir = unique_temp_dir();
        tokio::fs::create_dir_all(&dir).await.unwrap();
        let curl_path = dir.join("fake-curl.sh");
        let script = r#"#!/bin/sh
config="$(cat)"
if printf '%s' "$config" | grep -q '/login/oauth/access_token'; then
  printf '%s\n200' '{"access_token":"github-access-token","token_type":"bearer","scope":"read:user,user:email"}'
elif printf '%s' "$config" | grep -q '/user/emails'; then
  printf '%s\n200' '[{"email":"octocat@example.com","primary":true,"verified":true}]'
elif printf '%s' "$config" | grep -q '/user'; then
  printf '%s\n200' '{"id":123456,"login":"octocat","name":"Octo Cat","avatar_url":"https://avatars.githubusercontent.com/u/123456"}'
else
  exit 2
fi
"#;
        tokio::fs::write(&curl_path, script).await.unwrap();
        {
            use std::os::unix::fs::PermissionsExt;
            tokio::fs::set_permissions(&curl_path, std::fs::Permissions::from_mode(0o700))
                .await
                .unwrap();
        }

        let cli = Cli::try_parse_from([
            "control-server",
            "--token-secret",
            "dev-secret",
            "--relay-addr",
            "relay.example.com:4433",
            "--punch-addr",
            "punch.example.com:3478",
            "--public-url",
            "https://control.example.com",
            "--github-client-id",
            "github-client-id",
            "--github-client-secret",
            "github-client-secret",
            "--github-curl-command",
            curl_path.to_str().unwrap(),
        ])
        .unwrap();
        let state = cli.control_state().unwrap();
        let start = state.start_github_oauth(None).unwrap();
        let github_state = query_value(&start.authorization_url, "state");

        let auth = state
            .github_oauth_callback("github-code", &github_state)
            .await
            .unwrap();

        assert!(!auth.user_id.as_str().is_empty());

        tokio::fs::remove_dir_all(dir).await.unwrap();
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

    fn unique_temp_dir() -> PathBuf {
        static NEXT_TEMP_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let id = NEXT_TEMP_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "mobilecode-connect-control-server-oauth-{suffix}-{id}"
        ))
    }
}
