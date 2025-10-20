# Workflow 2: Continuous Sync (File Watching)

**User Action:** Click "Start Sync" → App watches for file changes

## Flow Diagram

```
┌──────────────────────────────────────────────────────────────────┐
│                        FILE SYSTEM                                │
│  User edits /pages/my-note.md in Logseq                          │
│  File saved → OS emits file change event                         │
└───────────────────────────┬──────────────────────────────────────┘
                            │ inotify/FSEvents
                            ↓
┌──────────────────────────────────────────────────────────────────┐
│                  INFRASTRUCTURE - WATCHER                         │
│  LogseqFileWatcher (using notify crate)                          │
│    • Receives raw file event                                     │
│    • Debounces (500ms window)                                    │
│    • Filters for .md files in pages/journals/                    │
│    • Converts to FileEvent { path, kind }                        │
└───────────────────────────┬──────────────────────────────────────┘
                            │ FileEvent::Modified(path)
                            ↓
┌──────────────────────────────────────────────────────────────────┐
│                   APPLICATION - SYNC SERVICE                      │
│  SyncService::handle_event()                                     │
│    ┌──────────────────────────────────────────────────┐          │
│    │ Match event.kind:                                │          │
│    │   Created  → handle_file_created(path)           │          │
│    │   Modified → handle_file_updated(path)           │          │
│    │   Deleted  → handle_file_deleted(path)           │          │
│    └──────────────────────────────────────────────────┘          │
└───────────────────────────┬──────────────────────────────────────┘
                            │
                            ↓ (example: Modified event)
┌──────────────────────────────────────────────────────────────────┐
│  handle_file_updated(path):                                      │
│    1. Check FileMappingRepository for existing mapping           │
│    2. If stale (file modified > last sync):                      │
│       ├─ Parse file → Page                                       │
│       ├─ PageRepository.save(page)        [UPDATE]               │
│       ├─ FileMappingRepository.save(...)  [UPDATE timestamp]     │
│       ├─ SearchIndex.update_page(page)    [REINDEX]              │
│       └─ Emit SyncEvent::FileUpdated                             │
└──────────────────────────────────────────────────────────────────┘
```

## Code Example

```rust
// APPLICATION LAYER - SyncService

impl SyncService {
    pub async fn start_watching(&self, callback: Option<SyncCallback>) -> SyncResult<()> {
        loop {
            // Block until next event
            let event = self.watcher.recv().await?;

            match event.kind {
                FileEventKind::Created => self.handle_file_created(event.path).await?,
                FileEventKind::Modified => self.handle_file_updated(event.path).await?,
                FileEventKind::Deleted => self.handle_file_deleted(event.path).await?,
            }

            // Notify frontend
            if let Some(ref cb) = callback {
                cb(SyncEvent::FileUpdated(event.path.clone()));
            }
        }
    }

    async fn handle_file_updated(&self, path: PathBuf) -> SyncResult<()> {
        // 1. Get existing mapping
        let mapping_repo = self.mapping_repository.lock().await;
        let existing = mapping_repo.find_by_path(&path)?;

        // 2. Check if file actually changed
        let metadata = tokio::fs::metadata(&path).await?;
        let current_modified = metadata.modified()?;

        if let Some(mapping) = existing {
            if !mapping.is_stale(current_modified) {
                return Ok(()); // No changes, skip
            }
        }

        // 3. Re-parse file
        let page = LogseqMarkdownParser::parse_file(&path).await?;

        // 4. Update repository
        let mut page_repo = self.page_repository.lock().await;
        page_repo.save(page.clone())?;

        // 5. Update file mapping
        let mut mapping_repo = self.mapping_repository.lock().await;
        mapping_repo.save(FilePathMapping::new(path, page.id().clone(), ...))?;

        // 6. Update search index
        if let Some(ref index) = self.search_index {
            index.lock().await.update_page(&page)?;
            index.lock().await.commit()?;
        }

        Ok(())
    }

    async fn handle_file_deleted(&self, path: PathBuf) -> SyncResult<()> {
        // 1. Find mapping to get PageId
        let mut mapping_repo = self.mapping_repository.lock().await;
        let mapping = mapping_repo.find_by_path(&path)?
            .ok_or_else(|| SyncError::NotFound("No mapping for deleted file".into()))?;

        let page_id = mapping.page_id().clone();

        // 2. Delete from repository
        let mut page_repo = self.page_repository.lock().await;
        page_repo.delete(&page_id)?;

        // 3. Delete mapping (CASCADE in DB)
        mapping_repo.delete_by_path(&path)?;

        // 4. Delete from search index
        if let Some(ref index) = self.search_index {
            index.lock().await.delete_page(&page_id)?;
            index.lock().await.commit()?;
        }

        Ok(())
    }
}
```

