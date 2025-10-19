# File Event System

## Overview

Simple file watching system using the `notify` crate to monitor Logseq directory changes. Focuses on reliability and simplicity.

## Core Components

### File Watcher Service
- `FileWatcherService`: Main service that wraps the `notify` crate
  - Watches a single Logseq directory recursively
  - Filters events to only .md files in pages/ and journals/
  - Provides callback-based event notification

### Simple Event Types
```rust
#[derive(Debug, Clone)]
pub struct FileEvent {
    pub path: PathBuf,
    pub kind: FileEventKind,
    pub timestamp: SystemTime,
}

#[derive(Debug, Clone)]
pub enum FileEventKind {
    Created,
    Modified,
    Deleted,
}
```

### Basic Filtering
- Only process .md files
- Only watch pages/ and journals/ directories
- Ignore temporary files (.tmp, .swp, etc.)
- Simple path-based filtering

## Implementation Approach

### Watcher Setup
```rust
impl FileWatcherService {
    pub fn new() -> Self {
        // Use notify::recommended_watcher() for cross-platform compatibility
        // Set up recursive watching with simple event filtering
    }
    
    pub fn start_watching<F>(&mut self, path: PathBuf, callback: F) -> Result<(), WatchError>
    where F: Fn(FileEvent) + Send + 'static {
        // Start watching with the provided callback
        // Handle notify events and convert to our FileEvent type
    }
}
```

### Event Processing
- Direct callback to sync service (no complex event bus)
- Simple async channel if buffering is needed
- Basic error logging

### Cross-Platform Handling
- Use `notify::recommended_watcher()` for platform-appropriate backend
- Handle common cross-platform issues:
  - Case sensitivity differences
  - Path separator normalization
  - File locking on Windows

## Key Simplifications

**Removed:**
- Complex domain events and event bus
- Sophisticated debouncing (handled in sync service instead)
- Platform-specific handlers
- Event validation and deduplication
- Configuration management
- Detailed monitoring and metrics

**Kept:**
- Basic file filtering (.md files only)
- Cross-platform compatibility via notify crate
- Simple error handling
- Callback-based event notification

## Error Handling

### Simple Error Types
```rust
#[derive(Debug)]
pub enum WatchError {
    InvalidPath(PathBuf),
    PermissionDenied,
    WatcherFailed(notify::Error),
}
```

### Recovery Strategy
- Log errors and continue watching
- No automatic retry (keep it simple)
- Notify user of persistent issues via UI

## Testing Strategy

### Unit Tests
- Test file filtering logic
- Test event conversion from notify events

### Integration Tests
- Test with real file operations
- Test cross-platform behavior

This simplified approach provides reliable file watching with minimal complexity while still being testable and maintainable.
