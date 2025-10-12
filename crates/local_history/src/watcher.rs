use crate::identifiers::{FileId, WorkspaceId};
use crate::types::FileChange;
use anyhow::Result;
use notify::{Event, EventKind, RecursiveMode, Watcher as NotifyWatcher};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};

/// File watcher configuration
#[derive(Debug, Clone)]
pub struct WatchConfig {
    /// Debounce duration in milliseconds
    pub debounce_ms: u64,
    /// Patterns to exclude from watching
    pub exclude_patterns: Vec<String>,
}

impl Default for WatchConfig {
    fn default() -> Self {
        Self {
            debounce_ms: 500,
            exclude_patterns: vec![
                "node_modules".to_string(),
                "target".to_string(),
                ".git".to_string(),
                "*.log".to_string(),
                "*.tmp".to_string(),
            ],
        }
    }
}

/// File watcher for detecting changes
pub struct FileWatcher {
    #[allow(dead_code)]
    workspace_id: WorkspaceId,
    #[allow(dead_code)]
    watcher: Box<dyn NotifyWatcher>,
    receiver: Receiver<Result<Event, notify::Error>>,
    config: WatchConfig,
}

impl FileWatcher {
    pub fn new(workspace_id: WorkspaceId, paths: Vec<PathBuf>, config: WatchConfig) -> Result<Self> {
        let (tx, rx): (Sender<Result<Event, notify::Error>>, Receiver<Result<Event, notify::Error>>) = channel();

        let mut watcher = notify::recommended_watcher(move |res| {
            let _ = tx.send(res);
        })?;

        for path in paths {
            watcher.watch(&path, RecursiveMode::Recursive)?;
        }

        Ok(Self {
            workspace_id,
            watcher: Box::new(watcher),
            receiver: rx,
            config,
        })
    }

    pub fn should_ignore(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();
        for pattern in &self.config.exclude_patterns {
            if path_str.contains(pattern) {
                return true;
            }
        }
        false
    }

    pub fn poll_events(&self) -> Vec<FileChange> {
        let mut changes = Vec::new();
        
        while let Ok(Ok(event)) = self.receiver.try_recv() {
            for path in &event.paths {
                if self.should_ignore(path) {
                    continue;
                }

                let file_id = FileId::from_path(path);
                let timestamp = chrono::Utc::now();

                let change = match event.kind {
                    EventKind::Create(_) => FileChange::Created {
                        file_id,
                        path: path.clone(),
                        timestamp,
                    },
                    EventKind::Modify(_) => FileChange::Modified {
                        file_id,
                        path: path.clone(),
                        timestamp,
                    },
                    EventKind::Remove(_) => FileChange::Deleted {
                        file_id,
                        path: path.clone(),
                        timestamp,
                    },
                    _ => continue,
                };

                changes.push(change);
            }
        }

        changes
    }
}
