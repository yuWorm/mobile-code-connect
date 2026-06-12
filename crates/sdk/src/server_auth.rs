use std::{
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
    time::Duration,
};

use async_trait::async_trait;
use quic_tunnel_control_client::{
    BrowserServerAuthExchangeRequest, BrowserServerAuthStartResponse, ControlClientError,
    DeviceServerAuthPollResponse, DeviceServerAuthStartResponse, HttpControlClient,
    HttpControlClientOptions, PollServerAuthRequest, ServerAuthStatus, ServerCredentialResponse,
    StartServerAuthRequest,
};
use quic_tunnel_protocol::DeviceId;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::SdkError;

#[async_trait]
pub trait ServerAuthApi: Send {
    async fn start_browser_server_auth(
        &mut self,
        request: StartServerAuthRequest,
    ) -> Result<BrowserServerAuthStartResponse, ControlClientError>;

    async fn exchange_browser_server_auth(
        &mut self,
        request: BrowserServerAuthExchangeRequest,
    ) -> Result<ServerCredentialResponse, ControlClientError>;

    async fn start_device_server_auth(
        &mut self,
        request: StartServerAuthRequest,
    ) -> Result<DeviceServerAuthStartResponse, ControlClientError>;

    async fn poll_device_server_auth(
        &mut self,
        request: PollServerAuthRequest,
    ) -> Result<DeviceServerAuthPollResponse, ControlClientError>;
}

#[async_trait]
impl ServerAuthApi for HttpControlClient {
    async fn start_browser_server_auth(
        &mut self,
        request: StartServerAuthRequest,
    ) -> Result<BrowserServerAuthStartResponse, ControlClientError> {
        HttpControlClient::start_browser_server_auth(self, request).await
    }

    async fn exchange_browser_server_auth(
        &mut self,
        request: BrowserServerAuthExchangeRequest,
    ) -> Result<ServerCredentialResponse, ControlClientError> {
        HttpControlClient::exchange_browser_server_auth(self, request).await
    }

    async fn start_device_server_auth(
        &mut self,
        request: StartServerAuthRequest,
    ) -> Result<DeviceServerAuthStartResponse, ControlClientError> {
        HttpControlClient::start_device_server_auth(self, request).await
    }

