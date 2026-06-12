use mobilecode_connect_protocol::UserId;
use mobilecode_connect_sdk::{
    auth::AuthSdk,
    store::{FileTokenStore, StoredToken, TokenStore},
};

#[tokio::test]
async fn file_token_store_roundtrips_and_clears_saved_token() {
    let dir = unique_temp_dir();
    tokio::fs::create_dir_all(&dir).await.unwrap();
    let path = dir.join("user-token.json");
    let store = FileTokenStore::new(path.clone());
    let token = StoredToken {
        user_id: UserId::new("user_001"),
        access_token: "token.saved".to_string(),
        expire_at: 100,
    };

    assert_eq!(store.load_token().await.unwrap(), None);

    store.save_token(token.clone()).await.unwrap();
    assert_eq!(store.load_token().await.unwrap(), Some(token));

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = tokio::fs::metadata(&path)
            .await
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);
    }

    store.clear_token().await.unwrap();
    assert_eq!(store.load_token().await.unwrap(), None);

    tokio::fs::remove_dir_all(&dir).await.unwrap();
}

#[tokio::test]
async fn file_token_store_creates_parent_directories() {
    let dir = unique_temp_dir();
    let path = dir.join("nested").join("tokens").join("user-token.json");
    let store = FileTokenStore::new(path.clone());

    store
        .save_token(StoredToken {
            user_id: UserId::new("user_002"),
            access_token: "token.nested".to_string(),
            expire_at: 200,
        })
        .await
        .unwrap();

    assert!(path.is_file());
    assert_eq!(
        store.load_token().await.unwrap().unwrap().access_token,
        "token.nested"
    );

    tokio::fs::remove_dir_all(&dir).await.unwrap();
}

#[tokio::test]
async fn auth_sdk_can_be_constructed_with_file_token_store() {
    let dir = unique_temp_dir();
    let path = dir.join("user-token.json");
    let sdk =
        AuthSdk::with_file_token_store("http://127.0.0.1:4242", path).expect("build auth sdk");

    assert_eq!(sdk.current_token().await.unwrap(), None);
}

fn unique_temp_dir() -> std::path::PathBuf {
    static NEXT_TEMP_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let id = NEXT_TEMP_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    std::env::temp_dir().join(format!("quic-test-sdk-token-store-{suffix}-{id}"))
}
