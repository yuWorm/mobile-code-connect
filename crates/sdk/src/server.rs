use std::path::PathBuf;

use async_trait::async_trait;
use quic_tunnel_control_client::{
    AgentSessionAssignment, ControlClientError, HttpControlClient, HttpControlClientOptions,
};
use quic_tunnel_protocol::{Device, DeviceId, Service, SessionId};
use tokio::sync::{Mutex, MutexGuard};

use crate::{
    server_auth::{FileServerCredentialStore, ServerCredentialStore, StoredServerCredential},
    SdkError,
};

#[async_trait]
pub trait ServerApi: Send {
    fn set_bearer_token(&mut self, bearer_token: String);

    async fn register_device(&mut self, device: Device) -> Result<(), ControlClientError>;

    async fn register_services(&mut self, services: Vec<Service>)
        -> Result<(), ControlClientError>;

    async fn register_p2p_certificate(
        &mut self,
        device_id: &DeviceId,
        certificate_der: Vec<u8>,
    ) -> Result<(), ControlClientError>;

    async fn list_agent_sessions(
        &mut self,
        device_id: &DeviceId,
    ) -> Result<Vec<AgentSessionAssignment>, ControlClientError>;

    async fn claim_agent_session(
        &mut self,
        session_id: &SessionId,
    ) -> Result<AgentSessionAssignment, ControlClientError>;

    async fn mark_agent_session_bound(
        &mut self,
        session_id: &SessionId,
    ) -> Result<AgentSessionAssignment, ControlClientError>;

    async fn close_session(
        &mut self,
        session_id: &SessionId,
    ) -> Result<AgentSessionAssignment, ControlClientError>;
}

#[async_trait]
impl ServerApi for HttpControlClient {
    fn set_bearer_token(&mut self, bearer_token: String) {
        HttpControlClient::set_bearer_token(self, bearer_token);
    }

    async fn register_device(&mut self, device: Device) -> Result<(), ControlClientError> {
        HttpControlClient::register_device(self, device).await
    }

    async fn register_services(
        &mut self,
        services: Vec<Service>,
    ) -> Result<(), ControlClientError> {
        HttpControlClient::register_services(self, services).await
    }

    async fn register_p2p_certificate(
        &mut self,
        device_id: &DeviceId,
        certificate_der: Vec<u8>,
    ) -> Result<(), ControlClientError> {
        HttpControlClient::register_p2p_certificate(self, device_id, certificate_der).await
    }

    async fn list_agent_sessions(
        &mut self,
        device_id: &DeviceId,
    ) -> Result<Vec<AgentSessionAssignment>, ControlClientError> {
        HttpControlClient::list_agent_sessions(self, device_id).await
    }

    async fn claim_agent_session(
        &mut self,
        session_id: &SessionId,
    ) -> Result<AgentSessionAssignment, ControlClientError> {
        HttpControlClient::claim_agent_session(self, session_id).await
    }

    async fn mark_agent_session_bound(
        &mut self,
        session_id: &SessionId,
    ) -> Result<AgentSessionAssignment, ControlClientError> {
        HttpControlClient::mark_agent_session_bound(self, session_id).await
    }

    async fn close_session(
        &mut self,
        session_id: &SessionId,
    ) -> Result<AgentSessionAssignment, ControlClientError> {
        HttpControlClient::close_session(self, session_id).await
    }
}

pub struct ServerSdk<A, S> {
    api: Mutex<A>,
    credential_store: S,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerRegistrationInput {
    pub device: Device,
    pub services: Vec<Service>,
    pub p2p_certificate_der: Option<Vec<u8>>,
}

impl<A, S> ServerSdk<A, S>
where
    A: ServerApi,
    S: ServerCredentialStore,
{
    pub fn new(api: A, credential_store: S) -> Self {
        Self {
            api: Mutex::new(api),
            credential_store,
        }
    }

    pub async fn load_credential(&self) -> Result<Option<StoredServerCredential>, SdkError> {
        Ok(self.credential_store.load_credential().await?)
    }

    pub async fn clear_credential(&self) -> Result<(), SdkError> {
        self.credential_store.clear_credential().await?;
        Ok(())
    }

    pub async fn register_server(&self, input: ServerRegistrationInput) -> Result<(), SdkError> {
        self.register_device(input.device).await?;
        if let Some(certificate_der) = input.p2p_certificate_der {
            self.register_p2p_certificate(certificate_der).await?;
        }
        self.register_services(input.services).await?;
        Ok(())
    }

    pub async fn register_device(&self, device: Device) -> Result<(), SdkError> {
        let (mut api, _) = self.authorized_api().await?;
        api.register_device(device).await?;
        Ok(())
    }

    pub async fn register_services(&self, services: Vec<Service>) -> Result<(), SdkError> {
        let (mut api, _) = self.authorized_api().await?;
        api.register_services(services).await?;
        Ok(())
    }

    pub async fn register_p2p_certificate(&self, certificate_der: Vec<u8>) -> Result<(), SdkError> {
        let (mut api, credential) = self.authorized_api().await?;
        api.register_p2p_certificate(&credential.device_id, certificate_der)
            .await?;
        Ok(())
    }

    pub async fn list_sessions(&self) -> Result<Vec<AgentSessionAssignment>, SdkError> {
        let (mut api, credential) = self.authorized_api().await?;
        Ok(api.list_agent_sessions(&credential.device_id).await?)
    }

    pub async fn claim_session(
        &self,
        session_id: &SessionId,
    ) -> Result<AgentSessionAssignment, SdkError> {
        let (mut api, _) = self.authorized_api().await?;
        Ok(api.claim_agent_session(session_id).await?)
    }

    pub async fn mark_session_bound(
        &self,
        session_id: &SessionId,
    ) -> Result<AgentSessionAssignment, SdkError> {
        let (mut api, _) = self.authorized_api().await?;
        Ok(api.mark_agent_session_bound(session_id).await?)
    }

    pub async fn close_session(
        &self,
        session_id: &SessionId,
    ) -> Result<AgentSessionAssignment, SdkError> {
        let (mut api, _) = self.authorized_api().await?;
        Ok(api.close_session(session_id).await?)
    }

    async fn authorized_api(
        &self,
    ) -> Result<(MutexGuard<'_, A>, StoredServerCredential), SdkError> {
        let credential = self
            .credential_store
            .load_credential()
            .await?
            .ok_or(SdkError::NotAuthenticated)?;
        let mut api = self.api.lock().await;
        api.set_bearer_token(credential.server_token.clone());
        Ok((api, credential))
    }
}

impl<S> ServerSdk<HttpControlClient, S>
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
        Ok(Self::new(
            HttpControlClient::with_options(control_server, options)?,
            credential_store,
        ))
    }
}

impl ServerSdk<HttpControlClient, FileServerCredentialStore> {
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