    async fn poll_device_server_auth(
        &mut self,
        request: PollServerAuthRequest,
    ) -> Result<DeviceServerAuthPollResponse, ControlClientError> {
        HttpControlClient::poll_device_server_auth(self, request).await
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerLoginInput {
    pub device_id: DeviceId,
    pub device_name: String,
    pub server_public_key: String,
}

impl ServerLoginInput {
    fn start_request(&self) -> StartServerAuthRequest {
        StartServerAuthRequest {
            device_id: self.device_id.clone(),
            device_name: self.device_name.clone(),
            server_public_key: self.server_public_key.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserServerLogin {
    pub session_id: String,
    pub auth_url: String,
    pub expires_in: u64,
    device_name: String,
    server_public_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceCodeServerLogin {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub verification_uri_complete: String,
    pub expires_in: u64,
    pub interval: u64,
    device_name: String,
    server_public_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredServerCredential {
    pub control_server: String,
    pub credential_id: String,
    pub device_id: DeviceId,
    pub device_name: String,
    pub server_token: String,
    pub token_type: String,
}

impl StoredServerCredential {
    pub fn is_for_control_server(&self, control_server: impl AsRef<str>) -> bool {
        normalize_control_server(&self.control_server)
            == normalize_control_server(control_server.as_ref())
    }
}

#[async_trait]
pub trait ServerCredentialStore: Send + Sync {
    async fn load_credential(
        &self,
    ) -> Result<Option<StoredServerCredential>, ServerCredentialStoreError>;

    async fn save_credential(
        &self,
        credential: StoredServerCredential,
    ) -> Result<(), ServerCredentialStoreError>;

    async fn clear_credential(&self) -> Result<(), ServerCredentialStoreError>;
}

#[derive(Debug, Clone, Default)]
pub struct MemoryServerCredentialStore {
    credential: Arc<RwLock<Option<StoredServerCredential>>>,
}

#[async_trait]
impl ServerCredentialStore for MemoryServerCredentialStore {
    async fn load_credential(
        &self,
    ) -> Result<Option<StoredServerCredential>, ServerCredentialStoreError> {
        Ok(self
            .credential
            .read()
            .map_err(|_| ServerCredentialStoreError::LockPoisoned)?
            .clone())
    }

    async fn save_credential(
        &self,
        credential: StoredServerCredential,
    ) -> Result<(), ServerCredentialStoreError> {
        *self
            .credential
            .write()
            .map_err(|_| ServerCredentialStoreError::LockPoisoned)? = Some(credential);
        Ok(())
    }

    async fn clear_credential(&self) -> Result<(), ServerCredentialStoreError> {
        *self
            .credential
            .write()
            .map_err(|_| ServerCredentialStoreError::LockPoisoned)? = None;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct FileServerCredentialStore {
    path: PathBuf,
}

impl FileServerCredentialStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[async_trait]
impl ServerCredentialStore for FileServerCredentialStore {
    async fn load_credential(
        &self,
    ) -> Result<Option<StoredServerCredential>, ServerCredentialStoreError> {
        match tokio::fs::read(&self.path).await {
            Ok(body) => Ok(Some(serde_json::from_slice(&body)?)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    async fn save_credential(
        &self,
        credential: StoredServerCredential,
    ) -> Result<(), ServerCredentialStoreError> {
        if let Some(parent) = self
            .path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            tokio::fs::create_dir_all(parent).await?;
        }
        let body = serde_json::to_vec_pretty(&credential)?;
        tokio::fs::write(&self.path, body).await?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            tokio::fs::set_permissions(&self.path, std::fs::Permissions::from_mode(0o600)).await?;
        }

        Ok(())
    }

    async fn clear_credential(&self) -> Result<(), ServerCredentialStoreError> {
        match tokio::fs::remove_file(&self.path).await {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
    }
}

#[derive(Debug, Clone)]
pub enum SdkServerCredentialStore {
    Memory(MemoryServerCredentialStore),
    File(FileServerCredentialStore),
}

impl SdkServerCredentialStore {
    pub fn memory() -> Self {
        Self::Memory(MemoryServerCredentialStore::default())
    }

    pub fn file(path: impl Into<PathBuf>) -> Self {
        Self::File(FileServerCredentialStore::new(path))
    }
}

impl Default for SdkServerCredentialStore {
    fn default() -> Self {
        Self::memory()
    }
}

#[async_trait]
impl ServerCredentialStore for SdkServerCredentialStore {
    async fn load_credential(
        &self,
    ) -> Result<Option<StoredServerCredential>, ServerCredentialStoreError> {
        match self {
            Self::Memory(store) => store.load_credential().await,
            Self::File(store) => store.load_credential().await,
        }
    }

    async fn save_credential(
        &self,
        credential: StoredServerCredential,
    ) -> Result<(), ServerCredentialStoreError> {
        match self {
            Self::Memory(store) => store.save_credential(credential).await,
            Self::File(store) => store.save_credential(credential).await,
        }
    }

    async fn clear_credential(&self) -> Result<(), ServerCredentialStoreError> {
        match self {
            Self::Memory(store) => store.clear_credential().await,
            Self::File(store) => store.clear_credential().await,
        }
    }
}

pub struct ServerAuthSdk<A, S> {
    control_server: String,
    api: Mutex<A>,
    credential_store: S,
}

impl<A, S> ServerAuthSdk<A, S>
where
    A: ServerAuthApi,
    S: ServerCredentialStore,
{
    pub fn new(control_server: impl Into<String>, api: A, credential_store: S) -> Self {
        Self {
            control_server: control_server.into(),
            api: Mutex::new(api),
            credential_store,
        }
    }

    pub async fn start_browser_login(
        &self,
        input: ServerLoginInput,
    ) -> Result<BrowserServerLogin, SdkError> {
        let mut api = self.api.lock().await;
        let response = api.start_browser_server_auth(input.start_request()).await?;
        Ok(BrowserServerLogin {
            session_id: response.session_id,
            auth_url: response.auth_url,
            expires_in: response.expires_in,
            device_name: input.device_name,
            server_public_key: input.server_public_key,
        })
    }

    pub async fn complete_browser_login(
        &self,
        pending: BrowserServerLogin,
        server_auth_code: impl Into<String>,
    ) -> Result<StoredServerCredential, SdkError> {
        let mut api = self.api.lock().await;
        let credential = api
            .exchange_browser_server_auth(BrowserServerAuthExchangeRequest {
                session_id: pending.session_id,
                server_auth_code: server_auth_code.into(),
                server_public_key: pending.server_public_key,
            })
            .await?;
        self.persist_credential(credential, pending.device_name)
            .await
    }

    pub async fn start_device_code_login(
        &self,
        input: ServerLoginInput,
    ) -> Result<DeviceCodeServerLogin, SdkError> {
        let mut api = self.api.lock().await;
        let response = api.start_device_server_auth(input.start_request()).await?;
        Ok(DeviceCodeServerLogin {
            device_code: response.device_code,
            user_code: response.user_code,
            verification_uri: response.verification_uri,
            verification_uri_complete: response.verification_uri_complete,
            expires_in: response.expires_in,
            interval: response.interval,
            device_name: input.device_name,
            server_public_key: input.server_public_key,
        })
    }

    pub async fn complete_device_code_login(
        &self,
        pending: DeviceCodeServerLogin,
        fallback_poll_interval: Duration,
    ) -> Result<StoredServerCredential, SdkError> {
        loop {
            let poll = {
                let mut api = self.api.lock().await;
                api.poll_device_server_auth(PollServerAuthRequest {
                    device_code: pending.device_code.clone(),
                    server_public_key: pending.server_public_key.clone(),
                })
                .await?
            };
            match poll.status {
                ServerAuthStatus::Approved => {
                    let credential = poll
                        .credential
                        .ok_or(SdkError::ServerAuthApprovedWithoutCredential)?;
                    return self
                        .persist_credential(credential, pending.device_name.clone())
                        .await;
                }
                ServerAuthStatus::Pending
                | ServerAuthStatus::AuthorizationPending
                | ServerAuthStatus::SlowDown => {
                    tokio::time::sleep(poll_interval(poll.interval, fallback_poll_interval)).await;
                }
                ServerAuthStatus::Denied | ServerAuthStatus::AccessDenied => {
                    return Err(SdkError::ServerAuthDenied);
                }
                ServerAuthStatus::Expired => return Err(SdkError::ServerAuthExpired),
                ServerAuthStatus::Consumed => return Err(SdkError::ServerAuthConsumed),
            }
        }
    }

    pub async fn load_credential(&self) -> Result<Option<StoredServerCredential>, SdkError> {
        Ok(self.credential_store.load_credential().await?)
    }

    pub async fn clear_credential(&self) -> Result<(), SdkError> {
        self.credential_store.clear_credential().await?;
        Ok(())
    }

    async fn persist_credential(
        &self,
        credential: ServerCredentialResponse,
        device_name: String,
    ) -> Result<StoredServerCredential, SdkError> {
        let stored = StoredServerCredential {
            control_server: self.control_server.clone(),
            credential_id: credential.credential_id,
            device_id: credential.device_id,
            device_name,
            server_token: credential.server_token,
            token_type: credential.token_type,
        };
        self.credential_store
            .save_credential(stored.clone())
            .await?;
        Ok(stored)
    }
}

impl<S> ServerAuthSdk<HttpControlClient, S>
where
    S: ServerCredentialStore,
{
    pub fn with_http_client(
        control_server: impl AsRef<str>,
        credential_store: S,
    ) -> Result<Self, SdkError> {
        Self::with_http_client_options(
            control_server,
            credential_store,
            HttpControlClientOptions::default(),
        )
    }

    pub fn with_http_client_options(
        control_server: impl AsRef<str>,
        credential_store: S,
        options: HttpControlClientOptions,
    ) -> Result<Self, SdkError> {
        let control_server = control_server.as_ref().to_string();
        Ok(Self::new(
            control_server.clone(),
            HttpControlClient::with_options(control_server, options)?,
            credential_store,
        ))
    }
}

impl ServerAuthSdk<HttpControlClient, MemoryServerCredentialStore> {
    pub fn in_memory(control_server: impl AsRef<str>) -> Result<Self, SdkError> {
        Self::with_http_client(control_server, MemoryServerCredentialStore::default())
    }
}

impl ServerAuthSdk<HttpControlClient, FileServerCredentialStore> {
    pub fn with_file_credential_store(
        control_server: impl AsRef<str>,
        credential_path: impl Into<PathBuf>,
    ) -> Result<Self, SdkError> {
        Self::with_http_client(
            control_server,
            FileServerCredentialStore::new(credential_path),
        )
    }
}

fn poll_interval(server_interval_sec: u64, fallback: Duration) -> Duration {
    if server_interval_sec > 0 {
        Duration::from_secs(server_interval_sec)
    } else {
        fallback
    }
}

fn normalize_control_server(control_server: &str) -> &str {
    control_server.trim().trim_end_matches('/')
}

#[derive(Debug, thiserror::Error)]
pub enum ServerCredentialStoreError {
    #[error("server credential store lock poisoned")]
    LockPoisoned,
    #[error("io failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("json failed: {0}")]
    Json(#[from] serde_json::Error),
}
