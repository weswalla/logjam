# 2025.10.18

- logseq page URLs: `logseq://graph/logseq-notes?page=notes`
  - there is a new domain object / value object here `LogseqUrl`
- also want to solidify / design two more major abstractions / components of the architecture

  - file syncing (edit events)
  - directory importing
  - infrastructure layer
    - db - sql?
    - embedding and similarity search (rust libraries to use?)
    - iframely or rust library for getting url metadata?
    - how will frontend communicate with backend (in the tauri app)
    - desktop app bundling
  - frontend
    - solidJS? also build tooling: vite
  - URL domain and application layer
    - want to be able to start simple (only get metadata but also be able to update it if I'd like) but eventually move to more like a search engine style indexing (full html and such)
  - embedding domain constructs - "chunking" of "blocks" which ideally is handled by the block or other domain / application layer classes for being able to process the contents of the block first (like removing special characters and links and so on - following best practices for embedding pre-processing)

- file syncing
  - basically monitoring the directory for file events
    - which will trigger creation / updates to existing entities / aggregates
    - update indexes
    - can this be done in a decoupled event driven way?
    - how much of this layer should be abstracted away - do we only deal with pages and blocks and urls for domain concepts, or do we introduce files and such? I suppose a subdomain could be directory - files and such
- directory importing
  - two types - first time ever or sync since last opened app
    - first time - gets all files processes them indexes and embeds them all, extracts url get's metadata and so on (again, these should be abstracted away behind interfaces at domain and application layer, so easy to isolate specific features and test them with mock implementations of other aspects)
    - sync since last opened
      - identify all files that have been created or updated since the last sync, process them and index them (same kind of processing as the first time, so ideally there is an overlap in services / interfaces / processes here)
    - both of these approaches should distribute the workload using task queue and parallelism so the app can continue while this stuff happens in the background (especially for the first sync) - if possible, the status of this syncing should also be available to a client somehow (see how many files and URLs are being processed etc.)
- infrastructure layer
  - semantic search options in rust:
    - https://www.reddit.com/r/rust/comments/15qsd7m/blazing_fast_opensource_semantic_searchasyoutype/
    - https://github.com/qdrant/page-search
    - https://github.com/Anush008/fastembed-rs
    - https://docs.rs/crate/bge/latest
    - https://docs.rs/semantic-search/latest/semantic_search/#:~:text=semantic-search%20is,top-k%20similar&text=See%20more,See%20more
    - https://docs.rs/qdrant-client/latest/qdrant_client/
    - looks like the right stack is fastembed-rs + qdrant
  - frontend <-> backend communication
- URL domain and application layer
- TODAYS PLAN OF ACTION
  - make sure basic file parsing to page and blocks is working
  - set up directory importing (following DDD and layered architecture)
    - "Logseq notes are made up of markdown files in a file directory. All relevant files are in 2 subdirectories `journals` and `pages`."
  - set up directory listeners (listening to file events and updating persistence / repositories)
  - set up directory syncing (for sync when opening app after some time)
    - making sure to get new files and updated files since the last time
  - set up block embedding (pre-processing with good abstractions) with vector storage and search
  - set up URL metadata parsing, indexing, persistence
  - set up tauri app + minimal frontend with some kind of API between them

## Implementation Summary & Alignment (2025.10.18)

### Technology Stack Confirmed
- **notify** for file system event monitoring
- **SQLite** (via tauri-plugin-sql) for persistence
- **tantivy** for text search (when implementing search)
- Semantic search (fastembed-rs + qdrant) deferred to later

### Current Focus Scope
1. File event handling with notify crate
2. ImportLogseqDirectory UseCase
3. LogseqDirectorySync UseCase  
4. Basic SQLite persistence
5. Good test coverage

### Architecture Approach
- Simplified DDD (not over-engineered for personal project)
- Clear separation of domain/application/infrastructure layers
- Direct callbacks from file watcher to sync service (no complex event bus)
- Async processing with bounded concurrency
- Simple debouncing for file changes

### Implementation Path
1. **Domain Layer:** Use existing Page/Block entities, add any needed value objects
2. **Application Layer:** ImportLogseqDirectory and LogseqDirectorySync use cases
3. **Infrastructure Layer:** File watching (notify), persistence (SQLite), file I/O
4. **Testing:** Unit tests for domain logic, integration tests with real files

### Key Decisions
- Feature markdown files provide good simplified foundation
- Direct callback approach from file watcher to sync service
- SQLite perfect for personal project persistence needs
- Tantivy for traditional text search initially
- Semantic search capabilities added later as separate feature
