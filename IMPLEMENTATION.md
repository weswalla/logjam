# Logseq Directory Import & File Sync Implementation

This document describes the implementation of the Logseq directory import and file synchronization features using a simplified DDD architecture.

## Overview

This implementation provides the core file processing system for importing and syncing Logseq markdown directories. It follows Domain-Driven Design principles while maintaining pragmatism suitable for a personal project.

## Architecture

The implementation follows a three-layer architecture:

```
┌─────────────────────────────────────────────────────┐
│                  Application Layer                   │
│  ┌────────────────────┐  ┌────────────────────────┐ │
│  │  ImportService     │  │   SyncService          │ │
│  │  - Concurrent      │  │   - File watching      │ │
│  │    processing      │  │   - Debouncing         │ │
│  │  - Progress        │  │   - Auto-sync          │ │
│  │    tracking        │  │                        │ │
│  └────────────────────┘  └────────────────────────┘ │
└─────────────────────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────┐
│                    Domain Layer                      │
│  ┌──────────────┐  ┌──────────────┐  ┌────────────┐│
│  │   Page       │  │    Block     │  │   Events   ││
│  │  (Aggregate) │  │   (Entity)   │  │            ││
│  └──────────────┘  └──────────────┘  └────────────┘│
│  ┌─────────────────────────────────────────────────┤
│  │   Value Objects:                                 │
│  │   - PageId, BlockId, Url, PageReference         │
│  │   - LogseqDirectoryPath, ImportProgress         │
│  └──────────────────────────────────────────────────┘
└─────────────────────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────┐
│                Infrastructure Layer                  │
│  ┌────────────────────┐  ┌────────────────────────┐ │
│  │  File System       │  │   Parsers              │ │
│  │  - Discovery       │  │   - Markdown parser    │ │
│  │  - Watcher         │  │   - URL extraction     │ │
│  │  - Debouncer       │  │   - Reference extract  │ │
│  └────────────────────┘  └────────────────────────┘ │
└─────────────────────────────────────────────────────┘
```

## Components

### Domain Layer

#### Value Objects (`backend/src/domain/value_objects.rs`)

**New additions:**

- **`LogseqDirectoryPath`**: Validated directory path containing `pages/` and `journals/` subdirectories
  - Validates directory exists and has required structure
  - Provides convenient accessors for subdirectories

- **`ImportProgress`**: Tracks import operation progress
  - Total files, processed files, current file
  - Percentage calculation for UI display

**Existing (reused):**
- `PageId`, `BlockId`, `Url`, `PageReference`, `BlockContent`, `IndentLevel`

#### Domain Events (`backend/src/domain/events.rs`)

**Import Events:**
- `ImportStarted` - Import operation begins
- `FileProcessed` - Individual file processed
- `ImportCompleted` - Import finished successfully
- `ImportFailed` - Import failed with errors

**Sync Events:**
- `SyncStarted` - Sync operation begins
- `FileCreatedEvent` - New file detected and synced
- `FileUpdatedEvent` - File modified and synced
- `FileDeletedEvent` - File deleted and synced
- `SyncCompleted` - Sync batch completed

### Infrastructure Layer

#### Logseq Markdown Parser (`backend/src/infrastructure/parsers/logseq_markdown.rs`)

Converts Logseq markdown files into `Page` and `Block` domain objects.

