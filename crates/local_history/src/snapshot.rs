use crate::identifiers::{FileId, SnapshotId, WorkspaceId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Label types for snapshots
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SnapshotLabel {
    /// Manual label set by user
    Manual(String),
    /// Before commit
    BeforeCommit,
    /// After commit
    AfterCommit,
    /// Before refactor
    BeforeRefactor,
    /// External change detected
    ExternalChange,
    /// VCS checkout
    Checkout,
    /// Other VCS operation
    VcsOperation(String),
}

/// Snapshot metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotMetadata {
    pub id: SnapshotId,
    pub workspace_id: WorkspaceId,
    pub file_id: FileId,
    pub timestamp: DateTime<Utc>,
    pub label: Option<SnapshotLabel>,
    pub parent_id: Option<SnapshotId>,
    pub content_hash: [u8; 32],
    pub size: u64,
    pub is_compressed: bool,
}

/// A snapshot of file content at a point in time
#[derive(Debug, Clone)]
pub struct Snapshot {
    pub metadata: SnapshotMetadata,
    pub content: Vec<u8>,
}

impl Snapshot {
    pub fn new(
        workspace_id: WorkspaceId,
        file_id: FileId,
        content: Vec<u8>,
        label: Option<SnapshotLabel>,
        parent_id: Option<SnapshotId>,
    ) -> Self {
        let timestamp = Utc::now();
        let content_hash = *blake3::hash(&content).as_bytes();
        let size = content.len() as u64;
        
        let id = SnapshotId::new(&file_id, timestamp.timestamp(), parent_id.as_ref(), &content_hash);

        let metadata = SnapshotMetadata {
            id,
            workspace_id,
            file_id,
            timestamp,
            label,
            parent_id,
            content_hash,
            size,
            is_compressed: false,
        };

        Self { metadata, content }
    }

    pub fn compress(&mut self) -> anyhow::Result<()> {
        if !self.metadata.is_compressed {
            self.content = zstd::encode_all(&self.content[..], 3)?;
            self.metadata.is_compressed = true;
        }
        Ok(())
    }

    pub fn decompress(&mut self) -> anyhow::Result<()> {
        if self.metadata.is_compressed {
            self.content = zstd::decode_all(&self.content[..])?;
            self.metadata.is_compressed = false;
        }
        Ok(())
    }
}
