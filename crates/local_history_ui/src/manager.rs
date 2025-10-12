use anyhow::Result;
use gpui::{App, Context, Entity, Global};
use local_history::{FileId, LocalHistory, SnapshotLabel, StorageConfig, WorkspaceId};
use std::path::Path;

pub fn init(cx: &mut App) {
    let config = StorageConfig::default();
    if let Ok(history) = LocalHistory::new(config) {
        let manager = cx.new(|_| LocalHistoryManager::new(history));
        cx.set_global(GlobalLocalHistoryManager(manager));
    }
}

struct GlobalLocalHistoryManager(Entity<LocalHistoryManager>);

impl Global for GlobalLocalHistoryManager {}

pub struct LocalHistoryManager {
    history: LocalHistory,
}

impl LocalHistoryManager {
    pub fn new(history: LocalHistory) -> Self {
        Self { history }
    }

    pub fn global(cx: &App) -> Option<Entity<Self>> {
        cx.try_global::<GlobalLocalHistoryManager>()
            .map(|model| model.0.clone())
    }

    /// Create a snapshot for a file
    pub async fn snapshot_file(
        &self,
        workspace_id: WorkspaceId,
        path: &Path,
        content: Vec<u8>,
        label: Option<SnapshotLabel>,
    ) -> Result<()> {
        let file_id = FileId::from_path(path);
        self.history
            .create_snapshot(workspace_id, file_id, content, label)
            .await?;
        Ok(())
    }

    /// Create a manual snapshot with a custom label
    pub async fn create_manual_snapshot(
        &self,
        workspace_id: WorkspaceId,
        path: &Path,
        content: Vec<u8>,
        label: String,
    ) -> Result<()> {
        self.snapshot_file(
            workspace_id,
            path,
            content,
            Some(SnapshotLabel::Manual(label)),
        )
        .await
    }

    /// Create a "Before Commit" snapshot
    pub async fn snapshot_before_commit(
        &self,
        workspace_id: WorkspaceId,
        path: &Path,
        content: Vec<u8>,
    ) -> Result<()> {
        self.snapshot_file(workspace_id, path, content, Some(SnapshotLabel::BeforeCommit))
            .await
    }

    /// Create an "After Commit" snapshot
    pub async fn snapshot_after_commit(
        &self,
        workspace_id: WorkspaceId,
        path: &Path,
        content: Vec<u8>,
    ) -> Result<()> {
        self.snapshot_file(workspace_id, path, content, Some(SnapshotLabel::AfterCommit))
            .await
    }

    /// List snapshots for a file
    pub async fn list_file_snapshots(&self, path: &Path) -> Result<Vec<local_history::SnapshotMetadata>> {
        let file_id = FileId::from_path(path);
        self.history.list_snapshots(file_id).await
    }

    /// Get content from a snapshot
    pub async fn get_snapshot_content(
        &self,
        snapshot_id: local_history::SnapshotId,
    ) -> Result<Vec<u8>> {
        self.history.get_snapshot_content(snapshot_id).await
    }
}
