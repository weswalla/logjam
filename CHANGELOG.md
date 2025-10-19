# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

#### Domain Layer
- **Value Objects**:
  - `LogseqDirectoryPath` - Validated Logseq directory path with `pages/` and `journals/` subdirectories
  - `ImportProgress` - Tracks import operation progress (files processed, total files, percentage)

- **Domain Events**:
  - Import events: `ImportStarted`, `FileProcessed`, `ImportCompleted`, `ImportFailed`
  - Sync events: `SyncStarted`, `FileCreatedEvent`, `FileUpdatedEvent`, `FileDeletedEvent`, `SyncCompleted`

#### Infrastructure Layer
- **Logseq Markdown Parser** (`infrastructure/parsers/logseq_markdown.rs`):
  - Async markdown file parsing with Tokio
  - Indentation-based hierarchy parsing (tabs or 2-space indents)
  - Automatic URL extraction from content
  - Page reference (`[[page]]`) and tag (`#tag`) extraction
  - Converts markdown files to `Page` and `Block` domain objects

- **File System Utilities** (`infrastructure/file_system/`):
  - `discover_markdown_files()` - Recursively find all `.md` files
  - `discover_logseq_files()` - Find markdown files in `pages/` and `journals/` directories
  - `LogseqFileWatcher` - Cross-platform file watching with debouncing
  - Filter to only `.md` files in Logseq directories
  - Event types: Created, Modified, Deleted

#### Application Layer
- **ImportService** (`application/services/import_service.rs`):
  - Import entire Logseq directories
  - Bounded concurrency (4-6 files at once, configurable)
  - Progress tracking with real-time callbacks
  - Graceful error handling (continues on individual file failures)
  - Returns `ImportSummary` with statistics

- **SyncService** (`application/services/sync_service.rs`):
  - Incremental file synchronization
  - 500ms debouncing window (configurable)
  - Auto-sync on file changes (create, update, delete)
  - Event callbacks for sync operations
  - Runs indefinitely watching for changes

#### Dependencies
- `notify` (6.1) - Cross-platform file system event monitoring
- `notify-debouncer-mini` (0.4) - Event debouncing
- `tokio` (1.41) with features: fs, rt-multi-thread, macros, sync, time
- `serde` (1.0) with derive feature
- `serde_json` (1.0)
- `thiserror` (2.0) - Ergonomic error handling
- `anyhow` (1.0) - Application-level error handling
- `tracing` (0.1) - Structured logging
- `tracing-subscriber` (0.3) with env-filter
- `uuid` (1.11) with v4 and serde features
- `tempfile` (3.14) - Dev dependency for tests

#### Documentation
- Comprehensive `IMPLEMENTATION.md` with:
  - Architecture overview
  - Component documentation
  - Usage examples
  - Testing strategy
  - Design decisions
  - Future enhancements

### Changed
- Updated `backend/src/lib.rs` to include infrastructure module
- Updated `backend/src/application/mod.rs` to include services module and re-export types

### Technical Notes
- Implementation follows simplified DDD architecture suitable for personal projects
- Async/await throughout with Tokio runtime
- Bounded concurrency using `tokio::sync::Semaphore`
- File system is always the source of truth (no conflict resolution)
- Comprehensive unit tests for all components
- Integration tests ready for implementation

## [0.1.0] - Initial Release

### Added
- Domain layer with Page and Block entities
- Value objects: PageId, BlockId, Url, PageReference, BlockContent, IndentLevel
- Domain events for page and block operations
- Application layer with repository pattern
- Basic use cases: IndexPage, BatchIndexPages, SearchPagesAndBlocks

[Unreleased]: https://github.com/yourusername/logjam/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/yourusername/logjam/releases/tag/v0.1.0
