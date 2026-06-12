use std::path::{Path, PathBuf};

use rusqlite::{params, Connection, OptionalExtension};

use crate::store::ControlStore;

const SNAPSHOT_KEY: &str = "main";

#[derive(Clone, Debug)]
pub(crate) struct SqliteControlStore {
    path: PathBuf,
}

impl SqliteControlStore {
    pub(crate) fn open(path: impl AsRef<Path>) -> Result<Self, SqliteStoreError> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent)?;
        }
        let store = Self { path };
        store.ensure_schema()?;
        Ok(store)
    }

    pub(crate) fn load_snapshot(&self) -> Result<Option<ControlStore>, SqliteStoreError> {
        let connection = self.connection()?;
        let value: Option<String> = connection
            .query_row(
                "SELECT value FROM control_state_snapshots WHERE key = ?1",
                [SNAPSHOT_KEY],
                |row| row.get(0),
            )
            .optional()?;

        value
            .map(|value| serde_json::from_str(&value))
            .transpose()
            .map_err(Into::into)
    }

    pub(crate) fn save_snapshot(&self, store: &ControlStore) -> Result<(), SqliteStoreError> {
        let value = serde_json::to_string(store)?;
        let connection = self.connection()?;
        connection.execute(
            "INSERT INTO control_state_snapshots (key, value)
             VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![SNAPSHOT_KEY, value],
        )?;
        Ok(())
    }

    fn ensure_schema(&self) -> Result<(), SqliteStoreError> {
        let connection = self.connection()?;
        connection.execute_batch(
            "CREATE TABLE IF NOT EXISTS control_state_snapshots (
                key TEXT PRIMARY KEY NOT NULL,
                value TEXT NOT NULL
            );",
        )?;
        Ok(())
    }

    fn connection(&self) -> Result<Connection, SqliteStoreError> {
        Connection::open(&self.path).map_err(Into::into)
    }
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum SqliteStoreError {
    #[error("sqlite store io failed")]
    Io(#[from] std::io::Error),
    #[error("sqlite store query failed")]
    Sqlite(#[from] rusqlite::Error),
    #[error("sqlite store serialization failed")]
    Serde(#[from] serde_json::Error),
}
