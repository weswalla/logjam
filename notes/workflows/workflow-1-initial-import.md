# Workflow 1: Initial Import

**User Action:** Click "Import Logseq Directory" → Select `/path/to/logseq`

## Flow Diagram

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

## Code Flow (Simplified)

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

## Data Transformations

```
File System           →  Domain              →  Database          →  Vector Store
────────────          ────────                 ────────             ────────────

/pages/my-note.md     Page {                 pages:               Qdrant Collection:
  - Line 1              id: "my-note"       id: "my-note"         "logseq_blocks"
  - Line 2              title: "my-note"    title: "my-note"
    - Nested            blocks: [                                   Point 1:
                          Block {          blocks:                    chunk_id: "block-1-chunk-0"
                            id: "block-1"    id: "block-1"            vector: [0.12, -0.45, 0.89, ...]
                            content: "..."   page_id: "my-note"       payload: {
                            indent: 0        content: "Line 1"          original: "Line 1"
                          },                 indent_level: 0          preprocessed: "Page: my-note. Line 1"
                          Block {                                    }
                            id: "block-2"  blocks:
                            content: "..."   id: "block-2"          Point 2:
                            indent: 1        page_id: "my-note"       chunk_id: "block-2-chunk-0"
                          }                  content: "Nested"        vector: [0.34, 0.21, -0.67, ...]
                        ]                    parent_id: "block-1"     payload: {
                      }                      indent_level: 1          original: "Nested"
                                                                      preprocessed: "Page: my-note. Nested"
                                                                    }
```

**Note:** Embedding generation is optional and can be configured. If disabled, only keyword search (Tantivy) will be available.

## Key Components

### File Discovery
- Recursively scans `pages/` and `journals/` directories
- Filters for `.md` files only
- Returns list of file paths to process

### Parsing
- Reads markdown file content
- Extracts page title from filename
- Parses content into hierarchical blocks
- Extracts URLs and page references from block content
- Builds domain `Page` aggregate with all blocks

### Persistence
- Saves `Page` aggregate to SQLite database
- Creates file-to-page mapping for sync tracking
- Uses database transactions for consistency

### Search Indexing
- **Keyword Search (Tantivy):** Indexes page titles and block content for fast text search
- **Semantic Search (Optional):** Generates embeddings and stores in vector database

### Progress Reporting
- Emits events during processing for UI updates
- Reports files processed, current file, errors encountered
- Allows frontend to show real-time progress

## Error Handling

Import can fail at multiple stages:
- **File System:** Directory doesn't exist, permission denied
- **Parsing:** Invalid markdown, encoding issues
- **Database:** Constraint violations, disk full
- **Search Index:** Index corruption, out of memory

All errors are wrapped and propagated up through the layers, with appropriate error types for the frontend to handle gracefully.
