# Logjam Architecture Overview

**A comprehensive guide to understanding the Logjam codebase architecture, data flow, and implementation patterns.**

---

## Table of Contents

1. [Introduction](#introduction)
2. [High-Level Architecture](#high-level-architecture)
3. [DDD Building Blocks](#ddd-building-blocks)
4. [Layer-by-Layer Breakdown](#layer-by-layer-breakdown)
5. [End-to-End Workflows](#end-to-end-workflows)
6. [Code Patterns & Examples](#code-patterns--examples)
7. [Quick Reference](#quick-reference)

---

## Introduction

Logjam is a knowledge management application that imports, syncs, and searches Logseq markdown directories. It's built using **Domain-Driven Design (DDD)** principles with a clean layered architecture.

### Core Capabilities

- **Import:** Bulk import Logseq directories (one-time operation)
- **Sync:** Continuous file watching and incremental updates
- **Persistence:** SQLite storage for pages, blocks, and file mappings
- **Full-Text Search:** Tantivy (fuzzy, ranked keyword search)
- **Semantic Search:** Vector embeddings with similarity search (RAG-ready)
- **Hybrid Search:** Combine keyword + semantic results
- **UI Integration:** Tauri commands expose backend to frontend

### Technology Stack

```
Frontend:      TypeScript + React/Svelte (Tauri web view)
Backend:       Rust (async with Tokio)
Database:      SQLite (via sqlx)
Vector Store:  Qdrant (embedded mode)
Embeddings:    fastembed-rs (local embedding generation)
Text Search:   Tantivy (embedded search engine)
IPC:           Tauri (Rust ↔ Frontend bridge)
```

---

## High-Level Architecture

### 4-Layer Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     PRESENTATION LAYER                       │
│  (Tauri Commands, Event Emitters, Frontend API)             │
│                                                              │
│  • import_directory()     • search()                         │
│  • start_sync()           • get_all_pages()                  │
│  • Events: import-progress, sync-event                       │
└──────────────────────┬───────────────────────────────────────┘
                       │ DTOs (Data Transfer Objects)
                       ↓
┌─────────────────────────────────────────────────────────────┐
│                   APPLICATION LAYER                          │
│  (Use Cases, Services, Repository Traits)                   │
│                                                              │
│  Services:  ImportService, SyncService, SearchService        │
│  Use Cases: EmbedBlocks, SemanticSearch, UpdateEmbeddings   │
│  Repos:     PageRepository, FileMappingRepository            │
│             EmbeddingRepository, EmbeddingModelRepository    │
└──────────────────────┬───────────────────────────────────────┘
                       │ Domain Objects (Page, Block)
                       ↓
┌─────────────────────────────────────────────────────────────┐
│                     DOMAIN LAYER                             │
│  (Pure Business Logic - NO external dependencies)           │
│                                                              │
│  Aggregates:    Page, EmbeddedBlock                          │
│  Entities:      Block, TextChunk                             │
│  Value Objects: PageId, BlockId, Url, PageReference,         │
│                 EmbeddingVector, ChunkId, SimilarityScore    │
│  Events:        PageCreated, FileProcessed, SyncCompleted    │
└──────────────────────┬───────────────────────────────────────┘
                       │ Domain abstractions
                       ↓
┌─────────────────────────────────────────────────────────────┐
│                  INFRASTRUCTURE LAYER                        │
│  (Technical Implementation Details)                          │
│                                                              │
│  Persistence:   SqlitePageRepository                         │
│                 SqliteFileMappingRepository                  │
│  Parsers:       LogseqMarkdownParser                         │
│  File System:   LogseqFileWatcher, file discovery            │
│  Text Search:   TantivySearchIndex                           │
│  Embeddings:    FastEmbedService, EmbeddingModelManager      │
│  Text Proc:     TextPreprocessor (Logseq syntax removal)     │
│  Vector Store:  QdrantVectorStore, VectorCollectionManager   │
└─────────────────────────────────────────────────────────────┘
```

### Dependency Rule

**Critical principle:** Dependencies point INWARD only.

```
Presentation → Application → Domain ← Infrastructure
                                ↑
                                │
                    No dependencies on outer layers!
```

- **Domain Layer:** Zero dependencies (pure Rust, no external crates except std)
- **Application Layer:** Depends only on Domain
- **Infrastructure Layer:** Depends on Domain + Application (implements traits)
- **Presentation Layer:** Depends on Application + Infrastructure (wires everything together)

---

## DDD Building Blocks

### Domain Layer Abstractions

Logjam uses classic DDD patterns defined in `backend/src/domain/base.rs`:

```rust
// 1. Value Objects (Immutable, equality based on attributes)
pub trait ValueObject: Clone + PartialEq + Eq + Debug {}

// Examples: PageId, BlockId, Url, PageReference, IndentLevel
```

```rust
// 2. Entities (Identity-based equality)
pub trait Entity: Debug {
    type Id: ValueObject;
    fn id(&self) -> &Self::Id;
}

// Example: Block (has BlockId identity)
```

```rust
// 3. Aggregate Roots (Consistency boundaries)
pub trait AggregateRoot: Entity {
    fn apply_event(&mut self, event: &DomainEventEnum);
}

// Example: Page (owns Blocks, enforces invariants)
```

```rust
// 4. Domain Events (Things that happened)
pub trait DomainEvent: Debug + Clone {
    fn event_type(&self) -> &'static str;
    fn aggregate_id(&self) -> String;
}

// Examples: PageCreated, BlockAdded, FileProcessed
```

### Real Examples from Codebase

#### Value Object: PageId

```rust
// backend/src/domain/value_objects.rs

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PageId(String);

impl PageId {
    pub fn new(id: impl Into<String>) -> DomainResult<Self> {
        let id = id.into();
        if id.is_empty() {
            return Err(DomainError::InvalidValue("PageId cannot be empty".into()));
        }
        Ok(PageId(id))
    }

    pub fn as_str(&self) -> &str { &self.0 }
}

impl ValueObject for PageId {}
```

**Key pattern:** Constructor validation, immutability, private fields.

#### Entity: Block

```rust
// backend/src/domain/entities.rs

#[derive(Debug, Clone)]
pub struct Block {
    id: BlockId,                          // Identity
    content: BlockContent,
    indent_level: IndentLevel,
    parent_id: Option<BlockId>,
    child_ids: Vec<BlockId>,
    urls: Vec<Url>,
    page_references: Vec<PageReference>,
}

impl Entity for Block {
    type Id = BlockId;
    fn id(&self) -> &Self::Id { &self.id }
}
```

**Key pattern:** Has identity (`BlockId`), mutable state, behavior methods.

#### Aggregate Root: Page

```rust
// backend/src/domain/aggregates.rs

#[derive(Debug, Clone)]
pub struct Page {
    id: PageId,                           // Aggregate ID
    title: String,
    blocks: HashMap<BlockId, Block>,      // Owned entities
    root_block_ids: Vec<BlockId>,
}

impl Page {
    // Enforces invariants
    pub fn add_block(&mut self, block: Block) -> DomainResult<()> {
        // INVARIANT: Parent must exist before adding child
        if let Some(parent_id) = block.parent_id() {
            if !self.blocks.contains_key(parent_id) {
                return Err(DomainError::NotFound(
                    format!("Parent block {} not found", parent_id.as_str())
                ));
            }
        }

        self.blocks.insert(block.id().clone(), block);
        Ok(())
    }

    // Recursive operations
    pub fn remove_block(&mut self, block_id: &BlockId) -> DomainResult<bool> {
        // Recursively delete entire subtree
        let descendants = self.get_descendants(block_id)?;
        for desc_id in descendants {
            self.blocks.remove(&desc_id);
        }
        Ok(self.blocks.remove(block_id).is_some())
    }
}

impl AggregateRoot for Page {
    fn apply_event(&mut self, event: &DomainEventEnum) {
        // Placeholder for event sourcing
    }
}
```

**Key pattern:** Consistency boundary - all block operations go through Page methods.

---

## Layer-by-Layer Breakdown

### 1. Domain Layer (`backend/src/domain/`)

**Purpose:** Define business rules and entities (file system, database agnostic).

```
domain/
├── base.rs                # DDD trait definitions
├── value_objects.rs       # PageId, BlockId, Url, etc.
├── entities.rs            # Block entity
├── aggregates.rs          # Page aggregate root
├── events.rs              # Domain events
└── mod.rs
```

**Key Concepts:**

- **Page Aggregate:** Owns blocks, enforces hierarchy rules
- **Value Objects:** Self-validating, immutable data
- **No I/O:** All methods are synchronous, pure transformations

**Example - URL extraction:**

```rust
// Domain layer doesn't care HOW URLs are extracted from text
// It just defines WHAT a URL is

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Url {
    url: String,
    domain: Option<String>,
}

impl Url {
    pub fn new(url: impl Into<String>) -> DomainResult<Self> {
        let url = url.into();

        // Validation
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err(DomainError::InvalidValue("URL must start with http(s)".into()));
        }

        // Extract domain
        let domain = url.split('/')
            .nth(2)
            .map(String::from);

        Ok(Self { url, domain })
    }
}
```

### 2. Application Layer (`backend/src/application/`)

**Purpose:** Orchestrate use cases by coordinating domain objects and repositories.

```
application/
├── repositories/
│   ├── page_repository.rs         # Trait definition
│   └── file_mapping_repository.rs # Trait definition
├── services/
│   ├── import_service.rs          # Bulk import orchestration
│   ├── sync_service.rs            # Continuous sync orchestration
│   └── search_service.rs          # Search queries
├── dto/                           # Data Transfer Objects
└── use_cases/                     # CQRS-style commands/queries
```

**Repository Pattern:**

```rust
// backend/src/application/repositories/page_repository.rs

pub trait PageRepository {
    fn save(&mut self, page: Page) -> DomainResult<()>;
    fn find_by_id(&self, id: &PageId) -> DomainResult<Option<Page>>;
    fn find_by_title(&self, title: &str) -> DomainResult<Option<Page>>;
    fn find_all(&self) -> DomainResult<Vec<Page>>;
    fn delete(&mut self, id: &PageId) -> DomainResult<bool>;
}
```

**Service Pattern:**

```rust
// backend/src/application/services/import_service.rs

pub struct ImportService<R: PageRepository> {
    repository: R,
    max_concurrent_files: usize,
}

impl<R: PageRepository> ImportService<R> {
    pub async fn import_directory(
        &mut self,
        directory_path: LogseqDirectoryPath,
        progress_callback: Option<ProgressCallback>,
    ) -> ImportResult<ImportSummary> {
        // 1. Discover files
        // 2. Parse in parallel (bounded concurrency)
        // 3. Save to repository
        // 4. Report progress
    }
}
```

**Key pattern:** Generic over repository trait (dependency injection).

### 3. Infrastructure Layer (`backend/src/infrastructure/`)

**Purpose:** Implement technical details (DB, file I/O, parsing, search).

```
infrastructure/
├── persistence/
│   ├── sqlite_page_repository.rs      # SQLite implementation
│   ├── sqlite_file_mapping_repository.rs
│   ├── models.rs                      # Database row structs
│   └── mappers.rs                     # Domain ↔ DB conversion
├── parsers/
│   └── logseq_markdown.rs             # .md → Page/Block
├── file_system/
│   ├── discovery.rs                   # Find .md files
│   └── watcher.rs                     # File change detection
└── search/
    ├── tantivy_index.rs               # Search index
    └── schema.rs                      # Search document schema
```

**Example - Repository Implementation:**

```rust
// backend/src/infrastructure/persistence/sqlite_page_repository.rs

pub struct SqlitePageRepository {
    pool: SqlitePool,
}

impl PageRepository for SqlitePageRepository {
    fn save(&mut self, page: Page) -> DomainResult<()> {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                // 1. Convert domain Page to database rows
                let (page_row, block_rows, url_rows, ref_rows) =
                    PageMapper::from_domain(&page);

                // 2. Begin transaction
                let mut tx = self.pool.begin().await?;

                // 3. Upsert page
                sqlx::query("INSERT INTO pages (...) VALUES (...) ON CONFLICT(...) DO UPDATE...")
                    .bind(&page_row.id)
                    .bind(&page_row.title)
                    .execute(&mut tx).await?;

                // 4. Delete old blocks
                sqlx::query("DELETE FROM blocks WHERE page_id = ?")
                    .bind(&page_row.id)
                    .execute(&mut tx).await?;

                // 5. Insert new blocks
                for block in block_rows { /* ... */ }

                // 6. Commit
                tx.commit().await?;

                Ok(())
            })
        })
    }
}
```

**Example - Parser:**

```rust
// backend/src/infrastructure/parsers/logseq_markdown.rs

pub struct LogseqMarkdownParser;

impl LogseqMarkdownParser {
    pub async fn parse_file(path: &Path) -> ParseResult<Page> {
        // 1. Read file
        let content = tokio::fs::read_to_string(path).await?;

        // 2. Extract title from filename
        let title = path.file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| ParseError::InvalidMarkdown("No filename".into()))?;

        let page_id = PageId::new(title)?;

        // 3. Parse content into blocks
        Self::parse_content(&content, page_id, title.to_string())
    }

    fn parse_content(content: &str, page_id: PageId, title: String) -> ParseResult<Page> {
        let mut page = Page::new(page_id, title);

        // Parse each line as a block
        for line in content.lines() {
            if line.trim().is_empty() { continue; }

            // Calculate indent level
            let indent_level = Self::calculate_indent(line);

            // Extract content (remove bullet markers)
            let content = Self::extract_content(line);

            // Extract URLs and page references
            let urls = Self::extract_urls(&content);
            let refs = Self::extract_page_references(&content);

            // Create block
            let block = Block::new_root(
                BlockId::generate(),
                BlockContent::new(content)?,
            );

            page.add_block(block)?;
        }

        Ok(page)
    }
}
```

### 4. Presentation Layer (`backend/src/tauri/`)

**Purpose:** Expose backend to frontend via Tauri commands and events.

```
tauri/
├── state.rs           # AppState (shared state)
├── dto.rs             # Serializable DTOs
├── mappers.rs         # Domain → DTO conversion
└── commands/
    ├── import.rs      # Import commands
    ├── sync.rs        # Sync commands
    ├── pages.rs       # Page query commands
    └── search.rs      # Search commands
```

**Example - Tauri Command:**

```rust
// backend/src/tauri/commands/import.rs

#[tauri::command]
pub async fn import_directory(
    app: AppHandle,
    state: State<'_, AppState>,
    request: ImportRequest,
) -> Result<ImportSummaryDto, ErrorResponse> {
    // 1. Validate input
    let logseq_dir = LogseqDirectoryPath::new(&request.directory_path)
        .map_err(|e| ErrorResponse::new(e, "ValidationError"))?;

    // 2. Create service with repositories from state
    let page_repo = state.page_repository.lock().await.clone();
    let mapping_repo = state.mapping_repository.lock().await.clone();
    let mut import_service = ImportService::new(page_repo, mapping_repo);

    // 3. Setup progress callback (emit events to frontend)
    let app_clone = app.clone();
    let progress_callback = move |event| {
        let dto_event = DtoMapper::import_event_to_dto(event);
        let _ = app_clone.emit("import-progress", dto_event);
    };

    // 4. Execute import
    let summary = import_service
        .import_directory(logseq_dir, Some(Arc::new(progress_callback)))
        .await
        .map_err(|e| ErrorResponse::new(e, "ImportError"))?;

    // 5. Convert to DTO and return
    Ok(DtoMapper::import_summary_to_dto(&summary))
}
```

---

## End-to-End Workflows

### Workflow 1: Initial Import

**User Action:** Click "Import Logseq Directory" → Select `/path/to/logseq`

```
┌──────────────────────────────────────────────────────────────────┐
│                        FRONTEND                                   │
│  User clicks import → TauriApi.importDirectory()                 │
└───────────────────────────┬──────────────────────────────────────┘
                            │ invoke("import_directory", {...})
                            ↓
┌──────────────────────────────────────────────────────────────────┐
│                    TAURI COMMAND LAYER                            │
│  import_directory(app, state, request)                           │
│    1. Validate LogseqDirectoryPath                               │
│    2. Create ImportService                                       │
│    3. Setup progress callback                                    │
└───────────────────────────┬──────────────────────────────────────┘
                            │ import_service.import_directory()
                            ↓
┌──────────────────────────────────────────────────────────────────┐
│                   APPLICATION LAYER                               │
│  ImportService::import_directory()                               │
│    1. Discover files: discover_logseq_files(dir)                 │
│    2. For each file (parallel, bounded concurrency):             │
│       ├─ LogseqMarkdownParser::parse_file(path)                  │
│       ├─ page_repository.save(page)                              │
│       ├─ mapping_repository.save(mapping)                        │
│       ├─ tantivy_index.index_page(page)          [KEYWORD]       │
│       ├─ embed_blocks.execute(page.blocks())      [SEMANTIC]     │
│       └─ emit progress event                                     │
│    3. Return ImportSummary                                       │
└───────────────────────────┬──────────────────────────────────────┘
                            │ Calls to infrastructure...
                            ↓
┌──────────────────────────────────────────────────────────────────┐
│                  INFRASTRUCTURE LAYER                             │
│                                                                   │
│  File Discovery (infrastructure/file_system/discovery.rs):       │
│    • Recursively scan pages/ and journals/                       │
│    • Filter for .md files                                        │
│    • Return Vec<PathBuf>                                         │
│                                                                   │
│  Parser (infrastructure/parsers/logseq_markdown.rs):             │
│    • Read file content                                           │
│    • Parse markdown lines → Blocks                               │
│    • Extract URLs, page references                               │
│    • Build Page aggregate                                        │
│                                                                   │
│  Persistence (infrastructure/persistence/):                      │
│    • SqlitePageRepository::save()                                │
│      └─ INSERT pages, blocks, urls, refs (transaction)           │
│    • SqliteFileMappingRepository::save()                         │
│      └─ INSERT file_page_mappings                                │
│                                                                   │
│  Keyword Search Index (infrastructure/search/tantivy_index.rs):  │
│    • TantivySearchIndex::index_page()                            │
│      └─ Add page doc + block docs to inverted index              │
│                                                                   │
│  Semantic Search (infrastructure/embeddings/):                   │
│    • EmbedBlocks::execute()                                      │
│      ├─ TextPreprocessor: Remove [[links]], #tags, add context   │
│      ├─ FastEmbedService: Generate embeddings (batch of 32)      │
│      └─ QdrantVectorStore: Store vectors in HNSW index           │
└──────────────────────────────────────────────────────────────────┘
```

**Code Flow (Simplified):**

```rust
// 1. FRONTEND (TypeScript)
const summary = await TauriApi.importDirectory({
    directory_path: "/Users/me/logseq"
});

// 2. TAURI COMMAND
#[tauri::command]
async fn import_directory(state: State<AppState>, request: ImportRequest)
    -> Result<ImportSummaryDto>
{
    let mut service = ImportService::new(
        state.page_repository.lock().await,
        state.mapping_repository.lock().await
    );

    let summary = service.import_directory(logseq_dir, callback).await?;
    Ok(DtoMapper::to_dto(summary))
}

// 3. APPLICATION SERVICE
impl ImportService {
    async fn import_directory(&mut self, dir: LogseqDirectoryPath) -> ImportResult<ImportSummary> {
        let files = discover_logseq_files(dir.as_path()).await?;

        for file in files {
            let page = LogseqMarkdownParser::parse_file(&file).await?;

            // Save to database
            self.page_repository.save(page.clone())?;

            // Index for keyword search
            if let Some(ref tantivy_index) = self.tantivy_index {
                tantivy_index.lock().await.index_page(&page)?;
            }

            // Generate embeddings for semantic search
            if let Some(ref embed_blocks) = self.embed_blocks {
                embed_blocks.execute(page.all_blocks().collect(), &page).await?;
            }

            // ... emit progress
        }

        Ok(summary)
    }
}

// 4. INFRASTRUCTURE - PARSER
impl LogseqMarkdownParser {
    async fn parse_file(path: &Path) -> ParseResult<Page> {
        let content = tokio::fs::read_to_string(path).await?;
        // ... parse into Page aggregate
        Ok(page)
    }
}

// 5. INFRASTRUCTURE - REPOSITORY
impl PageRepository for SqlitePageRepository {
    fn save(&mut self, page: Page) -> DomainResult<()> {
        // Transaction: INSERT pages, blocks, urls, refs
        Ok(())
    }
}
```

**Data Transformations:**

```
File System           Domain              Database            Vector Store
────────────          ────────            ────────            ────────────

/pages/my-note.md     Page {              pages:              Qdrant Collection:
  - Line 1              id: "my-note"       id: "my-note"       "logseq_blocks"
  - Line 2              title: "my-note"    title: "my-note"
    - Nested            blocks: [                               Point 1:
                          Block {          blocks:                chunk_id: "block-1-chunk-0"
                            id: "block-1"    id: "block-1"        vector: [0.12, -0.45, 0.89, ...]
                            content: "..."   page_id: "my-note"   payload: {
                            indent: 0        content: "Line 1"      original: "Line 1"
                          },                 indent_level: 0      preprocessed: "Page: my-note. Line 1"
                          Block {                                }
                            id: "block-2"  blocks:
                            content: "..."   id: "block-2"      Point 2:
                            indent: 1        page_id: "my-note"   chunk_id: "block-2-chunk-0"
                          }                  content: "Nested"    vector: [0.34, 0.21, -0.67, ...]
                        ]                    parent_id: "block-1" payload: {
                      }                      indent_level: 1      original: "Nested"
                                                                  preprocessed: "Page: my-note. Nested"
                                                                }
```

**Note:** Embedding generation is optional and can be configured. If disabled, only keyword search (Tantivy) will be available.

### Workflow 2: Continuous Sync (File Watching)

**User Action:** Click "Start Sync" → App watches for file changes

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

**Code Example:**

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

**Key Insight - File→Page Mapping:**

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

### Workflow 3: Full-Text Search

**User Action:** Type "algorithm" in search box

```
┌──────────────────────────────────────────────────────────────────┐
│                        FRONTEND                                   │
│  <input onChange={query => search(query)} />                     │
│  User types: "algorithm"                                         │
└───────────────────────────┬──────────────────────────────────────┘
                            │ TauriApi.search({ query: "algorithm", ... })
                            ↓
┌──────────────────────────────────────────────────────────────────┐
│                    TAURI COMMAND                                  │
│  search(state, request) → SearchResultDto[]                      │
└───────────────────────────┬──────────────────────────────────────┘
                            │
                            ↓
┌──────────────────────────────────────────────────────────────────┐
│                  APPLICATION - SEARCH SERVICE                     │
│  SearchService::search(query, limit)                             │
└───────────────────────────┬──────────────────────────────────────┘
                            │
                            ↓
┌──────────────────────────────────────────────────────────────────┐
│              INFRASTRUCTURE - TANTIVY INDEX                       │
│  TantivySearchIndex::search("algorithm", 20)                     │
│                                                                   │
│  1. Parse query into Tantivy Query object                        │
│     ├─ QueryParser for fields: [page_title, block_content, ...]  │
│     └─ Parse "algorithm" into terms                              │
│                                                                   │
│  2. Execute search with BM25 ranking                             │
│     ├─ Searcher scans inverted index                             │
│     ├─ Calculate relevance scores                                │
│     └─ Return top 20 documents                                   │
│                                                                   │
│  3. Convert Tantivy documents → SearchResult                     │
│     └─ Extract page_id, block_id, content from stored fields     │
└───────────────────────────┬──────────────────────────────────────┘
                            │ Vec<SearchResult>
                            ↓
┌──────────────────────────────────────────────────────────────────┐
│  Return to frontend:                                             │
│  [                                                               │
│    BlockResult {                                                 │
│      page_id: "data-structures",                                 │
│      block_id: "block-42",                                       │
│      block_content: "Binary search algorithm is O(log n)",       │
│      score: 8.7                                                  │
│    },                                                            │
│    PageResult {                                                  │
│      page_id: "algorithms",                                      │
│      page_title: "Algorithms & Complexity",                      │
│      score: 6.2                                                  │
│    }                                                             │
│  ]                                                               │
└──────────────────────────────────────────────────────────────────┘
```

**Tantivy Index Structure:**

```
┌─────────────────────────────────────────────────────────────────┐
│                      TANTIVY INDEX                               │
│                                                                  │
│  Document Type 1: PAGE DOCUMENTS                                │
│  ┌────────────────────────────────────────────────────────┐     │
│  │ page_id: "algorithms"                                  │     │
│  │ page_title: "Algorithms & Complexity"  [SEARCHABLE]    │     │
│  │ document_type: "/page"                 [FACET]         │     │
│  └────────────────────────────────────────────────────────┘     │
│                                                                  │
│  Document Type 2: BLOCK DOCUMENTS                               │
│  ┌────────────────────────────────────────────────────────┐     │
│  │ page_id: "data-structures"                             │     │
│  │ block_id: "block-42"                                   │     │
│  │ page_title: "Data Structures"                          │     │
│  │ block_content: "Binary search algorithm..."[SEARCHABLE]│     │
│  │ urls: "https://en.wikipedia.org/wiki/Binary_search"    │     │
│  │ page_references: "algorithms complexity"               │     │
│  │ document_type: "/block"                [FACET]         │     │
│  │ indent_level: 1                        [INDEXED]       │     │
│  │ url_domains: "/domain/en.wikipedia.org"[FACET]         │     │
│  └────────────────────────────────────────────────────────┘     │
│                                                                  │
│  Inverted Index (for fast term lookup):                         │
│  ┌────────────────────────────────────────────────────────┐     │
│  │ "algorithm" → [doc_1, doc_5, doc_42, ...]             │     │
│  │ "binary"    → [doc_42, doc_103, ...]                  │     │
│  │ "search"    → [doc_42, doc_55, ...]                   │     │
│  └────────────────────────────────────────────────────────┘     │
└─────────────────────────────────────────────────────────────────┘
```

**Search Query Types:**

```rust
// 1. BASIC SEARCH (exact terms)
search_service.search("machine learning", 20)
// → Finds documents with "machine" AND/OR "learning"

// 2. FUZZY SEARCH (typo-tolerant, Levenshtein distance ≤ 2)
search_service.fuzzy_search("algoritm", 20)
// → Matches "algorithm" (edit distance = 1)

// 3. FILTERED SEARCH (facets)
search_service.search_with_filters("rust", 20, SearchFilters {
    document_type: Some("block"),      // Only search blocks
    reference_type: Some("tag"),       // Only blocks with tags
})

// 4. SPECIALIZED SEARCHES
search_service.search_pages("rust", 20)      // Only page titles
search_service.search_blocks("rust", 20)     // Only block content
search_service.search_tags("programming", 20) // Only tagged blocks
```

### Workflow 4: Semantic Search with Embeddings

**User Action:** Ask natural language question: "How do I optimize database queries?"

**Purpose:** Unlike keyword search (Tantivy), semantic search understands *meaning*. It finds conceptually similar content even without exact keyword matches.

```
┌──────────────────────────────────────────────────────────────────┐
│                        FRONTEND                                   │
│  User types: "How do I optimize database queries?"               │
│  (No exact keywords like "SQL" or "index" in query)              │
└───────────────────────────┬──────────────────────────────────────┘
                            │ TauriApi.semanticSearch({ query: "..." })
                            ↓
┌──────────────────────────────────────────────────────────────────┐
│                    TAURI COMMAND                                  │
│  semantic_search(state, request) → SemanticResultDto[]           │
└───────────────────────────┬──────────────────────────────────────┘
                            │
                            ↓
┌──────────────────────────────────────────────────────────────────┐
│              APPLICATION - SEMANTIC SEARCH USE CASE               │
│  SemanticSearch::execute(request)                                │
│                                                                   │
│  Step 1: Generate query embedding                                │
│    query_vector = fastembed_service.generate_embeddings([query]) │
│    → [0.12, -0.45, 0.89, ..., 0.34]  (384 dimensions)            │
└───────────────────────────┬──────────────────────────────────────┘
                            │ EmbeddingVector (query)
                            ↓
┌──────────────────────────────────────────────────────────────────┐
│          INFRASTRUCTURE - QDRANT VECTOR STORE                     │
│  QdrantVectorStore::similarity_search(query_vector, limit)       │
│                                                                   │
│  Step 2: Similarity search (cosine similarity)                   │
│    ├─ Compare query_vector to all chunk embeddings               │
│    ├─ Calculate cosine similarity scores                         │
│    └─ Return top K most similar chunks                           │
│                                                                   │
│  Vector Index (HNSW - Hierarchical Navigable Small World):       │
│    • Approximate nearest neighbor (ANN) search                   │
│    • O(log n) complexity instead of O(n)                         │
│    • Trade-off: 95%+ accuracy with 100x speedup                  │
└───────────────────────────┬──────────────────────────────────────┘
                            │ Vec<ScoredChunk>
                            ↓
┌──────────────────────────────────────────────────────────────────┐
│  Results (ranked by semantic similarity):                        │
│  [                                                               │
│    ScoredChunk {                                                 │
│      chunk_id: "chunk-147",                                      │
│      page_id: "database-performance",                            │
│      block_id: "block-89",                                       │
│      content: "Adding indexes on foreign keys dramatically       │
│                improves JOIN performance. Use EXPLAIN to..."     │
│      similarity_score: 0.87  ← High semantic match!              │
│    },                                                            │
│    ScoredChunk {                                                 │
│      chunk_id: "chunk-203",                                      │
│      page_id: "sql-tips",                                        │
│      content: "Query planning: PostgreSQL query planner uses     │
│                statistics to optimize execution..."              │
│      similarity_score: 0.82                                      │
│    }                                                             │
│  ]                                                               │
│                                                                   │
│  Note: Neither result contains "optimize database queries"       │
│        but both are semantically related!                        │
└──────────────────────────────────────────────────────────────────┘
```

#### Chunking Strategy

**Problem:** Embeddings have token limits (usually 512 tokens). We need to split pages into chunks.

**Chunking Approaches:**

```
┌─────────────────────────────────────────────────────────────────┐
│                   CHUNKING STRATEGIES                            │
│                                                                  │
│  1. BLOCK-BASED WITH PREPROCESSING (Logseq-aware)               │
│  ┌────────────────────────────────────────────────────────┐     │
│  │ Step 1: Remove Logseq syntax                           │     │
│  │   "Check [[Page Reference]] and #tag"                  │     │
│  │   → "Check Page Reference and tag"                     │     │
│  │                                                        │     │
│  │ Step 2: Add context markers                           │     │
│  │   Block: "Neural networks..."                         │     │
│  │   → "Page: Machine Learning. Neural networks..."      │     │
│  │                                                        │     │
│  │ Step 3: Create chunks (1 block = 1 chunk if ≤512 tok) │     │
│  │                                                        │     │
│  │ ✅ Preserves hierarchical context                      │     │
│  │ ✅ Clean text for better embeddings                    │     │
│  │ ❌ Blocks can still be too small or too large         │     │
│  └────────────────────────────────────────────────────────┘     │
│                                                                  │
│  2. ROLLING WINDOW CHUNKING (Overlapping)                       │
│  ┌────────────────────────────────────────────────────────┐     │
│  │ Fixed-size chunks with overlap                         │     │
│  │                                                        │     │
│  │ Text: "ABCDEFGHIJ"                                    │     │
│  │ Chunk 1: [ABC]                                        │     │
│  │ Chunk 2:   [CDE]  ← 1 token overlap                   │     │
│  │ Chunk 3:     [EFG]                                    │     │
│  │ Chunk 4:       [GHI]                                  │     │
│  │                                                        │     │
│  │ ✅ Ensures context isn't lost at boundaries            │     │
│  │ ❌ More chunks = more storage + compute               │     │
│  └────────────────────────────────────────────────────────┘     │
│                                                                  │
│  3. SEMANTIC CHUNKING (Context-aware)                           │
│  ┌────────────────────────────────────────────────────────┐     │
│  │ Split at topic boundaries (sentence similarity)        │     │
│  │                                                        │     │
│  │ Paragraph 1: "Rust ownership rules..."               │     │
│  │ Paragraph 2: "Borrowing prevents data races..."      │     │
│  │ ↓ High similarity → same chunk                        │     │
│  │                                                        │     │
│  │ Paragraph 3: "JavaScript async/await..."             │     │
│  │ ↓ Low similarity → new chunk                          │     │
│  │                                                        │     │
│  │ ✅ Chunks are topically coherent                       │     │
│  │ ❌ Computationally expensive                          │     │
│  └────────────────────────────────────────────────────────┘     │
│                                                                  │
│  RECOMMENDED: Block-based with preprocessing                    │
│    • Preprocess: Remove Logseq syntax, add context markers      │
│    • 1 block = 1 chunk if ≤ 512 tokens                          │
│    • Split large blocks with 50-token overlap                   │
│    • Batch processing: 32 blocks per batch for efficiency       │
└─────────────────────────────────────────────────────────────────┘
```

#### Embedding Generation Pipeline

**Full workflow from import to embedding:**

```rust
// 1. IMPORT/SYNC: Page is saved to database
page_repository.save(page)?;

// 2. PREPROCESSING & CHUNKING: Create TextChunks from blocks
let text_preprocessor = TextPreprocessor::new();
let chunks: Vec<TextChunk> = page.all_blocks()
    .flat_map(|block| {
        // Preprocess: Remove [[links]], #tags, clean markdown
        let preprocessed = text_preprocessor.preprocess_block(block);

        // Add context: page title, parent hierarchy
        let with_context = text_preprocessor.add_context_markers(&preprocessed, block);

        // Chunk if needed (512 token limit, 50 token overlap)
        TextChunk::from_block(block, page.title(), with_context)
    })
    .collect();

// Example chunk:
// TextChunk {
//   chunk_id: "block-1-chunk-0",
//   block_id: "block-1",
//   page_id: "machine-learning",
//   original_content: "Check [[Neural Networks]] for #deep-learning info",
//   preprocessed_content: "Page: Machine Learning. Check Neural Networks for deep-learning info",
//   chunk_index: 0,
//   total_chunks: 1
// }

// 3. BATCH EMBEDDING: Generate vectors for chunks (32 at a time)
let batch_size = 32;
for chunk_batch in chunks.chunks(batch_size) {
    let texts: Vec<String> = chunk_batch.iter()
        .map(|c| c.preprocessed_text().to_string())
        .collect();

    // Use fastembed-rs for local embedding generation
    let embeddings = fastembed_service.generate_embeddings(texts).await?;
    // embeddings = Vec<Vec<f32>> with 384 dimensions (all-MiniLM-L6-v2)

    // 4. STORAGE: Save to Qdrant vector database
    qdrant_store.upsert_embeddings(chunk_batch, embeddings).await?;
}

// 5. INDEX: Qdrant builds HNSW index automatically (no manual commit needed)
```

#### Code Example: Text Preprocessing

```rust
// backend/src/infrastructure/embeddings/text_preprocessor.rs

pub struct TextPreprocessor;

impl TextPreprocessor {
    pub fn preprocess_block(&self, block: &Block) -> String {
        let content = block.content().as_str();

        // 1. Remove Logseq-specific syntax
        let cleaned = self.remove_logseq_syntax(content);

        // 2. Clean markdown formatting
        let cleaned = self.clean_markdown(&cleaned);

        cleaned
    }

    fn remove_logseq_syntax(&self, text: &str) -> String {
        let mut result = text.to_string();

        // Remove [[page references]] but keep the text
        // "Check [[Neural Networks]]" → "Check Neural Networks"
        result = Regex::new(r"\[\[([^\]]+)\]\]")
            .unwrap()
            .replace_all(&result, "$1")
            .to_string();

        // Remove #tags but keep the text
        // "Learn #machine-learning" → "Learn machine-learning"
        result = Regex::new(r"#(\S+)")
            .unwrap()
            .replace_all(&result, "$1")
            .to_string();

        // Remove TODO/DONE markers
        result = Regex::new(r"(TODO|DONE|LATER|NOW|WAITING)\s+")
            .unwrap()
            .replace_all(&result, "")
            .to_string();

        result
    }

    fn clean_markdown(&self, text: &str) -> String {
        // Remove bold/italic markers but keep text
        // Remove code block markers
        // Keep content readable for embeddings
        // ... implementation
    }

    pub fn add_context_markers(&self, text: &str, block: &Block, page: &Page) -> String {
        let mut contextualized = String::new();

        // Add page title as context
        contextualized.push_str(&format!("Page: {}. ", page.title()));

        // Add parent block context for nested blocks
        if let Some(parent_id) = block.parent_id() {
            if let Some(parent) = page.get_block(parent_id) {
                contextualized.push_str(&format!("Parent: {}. ", parent.content().as_str()));
            }
        }

        // Add the actual content
        contextualized.push_str(text);

        contextualized
    }
}
```

#### Code Example: EmbedBlocks Use Case

```rust
// backend/src/application/use_cases/embed_blocks.rs

pub struct EmbedBlocks {
    embedding_service: Arc<FastEmbedService>,
    vector_store: Arc<QdrantVectorStore>,
    embedding_repository: Arc<dyn EmbeddingRepository>,
    preprocessor: TextPreprocessor,
}

impl EmbedBlocks {
    pub async fn execute(&self, blocks: Vec<Block>, page: &Page) -> DomainResult<()> {
        // 1. Preprocess blocks into TextChunks
        let chunks = self.create_chunks_from_blocks(blocks, page)?;

        // 2. Generate embeddings in batches (32 at a time for efficiency)
        let batch_size = 32;
        for chunk_batch in chunks.chunks(batch_size) {
            let texts: Vec<String> = chunk_batch.iter()
                .map(|c| c.preprocessed_text().to_string())
                .collect();

            // Generate embeddings using fastembed-rs
            let embeddings = self.embedding_service
                .generate_embeddings(texts)
                .await?;

            // 3. Store in vector database with metadata
            self.store_embeddings(chunk_batch, embeddings).await?;
        }

        Ok(())
    }

    fn create_chunks_from_blocks(&self, blocks: Vec<Block>, page: &Page) -> DomainResult<Vec<TextChunk>> {
        let mut chunks = Vec::new();

        for block in blocks {
            // Preprocess: remove Logseq syntax, clean markdown
            let cleaned = self.preprocessor.preprocess_block(&block);

            // Add context: page title, parent hierarchy
            let with_context = self.preprocessor.add_context_markers(&cleaned, &block, page);

            // Create chunks (split if > 512 tokens, 50 token overlap)
            let block_chunks = TextChunk::from_block(&block, page.title(), with_context);
            chunks.extend(block_chunks);
        }

        Ok(chunks)
    }

    async fn store_embeddings(&self, chunks: &[TextChunk], embeddings: Vec<Vec<f32>>) -> DomainResult<()> {
        for (chunk, embedding) in chunks.iter().zip(embeddings.iter()) {
            // Create EmbeddingVector value object
            let embedding_vector = EmbeddingVector::new(embedding.clone())?;

            // Create EmbeddedBlock aggregate
            let embedded_block = EmbeddedBlock::new(
                chunk.block_id().clone(),
                chunk.page_id().clone(),
                embedding_vector,
                chunk.clone(),
            );

            // Store in Qdrant with full payload
            self.vector_store.upsert_point(
                chunk.chunk_id(),
                embedding.clone(),
                Payload {
                    chunk_id: chunk.chunk_id().as_str(),
                    block_id: chunk.block_id().as_str(),
                    page_id: chunk.page_id().as_str(),
                    page_title: chunk.page_title(),
                    chunk_index: chunk.chunk_index(),
                    total_chunks: chunk.total_chunks(),
                    original_content: chunk.original_content(),
                    preprocessed_content: chunk.preprocessed_text(),
                    hierarchy_path: chunk.hierarchy_path(),
                }
            ).await?;

            // Track in repository
            self.embedding_repository.save(embedded_block).await?;
        }

        Ok(())
    }
}
```

#### Infrastructure: Qdrant Vector Store

```rust
// backend/src/infrastructure/vector_store/qdrant_store.rs

use qdrant_client::{client::QdrantClient, qdrant::*};

pub struct QdrantVectorStore {
    client: QdrantClient,
    collection_name: String,
}

impl QdrantVectorStore {
    pub async fn new_embedded() -> Result<Self> {
        // Embedded mode - no separate Qdrant server needed
        let client = QdrantClient::from_url("http://localhost:6334").build()?;

        let collection_name = "logseq_blocks".to_string();

        // Create collection: 384 dimensions, cosine similarity
        client.create_collection(&CreateCollection {
            collection_name: collection_name.clone(),
            vectors_config: Some(VectorsConfig {
                config: Some(Config::Params(VectorParams {
                    size: 384,  // all-MiniLM-L6-v2
                    distance: Distance::Cosine.into(),
                    hnsw_config: Some(HnswConfigDiff {
                        m: Some(16),           // connections per layer
                        ef_construct: Some(100), // build-time accuracy
                        ..Default::default()
                    }),
                    ..Default::default()
                })),
            }),
            ..Default::default()
        }).await?;

        Ok(Self { client, collection_name })
    }

    pub async fn upsert_point(
        &self,
        chunk_id: &ChunkId,
        embedding: Vec<f32>,
        payload: Payload,
    ) -> Result<()> {
        let point = PointStruct {
            id: Some(PointId::from(chunk_id.as_str())),
            vectors: Some(Vectors::from(embedding)),
            payload: payload.into_map(),
        };

        self.client.upsert_points(
            &self.collection_name,
            None,
            vec![point],
            None,
        ).await?;

        Ok(())
    }

    pub async fn similarity_search(
        &self,
        query_embedding: EmbeddingVector,
        limit: usize,
    ) -> Result<Vec<ScoredChunk>> {
        let search_result = self.client.search_points(&SearchPoints {
            collection_name: self.collection_name.clone(),
            vector: query_embedding.as_vec(),
            limit: limit as u64,
            with_payload: Some(WithPayloadSelector::from(true)),
            score_threshold: Some(0.5),  // Minimum similarity
            ..Default::default()
        }).await?;

        Ok(search_result.result.into_iter()
            .map(|scored_point| ScoredChunk {
                chunk_id: ChunkId::new(scored_point.id.unwrap().to_string()).unwrap(),
                block_id: BlockId::new(
                    scored_point.payload.get("block_id").unwrap().as_str().unwrap()
                ).unwrap(),
                page_id: PageId::new(
                    scored_point.payload.get("page_id").unwrap().as_str().unwrap()
                ).unwrap(),
                similarity_score: SimilarityScore::new(scored_point.score),
                content: scored_point.payload.get("original_content")
                    .unwrap().as_str().unwrap().to_string(),
            })
            .collect())
    }
}
```

#### Hybrid Search: Combining Keyword + Semantic

**Best results come from combining both approaches:**

```rust
// backend/src/application/services/hybrid_search_service.rs

pub struct HybridSearchService {
    text_search: SearchService,           // Tantivy
    semantic_search: EmbeddingService,    // Qdrant
}

impl HybridSearchService {
    pub async fn hybrid_search(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<HybridResult>> {
        // 1. Parallel search (both at once)
        let (text_results, semantic_results) = tokio::join!(
            self.text_search.search(query, limit),
            self.semantic_search.semantic_search(query, limit),
        );

        // 2. Reciprocal Rank Fusion (RRF) for score combination
        // Formula: score = Σ(1 / (k + rank_i)) where k = 60
        let mut combined_scores: HashMap<String, f32> = HashMap::new();

        for (rank, result) in text_results?.iter().enumerate() {
            let key = format!("{}:{}", result.page_id(), result.block_id());
            let rrf_score = 1.0 / (60.0 + rank as f32);
            *combined_scores.entry(key).or_insert(0.0) += rrf_score * 0.7; // 70% weight
        }

        for (rank, result) in semantic_results?.iter().enumerate() {
            let key = format!("{}:{}", result.page_id, result.chunk_id);
            let rrf_score = 1.0 / (60.0 + rank as f32);
            *combined_scores.entry(key).or_insert(0.0) += rrf_score * 0.3; // 30% weight
        }

        // 3. Sort by combined score
        let mut results: Vec<_> = combined_scores.into_iter().collect();
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        // 4. Return top K
        Ok(results.into_iter()
            .take(limit)
            .map(|(id, score)| HybridResult { id, score })
            .collect())
    }
}
```

**Why Hybrid?**

```
Keyword Search (Tantivy):
✅ Exact matches (code, filenames, specific terms)
✅ Very fast (milliseconds)
❌ Misses synonyms ("car" won't find "automobile")
❌ No semantic understanding

Semantic Search (Embeddings):
✅ Understands meaning ("fast car" finds "quick vehicle")
✅ Handles paraphrasing
❌ Slower (tens of milliseconds)
❌ Can miss exact technical terms

Hybrid:
✅ Best of both worlds
✅ Technical terms + conceptual understanding
```

#### Integration with Import/Sync

**Automatic embedding during import:**

```rust
// backend/src/application/services/import_service.rs

impl ImportService {
    pub async fn import_directory(&mut self, dir: LogseqDirectoryPath) -> Result<ImportSummary> {
        for file in files {
            // 1. Parse file
            let page = LogseqMarkdownParser::parse_file(&file).await?;

            // 2. Save to database
            self.page_repository.save(page.clone())?;

            // 3. Index in Tantivy (keyword search)
            if let Some(ref tantivy_index) = self.tantivy_index {
                tantivy_index.lock().await.index_page(&page)?;
            }

            // 4. Generate embeddings and index (semantic search)
            if let Some(ref embedding_service) = self.embedding_service {
                embedding_service.embed_and_index_page(&page).await?;
            }

            // 5. Save file mapping
            self.mapping_repository.save(mapping)?;
        }

        // Commit both indexes
        self.tantivy_index.lock().await.commit()?;
        // Qdrant commits automatically

        Ok(summary)
    }
}
```

**Automatic re-embedding on sync:**

```rust
// backend/src/application/services/sync_service.rs

async fn handle_file_updated(&self, path: PathBuf) -> SyncResult<()> {
    let page = LogseqMarkdownParser::parse_file(&path).await?;

    // Update database
    self.page_repository.lock().await.save(page.clone())?;

    // Update Tantivy index
    if let Some(ref index) = self.tantivy_index {
        index.lock().await.update_page(&page)?;
        index.lock().await.commit()?;
    }

    // Update embeddings
    if let Some(ref embedding_service) = self.embedding_service {
        // Delete old chunks for this page
        embedding_service.delete_page_chunks(&page.id()).await?;

        // Re-embed and index
        embedding_service.embed_and_index_page(&page).await?;
    }

    Ok(())
}
```

---

## Code Patterns & Examples

### Pattern 1: Value Object Validation

**All value objects validate at construction:**

```rust
// ✅ GOOD: Validation in constructor
impl Url {
    pub fn new(url: impl Into<String>) -> DomainResult<Self> {
        let url = url.into();

        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err(DomainError::InvalidValue("Invalid URL".into()));
        }

        Ok(Self { url, domain: extract_domain(&url) })
    }
}

// ❌ BAD: No validation
impl Url {
    pub fn new(url: String) -> Self {
        Self { url }  // Could be invalid!
    }
}
```

**Usage:**

```rust
// Construction can fail (returns Result)
let url = Url::new("https://example.com")?;  // ✅ Valid
let bad = Url::new("not-a-url")?;            // ❌ Returns Err
```

### Pattern 2: Aggregate Invariants

**Page aggregate enforces hierarchy rules:**

```rust
impl Page {
    // INVARIANT: Parent block must exist before adding child
    pub fn add_block(&mut self, block: Block) -> DomainResult<()> {
        if let Some(parent_id) = block.parent_id() {
            if !self.blocks.contains_key(parent_id) {
                return Err(DomainError::NotFound(
                    format!("Parent block {} does not exist", parent_id.as_str())
                ));
            }

            // Update parent's child_ids
            if let Some(parent) = self.blocks.get_mut(parent_id) {
                parent.add_child(block.id().clone());
            }
        } else {
            // Root block
            self.root_block_ids.push(block.id().clone());
        }

        self.blocks.insert(block.id().clone(), block);
        Ok(())
    }
}
```

**This prevents:**

```rust
❌ let orphan_block = Block::new_child(
    BlockId::generate(),
    content,
    indent_level,
    BlockId::new("non-existent-parent")  // Parent doesn't exist!
);
page.add_block(orphan_block)?;  // Returns Err - prevented!
```

### Pattern 3: Repository Trait + Multiple Implementations

**Define trait in Application layer:**

```rust
// backend/src/application/repositories/page_repository.rs
pub trait PageRepository {
    fn save(&mut self, page: Page) -> DomainResult<()>;
    fn find_by_id(&self, id: &PageId) -> DomainResult<Option<Page>>;
    // ...
}
```

**Implement in Infrastructure layer:**

```rust
// backend/src/infrastructure/persistence/sqlite_page_repository.rs
pub struct SqlitePageRepository { /* ... */ }

impl PageRepository for SqlitePageRepository {
    fn save(&mut self, page: Page) -> DomainResult<()> {
        // SQL implementation
    }
}

// backend/tests/helpers/in_memory_repository.rs (for testing)
pub struct InMemoryPageRepository {
    pages: HashMap<PageId, Page>,
}

impl PageRepository for InMemoryPageRepository {
    fn save(&mut self, page: Page) -> DomainResult<()> {
        self.pages.insert(page.id().clone(), page);
        Ok(())
    }
}
```

**Use via dependency injection:**

```rust
// Production
let repo = SqlitePageRepository::new("db.sqlite").await?;
let service = ImportService::new(repo);

// Testing
let repo = InMemoryPageRepository::new();
let service = ImportService::new(repo);
```

### Pattern 4: Error Conversion Chain

**Errors flow upward and get wrapped:**

```rust
// Domain Layer
pub enum DomainError {
    InvalidValue(String),
    NotFound(String),
}

// Infrastructure Layer
pub enum ParseError {
    Io(#[from] std::io::Error),
    Domain(#[from] DomainError),
}

// Application Layer
pub enum ImportError {
    FileSystem(#[from] std::io::Error),
    Parse(#[from] ParseError),
    Repository(#[from] DomainError),
}

// Presentation Layer
pub struct ErrorResponse {
    error: String,
    error_type: String,
}

impl From<ImportError> for ErrorResponse {
    fn from(err: ImportError) -> Self {
        ErrorResponse {
            error: err.to_string(),
            error_type: match err {
                ImportError::FileSystem(_) => "FileSystemError",
                ImportError::Parse(_) => "ParseError",
                ImportError::Repository(_) => "RepositoryError",
            }.into(),
        }
    }
}
```

**Flow:**

```
std::io::Error
    ↓ #[from]
ParseError::Io
    ↓ #[from]
ImportError::Parse
    ↓ From trait
ErrorResponse { error_type: "ParseError" }
    ↓ serialize
Frontend sees: { "error": "...", "error_type": "ParseError" }
```

### Pattern 5: DTO Mapping (Domain ↔ Serialization)

**Domain objects are NOT serializable (intentionally):**

```rust
// Domain layer - NO Serialize/Deserialize
#[derive(Debug, Clone)]
pub struct Page {
    id: PageId,
    title: String,
    blocks: HashMap<BlockId, Block>,
}
```

**Create DTOs in Presentation layer:**

```rust
// Presentation layer - IS serializable
#[derive(Serialize, Deserialize)]
pub struct PageDto {
    pub id: String,              // PageId → String
    pub title: String,
    pub blocks: Vec<BlockDto>,   // HashMap → Vec
}

// Mapper
impl DtoMapper {
    pub fn page_to_dto(page: &Page) -> PageDto {
        PageDto {
            id: page.id().as_str().to_string(),
            title: page.title().to_string(),
            blocks: page.all_blocks().map(Self::block_to_dto).collect(),
        }
    }
}
```

**Why?** Domain objects may have complex invariants, references, or non-serializable fields. DTOs are simplified for wire transfer.

### Pattern 6: Event-Driven Progress Reporting

**Services emit events for UI updates:**

```rust
// Define callback type
pub type ProgressCallback = Arc<dyn Fn(ImportProgressEvent) + Send + Sync>;

// Service accepts optional callback
impl ImportService {
    pub async fn import_directory(
        &mut self,
        dir: LogseqDirectoryPath,
        progress_callback: Option<ProgressCallback>,
    ) -> ImportResult<ImportSummary> {
        // Emit "Started" event
        if let Some(ref callback) = progress_callback {
            callback(ImportProgressEvent::Started(progress));
        }

        for file in files {
            // Process file...

            // Emit "FileProcessed" event
            if let Some(ref callback) = progress_callback {
                callback(ImportProgressEvent::FileProcessed(updated_progress));
            }
        }

        // Emit "Completed" event
        if let Some(ref callback) = progress_callback {
            callback(ImportProgressEvent::Completed(summary));
        }

        Ok(summary)
    }
}
```

**Tauri bridges events to frontend:**

```rust
let app_clone = app.clone();
let callback = move |event: ImportProgressEvent| {
    // Convert to DTO
    let dto = DtoMapper::event_to_dto(event);

    // Emit to frontend via Tauri event system
    let _ = app_clone.emit("import-progress", dto);
};

service.import_directory(dir, Some(Arc::new(callback))).await?;
```

**Frontend listens:**

```typescript
import { listen } from '@tauri-apps/api/event';

listen('import-progress', (event) => {
    const progress = event.payload;

    if (progress.type === 'FileProcessed') {
        console.log(`Processed ${progress.current}/${progress.total}`);
        updateProgressBar(progress.current / progress.total * 100);
    }
});
```

---

## Quick Reference

### File Locations Cheat Sheet

| Component | File Path |
|-----------|-----------|
| **Domain** |
| Page aggregate | `backend/src/domain/aggregates.rs` |
| Block entity | `backend/src/domain/entities.rs` |
| Value objects | `backend/src/domain/value_objects.rs` |
| Domain events | `backend/src/domain/events.rs` |
| **Application** |
| PageRepository trait | `backend/src/application/repositories/page_repository.rs` |
| EmbeddingRepository trait | `backend/src/application/repositories/embedding_repository.rs` |
| EmbeddingModelRepository | `backend/src/application/repositories/embedding_model_repository.rs` |
| ImportService | `backend/src/application/services/import_service.rs` |
| SyncService | `backend/src/application/services/sync_service.rs` |
| SearchService | `backend/src/application/services/search_service.rs` |
| **Use Cases** |
| EmbedBlocks | `backend/src/application/use_cases/embed_blocks.rs` |
| SemanticSearch | `backend/src/application/use_cases/semantic_search.rs` |
| UpdateEmbeddings | `backend/src/application/use_cases/update_embeddings.rs` |
| **Infrastructure** |
| SQLite repository | `backend/src/infrastructure/persistence/sqlite_page_repository.rs` |
| File mapping repo | `backend/src/infrastructure/persistence/sqlite_file_mapping_repository.rs` |
| Markdown parser | `backend/src/infrastructure/parsers/logseq_markdown.rs` |
| File watcher | `backend/src/infrastructure/file_system/watcher.rs` |
| Text search index | `backend/src/infrastructure/search/tantivy_index.rs` |
| FastEmbed service | `backend/src/infrastructure/embeddings/fastembed_service.rs` |
| Text preprocessor | `backend/src/infrastructure/embeddings/text_preprocessor.rs` |
| Embedding model manager | `backend/src/infrastructure/embeddings/model_manager.rs` |
| Qdrant vector store | `backend/src/infrastructure/vector_store/qdrant_store.rs` |
| Vector collection manager | `backend/src/infrastructure/vector_store/collection_manager.rs` |
| **Presentation** |
| Tauri commands | `backend/src/tauri/commands/*.rs` |
| DTOs | `backend/src/tauri/dto.rs` |
| DTO mappers | `backend/src/tauri/mappers.rs` |

### Key Type Conversions

```
File System           →  Domain              →  Database          →  Frontend
─────────────            ──────                 ────────             ─────────
PathBuf                  LogseqDirectoryPath   (not stored)         String

/pages/note.md          Page {                 pages:               PageDto {
  Content lines           id: PageId             id: TEXT            id: string
                          title: String          title: TEXT         title: string
                          blocks: HashMap        ↓                   blocks: Array
                        }                      blocks:              }
                                                 page_id: TEXT
                                                 content: TEXT

"https://..."           Url {                  block_urls:          UrlDto {
                          url: String            url: TEXT            url: string
                          domain: Option         domain: TEXT         domain?: string
                        }                                            }

"[[page link]]"         PageReference {        block_page_refs:     PageRefDto {
                          text: String           text: TEXT           text: string
                          type: RefType          type: TEXT           type: "link"
                        }                                            }
```

### Common Operations

#### Create and Save a Page

```rust
// 1. Create page aggregate
let page_id = PageId::new("my-page")?;
let mut page = Page::new(page_id, "My Page".to_string());

// 2. Add blocks
let root_block = Block::new_root(
    BlockId::generate(),
    BlockContent::new("Root content")?,
);
page.add_block(root_block.clone())?;

let child_block = Block::new_child(
    BlockId::generate(),
    BlockContent::new("Child content")?,
    IndentLevel::new(1)?,
    root_block.id().clone(),
);
page.add_block(child_block)?;

// 3. Save to repository
repository.save(page)?;

// 4. Index in search
search_index.index_page(&page)?;
search_index.commit()?;
```

#### Query Pages

```rust
// By ID
let page = repository.find_by_id(&page_id)?;

// By title
let page = repository.find_by_title("My Page")?;

// All pages
let pages = repository.find_all()?;
```

#### Search

**Keyword Search (Tantivy):**

```rust
// Basic search
let results = search_service.search("rust programming", 20)?;

// Fuzzy search (typo-tolerant)
let results = search_service.fuzzy_search("algoritm", 20)?;

// Filter by type
let results = search_service.search_pages("rust", 20)?;  // Pages only
let results = search_service.search_tags("programming", 20)?;  // Tags only
```

**Semantic Search (Embeddings):**

```rust
// Semantic search (understands meaning, not just keywords)
let results = embedding_service.semantic_search(
    "How do I improve performance?",  // Natural language query
    20
)?;

// Returns conceptually similar chunks even without keyword matches
```

**Hybrid Search (Best of Both):**

```rust
// Combine keyword + semantic search with RRF fusion
let results = hybrid_search_service.hybrid_search(
    "optimize database queries",
    20
)?;

// Returns both exact keyword matches AND semantically similar content
```

#### Chunking and Embedding

```rust
// 1. Chunk a page into embeddable pieces
let chunks = block_chunker.chunk_page(&page)?;

// 2. Generate embeddings for each chunk
for chunk in chunks {
    let embedding = embedding_model.encode(&chunk.content)?;
    // embedding = Vec<f32> with 384 dimensions

    // 3. Store in vector database
    vector_repository.insert(VectorRecord {
        chunk_id: chunk.id,
        page_id: chunk.page_id,
        embedding: embedding,
        metadata: chunk.metadata,
    })?;
}

// 4. Query by semantic similarity
let query_embedding = embedding_model.encode("machine learning algorithms")?;
let similar_chunks = vector_repository.search_similar(&query_embedding, 10)?;
```

### Database Schema Summary

```sql
pages
├─ id (PK)
├─ title
├─ created_at
└─ updated_at

blocks
├─ id (PK)
├─ page_id (FK → pages.id, CASCADE)
├─ content
├─ indent_level
├─ parent_id (FK → blocks.id, CASCADE)
├─ position
└─ ...

block_urls
├─ block_id (FK → blocks.id, CASCADE)
├─ url
├─ domain
└─ position

block_page_references
├─ block_id (FK → blocks.id, CASCADE)
├─ reference_text
├─ reference_type ('link' | 'tag')
└─ position

file_page_mappings
├─ file_path (PK)
├─ page_id (FK → pages.id, CASCADE)
├─ file_modified_at
├─ file_size_bytes
└─ checksum
```

### Search Index Schemas

**Tantivy Index (Keyword Search):**

```
Tantivy Index Documents:

PAGE DOC:
  page_id: TEXT (stored)
  page_title: TEXT (indexed, stored)
  document_type: FACET ("/page")

BLOCK DOC:
  page_id: TEXT (stored)
  block_id: TEXT (stored)
  page_title: TEXT (indexed, stored)
  block_content: TEXT (indexed, stored)
  urls: TEXT (indexed)
  page_references: TEXT (indexed)
  document_type: FACET ("/block")
  reference_type: FACET ("/reference/link" or "/reference/tag")
  indent_level: U64 (indexed)
  url_domains: FACET ("/domain/{domain}")
```

**Qdrant Vector Store (Semantic Search):**

```
Collection: logseq_blocks
Vector Config:
  - Size: 384 (all-MiniLM-L6-v2 default)
  - Distance: Cosine Similarity
  - Index: HNSW (Hierarchical Navigable Small World)

Point Structure (matches SemanticSearch.md):
  id: chunk_id (e.g., "block-123-chunk-0")
  vector: [f32; 384]  // Embedding vector
  payload: {
    "chunk_id": "block-123-chunk-0",
    "block_id": "block-123",
    "page_id": "page-456",
    "page_title": "Programming Notes",
    "chunk_index": 0,              // For multi-chunk blocks
    "total_chunks": 1,
    "original_content": "Original block text with [[links]] and #tags",
    "preprocessed_content": "Cleaned text: links and tags",
    "hierarchy_path": ["Parent block", "Current block"],
    "created_at": "2025-10-18T10:00:00Z",
    "updated_at": "2025-10-18T10:00:00Z"
  }

Index Type: HNSW (Approximate Nearest Neighbor)
  - M: 16 (connections per layer)
  - ef_construct: 100 (construction-time accuracy)
  - ef: configurable (search-time accuracy)
```

---

## Architectural Principles

### 1. Separation of Concerns

**Each layer has distinct responsibilities:**

- **Domain:** Business rules, invariants (no I/O, no external libs)
- **Application:** Orchestration, use cases (coordinates domain + infra)
- **Infrastructure:** Technical details (DB, files, HTTP, etc.)
- **Presentation:** User interface, API contracts (DTOs, commands)

### 2. Dependency Inversion

**Depend on abstractions, not implementations:**

```rust
// ✅ GOOD: Service depends on trait
impl<R: PageRepository> ImportService<R> {
    // Works with ANY PageRepository implementation
}

// ❌ BAD: Service depends on concrete type
impl ImportService {
    repository: SqlitePageRepository,  // Tightly coupled!
}
```

### 3. Immutability by Default

**Value objects are immutable:**

```rust
// ✅ GOOD: Update returns new instance
impl FilePathMapping {
    pub fn with_updated_metadata(self, ...) -> Self {
        Self { new_fields, ..self }
    }
}

// ❌ BAD: Mutable value object
impl FilePathMapping {
    pub fn update_metadata(&mut self, ...) {
        self.file_modified_at = ...;  // Violates value object pattern
    }
}
```

### 4. Fail Fast with Validation

**Validate at construction, not usage:**

```rust
// ✅ GOOD: Invalid state is unrepresentable
let url = Url::new("invalid")?;  // Fails here
println!("{}", url.as_str());    // Can't reach if invalid

// ❌ BAD: Validation scattered throughout code
let url = Url { url: "invalid".into() };
if !url.is_valid() { panic!(); }  // Too late!
```

### 5. Explicit Error Handling

**No panics in production code:**

```rust
// ✅ GOOD: Return Result
pub fn parse_file(path: &Path) -> ParseResult<Page> {
    let content = read_to_string(path)?;  // Error propagated
    // ...
}

// ❌ BAD: Panic
pub fn parse_file(path: &Path) -> Page {
    let content = read_to_string(path).unwrap();  // Crashes on error!
    // ...
}
```

---

## Summary

This architecture provides:

- ✅ **Testability:** Mock repositories, in-memory implementations
- ✅ **Maintainability:** Clear boundaries, single responsibility
- ✅ **Flexibility:** Swap implementations (SQLite → Postgres, Tantivy → Meilisearch)
- ✅ **Type Safety:** Rust's type system prevents invalid states
- ✅ **Performance:** Async I/O, bounded concurrency, search indexing
- ✅ **Scalability:** Incremental sync, efficient queries, indexed search

**Next Steps:**

1. **Read feature implementation plans** in `notes/features/`:
   - [`sqlite-persistence.md`](features/sqlite-persistence.md) - SQLite database implementation
   - [`file-page-mapping.md`](features/file-page-mapping.md) - File→Page bidirectional mapping
   - [`tauri-integration.md`](features/tauri-integration.md) - Frontend API and commands
   - [`tantivy-search.md`](features/tantivy-search.md) - Full-text keyword search
   - [`SemanticSearch.md`](features/SemanticSearch.md) - Embeddings and vector search with fastembed-rs

2. **Explore code** starting from `backend/src/domain/`
3. **Run tests:** `cargo test`
4. **Review** IMPLEMENTATION.md for architectural decisions
5. **See examples** in end-to-end workflows above

---

**End of Overview** | Last Updated: 2025-01-19

## Quick Navigation

- **For new developers:** Start with "High-Level Architecture" → "DDD Building Blocks" → Pick a workflow
- **For implementation:** Read relevant feature plan → Check "Code Patterns" → Find files in "Quick Reference"
- **For understanding data flow:** Follow "Workflow 1" (Import) end-to-end with diagrams
- **For search features:** See "Workflow 3" (Keyword) + "Workflow 4" (Semantic) + Hybrid Search section
- **For debugging:** Use layer boundaries to isolate issues, check error conversion chain