## Key Insight - File→Page Mapping

Without file mappings, we can't handle deletions:

```
❌ PROBLEM:
File deleted: /pages/my-note.md
Which Page to delete? We don't know the PageId!

✅ SOLUTION (with FileMappingRepository):
1. Query: SELECT page_id FROM file_page_mappings WHERE file_path = '/pages/my-note.md'
2. Result: page_id = "my-note"
3. Delete: PageRepository.delete("my-note")
```

## Event Types

### File System Events
- **Created:** New `.md` file added to `pages/` or `journals/`
- **Modified:** Existing file content changed
- **Deleted:** File removed from file system
- **Renamed:** File moved or renamed (treated as delete + create)

### Sync Events (Emitted to Frontend)
- **SyncStarted:** File watching began
- **FileCreated:** New page imported
- **FileUpdated:** Existing page updated
- **FileDeleted:** Page removed
- **SyncError:** Error processing file change

## Debouncing

File watching uses debouncing to handle rapid file changes:

```rust
// Multiple rapid saves within 500ms window:
// Save 1: 10:00:00.100
// Save 2: 10:00:00.200  ← Ignored (within debounce window)
// Save 3: 10:00:00.300  ← Ignored (within debounce window)
// Process: 10:00:00.800  ← Only final state processed
```

This prevents:
- Processing incomplete file writes
- Overwhelming the system with rapid changes
- Duplicate work from text editor auto-saves

## Staleness Detection

The sync service only processes files that have actually changed:

```rust
pub struct FilePathMapping {
    file_path: PathBuf,
    page_id: PageId,
    file_modified_at: SystemTime,
    file_size_bytes: u64,
    checksum: Option<String>,
}

impl FilePathMapping {
    pub fn is_stale(&self, current_modified: SystemTime) -> bool {
        current_modified > self.file_modified_at
    }
}
```

This prevents unnecessary work when:
- File system events fire but content hasn't changed
- Multiple events are generated for the same change
- File metadata changes but content is identical

## Error Recovery

Sync service handles various error conditions gracefully:

### Temporary File System Issues
- **File locked:** Retry after delay
- **Permission denied:** Log error, continue watching
- **File disappeared:** Treat as deletion

### Parse Errors
- **Invalid markdown:** Log error, preserve old version
- **Encoding issues:** Try different encodings
- **Corrupted file:** Restore from backup if available

### Database Errors
- **Constraint violation:** Log error, skip update
- **Disk full:** Pause sync, notify user
- **Connection lost:** Reconnect and retry

## Performance Considerations

### Bounded Concurrency
- Process file changes sequentially to avoid conflicts
- Use async I/O to avoid blocking the watcher thread
- Batch multiple changes when possible

### Index Updates
- **Tantivy:** Batch updates and commit periodically
- **Qdrant:** Update embeddings asynchronously
- **Database:** Use transactions for consistency

### Memory Usage
- Don't load entire files into memory unnecessarily
- Stream large files during parsing
- Clean up temporary data promptly

## Integration with Import

Sync service can be used for both:
1. **Continuous watching:** Long-running file monitoring
2. **One-time sync:** Check for changes since last import

```rust
impl SyncService {
    // Continuous watching (runs until stopped)
    pub async fn start_watching(&self) -> SyncResult<()> { ... }
    
    // One-time sync (returns when complete)
    pub async fn sync_once(&self) -> SyncResult<SyncSummary> { ... }
}
```

This allows the import service to use sync logic for incremental updates.
