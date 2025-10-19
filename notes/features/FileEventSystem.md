# File Event System

## Overview

The File Event System provides real-time monitoring of the Logseq directory for file changes. It serves as the foundation for the sync system and uses the `notify` Rust crate for cross-platform file system event detection.

## Domain Layer

### Value Objects
- `FilePath`: Validated file path with Logseq-specific validation
- `EventType`: Enumeration of file system event types (Create, Modify, Delete, Rename)
- `EventTimestamp`: When the event occurred
- `FileExtension`: Validated file extension (must be .md for Logseq files)

### Entities
- `FileEvent`: Represents a single file system event with metadata
- `WatchedDirectory`: Represents a directory being monitored

### Domain Events
- `FileCreated`: A new markdown file was created
- `FileModified`: An existing markdown file was modified
- `FileDeleted`: A markdown file was deleted
- `FileRenamed`: A markdown file was renamed or moved
- `WatchingStarted`: File system watching began for a directory
- `WatchingStopped`: File system watching stopped
- `WatchingError`: An error occurred in the file watching system

### Domain Services
- `FileEventValidator`: Validates that events are relevant to Logseq (markdown files in pages/ or journals/)
- `EventDeduplicator`: Removes duplicate events that can occur with some file systems
- `PathNormalizer`: Normalizes file paths across different operating systems

## Application Layer

### Use Cases
- `StartWatching`: Begin monitoring a Logseq directory for changes
- `StopWatching`: Stop monitoring a directory
- `ProcessFileEvent`: Handle a single file system event
- `GetWatchingStatus`: Check if a directory is currently being watched
- `ConfigureWatching`: Update watching configuration (filters, debounce settings, etc.)

### DTOs
- `WatchRequest`: Request to start watching a directory
- `FileEventDto`: Data transfer object for file events
- `WatchingStatusResponse`: Current status of file watching
- `WatchingConfiguration`: Configuration options for file watching

### Application Services
- `FileWatchingService`: Orchestrates file watching operations
- `EventProcessor`: Processes and routes file events to appropriate handlers
- `EventBuffer`: Buffers events for batch processing or debouncing

### Repository Interfaces
- `WatchedDirectoryRepository`: Persists information about watched directories
- `FileEventLogRepository`: Optional logging of file events for debugging

## Infrastructure Layer

### File System Integration
- `NotifyFileWatcher`: Wraps the `notify` crate for cross-platform file watching
- `EventAdapter`: Converts notify events to domain events
- `PathResolver`: Resolves relative paths and handles symlinks

### Event Processing
- `EventDebouncer`: Uses `notify-debouncer-mini` to reduce event noise
- `EventFilter`: Filters events based on file type, path patterns, and other criteria
- `EventQueue`: Manages queued events with backpressure handling

### Platform-Specific Handling
- `WindowsEventHandler`: Handles Windows-specific file system quirks
- `MacOSEventHandler`: Handles macOS-specific file system behavior
- `LinuxEventHandler`: Handles Linux-specific file system behavior

### Tauri Integration
- `FileEventController`: Tauri command handlers for file watching operations
- `EventEmitter`: Emits Tauri events to notify the frontend of file changes

## Architecture Patterns

### Observer Pattern
Components can subscribe to file events without tight coupling to the file watching system.

### Adapter Pattern
The notify crate is wrapped with domain-specific adapters to isolate external dependencies.

### Chain of Responsibility
File events pass through a chain of processors (filter → debounce → validate → route).

### Publisher-Subscriber Pattern
File events are published to multiple subscribers (sync system, UI updates, logging).

## Event Processing Pipeline

### Stage 1: Detection
1. `notify` crate detects file system event
2. Raw event is captured with timestamp
3. Event is queued for processing

### Stage 2: Filtering
1. Check if file is in pages/ or journals/ directory
2. Verify file has .md extension
3. Apply user-configured filters
4. Drop irrelevant events

