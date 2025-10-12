use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::Path;
use uuid::Uuid;

/// Unique identifier for a workspace
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WorkspaceId(Uuid);

impl WorkspaceId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    pub fn to_uuid(&self) -> Uuid {
        self.0
    }
}

impl Default for WorkspaceId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for WorkspaceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Unique identifier for a file
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FileId {
    /// Canonical path hash (case-sensitive according to OS)
    pub path_hash: u64,
    /// Inode if available
    pub inode: Option<u64>,
    /// Device ID if available
    pub device_id: Option<u64>,
}

impl FileId {
    pub fn from_path(path: &Path) -> Self {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        path.to_string_lossy().hash(&mut hasher);
        let path_hash = hasher.finish();

        #[cfg(unix)]
        let (inode, device_id) = {
            use std::os::unix::fs::MetadataExt;
            if let Ok(metadata) = std::fs::metadata(path) {
                (Some(metadata.ino()), Some(metadata.dev()))
            } else {
                (None, None)
            }
        };

        #[cfg(not(unix))]
        let (inode, device_id) = (None, None);

        Self {
            path_hash,
            inode,
            device_id,
        }
    }

    pub fn path_hash(&self) -> u64 {
        self.path_hash
    }
}

/// Unique identifier for a snapshot
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SnapshotId {
    hash: [u8; 32], // blake3 hash
}

impl SnapshotId {
    pub fn new(file_id: &FileId, timestamp: i64, parent_id: Option<&SnapshotId>, content_hash: &[u8; 32]) -> Self {
        let mut hasher = blake3::Hasher::new();
        
        hasher.update(&file_id.path_hash.to_le_bytes());
        if let Some(inode) = file_id.inode {
            hasher.update(&inode.to_le_bytes());
        }
        if let Some(device) = file_id.device_id {
            hasher.update(&device.to_le_bytes());
        }
        hasher.update(&timestamp.to_le_bytes());
        if let Some(parent) = parent_id {
            hasher.update(&parent.hash);
        }
        hasher.update(content_hash);

        let hash = *hasher.finalize().as_bytes();
        Self { hash }
    }

    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self { hash: bytes }
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.hash
    }

    pub fn to_hex(&self) -> String {
        hex::encode(self.hash)
    }
}

impl fmt::Display for SnapshotId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}
