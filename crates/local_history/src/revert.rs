use crate::identifiers::SnapshotId;
use crate::storage::HistoryStorage;
use anyhow::{Context, Result};
use std::path::Path;

/// Revert a file to a specific snapshot
pub async fn revert_to_snapshot(
    storage: &HistoryStorage,
    snapshot_id: SnapshotId,
    target_path: &Path,
) -> Result<()> {
    // Get the snapshot content
    let content = storage.get_snapshot_content(snapshot_id).await
        .context("Failed to get snapshot content")?;

    // Create a safety snapshot before reverting
    let current_content = std::fs::read(target_path)
        .context("Failed to read current file")?;
    
    // Get metadata to retrieve workspace and file info
    let metadata = storage.get_snapshot(snapshot_id).await?
        .context("Snapshot not found")?;

    // Create safety snapshot
    storage.create_snapshot(
        metadata.workspace_id,
        metadata.file_id,
        current_content,
        Some(crate::snapshot::SnapshotLabel::Manual("Before Revert".to_string())),
    ).await.context("Failed to create safety snapshot")?;

    // Write the reverted content
    std::fs::write(target_path, content)
        .context("Failed to write reverted content")?;

    Ok(())
}

/// Preview changes that would be made by reverting to a snapshot
pub async fn preview_revert(
    storage: &HistoryStorage,
    snapshot_id: SnapshotId,
    current_path: &Path,
) -> Result<String> {
    let snapshot_content = storage.get_snapshot_content(snapshot_id).await?;
    let current_content = std::fs::read_to_string(current_path)?;
    
    let snapshot_text = String::from_utf8_lossy(&snapshot_content);
    let diff = crate::diff::format_unified_diff(
        &current_content,
        &snapshot_text,
        "current",
        "snapshot",
    );

    Ok(diff)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identifiers::{FileId, WorkspaceId};
    use crate::storage::StorageConfig;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_revert_preview() {
        let dir = tempdir().unwrap();
        let config = StorageConfig {
            base_dir: dir.path().to_path_buf(),
            ..Default::default()
        };
        
        let storage = HistoryStorage::new(config).unwrap();
        let workspace_id = WorkspaceId::new();
        
        let test_file = dir.path().join("test.txt");
        std::fs::write(&test_file, "original content").unwrap();
        
        let file_id = FileId::from_path(&test_file);
        let content = b"original content".to_vec();
        
        let snapshot_id = storage.create_snapshot(workspace_id, file_id, content, None)
            .await
            .unwrap();

        // Modify the file
        std::fs::write(&test_file, "modified content").unwrap();

        // Preview revert
        let preview = preview_revert(&storage, snapshot_id, &test_file).await;
        assert!(preview.is_ok());
    }
}
