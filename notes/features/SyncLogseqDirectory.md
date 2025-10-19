# Sync Logseq Directory Feature

## Overview

The SyncLogseqDirectory feature handles incremental updates to the search index when files in the Logseq directory change. This builds on the file event system to provide real-time synchronization while being efficient and reliable.

## Domain Layer

### Value Objects
- `SyncOperation`: Represents a single sync operation (Create, Update, Delete)
- `FileChecksum`: Hash-based file content verification
- `LastSyncTimestamp`: Tracks when the directory was last synchronized
- `SyncBatchId`: Groups related sync operations together

### Entities
- `SyncEvent`: Represents a single file system event that needs processing
- `FileSnapshot`: Captures the state of a file at a point in time
- `SyncConflict`: Represents conflicts between file system state and index state

### Aggregates
- `SyncSession`: Manages a batch of sync operations and ensures consistency
- `DirectoryState`: Tracks the current state of the Logseq directory

### Domain Events
- `SyncStarted`: Fired when a sync operation begins
- `FileChanged`: Fired when a file change is detected
- `FileIndexed`: Fired when a file has been successfully re-indexed
- `FileSyncFailed`: Fired when a file sync operation fails
- `SyncCompleted`: Fired when all pending changes have been processed
- `ConflictDetected`: Fired when there's a conflict between file system and index

### Domain Services
- `FileChangeDetector`: Determines what type of change occurred to a file
- `ConflictResolver`: Resolves conflicts between file system state and index state
- `SyncPriorityCalculator`: Determines the order in which files should be processed

## Application Layer

### Use Cases
- `StartDirectorySync`: Initiates a full directory sync (checks all files)
- `ProcessFileSystemEvent`: Handles a single file system event
- `ResolveSyncConflict`: Resolves conflicts between file system and index
- `GetSyncStatus`: Returns the current sync status and any pending operations
- `PauseSyncOperations`: Temporarily pauses sync operations
- `ResumeSyncOperations`: Resumes paused sync operations

### DTOs
- `SyncStatusResponse`: Contains information about sync state and pending operations
- `FileChangeEvent`: Represents a file system change event
- `SyncConflictInfo`: Information about detected conflicts
- `SyncConfiguration`: Settings for sync behavior

### Application Services
- `SyncOrchestrator`: Coordinates sync operations and manages the sync queue
- `EventProcessor`: Processes file system events and converts them to sync operations
- `ConflictManager`: Manages and resolves sync conflicts
- `SyncScheduler`: Manages timing and batching of sync operations

### Repository Interfaces
- `SyncStateRepository`: Persists sync state and metadata
- `FileMetadataRepository`: Stores file metadata for change detection
- `ConflictRepository`: Stores information about sync conflicts

## Infrastructure Layer

### File System Monitoring
- `FileSystemWatcher`: Wraps the notify crate for file system events
- `EventFilter`: Filters out irrelevant file system events
- `EventDebouncer`: Debounces rapid file changes to avoid excessive processing

### Change Detection
- `FileHashCalculator`: Calculates checksums for change detection
- `TimestampComparator`: Compares file modification times
- `ContentComparator`: Performs deep content comparison when needed

### Persistence
- `SqliteSyncStateRepository`: Persists sync state to SQLite
- `SqliteFileMetadataRepository`: Stores file metadata and checksums

### Task Management
- `SyncTaskQueue`: Manages queued sync operations with priority
- `BatchProcessor`: Groups related sync operations for efficiency
- `RetryManager`: Handles failed sync operations with exponential backoff

## Architecture Patterns

### Event-Driven Architecture
File system events drive sync operations through a clean event pipeline.

### CQRS (Command Query Responsibility Segregation)
Separate models for reading sync status vs. executing sync operations.

### Saga Pattern
Complex sync operations are broken down into compensatable steps.

### Circuit Breaker Pattern
Prevents cascade failures when file system operations fail repeatedly.

### Bulkhead Pattern
Isolates different types of sync operations to prevent one from affecting others.

## Sync Strategies

### Real-time Sync
- Process file system events immediately
- Suitable for small changes and active editing sessions
- Uses file system watchers for instant notification

### Batch Sync
- Group multiple changes together for efficient processing
- Suitable for large changes or when resuming after being offline
- Uses periodic scanning combined with event processing

### Hybrid Sync
- Combines real-time and batch approaches
- Real-time for immediate changes, batch for cleanup and verification
- Provides both responsiveness and reliability

## Conflict Resolution

### Conflict Types
- **File Modified Externally**: File changed outside of sync process
- **Index Corruption**: Index state doesn't match file system
- **Concurrent Modifications**: Multiple processes modifying files
- **Permission Issues**: Cannot read or access files

### Resolution Strategies
- **File System Wins**: Always use file system as source of truth
- **Index Wins**: Preserve index state (useful for rollback scenarios)
- **Manual Resolution**: Present conflicts to user for decision
- **Timestamp-based**: Use modification time to determine winner

## Performance Optimizations

### Debouncing
- Group rapid file changes to avoid excessive processing
- Configurable debounce intervals based on file type and size
- Smart debouncing that considers file editing patterns

### Incremental Processing
- Only process changed portions of files when possible
- Use content-aware diffing for large files
- Maintain block-level change tracking

### Caching
- Cache file metadata to avoid repeated file system calls
- Cache parsed content for recently accessed files
- Use LRU eviction for memory management

### Batching
- Batch database operations for efficiency
- Group related file operations together
- Use transaction boundaries for consistency

## Error Handling and Recovery

### Transient Errors
- Network issues (for network-mounted directories)
- Temporary file locks
- Permission issues

### Permanent Errors
- File corruption
- Disk space issues
- Invalid file formats

### Recovery Mechanisms
- Automatic retry with exponential backoff
- Fallback to full directory scan
- Graceful degradation (continue with other files)
- User notification for manual intervention

## Monitoring and Observability

### Metrics
- Sync operation latency
- Error rates by operation type
- Queue depth and processing rates
- File system event rates

### Logging
- Structured logging for all sync operations
- Performance metrics and timing
- Error details with context
- User actions and their outcomes

### Health Checks
- File system accessibility
- Index consistency checks
- Queue health monitoring
- Resource usage tracking

## Configuration

### Sync Behavior
- Real-time vs. batch sync preferences
- Debounce intervals
- Retry policies and limits
- Conflict resolution strategies

### Performance Tuning
- Maximum concurrent operations
- Batch sizes for database operations
- Memory limits for caching
- File size limits for processing

### Monitoring
- Log levels and destinations
- Metric collection intervals
- Health check frequencies
- Alert thresholds

## Integration with File Event System

### Event Pipeline
1. File system event detected by notify crate
2. Event filtered and debounced
3. Event converted to sync operation
4. Sync operation queued and prioritized
5. Operation executed with error handling
6. Results published as domain events

### Event Types Mapping
- `Create` → Index new file
- `Modify` → Re-index changed file
- `Delete` → Remove from index
- `Rename` → Update file path in index

### Backpressure Management
- Limit queue sizes to prevent memory issues
- Prioritize operations based on file importance
- Drop or batch low-priority operations under load

This architecture ensures that directory synchronization is:
- **Efficient**: Only processes changed files with smart batching
- **Reliable**: Handles errors gracefully with retry mechanisms
- **Consistent**: Maintains index consistency with conflict resolution
- **Observable**: Provides rich monitoring and debugging capabilities
- **Configurable**: Allows tuning for different use cases and performance requirements
