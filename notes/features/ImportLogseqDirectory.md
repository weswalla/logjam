# Import Logseq Directory Feature

## Overview

Simple, focused import system for indexing a Logseq directory. Emphasizes maintainability and testability without excessive abstraction.

## Core Components

### Domain Types (Value Objects)
- `ImportProgress`: Current state (files processed, total, current file)
- `LogseqDirectoryPath`: Validated directory path

### Main Service
- `ImportService`: Handles the entire import process
  - Validates directory structure (pages/ and journals/ exist)
  - Discovers markdown files
  - Parses files into Pages/Blocks using existing domain objects
  - Saves to existing PageRepository
  - Emits progress events to UI

### Simple Event System
- `ImportStarted`, `FileProcessed`, `ImportCompleted`, `ImportFailed`
- Direct Tauri event emission (no complex event bus)

## Implementation Approach

### File Processing
```rust
// Simple async processing with bounded concurrency
async fn import_directory(path: LogseqDirectoryPath) -> Result<(), ImportError> {
    let files = discover_markdown_files(&path).await?;
    let progress = Arc::new(AtomicUsize::new(0));
    
    // Process files with limited concurrency
    let semaphore = Arc::new(Semaphore::new(4)); // Max 4 concurrent files
    let tasks: Vec<_> = files.into_iter().map(|file| {
        let semaphore = semaphore.clone();
        let progress = progress.clone();
        tokio::spawn(async move {
            let _permit = semaphore.acquire().await;
            process_file(file).await?;
            progress.fetch_add(1, Ordering::Relaxed);
            emit_progress_event(progress.load(Ordering::Relaxed));
        })
    }).collect();
    
    // Wait for all tasks
    for task in tasks {
        task.await??;
    }
}
```

### Error Handling
- Use `Result<T, ImportError>` throughout
- Continue processing other files if one fails
- Collect errors and report at the end

### Progress Tracking
- Simple atomic counters
- Emit Tauri events directly to UI
- No complex progress aggregation

## Key Simplifications

**Removed:**
- Complex aggregate roots and entities
- Separate repository interfaces for import state
- Command/Observer patterns
- Unit of Work pattern
- Sophisticated retry mechanisms
- Import session persistence

**Kept:**
- Clear separation between domain logic and file I/O
- Async processing with concurrency limits
- Progress reporting
- Error handling that doesn't stop the entire import
- Testable design with mockable file operations

## Testing Strategy

### Unit Tests
- Test file discovery logic
- Test markdown parsing
- Mock file system operations

### Integration Tests
- Test with real small Logseq directory
- Test error scenarios (permission denied, corrupted files)

This simplified approach provides 80% of the benefits with 20% of the complexity.
