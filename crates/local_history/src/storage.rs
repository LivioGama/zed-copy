use crate::identifiers::{FileId, SnapshotId, WorkspaceId};
use crate::snapshot::{Snapshot, SnapshotLabel, SnapshotMetadata};
use anyhow::{Context, Result};
use parking_lot::RwLock;
use rusqlite::{params, Connection, OptionalExtension};
use std::path::PathBuf;
use std::sync::Arc;

/// Storage configuration
#[derive(Debug, Clone)]
pub struct StorageConfig {
    /// Base directory for storage
    pub base_dir: PathBuf,
    /// Maximum total size in bytes (default: 10GB)
    pub max_total_size: u64,
    /// Maximum age in days (default: 30)
    pub max_age_days: u32,
    /// Maximum snapshots per file (default: 200)
    pub max_snapshots_per_file: u32,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            base_dir: PathBuf::from(".local_history"),
            max_total_size: 10 * 1024 * 1024 * 1024, // 10GB
            max_age_days: 30,
            max_snapshots_per_file: 200,
        }
    }
}

/// Main storage implementation
pub struct HistoryStorage {
    config: StorageConfig,
    db: Arc<RwLock<Connection>>,
    blob_dir: PathBuf,
}

impl HistoryStorage {
    pub fn new(config: StorageConfig) -> Result<Self> {
        std::fs::create_dir_all(&config.base_dir)?;
        
        let db_path = config.base_dir.join("history.db");
        let conn = Connection::open(&db_path)?;
        
        // Initialize schema
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS snapshots (
                id BLOB PRIMARY KEY,
                workspace_id BLOB NOT NULL,
                file_id_hash INTEGER NOT NULL,
                file_id_inode INTEGER,
                file_id_device INTEGER,
                timestamp INTEGER NOT NULL,
                label_type TEXT,
                label_value TEXT,
                parent_id BLOB,
                content_hash BLOB NOT NULL,
                size INTEGER NOT NULL,
                is_compressed INTEGER NOT NULL,
                blob_path TEXT NOT NULL,
                created_at INTEGER NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_workspace_file 
            ON snapshots(workspace_id, file_id_hash);

            CREATE INDEX IF NOT EXISTS idx_timestamp 
            ON snapshots(timestamp);

            CREATE TABLE IF NOT EXISTS exclusions (
                id INTEGER PRIMARY KEY,
                workspace_id BLOB NOT NULL,
                pattern TEXT NOT NULL
            );
            "#,
        )?;

        let blob_dir = config.base_dir.join("blobs");
        std::fs::create_dir_all(&blob_dir)?;

        Ok(Self {
            config,
            db: Arc::new(RwLock::new(conn)),
            blob_dir,
        })
    }