**Features:**
- Async file reading with Tokio
- Indentation-based hierarchy parsing (tabs or 2-space indents)
- Bullet point marker removal (`-`, `*`, `+`)
- URL extraction (http:// and https://)
- Page reference extraction (`[[page]]`)
- Tag extraction (`#tag`)
- Proper parent-child block relationships

**Example:**
```markdown
- Root block with https://example.com
  - Child block mentioning [[another page]]
  - Another child with #tag
- Second root block
```

Becomes:
```
Page
├─ Block 1 (indent 0) + URL
│  ├─ Block 1.1 (indent 1) + PageReference
│  └─ Block 1.2 (indent 1) + Tag
└─ Block 2 (indent 0)
```

#### File Discovery (`backend/src/infrastructure/file_system/discovery.rs`)

**Functions:**
- `discover_markdown_files(dir)` - Recursively find all `.md` files
- `discover_logseq_files(dir)` - Find `.md` files in `pages/` and `journals/`

**Features:**
- Skips hidden directories (starting with `.`)
- Skips Logseq internal directory (`logseq/`)
- Async with Tokio

#### File Watcher (`backend/src/infrastructure/file_system/watcher.rs`)

**`LogseqFileWatcher`** - Watches directory for file changes using `notify` crate

**Features:**
- Cross-platform file watching (`RecommendedWatcher`)
- Built-in debouncing (500ms default) using `notify-debouncer-mini`
- Filters to only `.md` files in `pages/` or `journals/`
- Event types: Created, Modified, Deleted
- Non-blocking (`try_recv`) and blocking (`recv`) modes

### Application Layer

#### ImportService (`backend/src/application/services/import_service.rs`)

Handles importing entire Logseq directories.

**Features:**
- **Bounded concurrency**: Processes 4 files concurrently (configurable with `with_concurrency()`)
- **Progress tracking**: Real-time progress updates via callbacks
- **Error resilience**: Continues processing if individual files fail
- **Progress events**: `Started`, `FileProcessed`, `Completed`, `Failed`

**Usage:**
```rust
let mut service = ImportService::new(repository)
    .with_concurrency(6);

let summary = service.import_directory(
    directory_path,
    Some(progress_callback)
).await?;

println!("Imported {}/{} files",
    summary.pages_imported,
    summary.total_files
);
```

**Implementation Details:**
- Uses `tokio::sync::Semaphore` for bounded concurrency
- Async channel (`mpsc`) for collecting results
- Tracks errors without stopping import
- Returns `ImportSummary` with statistics

#### SyncService (`backend/src/application/services/sync_service.rs`)

Handles incremental updates when files change.

**Features:**
- **File watching**: Monitors directory for changes
- **Debouncing**: 500ms window to handle rapid changes (configurable)
- **Auto-sync**: Runs indefinitely watching for changes
- **Event callbacks**: Real-time sync event notifications

**Usage:**
```rust
let service = SyncService::new(
    repository,
    directory_path,
    Some(Duration::from_millis(500))
)?;

service.start_watching(Some(sync_callback)).await?;
```

**Sync Operations:**
- **Create**: Parse new file and save to repository
- **Update**: Re-parse modified file and update repository
- **Delete**: Log deletion (full implementation needs file→page mapping)

**Note**: File deletion handling is simplified. A production implementation would maintain a bidirectional mapping between file paths and page IDs.

## Testing Strategy

### Unit Tests

All components include unit tests:

1. **Value Objects** (`value_objects.rs`)
   - `LogseqDirectoryPath` validation
   - `ImportProgress` tracking and percentage calculation

2. **Domain Events** (`events.rs`)
   - Event type and aggregate ID verification
   - All import and sync events tested

3. **Markdown Parser** (`logseq_markdown.rs`)
   - Indentation calculation
   - Content extraction (bullet point removal)
   - URL extraction
   - Page reference and tag extraction
   - Full markdown parsing with hierarchy

4. **File Discovery** (`discovery.rs`)
   - Recursive file discovery
   - Logseq-specific directory filtering
   - Uses `tempfile` for isolated tests

5. **File Watcher** (`watcher.rs`)
   - Event filtering (markdown files only)
   - Logseq directory filtering

6. **Import Service** (`import_service.rs`)
   - Import summary statistics
   - Success rate calculation
   - Mock repository for isolated testing

### Integration Tests

Create integration tests in `backend/tests/`:

```rust
#[tokio::test]
async fn test_full_import_workflow() {
    // 1. Create temporary Logseq directory
    // 2. Add sample markdown files
    // 3. Run ImportService
    // 4. Verify all pages imported
    // 5. Verify block hierarchy preserved
}

#[tokio::test]
async fn test_file_sync_workflow() {
    // 1. Import initial files
    // 2. Modify a file
    // 3. Verify SyncService detects change
    // 4. Verify repository updated
}
```

## Dependencies Added

### Production Dependencies
```toml
notify = "6.1"                      # Cross-platform file watching
notify-debouncer-mini = "0.4"       # Event debouncing
tokio = { version = "1.41", features = ["fs", "rt-multi-thread", "macros", "sync", "time"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
thiserror = "2.0"                   # Error handling
anyhow = "1.0"
tracing = "0.1"                     # Structured logging
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
uuid = { version = "1.11", features = ["v4", "serde"] }
```

### Dev Dependencies
```toml
tempfile = "3.14"                   # Temporary directories for tests
```

## Simplified Design Decisions

Following the "pragmatic DDD for personal projects" philosophy:

1. **No Complex Event Sourcing**: Events are for notifications, not persistence
2. **Direct Callbacks**: No event bus/CQRS complexity
3. **Simple Error Handling**: Continue on error, collect failures
4. **File System as Source of Truth**: No conflict resolution needed
5. **In-Memory Progress**: No import session persistence
6. **Simplified Deletion**: Log only (full implementation deferred)

## Future Enhancements

### Short Term
1. **SQLite Persistence**: Implement `PageRepository` with SQLite
2. **File→Page Mapping**: Enable proper deletion handling
3. **Error Retry**: Simple retry for transient errors (file locks)
4. **Metrics**: Track import/sync performance

### Medium Term
1. **Full-Text Search**: Integrate Tantivy for BM25 search
2. **Tauri Integration**: Add commands and event emitters
3. **UI Progress**: Real-time import/sync status display
4. **Configuration**: Debounce duration, concurrency limits

### Long Term
1. **Semantic Search**: fastembed-rs + Qdrant integration
2. **URL Metadata**: Parse and index linked content
3. **Advanced Conflict Resolution**: Handle simultaneous edits
4. **Performance Optimization**: Incremental parsing, caching

## Running the Code

### Build
```bash
cargo build
```

### Run Tests
```bash
cargo test
```

### Run Unit Tests Only
```bash
cargo test --lib
```

### Run Integration Tests
```bash
cargo test --test integration_test
```

### Run with Logging
```bash
RUST_LOG=debug cargo test
```

## Example Usage

```rust
use backend::application::{ImportService, SyncService};
use backend::domain::value_objects::LogseqDirectoryPath;
use std::sync::Arc;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize repository (mock for now, SQLite later)
    let repository = MockPageRepository::new();

    // Validate Logseq directory
    let dir_path = LogseqDirectoryPath::new("/path/to/logseq")?;

    // Import the directory
    let mut import_service = ImportService::new(repository.clone())
        .with_concurrency(6);

    let progress_callback = Arc::new(|event| {
        match event {
            ImportProgressEvent::Started { total_files } => {
                println!("Starting import of {} files", total_files);
            }
            ImportProgressEvent::FileProcessed { file_path, progress } => {
                println!("Processed {} ({:.1}%)",
                    file_path.display(),
                    progress.percentage()
                );
            }
            ImportProgressEvent::Completed { pages_imported, duration_ms } => {
                println!("Imported {} pages in {}ms", pages_imported, duration_ms);
            }
            ImportProgressEvent::Failed { error, files_processed } => {
                eprintln!("Import failed after {} files: {}", files_processed, error);
            }
        }
    });

    let summary = import_service.import_directory(
        dir_path.clone(),
        Some(progress_callback)
    ).await?;

    println!("Import complete: {}/{} files ({}% success)",
        summary.pages_imported,
        summary.total_files,
        summary.success_rate()
    );

    // Start sync service
    let sync_service = SyncService::new(
        repository,
        dir_path,
        Some(Duration::from_millis(500))
    )?;

    let sync_callback = Arc::new(|event| {
        match event {
            SyncEvent::FileCreated { file_path } => {
                println!("New file: {}", file_path.display());
            }
            SyncEvent::FileUpdated { file_path } => {
                println!("Updated: {}", file_path.display());
            }
            SyncEvent::FileDeleted { file_path } => {
                println!("Deleted: {}", file_path.display());
            }
            _ => {}
        }
    });

    // This runs indefinitely
    sync_service.start_watching(Some(sync_callback)).await?;

    Ok(())
}
```

## References

- [Architecture Notes](./notes/features/)
- [Technology Stack](./notes/dependencies/)
- [Working Notes](./notes/working_notes.md)
- [Linear Issue PER-5](https://linear.app/logjam/issue/PER-5)