### Stage 3: Debouncing
1. Group rapid successive events
2. Apply configurable debounce intervals
3. Merge related events (e.g., multiple modify events)

### Stage 4: Validation
1. Validate file paths exist and are accessible
2. Check file permissions
3. Verify file is valid markdown

### Stage 5: Domain Event Creation
1. Convert to appropriate domain event type
2. Add metadata (file size, modification time, etc.)
3. Assign unique event ID

### Stage 6: Publishing
1. Publish to event bus
2. Route to appropriate handlers
3. Log event for debugging

## Error Handling

### File System Errors
- **Permission Denied**: Log error and continue watching other files
- **File Not Found**: Handle race conditions where files are deleted quickly
- **Network Issues**: Handle network-mounted directories gracefully
- **Disk Full**: Detect and report storage issues

### Watching Errors
- **Watcher Creation Failed**: Fallback to polling mode
- **Event Buffer Overflow**: Drop oldest events and log warning
- **Invalid Directory**: Validate directory before starting watcher

### Recovery Strategies
- **Automatic Retry**: Retry failed operations with exponential backoff
- **Fallback Mechanisms**: Use polling when native watching fails
- **Graceful Degradation**: Continue with reduced functionality
- **User Notification**: Inform user of persistent issues

## Performance Considerations

### Event Volume Management
- Debounce rapid events to reduce processing load
- Use bounded queues to prevent memory exhaustion
- Implement backpressure to slow down event generation

### Resource Usage
- Monitor memory usage of event buffers
- Limit number of concurrent file operations
- Use efficient data structures for event storage

### Scalability
- Handle large directories with thousands of files
- Process events asynchronously to avoid blocking
- Use streaming for large file operations

## Configuration Options

### Watching Behavior
- Recursive vs. non-recursive watching
- Include/exclude patterns for files and directories
- Debounce intervals for different event types
- Maximum event queue size

### Performance Tuning
- Event buffer sizes
- Processing thread pool sizes
- Polling intervals (fallback mode)
- Memory limits for event storage

### Debugging
- Event logging levels
- Performance metrics collection
- Debug event tracing
- Error reporting verbosity

## Platform-Specific Considerations

### Windows
- Handle file locking issues with editors
- Deal with short filename generation
- Handle case-insensitive file systems
- Work around Windows Defender scanning delays

### macOS
- Handle FSEvents volume limitations
- Deal with case-insensitive but case-preserving file systems
- Handle Time Machine and other backup software interference
- Work with file system compression

### Linux
- Handle inotify limitations (watch limit, event types)
- Deal with different file system types (ext4, btrfs, etc.)
- Handle container and virtualization scenarios
- Work with network file systems (NFS, CIFS)

## Testing Strategy

### Unit Tests
- Test event filtering logic
- Test debouncing algorithms
- Test error handling scenarios
- Mock file system operations

### Integration Tests
- Test with real file system operations
- Test cross-platform compatibility
- Test with different file system types
- Test performance under load

### End-to-End Tests
- Test complete event pipeline
- Test integration with sync system
- Test user scenarios (editing files, bulk operations)
- Test error recovery scenarios

## Monitoring and Observability

### Metrics
- Event processing rates
- Queue depths and processing latency
- Error rates by type
- Resource usage (memory, CPU)

### Logging
- All file system events (configurable level)
- Error conditions with context
- Performance metrics
- Configuration changes

### Health Checks
- Watcher status and health
- Event processing pipeline health
- Resource usage monitoring
- Error rate monitoring

This file event system provides the foundation for real-time synchronization while being:
- **Reliable**: Handles errors gracefully and provides fallback mechanisms
- **Efficient**: Minimizes resource usage through smart filtering and debouncing
- **Cross-platform**: Works consistently across Windows, macOS, and Linux
- **Observable**: Provides rich monitoring and debugging capabilities
- **Configurable**: Allows tuning for different use cases and environments
