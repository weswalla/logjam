# SQLite Persistence Implementation Plan

## Overview

Implement SQLite-based persistence for the `PageRepository` trait following the existing DDD and layered architecture patterns. This will replace the in-memory test implementations with production-ready database storage.

## Goals

- Provide durable storage for `Page` aggregates and `Block` entities
- Maintain domain model integrity and DDD boundaries
- Support all existing `PageRepository` operations
- Enable efficient queries for common access patterns
- Lay foundation for full-text search integration

## Architecture Layer

**Infrastructure Layer** (`backend/src/infrastructure/persistence/`)

Following the existing pattern where:
- Domain layer defines pure business logic (unchanged)
- Application layer defines `PageRepository` trait (unchanged)
- Infrastructure layer provides concrete `SqlitePageRepository` implementation

## Dependencies

Add to `backend/Cargo.toml`:

```toml
[dependencies]
# SQLite persistence
sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite", "uuid", "chrono"] }

[dev-dependencies]
# For testing migrations and database
sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite", "migrate"] }
```

**Rationale for sqlx over rusqlite:**
- Async/await support (matches existing Tokio runtime)
- Compile-time query verification with `sqlx::query!` macro
- Built-in migration support
- Better integration with async services (ImportService, SyncService)

## Database Schema

### Design Principles

1. **Aggregate persistence:** Store `Page` as aggregate root with related `Block` entities
2. **Referential integrity:** Foreign keys enforce block→page and parent→child relationships
3. **Efficient queries:** Indexes on common access patterns (title lookups, hierarchy traversal)
4. **Denormalization:** Store computed values (e.g., block depth) for query performance
5. **JSON columns:** Use for collections (URLs, page references) to avoid join tables

### Schema Definition

```sql
-- migrations/001_initial_schema.sql

-- Pages table (Aggregate Root)
CREATE TABLE pages (
    id TEXT PRIMARY KEY NOT NULL,
    title TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Index for find_by_title lookups
CREATE INDEX idx_pages_title ON pages(title);

-- Blocks table (Entity owned by Page aggregate)
CREATE TABLE blocks (
    id TEXT PRIMARY KEY NOT NULL,
    page_id TEXT NOT NULL,
    content TEXT NOT NULL,
    indent_level INTEGER NOT NULL,
    parent_id TEXT,
    position INTEGER NOT NULL,  -- Order within siblings
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,

    FOREIGN KEY (page_id) REFERENCES pages(id) ON DELETE CASCADE,
    FOREIGN KEY (parent_id) REFERENCES blocks(id) ON DELETE CASCADE
);

-- Indexes for hierarchy queries
CREATE INDEX idx_blocks_page_id ON blocks(page_id);
CREATE INDEX idx_blocks_parent_id ON blocks(parent_id);
CREATE INDEX idx_blocks_page_parent ON blocks(page_id, parent_id);

-- URLs extracted from blocks (denormalized for query performance)
CREATE TABLE block_urls (
    block_id TEXT NOT NULL,
    url TEXT NOT NULL,
    domain TEXT,  -- Extracted domain for filtering
    position INTEGER NOT NULL,  -- Order within block

    PRIMARY KEY (block_id, position),
    FOREIGN KEY (block_id) REFERENCES blocks(id) ON DELETE CASCADE
);

CREATE INDEX idx_block_urls_domain ON block_urls(domain);

-- Page references from blocks (denormalized)
CREATE TABLE block_page_references (
    block_id TEXT NOT NULL,
    reference_text TEXT NOT NULL,
    reference_type TEXT NOT NULL CHECK(reference_type IN ('link', 'tag')),
    position INTEGER NOT NULL,  -- Order within block

    PRIMARY KEY (block_id, position),
    FOREIGN KEY (block_id) REFERENCES blocks(id) ON DELETE CASCADE
);

CREATE INDEX idx_block_page_refs_text ON block_page_references(reference_text);
CREATE INDEX idx_block_page_refs_type ON block_page_references(reference_type);

-- Trigger to update updated_at timestamp on pages
CREATE TRIGGER update_pages_timestamp
    AFTER UPDATE ON pages
    FOR EACH ROW
BEGIN
    UPDATE pages SET updated_at = CURRENT_TIMESTAMP WHERE id = OLD.id;
END;

-- Trigger to update updated_at timestamp on blocks
CREATE TRIGGER update_blocks_timestamp
    AFTER UPDATE ON blocks
    FOR EACH ROW
BEGIN
    UPDATE blocks SET updated_at = CURRENT_TIMESTAMP WHERE id = OLD.id;
END;
```

