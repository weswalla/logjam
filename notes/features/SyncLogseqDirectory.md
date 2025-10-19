# Sync Logseq Directory Feature

## Overview

Lightweight sync system that reacts to file changes and updates the search index. Builds on file watching with minimal complexity.

## Core Components

### Main Service
- `SyncService`: Handles file change events and updates index
  - Receives file events from file watcher
  - Determines sync operation needed (create/update/delete)
  - Re-parses changed files and updates existing repositories
  - Simple debouncing to avoid excessive processing

### Simple Event Handling
```rust
enum SyncOperation {
    Create(PathBuf),
    Update(PathBuf),
    Delete(PathBuf),
}

impl SyncService {
    async fn handle_file_event(&self, event: FileEvent) -> Result<(), SyncError> {
        let operation = match event.kind {
            EventKind::Create => SyncOperation::Create(event.path),
            EventKind::Modify => SyncOperation::Update(event.path),
            EventKind::Remove => SyncOperation::Delete(event.path),
        };
        
        self.process_sync_operation(operation).await
    }
}
```

### File Change Detection
- Use file modification time for simple change detection
- Optional: Store file hashes in database for content-based detection
- File system wins conflicts (always trust the file system)

## Implementation Approach

### Debouncing Strategy
- Use `tokio::time::sleep` with a simple HashMap to group rapid changes
- 500ms debounce window (configurable)
- Process batches of changes together

### Error Handling
- Log errors but continue processing other files
- Simple retry for transient errors (file locked, etc.)
- No complex conflict resolution - file system is source of truth

### Integration with File Watcher
- Direct callback from file watcher to sync service
- Filter events to only .md files in pages/ and journals/
- Simple async channel for event processing

## Key Simplifications

**Removed:**
- Complex sync sessions and state management
- Sophisticated conflict resolution
- CQRS, Saga, Circuit Breaker patterns
- Separate sync repositories
- Priority queues and complex scheduling
- Detailed metrics and monitoring

**Kept:**
- Debouncing to handle rapid file changes
- Async processing to avoid blocking
- Basic error handling and logging
- Integration with existing domain objects and repositories

## Testing Strategy

### Unit Tests
- Test sync operation logic
- Test debouncing behavior
- Mock file system events

### Integration Tests
- Test with real file changes
- Test error scenarios

This approach provides reliable sync functionality with minimal overhead.
