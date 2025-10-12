use crate::identifiers::{FileId, WorkspaceId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Represents a file change event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FileChange {
    Created {
        file_id: FileId,
        path: std::path::PathBuf,
        timestamp: DateTime<Utc>,
    },
    Modified {
        file_id: FileId,
        path: std::path::PathBuf,
        timestamp: DateTime<Utc>,
    },
    Deleted {
        file_id: FileId,
        path: std::path::PathBuf,
        timestamp: DateTime<Utc>,
    },
    Renamed {
        old_file_id: FileId,
        new_file_id: FileId,
        old_path: std::path::PathBuf,
        new_path: std::path::PathBuf,
        timestamp: DateTime<Utc>,
    },
}

/// Group of snapshots that occurred within a time window
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotGroup {
    pub workspace_id: WorkspaceId,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub snapshot_ids: Vec<crate::identifiers::SnapshotId>,
}

impl SnapshotGroup {
    pub fn new(workspace_id: WorkspaceId) -> Self {
        let now = Utc::now();
        Self {
            workspace_id,
            start_time: now,
            end_time: now,
            snapshot_ids: Vec::new(),
        }
    }

    pub fn add_snapshot(&mut self, snapshot_id: crate::identifiers::SnapshotId) {
        self.snapshot_ids.push(snapshot_id);
        self.end_time = Utc::now();
    }

    pub fn duration_secs(&self) -> i64 {
        (self.end_time - self.start_time).num_seconds()
    }
}
