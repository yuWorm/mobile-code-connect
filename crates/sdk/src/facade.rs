use std::path::PathBuf;

use mobilecode_connect_control_client::{
    ControllerDevice, CreateSessionResponse, HttpControlClient, HttpControlClientOptions,
};
use mobilecode_connect_mobile_core::forward::LocalForwardHandle;
use mobilecode_connect_protocol::{ClientId, Device, DeviceId, Service, ServiceId};
use rustls::pki_types::CertificateDer;

use crate::{
    admin::AdminSdk,
    auth::{AuthSdk, LoginInput, RegisterInput},
    controller::{ControllerSdk, CreateSessionInput, RegisterControllerInput},
    mobile::{MobileTunnelConfig, MobileTunnelSdk, OpenServiceInput, P2pOrRelayTunnelConfig},
    server::ServerSdk,
    server_auth::{
        BrowserServerLogin, DeviceCodeServerLogin, SdkServerCredentialStore, ServerAuthSdk,
        ServerCredentialStore, ServerLoginInput, StoredServerCredential,
    },
    store::{SdkTokenStore, StoredToken, TokenStore},
    SdkError,
};

#[derive(Debug, Clone)]
pub struct MobileCodeConnectSdk {
    control_url: String,
    token_store: SdkTokenStore,
    server_credential_store: SdkServerCredentialStore,
    control_client_options: HttpControlClientOptions,
}

