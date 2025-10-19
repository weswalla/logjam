# Import Logseq Directory Feature

## Overview

The ImportLogseqDirectory feature handles the initial indexing of an entire Logseq directory structure. This is a complex operation that needs to be resilient, observable, and cancellable while following DDD principles.

## Domain Layer

### Value Objects
- `LogseqDirectoryPath`: Validates and represents the path to a Logseq directory
- `ImportProgress`: Represents the current state of an import operation (files processed, total files, current file, etc.)
- `ImportId`: Unique identifier for an import operation

### Entities
- `ImportOperation`: Tracks the state and progress of a single import operation
- `LogseqFile`: Represents a single markdown file with metadata (path, last modified, size, etc.)

### Aggregates
- `ImportSession`: Aggregate root that manages the entire import process, coordinates file discovery, parsing, and indexing

### Domain Events
- `ImportStarted`: Fired when an import operation begins
- `FileDiscovered`: Fired when a new markdown file is found
- `FileProcessed`: Fired when a file has been successfully indexed
- `FileProcessingFailed`: Fired when a file fails to process
- `ImportCompleted`: Fired when the entire import is finished
- `ImportCancelled`: Fired when an import is cancelled by the user
- `ImportFailed`: Fired when the import fails catastrophically

### Domain Services
- `LogseqDirectoryValidator`: Validates that a directory is a valid Logseq directory (contains pages/ and journals/ subdirectories)
- `MarkdownFileParser`: Parses markdown files into Page and Block domain objects
- `FileDiscoveryService`: Discovers all markdown files in the directory structure

## Application Layer

### Use Cases
- `StartImportOperation`: Initiates a new import operation
- `CancelImportOperation`: Cancels an ongoing import operation
- `GetImportProgress`: Retrieves the current progress of an import operation
- `ProcessLogseqFile`: Processes a single markdown file (used internally by the import orchestrator)

### DTOs
- `ImportRequest`: Contains the directory path and import options
- `ImportProgressResponse`: Contains progress information for the UI
- `ImportResult`: Contains the final result of an import operation

### Application Services
- `ImportOrchestrator`: Coordinates the entire import process, manages the task queue, and publishes progress updates
- `FileProcessingQueue`: Manages the queue of files to be processed (could use async channels or a proper task queue)

### Repository Interfaces
- `ImportSessionRepository`: Persists import session state
- `PageRepository`: Already exists, used to store parsed pages
- `FileMetadataRepository`: Stores metadata about processed files for future sync operations

## Infrastructure Layer

### File System
- `FileSystemService`: Handles file system operations (reading files, directory traversal)
- `LogseqDirectoryScanner`: Implements the actual directory scanning logic
- `MarkdownFileReader`: Reads and validates markdown files

### Persistence
- `SqliteImportSessionRepository`: Persists import sessions to SQLite
- `SqliteFileMetadataRepository`: Stores file metadata for sync operations

### Task Management
- `AsyncTaskQueue`: Manages background processing of files (could use tokio channels or a proper job queue)
- `ProgressPublisher`: Publishes progress updates to the UI (could use Tauri events)

### Tauri Integration
- `ImportController`: Tauri command handlers for import operations
- `ImportEventEmitter`: Emits Tauri events for progress updates

## Architecture Patterns

### Command Pattern
Each import operation is treated as a command that can be queued, executed, and potentially undone.

### Observer Pattern
Progress updates are published as domain events that the UI can subscribe to.

### Strategy Pattern
Different file processing strategies can be plugged in (e.g., fast vs. thorough parsing).

### Repository Pattern
All persistence operations go through repository interfaces.

### Unit of Work Pattern
Each file processing operation is wrapped in a unit of work for transactional consistency.

## Error Handling

### Domain Errors
- `InvalidLogseqDirectory`: Directory is not a valid Logseq directory
- `ImportAlreadyInProgress`: Attempt to start import when one is already running
- `ImportNotFound`: Attempt to operate on non-existent import

### Application Errors
- `FileAccessError`: Cannot read files due to permissions or I/O issues
- `ParsingError`: Cannot parse markdown file
- `StorageError`: Cannot persist data

### Recovery Strategies
- Partial imports: Continue processing other files if one fails
- Retry mechanism: Retry failed files with exponential backoff
- Checkpoint system: Save progress periodically to allow resuming

## Concurrency and Performance

### Async Processing
- Use Rust's async/await for I/O operations
- Process multiple files concurrently with bounded parallelism
- Use channels for communication between components

### Memory Management
- Stream file processing to avoid loading entire directory into memory
- Use lazy evaluation for file discovery
- Implement backpressure to prevent memory exhaustion

### Progress Tracking
- Atomic counters for thread-safe progress updates
- Periodic progress snapshots to avoid overwhelming the UI

## Testing Strategy

### Unit Tests
- Test domain logic in isolation
- Mock file system operations
- Test error conditions and edge cases

### Integration Tests
- Test with real file system operations
- Test the full import pipeline
- Test cancellation and error recovery

### Performance Tests
- Test with large Logseq directories
- Measure memory usage and processing speed
- Test concurrent operations

## UI Integration

### Progress Display
- Real-time progress bar with file counts
- Current file being processed
- Estimated time remaining
- Error summary

### User Controls
- Start/cancel import buttons
- Directory selection dialog
- Import options (e.g., skip certain file types)

### Error Reporting
- Detailed error messages for failed files
- Option to retry failed files
- Export error log for debugging

## Configuration

### Import Options
- Parallel processing limits
- File size limits
- File type filters
- Progress update frequency

### Performance Tuning
- Batch size for database operations
- Channel buffer sizes
- Timeout values

This architecture ensures that the import operation is:
- **Testable**: Clear separation of concerns with dependency injection
- **Maintainable**: Domain logic separated from infrastructure concerns
- **Scalable**: Async processing with bounded parallelism
- **Observable**: Rich progress reporting and error handling
- **Resilient**: Graceful error handling and recovery mechanisms