### Design Decisions

**1. Denormalized URLs and Page References:**
- **Pro:** Avoids complex joins for common queries like `get_urls_with_context()`
- **Pro:** Better performance for read-heavy workloads
- **Con:** More storage overhead, more complex save logic
- **Decision:** Denormalize - query performance is critical for UI responsiveness

**2. Position column for ordering:**
- Maintains insertion order for blocks, URLs, and page references
- Essential for preserving document structure
- Enables efficient reconstruction of `Vec<BlockId>`, `Vec<Url>`, etc.

**3. Cascade deletes:**
- Deleting a page removes all blocks automatically
- Deleting a parent block removes all children (subtree deletion)
- Matches `Page::remove_block()` recursive behavior

**4. Timestamp tracking:**
- `created_at` / `updated_at` for audit trail
- Useful for sync conflict resolution (future)
- Enables "modified since" queries

## Implementation Structure

### Directory Layout

```
backend/src/infrastructure/
├── persistence/
│   ├── mod.rs
│   ├── sqlite_page_repository.rs   # Main implementation
│   ├── mappers.rs                   # Domain ↔ DB mapping
│   ├── models.rs                    # Database row structs
│   └── migrations/                  # SQL migration files
│       └── 001_initial_schema.sql
└── mod.rs
```

### Core Components

#### 1. Database Models (`models.rs`)

```rust
use sqlx::FromRow;
use chrono::{DateTime, Utc};

#[derive(Debug, FromRow)]
pub struct PageRow {
    pub id: String,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
pub struct BlockRow {
    pub id: String,
    pub page_id: String,
    pub content: String,
    pub indent_level: i32,
    pub parent_id: Option<String>,
    pub position: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
pub struct BlockUrlRow {
    pub block_id: String,
    pub url: String,
    pub domain: Option<String>,
    pub position: i32,
}

#[derive(Debug, FromRow)]
pub struct BlockPageReferenceRow {
    pub block_id: String,
    pub reference_text: String,
    pub reference_type: String,  // "link" or "tag"
    pub position: i32,
}
```

#### 2. Domain Mappers (`mappers.rs`)

