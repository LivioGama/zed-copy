mod diff;
mod identifiers;
mod revert;
mod snapshot;
mod storage;
mod types;
mod watcher;

pub use diff::{DiffChange, diff_lines, format_unified_diff};
pub use identifiers::{FileId, SnapshotId, WorkspaceId};
pub use revert::{preview_revert, revert_to_snapshot};
pub use snapshot::{Snapshot, SnapshotLabel, SnapshotMetadata};
pub use storage::{HistoryStorage, StorageConfig};
pub use types::{FileChange, SnapshotGroup};
pub use watcher::{FileWatcher, WatchConfig};

use anyhow::Result;

/// Local History Manager - Main entry point for the local history system
pub struct LocalHistory {
    storage: HistoryStorage,
    watcher: Option<FileWatcher>,
}

impl LocalHistory {
    /// Create a new LocalHistory instance
    pub fn new(config: StorageConfig) -> Result<Self> {
        let storage = HistoryStorage::new(config)?;
        Ok(Self {
            storage,
            watcher: None,
        })
    }

    /// Start watching files in the given workspace
    pub async fn start_watching(&mut self, workspace_id: WorkspaceId, paths: Vec<std::path::PathBuf>, config: WatchConfig) -> Result<()> {
        let watcher = FileWatcher::new(workspace_id, paths, config)?;
        self.watcher = Some(watcher);
        Ok(())
    }

    /// Stop watching files
    pub fn stop_watching(&mut self) {
        self.watcher = None;
    }

    /// Create a manual snapshot
    pub async fn create_snapshot(
        &self,
        workspace_id: WorkspaceId,
        file_id: FileId,
        content: Vec<u8>,
        label: Option<SnapshotLabel>,
    ) -> Result<SnapshotId> {
        self.storage.create_snapshot(workspace_id, file_id, content, label).await
    }

    /// Get snapshot metadata
    pub async fn get_snapshot(&self, snapshot_id: SnapshotId) -> Result<Option<SnapshotMetadata>> {
        self.storage.get_snapshot(snapshot_id).await
    }

    /// Get file content from snapshot
    pub async fn get_snapshot_content(&self, snapshot_id: SnapshotId) -> Result<Vec<u8>> {
        self.storage.get_snapshot_content(snapshot_id).await
    }

    /// List snapshots for a file
    pub async fn list_snapshots(&self, file_id: FileId) -> Result<Vec<SnapshotMetadata>> {
        self.storage.list_snapshots(file_id).await
    }

    /// Delete old snapshots based on retention policy
    pub async fn prune_snapshots(&self, workspace_id: WorkspaceId) -> Result<()> {
        self.storage.prune_snapshots(workspace_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_local_history_creation() {
        let config = StorageConfig::default();
        let history = LocalHistory::new(config);
        assert!(history.is_ok());
    }
}