#[derive(Debug, Default)]
pub struct MobileCodeConnectSdkBuilder {
    control_url: Option<String>,
    token_store: Option<SdkTokenStore>,
    server_credential_store: Option<SdkServerCredentialStore>,
    control_client_options: Option<HttpControlClientOptions>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenMobileServiceInput {
    pub client_id: ClientId,
    pub device_id: DeviceId,
    pub service_id: ServiceId,
    pub local_port: u16,
}

#[derive(Debug)]
pub struct OpenedMobileService {
    tunnel: MobileTunnelSdk,
    forward: LocalForwardHandle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnsureBrowserServerLogin {
    Existing(StoredServerCredential),
    Pending(BrowserServerLogin),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnsureDeviceCodeServerLogin {
    Existing(StoredServerCredential),
    Pending(DeviceCodeServerLogin),
}

impl MobileCodeConnectSdk {
    pub fn builder() -> MobileCodeConnectSdkBuilder {
        MobileCodeConnectSdkBuilder::default()
    }

    pub fn control_url(&self) -> &str {
        &self.control_url
    }

    pub fn token_store(&self) -> SdkTokenStore {
        self.token_store.clone()
    }

    pub fn server_credential_store(&self) -> SdkServerCredentialStore {
        self.server_credential_store.clone()
    }

    pub fn control_client_options(&self) -> &HttpControlClientOptions {
        &self.control_client_options
    }

    pub async fn current_token(&self) -> Result<Option<StoredToken>, SdkError> {
        Ok(self.token_store.load_token().await?)
    }

    pub async fn current_valid_token(
        &self,
        now_epoch_sec: u64,
    ) -> Result<Option<StoredToken>, SdkError> {
        Ok(self
            .current_token()
            .await?
            .filter(|token| token.is_valid_at(now_epoch_sec)))
    }

    pub async fn current_server_credential(
        &self,
    ) -> Result<Option<StoredServerCredential>, SdkError> {
        Ok(self.server_credential_store.load_credential().await?)
    }

    pub async fn current_server_credential_for_control(
        &self,
    ) -> Result<Option<StoredServerCredential>, SdkError> {
        Ok(self
            .current_server_credential()
            .await?
            .filter(|credential| credential.is_for_control_server(&self.control_url)))
    }

    pub async fn clear_token(&self) -> Result<(), SdkError> {
        self.token_store.clear_token().await?;
        Ok(())
    }

    pub async fn clear_server_credential(&self) -> Result<(), SdkError> {
        self.server_credential_store.clear_credential().await?;
        Ok(())
    }

    pub fn auth(&self) -> Result<AuthSdk<HttpControlClient, SdkTokenStore>, SdkError> {
        AuthSdk::with_http_client_options(
            &self.control_url,
            self.token_store.clone(),
            self.control_client_options,
        )
    }

    pub fn controller(&self) -> Result<ControllerSdk<HttpControlClient, SdkTokenStore>, SdkError> {
        ControllerSdk::with_http_client_options(
            &self.control_url,
            self.token_store.clone(),
            self.control_client_options,
        )
    }

    pub fn admin(&self) -> Result<AdminSdk<HttpControlClient, SdkTokenStore>, SdkError> {
        AdminSdk::with_http_client_options(
            &self.control_url,
            self.token_store.clone(),
            self.control_client_options,
        )
    }

    pub fn server_auth(
        &self,
    ) -> Result<ServerAuthSdk<HttpControlClient, SdkServerCredentialStore>, SdkError> {
        ServerAuthSdk::with_http_client_options(
            &self.control_url,
            self.server_credential_store.clone(),
            self.control_client_options,
        )
    }

    pub fn server(
        &self,
    ) -> Result<ServerSdk<HttpControlClient, SdkServerCredentialStore>, SdkError> {
        ServerSdk::with_http_client_options(
            &self.control_url,
            self.server_credential_store.clone(),
            self.control_client_options,
        )
    }

    pub async fn ensure_browser_server_login(
        &self,
        input: ServerLoginInput,
    ) -> Result<EnsureBrowserServerLogin, SdkError> {
        if let Some(credential) = self.current_server_credential_for_control().await? {
            return Ok(EnsureBrowserServerLogin::Existing(credential));
        }
        Ok(EnsureBrowserServerLogin::Pending(
            self.server_auth()?.start_browser_login(input).await?,
        ))
    }

    pub async fn ensure_device_code_server_login(
        &self,
        input: ServerLoginInput,
    ) -> Result<EnsureDeviceCodeServerLogin, SdkError> {
        if let Some(credential) = self.current_server_credential_for_control().await? {
            return Ok(EnsureDeviceCodeServerLogin::Existing(credential));
        }
        Ok(EnsureDeviceCodeServerLogin::Pending(
            self.server_auth()?.start_device_code_login(input).await?,
        ))
    }

    pub async fn register(&self, input: RegisterInput) -> Result<StoredToken, SdkError> {
        self.auth()?.register(input).await
    }

    pub async fn ensure_register(&self, input: RegisterInput) -> Result<StoredToken, SdkError> {
        if let Some(token) = self.current_token().await? {
            return Ok(token);
        }
        self.register(input).await
    }

    pub async fn ensure_register_fresh(
        &self,
        input: RegisterInput,
        now_epoch_sec: u64,
    ) -> Result<StoredToken, SdkError> {
        if let Some(token) = self.current_valid_token(now_epoch_sec).await? {
            return Ok(token);
        }
        self.register(input).await
    }

    pub async fn login(&self, input: LoginInput) -> Result<StoredToken, SdkError> {
        self.auth()?.login(input).await
    }

    pub async fn ensure_login(&self, input: LoginInput) -> Result<StoredToken, SdkError> {
        if let Some(token) = self.current_token().await? {
            return Ok(token);
        }
        self.login(input).await
    }

    pub async fn ensure_login_fresh(
        &self,
        input: LoginInput,
        now_epoch_sec: u64,
    ) -> Result<StoredToken, SdkError> {
        if let Some(token) = self.current_valid_token(now_epoch_sec).await? {
            return Ok(token);
        }
        self.login(input).await
    }

    pub async fn update_password(
        &self,
        current_password: Option<String>,
        new_password: impl Into<String>,
    ) -> Result<(), SdkError> {
        self.auth()?
            .update_password(current_password, new_password)
            .await
    }

    pub async fn logout(&self) -> Result<(), SdkError> {
        self.clear_token().await
    }

    pub async fn register_controller(
        &self,
        input: RegisterControllerInput,
    ) -> Result<ControllerDevice, SdkError> {
        self.controller()?.register_controller(input).await
    }

    pub async fn ensure_controller(
        &self,
        input: RegisterControllerInput,
    ) -> Result<ControllerDevice, SdkError> {
        self.current_token()
            .await?
            .ok_or(SdkError::NotAuthenticated)?;
        self.register_controller(input).await
    }

    pub async fn list_devices(&self) -> Result<Vec<Device>, SdkError> {
        self.controller()?.list_devices().await
    }

    pub async fn list_device_services(
        &self,
        device_id: &DeviceId,
    ) -> Result<Vec<Service>, SdkError> {
        self.controller()?.list_device_services(device_id).await
    }

    pub async fn create_session(
        &self,
        input: CreateSessionInput,
    ) -> Result<CreateSessionResponse, SdkError> {
        self.controller()?.create_session(input).await
    }

    pub async fn start_mobile_tunnel_in_memory(
        &self,
        client_id: ClientId,
    ) -> Result<MobileTunnelSdk, SdkError> {
        MobileTunnelSdk::start_in_memory(self.mobile_config(client_id), self.token_store.clone())
            .await
    }

    pub async fn start_mobile_tunnel_with_control(
        &self,
        client_id: ClientId,
        relay_server_cert: CertificateDer<'static>,
    ) -> Result<MobileTunnelSdk, SdkError> {
        MobileTunnelSdk::start_with_control(
            self.mobile_config(client_id),
            self.token_store.clone(),
            relay_server_cert,
        )
        .await
    }

    pub async fn start_mobile_tunnel_p2p_or_relay(
        &self,
        client_id: ClientId,
        config: P2pOrRelayTunnelConfig,
    ) -> Result<MobileTunnelSdk, SdkError> {
        MobileTunnelSdk::start_with_control_p2p_or_relay(
            self.mobile_config(client_id),
            self.token_store.clone(),
            config,
        )
        .await
    }

    pub async fn open_mobile_service_in_memory(
        &self,
        input: OpenMobileServiceInput,
    ) -> Result<OpenedMobileService, SdkError> {
        let tunnel = self
            .start_mobile_tunnel_in_memory(input.client_id.clone())
            .await?;
        OpenedMobileService::open(tunnel, input).await
    }

    pub async fn open_mobile_service_with_control(
        &self,
        input: OpenMobileServiceInput,
        relay_server_cert: CertificateDer<'static>,
    ) -> Result<OpenedMobileService, SdkError> {
        let tunnel = self
            .start_mobile_tunnel_with_control(input.client_id.clone(), relay_server_cert)
            .await?;
        OpenedMobileService::open(tunnel, input).await
    }

    pub async fn open_mobile_service_p2p_or_relay(
        &self,
        input: OpenMobileServiceInput,
        config: P2pOrRelayTunnelConfig,
    ) -> Result<OpenedMobileService, SdkError> {
        let tunnel = self
            .start_mobile_tunnel_p2p_or_relay(input.client_id.clone(), config)
            .await?;
        OpenedMobileService::open(tunnel, input).await
    }

    fn mobile_config(&self, client_id: ClientId) -> MobileTunnelConfig {
        MobileTunnelConfig {
            control_server_url: self.control_url.clone(),
            client_id,
            control_client_options: self.control_client_options,
        }
    }
}

impl OpenedMobileService {
    pub fn tunnel(&self) -> &MobileTunnelSdk {
        &self.tunnel
    }

    pub fn forward(&self) -> &LocalForwardHandle {
        &self.forward
    }

    pub async fn close(self) -> Result<MobileTunnelSdk, SdkError> {
        let handle_id = self.forward.handle_id().to_string();
        self.tunnel.close_service(handle_id).await?;
        Ok(self.tunnel)
    }

    async fn open(
        tunnel: MobileTunnelSdk,
        input: OpenMobileServiceInput,
    ) -> Result<Self, SdkError> {
        let forward = tunnel
            .open_service(OpenServiceInput {
                device_id: input.device_id,
                service_id: input.service_id,
                local_port: input.local_port,
            })
            .await?;
        Ok(Self { tunnel, forward })
    }
}

impl MobileCodeConnectSdkBuilder {
    pub fn control_url(mut self, control_url: impl Into<String>) -> Self {
        self.control_url = Some(control_url.into());
        self
    }

    pub fn token_file(mut self, token_path: impl Into<PathBuf>) -> Self {
        self.token_store = Some(SdkTokenStore::file(token_path));
        self
    }

    pub fn memory_token_store(mut self) -> Self {
        self.token_store = Some(SdkTokenStore::memory());
        self
    }

    pub fn token_store(mut self, token_store: SdkTokenStore) -> Self {
        self.token_store = Some(token_store);
        self
    }

    pub fn server_credential_file(mut self, credential_path: impl Into<PathBuf>) -> Self {
        self.server_credential_store = Some(SdkServerCredentialStore::file(credential_path));
        self
    }

    pub fn memory_server_credential_store(mut self) -> Self {
        self.server_credential_store = Some(SdkServerCredentialStore::memory());
        self
    }

    pub fn server_credential_store(mut self, credential_store: SdkServerCredentialStore) -> Self {
        self.server_credential_store = Some(credential_store);
        self
    }

    pub fn control_client_options(mut self, options: HttpControlClientOptions) -> Self {
        self.control_client_options = Some(options);
        self
    }

    pub fn build(self) -> Result<MobileCodeConnectSdk, SdkError> {
        let control_url = self
            .control_url
            .map(|control_url| control_url.trim().to_string())
            .filter(|control_url| !control_url.is_empty())
            .ok_or_else(|| SdkError::InvalidConfig {
                reason: "control_url is required".to_string(),
            })?;

        Ok(MobileCodeConnectSdk {
            control_url,
            token_store: self.token_store.unwrap_or_default(),
            server_credential_store: self.server_credential_store.unwrap_or_default(),
            control_client_options: self.control_client_options.unwrap_or_default(),
        })
    }
}