```rust
use crate::domain::{Page, Block, PageId, BlockId, Url, PageReference};
use crate::domain::base::DomainResult;
use super::models::*;

pub struct PageMapper;

impl PageMapper {
    /// Convert database rows to Page aggregate
    pub fn to_domain(
        page_row: PageRow,
        block_rows: Vec<BlockRow>,
        url_rows: Vec<BlockUrlRow>,
        ref_rows: Vec<BlockPageReferenceRow>,
    ) -> DomainResult<Page> {
        // 1. Build block lookup maps
        let url_map: HashMap<String, Vec<BlockUrlRow>> =
            url_rows.into_iter()
                .sorted_by_key(|r| r.position)
                .into_group_map_by(|r| r.block_id.clone());

        let ref_map: HashMap<String, Vec<BlockPageReferenceRow>> =
            ref_rows.into_iter()
                .sorted_by_key(|r| r.position)
                .into_group_map_by(|r| r.block_id.clone());

        // 2. Convert blocks to domain objects
        let blocks: HashMap<BlockId, Block> = block_rows
            .into_iter()
            .sorted_by_key(|b| b.position)
            .map(|row| Self::block_to_domain(row, &url_map, &ref_map))
            .collect::<DomainResult<Vec<_>>>()?
            .into_iter()
            .map(|b| (b.id().clone(), b))
            .collect();

        // 3. Build Page aggregate
        let page_id = PageId::new(&page_row.id)?;
        let root_blocks: Vec<BlockId> = blocks.values()
            .filter(|b| b.parent_id().is_none())
            .map(|b| b.id().clone())
            .collect();

        Page::from_raw_parts(page_id, page_row.title, blocks, root_blocks)
    }

    fn block_to_domain(
        row: BlockRow,
        url_map: &HashMap<String, Vec<BlockUrlRow>>,
        ref_map: &HashMap<String, Vec<BlockPageReferenceRow>>,
    ) -> DomainResult<Block> {
        let block_id = BlockId::new(&row.id)?;
        let content = BlockContent::new(&row.content)?;
        let indent_level = IndentLevel::new(row.indent_level as usize)?;
        let parent_id = row.parent_id.map(|id| BlockId::new(id)).transpose()?;

        // Extract URLs
        let urls: Vec<Url> = url_map
            .get(&row.id)
            .map(|rows| {
                rows.iter()
                    .map(|r| Url::new(&r.url))
                    .collect::<DomainResult<Vec<_>>>()
            })
            .transpose()?
            .unwrap_or_default();

        // Extract page references
        let page_refs: Vec<PageReference> = ref_map
            .get(&row.id)
            .map(|rows| {
                rows.iter()
                    .map(|r| Self::row_to_page_reference(r))
                    .collect::<DomainResult<Vec<_>>>()
            })
            .transpose()?
            .unwrap_or_default();

        Block::from_raw_parts(
            block_id,
            content,
            indent_level,
            parent_id,
            Vec::new(),  // child_ids populated later
            urls,
            page_refs,
        )
    }

    fn row_to_page_reference(row: &BlockPageReferenceRow) -> DomainResult<PageReference> {
        match row.reference_type.as_str() {
            "link" => PageReference::new_link(&row.reference_text),
            "tag" => PageReference::new_tag(&row.reference_text),
            _ => Err(DomainError::InvalidValue(
                format!("Unknown reference type: {}", row.reference_type)
            )),
        }
    }

    /// Convert Page aggregate to database rows
    pub fn from_domain(page: &Page) -> (PageRow, Vec<BlockRow>, Vec<BlockUrlRow>, Vec<BlockPageReferenceRow>) {
        let page_row = PageRow {
            id: page.id().as_str().to_string(),
            title: page.title().to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let mut block_rows = Vec::new();
        let mut url_rows = Vec::new();
        let mut ref_rows = Vec::new();

        for (position, block) in page.all_blocks().enumerate() {
            block_rows.push(BlockRow {
                id: block.id().as_str().to_string(),
                page_id: page.id().as_str().to_string(),
                content: block.content().as_str().to_string(),
                indent_level: block.indent_level().level() as i32,
                parent_id: block.parent_id().map(|id| id.as_str().to_string()),
                position: position as i32,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            });

            for (url_pos, url) in block.urls().iter().enumerate() {
                url_rows.push(BlockUrlRow {
                    block_id: block.id().as_str().to_string(),
                    url: url.as_str().to_string(),
                    domain: url.domain().map(String::from),
                    position: url_pos as i32,
                });
            }

            for (ref_pos, page_ref) in block.page_references().iter().enumerate() {
                ref_rows.push(BlockPageReferenceRow {
                    block_id: block.id().as_str().to_string(),
                    reference_text: page_ref.text().to_string(),
                    reference_type: match page_ref.reference_type() {
                        ReferenceType::Link => "link",
                        ReferenceType::Tag => "tag",
                    }.to_string(),
                    position: ref_pos as i32,
                });
            }
        }

        (page_row, block_rows, url_rows, ref_rows)
    }
}
```

