# Local History UI

Integration layer for the local history system with Zed's workspace and editor.

## Usage

### Initialization

Add to your workspace initialization:

```rust
use local_history_ui;

// In workspace initialization
local_history_ui::init(cx);
```

### Manual Snapshots

```rust
use local_history_ui::LocalHistoryManager;

if let Some(manager) = LocalHistoryManager::global(cx) {
    manager.update(cx, |manager, cx| {
        cx.spawn(async move {
            let workspace_id = /* get from workspace */;
            let path = Path::new("src/main.rs");
            let content = std::fs::read(path)?;
            
            manager.create_manual_snapshot(
                workspace_id,
                path,
                content,
                "Before major refactor".to_string(),
            ).await?;
            
            Ok::<_, anyhow::Error>(())
        }).detach();
    });
}
```

### VCS Integration

```rust
// Before commit
manager.snapshot_before_commit(workspace_id, &path, content).await?;

// After commit
manager.snapshot_after_commit(workspace_id, &path, content).await?;
```

### Listing Snapshots

```rust
let snapshots = manager.list_file_snapshots(&path).await?;

for snapshot in snapshots {
    println!(
        "{}: {} - {:?}",
        snapshot.timestamp,
        snapshot.id,
        snapshot.label
    );
}
```

### Retrieving Content

```rust
let content = manager.get_snapshot_content(snapshot_id).await?;
let text = String::from_utf8_lossy(&content);
println!("{}", text);
```

## Integration Points

### File Save Events

To automatically create snapshots on file save, integrate with the buffer save event:

```rust
// In your workspace or editor plugin
cx.subscribe(&buffer, move |_, buffer, event, cx| {
    if let BufferEvent::Saved = event {
        if let Some(file) = buffer.file() {
            if let Some(manager) = LocalHistoryManager::global(cx) {
                manager.update(cx, |manager, cx| {
                    let path = file.path().clone();
                    let content = buffer.as_rope().to_string().into_bytes();
                    let workspace_id = /* ... */;
                    
                    cx.spawn(async move {
                        manager.snapshot_file(
                            workspace_id,
                            &path,
                            content,
                            None,
                        ).await
                    }).detach();
                });
            }
        }
    }
}).detach();
```

### External Change Detection

```rust
// When detecting external file changes
if let Some(manager) = LocalHistoryManager::global(cx) {
    manager.update(cx, |manager, cx| {
        cx.spawn(async move {
            manager.snapshot_file(
                workspace_id,
                &path,
                content,
                Some(SnapshotLabel::ExternalChange),
            ).await
        }).detach();
    });
}
```

## Future UI Components

Planned components:

- **History Panel**: Timeline view of snapshots
- **Diff Viewer**: Side-by-side comparison
- **Revert Dialog**: Preview and confirm revert operations
- **Context Menus**: Quick access to history actions

## License

GPL-3.0-or-later
