use async_trait::async_trait;
use mobilecode_connect_control_client::{
    ControlClientError, ControllerDevice, CreateSessionRequest, CreateSessionResponse,
    HttpControlClient, HttpControlClientOptions, RegisterControllerDeviceRequest,
};
use mobilecode_connect_protocol::{ClientId, Device, DeviceId, Service, ServiceId};
use tokio::sync::Mutex;

use crate::{store::TokenStore, SdkError};

#[async_trait]
pub trait ControllerApi: Send {
    fn set_bearer_token(&mut self, bearer_token: String);

    async fn register_controller(
        &mut self,
        request: RegisterControllerDeviceRequest,
    ) -> Result<ControllerDevice, ControlClientError>;

    async fn list_devices(&mut self) -> Result<Vec<Device>, ControlClientError>;

    async fn list_device_services(
        &mut self,
        device_id: &DeviceId,
    ) -> Result<Vec<Service>, ControlClientError>;

    async fn create_session(
        &mut self,
        request: CreateSessionRequest,
    ) -> Result<CreateSessionResponse, ControlClientError>;
}

#[async_trait]
impl ControllerApi for HttpControlClient {
    fn set_bearer_token(&mut self, bearer_token: String) {
        HttpControlClient::set_bearer_token(self, bearer_token);
    }

    async fn register_controller(
        &mut self,
        request: RegisterControllerDeviceRequest,
    ) -> Result<ControllerDevice, ControlClientError> {
        HttpControlClient::register_controller(self, request).await
    }

    async fn list_devices(&mut self) -> Result<Vec<Device>, ControlClientError> {
        HttpControlClient::list_devices(self).await
    }

    async fn list_device_services(
        &mut self,
        device_id: &DeviceId,
    ) -> Result<Vec<Service>, ControlClientError> {
        HttpControlClient::list_device_services(self, device_id).await
    }

    async fn create_session(
        &mut self,
        request: CreateSessionRequest,
    ) -> Result<CreateSessionResponse, ControlClientError> {
        HttpControlClient::create_session(self, request).await
    }
}

pub struct ControllerSdk<A, S> {
    api: Mutex<A>,
    token_store: S,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterControllerInput {
    pub client_id: ClientId,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateSessionInput {
    pub client_id: ClientId,
    pub device_id: DeviceId,
    pub service_id: ServiceId,
}

impl<A, S> ControllerSdk<A, S>
where
    A: ControllerApi,
    S: TokenStore,
{
    pub fn new(api: A, token_store: S) -> Self {
        Self {
            api: Mutex::new(api),
            token_store,
        }
    }

    pub async fn register_controller(
        &self,
        input: RegisterControllerInput,
    ) -> Result<ControllerDevice, SdkError> {
        let mut api = self.authorized_api().await?;
        Ok(api
            .register_controller(RegisterControllerDeviceRequest {
                client_id: input.client_id.to_string(),
                name: input.name,
            })
            .await?)
    }

    pub async fn list_devices(&self) -> Result<Vec<Device>, SdkError> {
        let mut api = self.authorized_api().await?;
        Ok(api.list_devices().await?)
    }

    pub async fn list_device_services(
        &self,
        device_id: &DeviceId,
    ) -> Result<Vec<Service>, SdkError> {
        let mut api = self.authorized_api().await?;
        Ok(api.list_device_services(device_id).await?)
    }

    pub async fn create_session(
        &self,
        input: CreateSessionInput,
    ) -> Result<CreateSessionResponse, SdkError> {
        let mut api = self.authorized_api().await?;
        Ok(api
            .create_session(CreateSessionRequest {
                client_id: input.client_id.to_string(),
                device_id: input.device_id,
                service_id: input.service_id,
            })
            .await?)
    }

    pub async fn current_token(&self) -> Result<Option<crate::store::StoredToken>, SdkError> {
        Ok(self.token_store.load_token().await?)
    }

    async fn authorized_api(&self) -> Result<tokio::sync::MutexGuard<'_, A>, SdkError> {
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

impl<S> ControllerSdk<HttpControlClient, S>
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