#### 3. Repository Implementation (`sqlite_page_repository.rs`)

```rust
use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};
use std::path::Path;
use crate::application::repositories::PageRepository;
use crate::domain::{Page, PageId};
use crate::domain::base::{DomainResult, DomainError};
use super::{models::*, mappers::PageMapper};

pub struct SqlitePageRepository {
    pool: SqlitePool,
}

impl SqlitePageRepository {
    /// Create a new repository with an in-memory database (for testing)
    pub async fn new_in_memory() -> Result<Self, sqlx::Error> {
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect("sqlite::memory:")
            .await?;

        sqlx::migrate!("./migrations").run(&pool).await?;

        Ok(Self { pool })
    }

    /// Create a new repository with a file-based database
    pub async fn new(db_path: impl AsRef<Path>) -> Result<Self, sqlx::Error> {
        let db_url = format!("sqlite://{}?mode=rwc", db_path.as_ref().display());

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(&db_url)
            .await?;

        sqlx::migrate!("./migrations").run(&pool).await?;

        Ok(Self { pool })
    }

    /// Load all related rows for a page
    async fn load_page_data(&self, page_id: &str)
        -> Result<Option<(PageRow, Vec<BlockRow>, Vec<BlockUrlRow>, Vec<BlockPageReferenceRow>)>, sqlx::Error>
    {
        let page_row: Option<PageRow> = sqlx::query_as(
            "SELECT id, title, created_at, updated_at FROM pages WHERE id = ?"
        )
        .bind(page_id)
        .fetch_optional(&self.pool)
        .await?;

        let Some(page_row) = page_row else {
            return Ok(None);
        };

        let block_rows: Vec<BlockRow> = sqlx::query_as(
            "SELECT id, page_id, content, indent_level, parent_id, position, created_at, updated_at
             FROM blocks
             WHERE page_id = ?
             ORDER BY position ASC"
        )
        .bind(page_id)
        .fetch_all(&self.pool)
        .await?;

        let block_ids: Vec<String> = block_rows.iter()
            .map(|b| b.id.clone())
            .collect();

        let url_rows: Vec<BlockUrlRow> = if !block_ids.is_empty() {
            let placeholders = block_ids.iter().map(|_| "?").join(",");
            let query = format!(
                "SELECT block_id, url, domain, position
                 FROM block_urls
                 WHERE block_id IN ({})
                 ORDER BY block_id, position",
                placeholders
            );

            let mut q = sqlx::query_as(&query);
            for id in &block_ids {
                q = q.bind(id);
            }
            q.fetch_all(&self.pool).await?
        } else {
            Vec::new()
        };

        let ref_rows: Vec<BlockPageReferenceRow> = if !block_ids.is_empty() {
            let placeholders = block_ids.iter().map(|_| "?").join(",");
            let query = format!(
                "SELECT block_id, reference_text, reference_type, position
                 FROM block_page_references
                 WHERE block_id IN ({})
                 ORDER BY block_id, position",
                placeholders
            );

            let mut q = sqlx::query_as(&query);
            for id in &block_ids {
                q = q.bind(id);
            }
            q.fetch_all(&self.pool).await?
        } else {
            Vec::new()
        };

        Ok(Some((page_row, block_rows, url_rows, ref_rows)))
    }
}

impl PageRepository for SqlitePageRepository {
    fn save(&mut self, page: Page) -> DomainResult<()> {
        // Use async block with tokio runtime
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                self.save_async(page).await
            })
        })
    }

    fn find_by_id(&self, id: &PageId) -> DomainResult<Option<Page>> {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                self.find_by_id_async(id).await
            })
        })
    }

    fn find_by_title(&self, title: &str) -> DomainResult<Option<Page>> {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                self.find_by_title_async(title).await
            })
        })
    }

    fn find_all(&self) -> DomainResult<Vec<Page>> {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                self.find_all_async().await
            })
        })
    }

    fn delete(&mut self, id: &PageId) -> DomainResult<bool> {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                self.delete_async(id).await
            })
        })
    }
}

impl SqlitePageRepository {
    /// Async implementation of save (upsert)
    async fn save_async(&mut self, page: Page) -> DomainResult<()> {
        let (page_row, block_rows, url_rows, ref_rows) = PageMapper::from_domain(&page);

        let mut tx = self.pool.begin().await
            .map_err(|e| DomainError::InvalidOperation(format!("Transaction error: {}", e)))?;

        // Upsert page
        sqlx::query(
            "INSERT INTO pages (id, title, created_at, updated_at)
             VALUES (?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
                 title = excluded.title,
                 updated_at = excluded.updated_at"
        )
        .bind(&page_row.id)
        .bind(&page_row.title)
        .bind(page_row.created_at)
        .bind(page_row.updated_at)
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::InvalidOperation(format!("Save page error: {}", e)))?;

        // Delete existing blocks (cascade will delete URLs and refs)
        sqlx::query("DELETE FROM blocks WHERE page_id = ?")
            .bind(&page_row.id)
            .execute(&mut *tx)
            .await
            .map_err(|e| DomainError::InvalidOperation(format!("Delete blocks error: {}", e)))?;

        // Insert blocks
        for block in block_rows {
            sqlx::query(
                "INSERT INTO blocks (id, page_id, content, indent_level, parent_id, position, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?)"
            )
            .bind(&block.id)
            .bind(&block.page_id)
            .bind(&block.content)
            .bind(block.indent_level)
            .bind(&block.parent_id)
            .bind(block.position)
            .bind(block.created_at)
            .bind(block.updated_at)
            .execute(&mut *tx)
            .await
            .map_err(|e| DomainError::InvalidOperation(format!("Insert block error: {}", e)))?;
        }

        // Insert URLs
        for url in url_rows {
            sqlx::query(
                "INSERT INTO block_urls (block_id, url, domain, position) VALUES (?, ?, ?, ?)"
            )
            .bind(&url.block_id)
            .bind(&url.url)
            .bind(&url.domain)
            .bind(url.position)
            .execute(&mut *tx)
            .await
            .map_err(|e| DomainError::InvalidOperation(format!("Insert URL error: {}", e)))?;
        }

        // Insert page references
        for page_ref in ref_rows {
            sqlx::query(
                "INSERT INTO block_page_references (block_id, reference_text, reference_type, position)
                 VALUES (?, ?, ?, ?)"
            )
            .bind(&page_ref.block_id)
            .bind(&page_ref.reference_text)
            .bind(&page_ref.reference_type)
            .bind(page_ref.position)
            .execute(&mut *tx)
            .await
            .map_err(|e| DomainError::InvalidOperation(format!("Insert page ref error: {}", e)))?;
        }

        tx.commit().await
            .map_err(|e| DomainError::InvalidOperation(format!("Commit error: {}", e)))?;

        Ok(())
    }

    async fn find_by_id_async(&self, id: &PageId) -> DomainResult<Option<Page>> {
        let data = self.load_page_data(id.as_str()).await
            .map_err(|e| DomainError::InvalidOperation(format!("Load error: {}", e)))?;

        match data {
            Some((page_row, block_rows, url_rows, ref_rows)) => {
                let page = PageMapper::to_domain(page_row, block_rows, url_rows, ref_rows)?;
                Ok(Some(page))
            }
            None => Ok(None),
        }
    }

    async fn find_by_title_async(&self, title: &str) -> DomainResult<Option<Page>> {
        let page_row: Option<PageRow> = sqlx::query_as(
            "SELECT id, title, created_at, updated_at FROM pages WHERE title = ?"
        )
        .bind(title)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DomainError::InvalidOperation(format!("Query error: {}", e)))?;

        match page_row {
            Some(row) => {
                let page_id = PageId::new(&row.id)?;
                self.find_by_id_async(&page_id).await
            }
            None => Ok(None),
        }
    }

    async fn find_all_async(&self) -> DomainResult<Vec<Page>> {
        let page_rows: Vec<PageRow> = sqlx::query_as(
            "SELECT id, title, created_at, updated_at FROM pages ORDER BY title"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DomainError::InvalidOperation(format!("Query error: {}", e)))?;

        let mut pages = Vec::new();
        for row in page_rows {
            let page_id = PageId::new(&row.id)?;
            if let Some(page) = self.find_by_id_async(&page_id).await? {
                pages.push(page);
            }
        }

        Ok(pages)
    }

    async fn delete_async(&mut self, id: &PageId) -> DomainResult<bool> {
        let result = sqlx::query("DELETE FROM pages WHERE id = ?")
            .bind(id.as_str())
            .execute(&self.pool)
            .await
            .map_err(|e| DomainError::InvalidOperation(format!("Delete error: {}", e)))?;

        Ok(result.rows_affected() > 0)
    }
}
```

