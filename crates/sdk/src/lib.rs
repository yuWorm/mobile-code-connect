pub mod admin;
pub mod auth;
pub mod client;
pub mod controller;
pub mod facade;
pub mod mobile;
pub mod server;
pub mod server_auth;
pub mod store;

pub use admin::AdminSdk;
pub use auth::{AuthSdk, LoginInput, RegisterInput};
pub use controller::{ControllerSdk, CreateSessionInput, RegisterControllerInput};
pub use facade::{
    EnsureBrowserServerLogin, EnsureDeviceCodeServerLogin, MobileCodeConnectSdk,
    MobileCodeConnectSdkBuilder, OpenMobileServiceInput, OpenedMobileService,
};
pub use mobile::{
    classify_browser_proxy_url_with_defaults, classify_browser_proxy_url_with_domain_suffix,
    BrowserProxyConfig, BrowserProxyDirectFallbackPolicy, BrowserProxyHandle, BrowserProxyRoute,
    BrowserProxyRouteKind, BrowserProxyStats, BrowserProxyUrlClassification, BrowserProxyUrlKind,
    MobileGrantPairingInput, MobileGrantPairingSession, MobileTunnelConfig, MobileTunnelSdk,
    OpenServiceInput, P2pOrRelayTunnelConfig,
};
pub use mobilecode_connect_control_client::HttpControlClientOptions;
pub use mobilecode_connect_mobile_core::{
    path::TunnelPath,
    status::{TunnelStatus, TunnelTransportStats},
};
pub use server::{ServerRegistrationInput, ServerSdk};
pub use server_auth::{
    FileServerCredentialStore, MemoryServerCredentialStore, SdkServerCredentialStore,
    ServerAuthSdk, ServerCredentialStore, ServerLoginInput, StoredServerCredential,
};
pub use store::{
    FileMobileGrantStore, FileTokenStore, MemoryMobileGrantStore, MemoryTokenStore,
    MobileGrantStore, SdkMobileGrantStore, SdkTokenStore, StoredToken, TokenStore,
};

#[derive(Debug, thiserror::Error)]
pub enum SdkError {
    #[error("control client failed: {0}")]
    Control(#[from] mobilecode_connect_control_client::ControlClientError),
    #[error("token store failed: {0}")]
    TokenStore(#[from] store::TokenStoreError),
    #[error("server credential store failed: {0}")]
    ServerCredentialStore(#[from] server_auth::ServerCredentialStoreError),
    #[error("mobile tunnel failed: {0}")]
    MobileTunnel(#[from] mobilecode_connect_mobile_core::client::TunnelError),
    #[error("browser proxy failed: {0}")]
    BrowserProxy(#[from] mobilecode_connect_mobile_core::browser_proxy::BrowserProxyError),
    #[error("mobile grant failed: {0}")]
    MobileGrant(#[from] mobilecode_connect_protocol::MobileGrantError),
    #[error("not authenticated")]
    NotAuthenticated,
    #[error("invalid sdk config: {reason}")]
    InvalidConfig { reason: String },
    #[error("server auth was approved without a credential")]
    ServerAuthApprovedWithoutCredential,
    #[error("server auth was denied")]
    ServerAuthDenied,
    #[error("server auth expired")]
    ServerAuthExpired,
    #[error("server auth was already consumed")]
    ServerAuthConsumed,
    #[error("mobile grant pairing was denied")]
    MobileGrantPairingDenied,
    #[error("mobile grant pairing expired")]
    MobileGrantPairingExpired,
    #[error("mobile grant pairing was approved without grant metadata")]
    MobileGrantPairingApprovedWithoutGrant,
}

impl SdkError {
    pub fn control_status_code(&self) -> Option<u16> {
        match self {
            Self::Control(mobilecode_connect_control_client::ControlClientError::HttpStatus {
                status_code,
                ..
            }) => Some(status_code.as_u16()),
            _ => None,
        }
    }

    pub fn is_unauthorized(&self) -> bool {
        matches!(self, Self::NotAuthenticated) || self.control_status_code() == Some(401)
    }

    pub fn is_forbidden(&self) -> bool {
        self.control_status_code() == Some(403)
    }

    pub fn requires_reauthentication(&self) -> bool {
        self.is_unauthorized()
    }
}
