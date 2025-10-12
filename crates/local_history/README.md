# Local History System

A JetBrains-style local history system for Zed, providing automatic file snapshots, version tracking, and restoration capabilities.

## Overview

The local history system consists of two crates:

1. **`local_history`** - Core library with storage, diff, and revert functionality
2. **`local_history_ui`** - Integration layer for workspace and UI components

## Features

### Core Functionality (✅ Implemented)

- **Snapshot Storage**: SQLite-based metadata with blob storage for content
- **Compression**: zstd compression for efficient storage
- **Unique Identifiers**: UUID-based workspace IDs, content-addressable file and snapshot IDs
- **Diff Engine**: Myers' algorithm for line-based text diffing
- **Revert Operations**: Restore files to previous snapshots with safety backups
- **Label System**: Support for manual labels and special events (Before/After Commit, Refactor, etc.)
- **Retention Policies**: Time-based pruning with label protection

### API Examples

#### Creating Snapshots

```rust
use local_history::{LocalHistory, StorageConfig, WorkspaceId, FileId, SnapshotLabel};

// Initialize
let config = StorageConfig::default();
let history = LocalHistory::new(config)?;

// Create a snapshot
let workspace_id = WorkspaceId::new();
let file_id = FileId::from_path(Path::new("src/main.rs"));
let content = std::fs::read("src/main.rs")?;

let snapshot_id = history.create_snapshot(
    workspace_id,
    file_id,
    content,
    Some(SnapshotLabel::BeforeCommit),
).await?;
```

#### Viewing Diffs

```rust
use local_history::diff::{diff_lines, format_unified_diff};

let old_text = "line1\nline2\nline3";
let new_text = "line1\nline2_modified\nline3";

// Get structured changes
let changes = diff_lines(old_text, new_text);

// Or get a unified diff
let diff = format_unified_diff(old_text, new_text, "old", "new");
println!("{}", diff);
```

#### Reverting Files

```rust
use local_history::revert::{preview_revert, revert_to_snapshot};

// Preview what will change
let preview = preview_revert(&storage, snapshot_id, current_path).await?;
println!("Changes:\n{}", preview);

// Perform the revert (creates a safety snapshot first)
revert_to_snapshot(&storage, snapshot_id, current_path).await?;
```

### Workspace Integration

```rust
use local_history_ui::LocalHistoryManager;
use gpui::App;

// Initialize in your app
local_history_ui::init(cx);

// Access globally
if let Some(manager) = LocalHistoryManager::global(cx) {
    manager.update(cx, |manager, cx| {
        // Create snapshots
        cx.spawn(async move {
            manager.snapshot_before_commit(
                workspace_id,
                &path,
                content,
            ).await
        });
    });
}
```

## Architecture

### Data Model

#### Identifiers

- **`WorkspaceId`**: UUID v4 for workspace identification
- **`FileId`**: Hash of canonical path + inode + device ID
- **`SnapshotId`**: Blake3 hash of (file_id, timestamp, parent_id, content_hash)

#### Storage Schema

```sql
CREATE TABLE snapshots (
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
```

### Blob Storage

Blobs are stored in a content-addressable format:
```
.local_history/
├── history.db           # SQLite metadata
└── blobs/
    ├── ab/             # First 2 hex chars of snapshot ID
    │   └── cdef...     # Rest of snapshot ID
    └── ...
```

### Compression

- Uses zstd compression (level 3) for all blob storage
- Automatic compression on snapshot creation
- Transparent decompression on retrieval

## Configuration

```rust
use local_history::StorageConfig;

let config = StorageConfig {
    base_dir: PathBuf::from(".local_history"),
    max_total_size: 10 * 1024 * 1024 * 1024, // 10GB
    max_age_days: 30,
    max_snapshots_per_file: 200,
};
```

## Label Types

The system supports various snapshot labels:

- **Manual(String)**: User-created snapshots with custom labels
- **BeforeCommit**: Auto-created before git commits
- **AfterCommit**: Auto-created after git commits  
- **BeforeRefactor**: Before major refactoring operations
- **ExternalChange**: When external tools modify files
- **Checkout**: Git checkout operations
- **VcsOperation(String)**: Other VCS operations

Labeled snapshots are protected from automatic pruning.

## Retention & Pruning

The system implements time-based retention:

```rust
// Prune old snapshots (respects labels)
history.prune_snapshots(workspace_id).await?;
```

Pruning rules:
- Deletes snapshots older than `max_age_days`
- **Never** deletes labeled snapshots
- Processes per workspace to avoid cross-workspace issues

## Future Enhancements

### Phase 2: File Monitoring (Planned)

- [ ] Automatic snapshots on file save
- [ ] Debounced snapshot creation (500-1500ms)
- [ ] Snapshot grouping by time windows
- [ ] External change detection
- [ ] File watcher integration with workspace

### Phase 3: Advanced Management (Planned)

- [ ] LRU pruning with size limits
- [ ] Per-workspace exclusion patterns (node_modules, target, .git)
- [ ] Global exclusion rules
- [ ] Quota enforcement

### Phase 4: Enhanced Diff & Revert (Planned)

- [ ] Binary file handling
- [ ] Folder recursive diff
- [ ] Selective/hunk-based revert
- [ ] Intraline diff highlighting

### Phase 5: UI Components (Planned)

- [ ] History timeline panel
- [ ] Diff visualization
- [ ] Context menu integration
- [ ] Search and filtering

### Phase 6: CLI & Daemon (Optional)

- [ ] Background daemon (lhd) for monitoring
- [ ] CLI tool (lh) for command-line access
- [ ] REST API for external integrations

## Testing

Run tests with:

```bash
cargo test -p local_history
cargo test -p local_history_ui
```

Current test coverage includes:
- Diff algorithm (insert, delete, modify)
- Snapshot creation and retrieval
- Revert with safety snapshots
- Storage layer operations

## Performance Considerations

- **Deduplication**: Content-addressable storage prevents duplicate blobs
- **Compression**: zstd provides ~70% size reduction for text files
- **Indexing**: SQLite indexes on workspace_id + file_id_hash for fast queries
- **Lazy Loading**: Blob content loaded only when needed

## Dependencies

Core dependencies:
- `rusqlite` - SQLite database
- `zstd` - Compression
- `blake3` - Cryptographic hashing
- `notify` - File system watching
- `uuid` - Unique identifiers
- `chrono` - Timestamp handling

## License

GPL-3.0-or-later