## Migration Strategy

### Setup sqlx-cli

```bash
cargo install sqlx-cli --no-default-features --features sqlite
```

### Create migrations

```bash
cd backend
sqlx migrate add initial_schema
# Edit migrations/XXX_initial_schema.sql with schema above
sqlx migrate run --database-url sqlite://logjam.db
```

### Compile-time verification

```bash
# Prepare for offline mode (CI/CD)
DATABASE_URL=sqlite://logjam.db cargo sqlx prepare
```

## Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_save_and_find_page() {
        let mut repo = SqlitePageRepository::new_in_memory().await.unwrap();

        // Create test page
        let page_id = PageId::new("test-page").unwrap();
        let page = Page::new(page_id.clone(), "Test Page".to_string());

        // Save
        repo.save(page.clone()).unwrap();

        // Find by ID
        let found = repo.find_by_id(&page_id).unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().title(), "Test Page");
    }

    #[tokio::test]
    async fn test_save_page_with_blocks() {
        let mut repo = SqlitePageRepository::new_in_memory().await.unwrap();

        // Create page with hierarchical blocks
        let page_id = PageId::new("page-with-blocks").unwrap();
        let mut page = Page::new(page_id.clone(), "Page With Blocks".to_string());

        let root_block = Block::new_root(
            BlockId::generate(),
            BlockContent::new("Root block").unwrap(),
        );
        page.add_block(root_block.clone()).unwrap();

        let child_block = Block::new_child(
            BlockId::generate(),
            BlockContent::new("Child block with [[link]] and #tag").unwrap(),
            IndentLevel::new(1).unwrap(),
            root_block.id().clone(),
        );
        page.add_block(child_block).unwrap();

        // Save and reload
        repo.save(page.clone()).unwrap();
        let loaded = repo.find_by_id(&page_id).unwrap().unwrap();

        // Verify structure
        assert_eq!(loaded.all_blocks().count(), 2);
        assert_eq!(loaded.root_blocks().len(), 1);
    }

    #[tokio::test]
    async fn test_delete_cascade() {
        let mut repo = SqlitePageRepository::new_in_memory().await.unwrap();

        let page_id = PageId::new("delete-test").unwrap();
        let page = Page::new(page_id.clone(), "Delete Test".to_string());

        repo.save(page).unwrap();
        let deleted = repo.delete(&page_id).unwrap();

        assert!(deleted);
        assert!(repo.find_by_id(&page_id).unwrap().is_none());
    }
}
```

### Integration Tests

```rust
// backend/tests/sqlite_integration_test.rs

