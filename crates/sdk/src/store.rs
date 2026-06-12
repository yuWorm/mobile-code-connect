use std::{
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};

use async_trait::async_trait;
use quic_tunnel_protocol::{MobileGrantCredential, UserId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredToken {
    pub user_id: UserId,
    pub access_token: String,
    pub expire_at: u64,
}

impl StoredToken {
    pub fn is_expired_at(&self, now_epoch_sec: u64) -> bool {
        self.expire_at <= now_epoch_sec
    }

    pub fn is_valid_at(&self, now_epoch_sec: u64) -> bool {
        !self.is_expired_at(now_epoch_sec)
    }
}

#[async_trait]
pub trait TokenStore: Send + Sync {
    async fn load_token(&self) -> Result<Option<StoredToken>, TokenStoreError>;
    async fn save_token(&self, token: StoredToken) -> Result<(), TokenStoreError>;
    async fn clear_token(&self) -> Result<(), TokenStoreError>;
}

#[derive(Debug, Clone, Default)]
pub struct MemoryTokenStore {
    token: Arc<RwLock<Option<StoredToken>>>,
}

#[async_trait]
impl TokenStore for MemoryTokenStore {
    async fn load_token(&self) -> Result<Option<StoredToken>, TokenStoreError> {
        Ok(self
            .token
            .read()
            .map_err(|_| TokenStoreError::LockPoisoned)?
            .clone())
    }

    async fn save_token(&self, token: StoredToken) -> Result<(), TokenStoreError> {
        *self
            .token
            .write()
            .map_err(|_| TokenStoreError::LockPoisoned)? = Some(token);
        Ok(())
    }

    async fn clear_token(&self) -> Result<(), TokenStoreError> {
        *self
            .token
            .write()
            .map_err(|_| TokenStoreError::LockPoisoned)? = None;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct FileTokenStore {
    path: PathBuf,
}

impl FileTokenStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[async_trait]
impl TokenStore for FileTokenStore {
    async fn load_token(&self) -> Result<Option<StoredToken>, TokenStoreError> {
        match tokio::fs::read(&self.path).await {
            Ok(body) => Ok(Some(serde_json::from_slice(&body)?)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    async fn save_token(&self, token: StoredToken) -> Result<(), TokenStoreError> {
        if let Some(parent) = self
            .path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            tokio::fs::create_dir_all(parent).await?;
        }
        let body = serde_json::to_vec_pretty(&token)?;
        tokio::fs::write(&self.path, body).await?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            tokio::fs::set_permissions(&self.path, std::fs::Permissions::from_mode(0o600)).await?;
        }

        Ok(())
    }

    async fn clear_token(&self) -> Result<(), TokenStoreError> {
        match tokio::fs::remove_file(&self.path).await {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TokenStoreError {
    #[error("token store lock poisoned")]
    LockPoisoned,
    #[error("io failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("json failed: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone)]
pub enum SdkTokenStore {
    Memory(MemoryTokenStore),
    File(FileTokenStore),
}

impl SdkTokenStore {
    pub fn memory() -> Self {
        Self::Memory(MemoryTokenStore::default())
    }

    pub fn file(path: impl Into<PathBuf>) -> Self {
        Self::File(FileTokenStore::new(path))
    }
}

impl Default for SdkTokenStore {
    fn default() -> Self {
        Self::memory()
    }
}

#[async_trait]
impl TokenStore for SdkTokenStore {
    async fn load_token(&self) -> Result<Option<StoredToken>, TokenStoreError> {
        match self {
            Self::Memory(store) => store.load_token().await,
            Self::File(store) => store.load_token().await,
        }
    }

    async fn save_token(&self, token: StoredToken) -> Result<(), TokenStoreError> {
        match self {
            Self::Memory(store) => store.save_token(token).await,
            Self::File(store) => store.save_token(token).await,
        }
    }

    async fn clear_token(&self) -> Result<(), TokenStoreError> {
        match self {
            Self::Memory(store) => store.clear_token().await,
            Self::File(store) => store.clear_token().await,
        }
    }
}

#[async_trait]
pub trait MobileGrantStore: Send + Sync {
    async fn load_mobile_grant(&self) -> Result<Option<MobileGrantCredential>, TokenStoreError>;
    async fn save_mobile_grant(&self, grant: MobileGrantCredential) -> Result<(), TokenStoreError>;
    async fn clear_mobile_grant(&self) -> Result<(), TokenStoreError>;
}

#[derive(Debug, Clone, Default)]
pub struct MemoryMobileGrantStore {
    grant: Arc<RwLock<Option<MobileGrantCredential>>>,
}

#[async_trait]
impl MobileGrantStore for MemoryMobileGrantStore {
    async fn load_mobile_grant(&self) -> Result<Option<MobileGrantCredential>, TokenStoreError> {
        Ok(self
            .grant
            .read()
            .map_err(|_| TokenStoreError::LockPoisoned)?
            .clone())
    }

    async fn save_mobile_grant(&self, grant: MobileGrantCredential) -> Result<(), TokenStoreError> {
        *self
            .grant
            .write()
            .map_err(|_| TokenStoreError::LockPoisoned)? = Some(grant);
        Ok(())
    }

    async fn clear_mobile_grant(&self) -> Result<(), TokenStoreError> {
        *self
            .grant
            .write()
            .map_err(|_| TokenStoreError::LockPoisoned)? = None;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct FileMobileGrantStore {
    path: PathBuf,
}

impl FileMobileGrantStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[async_trait]
impl MobileGrantStore for FileMobileGrantStore {
    async fn load_mobile_grant(&self) -> Result<Option<MobileGrantCredential>, TokenStoreError> {
        match tokio::fs::read(&self.path).await {
            Ok(body) => Ok(Some(serde_json::from_slice(&body)?)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    async fn save_mobile_grant(&self, grant: MobileGrantCredential) -> Result<(), TokenStoreError> {
        if let Some(parent) = self
            .path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            tokio::fs::create_dir_all(parent).await?;
        }
        let body = serde_json::to_vec_pretty(&grant)?;
        tokio::fs::write(&self.path, body).await?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            tokio::fs::set_permissions(&self.path, std::fs::Permissions::from_mode(0o600)).await?;
        }

        Ok(())
    }

    async fn clear_mobile_grant(&self) -> Result<(), TokenStoreError> {
        match tokio::fs::remove_file(&self.path).await {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
    }
}

#[derive(Debug, Clone)]
pub enum SdkMobileGrantStore {
    Memory(MemoryMobileGrantStore),
    File(FileMobileGrantStore),
}

impl SdkMobileGrantStore {
    pub fn memory() -> Self {
        Self::Memory(MemoryMobileGrantStore::default())
    }

    pub fn file(path: impl Into<PathBuf>) -> Self {
        Self::File(FileMobileGrantStore::new(path))
    }
}

impl Default for SdkMobileGrantStore {
    fn default() -> Self {
        Self::memory()
    }
}

#[async_trait]
impl MobileGrantStore for SdkMobileGrantStore {
    async fn load_mobile_grant(&self) -> Result<Option<MobileGrantCredential>, TokenStoreError> {
        match self {
            Self::Memory(store) => store.load_mobile_grant().await,
            Self::File(store) => store.load_mobile_grant().await,
        }
    }

    async fn save_mobile_grant(&self, grant: MobileGrantCredential) -> Result<(), TokenStoreError> {
        match self {
            Self::Memory(store) => store.save_mobile_grant(grant).await,
            Self::File(store) => store.save_mobile_grant(grant).await,
        }
    }

    async fn clear_mobile_grant(&self) -> Result<(), TokenStoreError> {
        match self {
            Self::Memory(store) => store.clear_mobile_grant().await,
            Self::File(store) => store.clear_mobile_grant().await,
        }
    }
}