    pub async fn create_snapshot(
        &self,
        workspace_id: WorkspaceId,
        file_id: FileId,
        content: Vec<u8>,
        label: Option<SnapshotLabel>,
    ) -> Result<SnapshotId> {
        let mut snapshot = Snapshot::new(workspace_id, file_id.clone(), content, label.clone(), None);
        snapshot.compress()?;

        // Save blob
        let blob_path = self.get_blob_path(&snapshot.metadata.id);
        std::fs::write(&blob_path, &snapshot.content)?;

        // Save metadata to DB
        let db = self.db.write();
        let (label_type, label_value) = match &label {
            Some(SnapshotLabel::Manual(s)) => (Some("manual"), Some(s.as_str())),
            Some(SnapshotLabel::BeforeCommit) => (Some("before_commit"), None),
            Some(SnapshotLabel::AfterCommit) => (Some("after_commit"), None),
            Some(SnapshotLabel::BeforeRefactor) => (Some("before_refactor"), None),
            Some(SnapshotLabel::ExternalChange) => (Some("external_change"), None),
            Some(SnapshotLabel::Checkout) => (Some("checkout"), None),
            Some(SnapshotLabel::VcsOperation(s)) => (Some("vcs_operation"), Some(s.as_str())),
            None => (None, None),
        };

        db.execute(
            r#"
            INSERT INTO snapshots (
                id, workspace_id, file_id_hash, file_id_inode, file_id_device,
                timestamp, label_type, label_value, parent_id, content_hash,
                size, is_compressed, blob_path, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
            "#,
            params![
                snapshot.metadata.id.as_bytes(),
                workspace_id.to_uuid().as_bytes(),
                file_id.path_hash() as i64,
                file_id.inode.map(|i| i as i64),
                file_id.device_id.map(|d| d as i64),
                snapshot.metadata.timestamp.timestamp(),
                label_type,
                label_value,
                snapshot.metadata.parent_id.map(|id| id.as_bytes().to_vec()),
                &snapshot.metadata.content_hash[..],
                snapshot.metadata.size as i64,
                if snapshot.metadata.is_compressed { 1 } else { 0 },
                blob_path.to_string_lossy().as_ref(),
                chrono::Utc::now().timestamp(),
            ],
        )?;

        Ok(snapshot.metadata.id)
    }

    pub async fn get_snapshot(&self, snapshot_id: SnapshotId) -> Result<Option<SnapshotMetadata>> {
        let db = self.db.read();
        let mut stmt = db.prepare(
            r#"
            SELECT workspace_id, file_id_hash, file_id_inode, file_id_device,
                   timestamp, label_type, label_value, parent_id, content_hash,
                   size, is_compressed
            FROM snapshots WHERE id = ?1
            "#,
        )?;

        let metadata = stmt.query_row(params![snapshot_id.as_bytes()], |row| {
            let workspace_bytes: Vec<u8> = row.get(0)?;
            let workspace_id = WorkspaceId::from_uuid(uuid::Uuid::from_slice(&workspace_bytes).unwrap());
            
            let file_id = FileId {
                path_hash: row.get::<_, i64>(1)? as u64,
                inode: row.get::<_, Option<i64>>(2)?.map(|i| i as u64),
                device_id: row.get::<_, Option<i64>>(3)?.map(|d| d as u64),
            };

            let timestamp = chrono::DateTime::from_timestamp(row.get(4)?, 0).unwrap();
            
            let label = match (row.get::<_, Option<String>>(5)?, row.get::<_, Option<String>>(6)?) {
                (Some(t), v) => match t.as_str() {
                    "manual" => Some(SnapshotLabel::Manual(v.unwrap_or_default())),
                    "before_commit" => Some(SnapshotLabel::BeforeCommit),
                    "after_commit" => Some(SnapshotLabel::AfterCommit),
                    "before_refactor" => Some(SnapshotLabel::BeforeRefactor),
                    "external_change" => Some(SnapshotLabel::ExternalChange),
                    "checkout" => Some(SnapshotLabel::Checkout),
                    "vcs_operation" => Some(SnapshotLabel::VcsOperation(v.unwrap_or_default())),
                    _ => None,
                },
                _ => None,
            };

            let parent_id = row.get::<_, Option<Vec<u8>>>(7)?.and_then(|bytes| {
                if bytes.len() == 32 {
                    let mut arr = [0u8; 32];
                    arr.copy_from_slice(&bytes);
                    Some(SnapshotId::from_bytes(arr))
                } else {
                    None
                }
            });

            let content_hash_vec: Vec<u8> = row.get(8)?;
            let mut content_hash = [0u8; 32];
            content_hash.copy_from_slice(&content_hash_vec);

            Ok(SnapshotMetadata {
                id: snapshot_id,
                workspace_id,
                file_id,
                timestamp,
                label,
                parent_id,
                content_hash,
                size: row.get::<_, i64>(9)? as u64,
                is_compressed: row.get::<_, i32>(10)? != 0,
            })
        }).optional()?;

        Ok(metadata)
    }

    pub async fn get_snapshot_content(&self, snapshot_id: SnapshotId) -> Result<Vec<u8>> {
        let blob_path = self.get_blob_path(&snapshot_id);
        let compressed = std::fs::read(&blob_path)
            .context("Failed to read snapshot blob")?;
        let decompressed = zstd::decode_all(&compressed[..])
            .context("Failed to decompress snapshot")?;
        Ok(decompressed)
    }

    pub async fn list_snapshots(&self, file_id: FileId) -> Result<Vec<SnapshotMetadata>> {
        let db = self.db.read();
        let mut stmt = db.prepare(
            r#"
            SELECT id, workspace_id, timestamp, label_type, label_value, 
                   parent_id, content_hash, size, is_compressed
            FROM snapshots 
            WHERE file_id_hash = ?1
            ORDER BY timestamp DESC
            "#,
        )?;

        let rows = stmt.query_map(params![file_id.path_hash() as i64], |row| {
            let id_bytes: Vec<u8> = row.get(0)?;
            let mut id_arr = [0u8; 32];
            id_arr.copy_from_slice(&id_bytes);
            let id = SnapshotId::from_bytes(id_arr);

            let workspace_bytes: Vec<u8> = row.get(1)?;
            let workspace_id = WorkspaceId::from_uuid(uuid::Uuid::from_slice(&workspace_bytes).unwrap());

            let timestamp = chrono::DateTime::from_timestamp(row.get(2)?, 0).unwrap();
            
            let label = match (row.get::<_, Option<String>>(3)?, row.get::<_, Option<String>>(4)?) {
                (Some(t), v) => match t.as_str() {
                    "manual" => Some(SnapshotLabel::Manual(v.unwrap_or_default())),
                    "before_commit" => Some(SnapshotLabel::BeforeCommit),
                    "after_commit" => Some(SnapshotLabel::AfterCommit),
                    "before_refactor" => Some(SnapshotLabel::BeforeRefactor),
                    "external_change" => Some(SnapshotLabel::ExternalChange),
                    "checkout" => Some(SnapshotLabel::Checkout),
                    "vcs_operation" => Some(SnapshotLabel::VcsOperation(v.unwrap_or_default())),
                    _ => None,
                },
                _ => None,
            };

            let parent_id = row.get::<_, Option<Vec<u8>>>(5)?.and_then(|bytes| {
                if bytes.len() == 32 {
                    let mut arr = [0u8; 32];
                    arr.copy_from_slice(&bytes);
                    Some(SnapshotId::from_bytes(arr))
                } else {
                    None
                }
            });

            let content_hash_vec: Vec<u8> = row.get(6)?;
            let mut content_hash = [0u8; 32];
            content_hash.copy_from_slice(&content_hash_vec);

            Ok(SnapshotMetadata {
                id,
                workspace_id,
                file_id: file_id.clone(),
                timestamp,
                label,
                parent_id,
                content_hash,
                size: row.get::<_, i64>(7)? as u64,
                is_compressed: row.get::<_, i32>(8)? != 0,
            })
        })?;

        let mut snapshots = Vec::new();
        for row in rows {
            snapshots.push(row?);
        }
        Ok(snapshots)
    }

    pub async fn prune_snapshots(&self, workspace_id: WorkspaceId) -> Result<()> {
        let cutoff_time = chrono::Utc::now()
            .checked_sub_signed(chrono::Duration::days(self.config.max_age_days as i64))
            .unwrap()
            .timestamp();

        let db = self.db.write();
        
        // Delete old snapshots without labels
        db.execute(
            r#"
            DELETE FROM snapshots 
            WHERE workspace_id = ?1 
              AND timestamp < ?2 
              AND label_type IS NULL
            "#,
            params![workspace_id.to_uuid().as_bytes(), cutoff_time],
        )?;

        Ok(())
    }

    fn get_blob_path(&self, snapshot_id: &SnapshotId) -> PathBuf {
        let hex = snapshot_id.to_hex();
        let (prefix, rest) = hex.split_at(2);
        self.blob_dir.join(prefix).join(rest)
    }
}