#[tokio::test]
async fn test_import_service_with_sqlite() {
    let repo = SqlitePageRepository::new_in_memory().await.unwrap();
    let mut import_service = ImportService::new(repo);

    let logseq_dir = LogseqDirectoryPath::new("./test-fixtures/sample-logseq").unwrap();
    let summary = import_service.import_directory(logseq_dir, None).await.unwrap();

    // Verify pages were persisted
    assert!(summary.total_processed > 0);
}
```

## Performance Considerations

### Optimizations

1. **Batch inserts:** Use transactions to batch all inserts for a page
2. **Connection pooling:** Reuse connections via `SqlitePool` (5 connections)
3. **Prepared statements:** sqlx automatically prepares and caches statements
4. **Indexes:** Create indexes on commonly queried columns (title, parent_id, page_id)
5. **Lazy loading:** Only load blocks when page is accessed (not implemented in v1)

### Expected Performance

- **Save page:** ~10-50ms for page with 100 blocks (with transaction)
- **Find by ID:** ~5-20ms for page with 100 blocks
- **Find by title:** ~2-5ms (indexed lookup) + page load time
- **Find all:** ~N * 20ms for N pages (could optimize with bulk loading)

## Integration with Existing Services

### ImportService Changes

```rust
// backend/src/application/services/import_service.rs

