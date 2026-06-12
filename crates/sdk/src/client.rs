use async_trait::async_trait;
use mobilecode_connect_control_client::{
    AuthResponse, ControlClientError, HttpControlClient, LoginRequest, RegisterUserRequest,
    UpdatePasswordRequest,
};

#[async_trait]
pub trait ControlApi: Send {
    fn set_bearer_token(&mut self, bearer_token: String);

    async fn register_user(
        &mut self,
        request: RegisterUserRequest,
    ) -> Result<AuthResponse, ControlClientError>;

    async fn login(&mut self, request: LoginRequest) -> Result<AuthResponse, ControlClientError>;

    async fn update_password(
        &mut self,
        request: UpdatePasswordRequest,
    ) -> Result<(), ControlClientError>;
}

#[async_trait]
impl ControlApi for HttpControlClient {
    fn set_bearer_token(&mut self, bearer_token: String) {
        HttpControlClient::set_bearer_token(self, bearer_token);
    }

    async fn register_user(
        &mut self,
        request: RegisterUserRequest,
    ) -> Result<AuthResponse, ControlClientError> {
        HttpControlClient::register_user(self, request).await
    }

    async fn login(&mut self, request: LoginRequest) -> Result<AuthResponse, ControlClientError> {
        HttpControlClient::login(self, request).await
    }

    async fn update_password(
        &mut self,
        request: UpdatePasswordRequest,
    ) -> Result<(), ControlClientError> {
        HttpControlClient::update_password(self, request).await
    }
}
