use std::path::PathBuf;

use mobilecode_connect_control_client::{
    AuthResponse, HttpControlClient, HttpControlClientOptions, LoginRequest, RegisterUserRequest,
    UpdatePasswordRequest,
};
use tokio::sync::Mutex;

use crate::{
    client::ControlApi,
    store::{FileTokenStore, MemoryTokenStore, StoredToken, TokenStore},
    SdkError,
};

pub struct AuthSdk<A, S> {
    api: Mutex<A>,
    token_store: S,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterInput {
    pub email: String,
    pub password: String,
    pub display_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginInput {
    pub email: String,
    pub password: String,
}

impl<A, S> AuthSdk<A, S>
where
    A: ControlApi,
    S: TokenStore,
{
    pub fn new(api: A, token_store: S) -> Self {
        Self {
            api: Mutex::new(api),
            token_store,
        }
    }

    pub async fn register(&self, input: RegisterInput) -> Result<StoredToken, SdkError> {
        let mut api = self.api.lock().await;
        let response = api
            .register_user(RegisterUserRequest {
                email: input.email,
                password: input.password,
                display_name: input.display_name,
            })
            .await?;
        self.persist_auth_response(&mut api, response).await
    }

    pub async fn login(&self, input: LoginInput) -> Result<StoredToken, SdkError> {
        let mut api = self.api.lock().await;
        let response = api
            .login(LoginRequest {
                email: input.email,
                password: input.password,
            })
            .await?;
        self.persist_auth_response(&mut api, response).await
    }

    pub async fn update_password(
        &self,
        current_password: Option<String>,
        new_password: impl Into<String>,
    ) -> Result<(), SdkError> {
        let token = self
            .token_store
            .load_token()
            .await?
            .ok_or(SdkError::NotAuthenticated)?;
        let mut api = self.api.lock().await;
        api.set_bearer_token(token.access_token);
        api.update_password(UpdatePasswordRequest {
            current_password,
            new_password: new_password.into(),
        })
        .await?;
        Ok(())
    }

    pub async fn current_token(&self) -> Result<Option<StoredToken>, SdkError> {
        Ok(self.token_store.load_token().await?)
    }

    pub async fn logout(&self) -> Result<(), SdkError> {
        self.token_store.clear_token().await?;
        Ok(())
    }

    async fn persist_auth_response(
        &self,
        api: &mut A,
        response: AuthResponse,
    ) -> Result<StoredToken, SdkError> {
        let token = StoredToken {
            user_id: response.user_id,
            access_token: response.access_token,
            expire_at: response.expire_at,
        };
        self.token_store.save_token(token.clone()).await?;
        api.set_bearer_token(token.access_token.clone());
        Ok(token)
    }
}

impl<S> AuthSdk<HttpControlClient, S>
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

impl AuthSdk<HttpControlClient, MemoryTokenStore> {
    pub fn in_memory(base_url: impl AsRef<str>) -> Result<Self, SdkError> {
        Self::with_http_client(base_url, MemoryTokenStore::default())
    }
}

impl AuthSdk<HttpControlClient, FileTokenStore> {
    pub fn with_file_token_store(
        base_url: impl AsRef<str>,
        token_path: impl Into<PathBuf>,
    ) -> Result<Self, SdkError> {
        Self::with_http_client(base_url, FileTokenStore::new(token_path))
    }
}