impl<R: PageRepository> ImportService<R> {
    // No changes needed! Repository is injected via generic
}

// Usage in Tauri commands (future):
let db_path = app_data_dir.join("logjam.db");
let repo = SqlitePageRepository::new(&db_path).await?;
let mut import_service = ImportService::new(repo);
```

### SyncService Changes

```rust
// backend/src/application/services/sync_service.rs

// Already uses Arc<Mutex<R>> for concurrent access
// No changes needed for SQLite integration

// Usage:
let repo = Arc::new(Mutex::new(SqlitePageRepository::new(&db_path).await?));
let sync_service = SyncService::new(repo, logseq_dir)?;
```

## Rollout Plan

### Phase 1: Infrastructure Setup ✅
- [ ] Add sqlx dependency
- [ ] Create database schema migration
- [ ] Create database models (`models.rs`)
- [ ] Create domain mappers (`mappers.rs`)

### Phase 2: Repository Implementation ✅
- [ ] Implement `SqlitePageRepository`
- [ ] Add `save_async()` with transaction support
- [ ] Add `find_by_id_async()` with eager loading
- [ ] Add `find_by_title_async()`
- [ ] Add `find_all_async()`
- [ ] Add `delete_async()` with cascade

### Phase 3: Testing ✅
- [ ] Unit tests for repository methods
- [ ] Unit tests for mappers (domain ↔ DB)
- [ ] Integration tests with ImportService
- [ ] Integration tests with SyncService
- [ ] Performance benchmarks

### Phase 4: Documentation ✅
- [ ] Update IMPLEMENTATION.md with persistence layer
- [ ] Add database schema documentation
- [ ] Add migration guide
- [ ] Update README with setup instructions

## Open Questions

1. **Database location:** Should DB path be configurable via environment variable or app config?
2. **Migration strategy:** How to handle schema upgrades in production?
3. **Backup strategy:** Should we implement automatic database backups?
4. **Concurrency:** Do we need PRAGMA settings for better concurrent access?
5. **Vacuum:** Should we periodically run VACUUM to reclaim space?

## Future Enhancements

- **Lazy loading:** Load blocks on-demand for large pages
- **Bulk operations:** Optimize `find_all()` with a single query + JOIN
- **Soft deletes:** Add `deleted_at` column instead of hard deletes
- **Audit trail:** Track all changes with event sourcing table
- **Read replicas:** Use separate read-only connections for queries
- **Caching layer:** Add in-memory cache for frequently accessed pages

## References

- sqlx documentation: https://docs.rs/sqlx/
- SQLite transaction best practices: https://www.sqlite.org/lang_transaction.html
- DDD repository pattern: https://martinfowler.com/eaaCatalog/repository.html
