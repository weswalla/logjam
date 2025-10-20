# File→Page Mapping Implementation Plan

## Overview

Implement bidirectional mapping between file system paths and domain `Page` entities to enable proper deletion handling, conflict resolution, and efficient sync operations. This addresses a critical gap in the current implementation where deleted files cannot be properly tracked.

## Problem Statement

### Current Limitations

The existing `SyncService` implementation has several issues:

1. **No deletion tracking:** When a `.md` file is deleted, we don't know which `Page` to remove from the repository
2. **No rename detection:** File renames appear as delete + create, losing page history
3. **No conflict resolution:** Can't detect if file and database are out of sync
4. **Inefficient sync:** Must parse file to determine page title for lookup
5. **No source of truth validation:** Can't verify if a page in DB still has a corresponding file

### Why This Matters

```rust
// Current SyncService behavior on file deletion:
match event.kind {
    FileEventKind::Deleted => {
        // PROBLEM: We don't know which PageId to delete!
        // File path: /path/to/pages/my-note.md
        // Page title: Could be anything (not necessarily "my-note")
        // PageId: Could be UUID or derived from title

        // Current workaround: Just log and ignore
        tracing::warn!("File deleted: {:?}", path);
    }
}
```

## Goals

1. **Enable file deletion sync:** Map file paths to PageIds for proper deletion
2. **Track file metadata:** Store modification times, checksums for conflict detection
3. **Support rename detection:** Recognize when a file moves without losing data
4. **Provide bidirectional lookup:** Find file by page ID and vice versa
5. **Maintain referential integrity:** Ensure mappings stay in sync with repository
6. **Persist mappings:** Store in database alongside pages

## Architecture Layer

**Infrastructure Layer** (`backend/src/infrastructure/persistence/`)

This is infrastructure-level concern because:
- File paths are technical implementation details, not domain concepts
- Mapping is required for infrastructure operations (sync, import)
- Domain layer should remain file-system agnostic

## Design Approach

### Option 1: Separate FileMappingRepository (Recommended)

**Pros:**
- Clean separation of concerns
- Independent of PageRepository implementation
- Easy to add additional metadata (checksums, sync status)
- Can query mappings without loading full pages

**Cons:**
- Additional repository to manage
- Need to keep mappings in sync with pages

### Option 2: Extend Page Domain Model

**Pros:**
- Single source of truth
- Atomic updates (page + mapping)

**Cons:**
- Violates DDD (file paths are not domain concepts)
- Couples domain to infrastructure
- Makes domain objects less portable

**Decision: Option 1** - Use separate repository following DDD principles.

## Database Schema

### New Tables

```sql
-- migrations/002_file_mapping.sql

-- File to page mapping table
CREATE TABLE file_page_mappings (
    file_path TEXT PRIMARY KEY NOT NULL,
    page_id TEXT NOT NULL,
    file_modified_at TIMESTAMP NOT NULL,
    file_size_bytes INTEGER NOT NULL,
    checksum TEXT,  -- SHA-256 hash of file content (optional, for conflict detection)
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,

    FOREIGN KEY (page_id) REFERENCES pages(id) ON DELETE CASCADE
);

-- Index for reverse lookup (page → file)
CREATE INDEX idx_file_mappings_page_id ON file_page_mappings(page_id);

-- Index for sync queries (find stale files)
CREATE INDEX idx_file_mappings_modified ON file_page_mappings(file_modified_at);

-- Trigger to update timestamp
CREATE TRIGGER update_file_mappings_timestamp
    AFTER UPDATE ON file_page_mappings
    FOR EACH ROW
BEGIN
    UPDATE file_page_mappings SET updated_at = CURRENT_TIMESTAMP WHERE file_path = OLD.file_path;
END;
```

### Design Decisions

**1. file_path as primary key:**
- Ensures one-to-one mapping (one file = one page)
- Fast lookup for deletion events
- Natural unique identifier from file system

**2. CASCADE on page deletion:**
- When page is deleted, mapping is automatically removed
- Maintains referential integrity
- Prevents orphaned mappings

**3. Checksum column (optional):**
- SHA-256 hash for content-based conflict detection
- Can detect "file modified externally" scenarios
- Trade-off: Computational cost vs. accuracy

**4. file_modified_at tracking:**
- Used by SyncService to detect stale files
- Enables "sync only modified files" optimization
- Critical for rename detection (unchanged modification time = rename)

## Domain Value Objects

### FilePathMapping (Value Object)

```rust
// backend/src/infrastructure/persistence/value_objects.rs

use std::path::{Path, PathBuf};
use chrono::{DateTime, Utc};
use crate::domain::{PageId, DomainResult, DomainError};

/// Represents a mapping between a file system path and a domain Page
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilePathMapping {
    file_path: PathBuf,
    page_id: PageId,
    file_modified_at: DateTime<Utc>,
    file_size_bytes: u64,
    checksum: Option<String>,
}

impl FilePathMapping {
    pub fn new(
        file_path: impl Into<PathBuf>,
        page_id: PageId,
        file_modified_at: DateTime<Utc>,
        file_size_bytes: u64,
        checksum: Option<String>,
    ) -> DomainResult<Self> {
        let file_path = file_path.into();

        if !file_path.is_absolute() {
            return Err(DomainError::InvalidValue(
                "File path must be absolute".to_string()
            ));
        }

        if file_path.extension().and_then(|s| s.to_str()) != Some("md") {
            return Err(DomainError::InvalidValue(
                "File path must be a .md file".to_string()
            ));
        }

        Ok(Self {
            file_path,
            page_id,
            file_modified_at,
            file_size_bytes,
            checksum,
        })
    }

    pub fn file_path(&self) -> &Path {
        &self.file_path
    }

    pub fn page_id(&self) -> &PageId {
        &self.page_id
    }

    pub fn file_modified_at(&self) -> DateTime<Utc> {
        self.file_modified_at
    }

    pub fn file_size_bytes(&self) -> u64 {
        self.file_size_bytes
    }

    pub fn checksum(&self) -> Option<&str> {
        self.checksum.as_deref()
    }

    /// Check if file metadata has changed (for conflict detection)
    pub fn is_stale(&self, current_modified_at: DateTime<Utc>) -> bool {
        current_modified_at > self.file_modified_at
    }

    /// Update metadata (returns new instance - immutable value object)
    pub fn with_updated_metadata(
        self,
        file_modified_at: DateTime<Utc>,
        file_size_bytes: u64,
        checksum: Option<String>,
    ) -> Self {
        Self {
            file_modified_at,
            file_size_bytes,
            checksum,
            ..self
        }
    }
}

impl ValueObject for FilePathMapping {}
```

## Repository Interface

### FileMappingRepository Trait

```rust
// backend/src/application/repositories/file_mapping_repository.rs

use std::path::Path;
use crate::domain::{PageId, DomainResult};
use crate::infrastructure::persistence::FilePathMapping;

/// Repository for managing file path to page ID mappings
pub trait FileMappingRepository {
    /// Save or update a file mapping
    fn save(&mut self, mapping: FilePathMapping) -> DomainResult<()>;

    /// Find mapping by file path
    fn find_by_path(&self, path: &Path) -> DomainResult<Option<FilePathMapping>>;

    /// Find mapping by page ID
    fn find_by_page_id(&self, page_id: &PageId) -> DomainResult<Option<FilePathMapping>>;

    /// Get all mappings
    fn find_all(&self) -> DomainResult<Vec<FilePathMapping>>;

    /// Delete mapping by file path
    fn delete_by_path(&mut self, path: &Path) -> DomainResult<bool>;

    /// Delete mapping by page ID
    fn delete_by_page_id(&mut self, page_id: &PageId) -> DomainResult<bool>;

    /// Find all files modified after a certain timestamp
    fn find_modified_after(&self, timestamp: DateTime<Utc>) -> DomainResult<Vec<FilePathMapping>>;

    /// Batch operations for efficiency
    fn save_batch(&mut self, mappings: Vec<FilePathMapping>) -> DomainResult<()>;
}
```

## Implementation

### SqliteFileMappingRepository

```rust
// backend/src/infrastructure/persistence/sqlite_file_mapping_repository.rs

use sqlx::{SqlitePool, FromRow};
use std::path::{Path, PathBuf};
use chrono::{DateTime, Utc};
use crate::application::repositories::FileMappingRepository;
use crate::domain::{PageId, DomainResult, DomainError};
use crate::infrastructure::persistence::FilePathMapping;

#[derive(Debug, FromRow)]
struct FileMappingRow {
    file_path: String,
    page_id: String,
    file_modified_at: DateTime<Utc>,
    file_size_bytes: i64,
    checksum: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

pub struct SqliteFileMappingRepository {
    pool: SqlitePool,
}

impl SqliteFileMappingRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    fn row_to_domain(row: FileMappingRow) -> DomainResult<FilePathMapping> {
        let page_id = PageId::new(&row.page_id)?;
        FilePathMapping::new(
            PathBuf::from(row.file_path),
            page_id,
            row.file_modified_at,
            row.file_size_bytes as u64,
            row.checksum,
        )
    }

    async fn save_async(&mut self, mapping: FilePathMapping) -> DomainResult<()> {
        sqlx::query(
            "INSERT INTO file_page_mappings
             (file_path, page_id, file_modified_at, file_size_bytes, checksum, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(file_path) DO UPDATE SET
                 page_id = excluded.page_id,
                 file_modified_at = excluded.file_modified_at,
                 file_size_bytes = excluded.file_size_bytes,
                 checksum = excluded.checksum,
                 updated_at = excluded.updated_at"
        )
        .bind(mapping.file_path().to_string_lossy().as_ref())
        .bind(mapping.page_id().as_str())
        .bind(mapping.file_modified_at())
        .bind(mapping.file_size_bytes() as i64)
        .bind(mapping.checksum())
        .bind(Utc::now())
        .bind(Utc::now())
        .execute(&self.pool)
        .await
        .map_err(|e| DomainError::InvalidOperation(format!("Save mapping error: {}", e)))?;

        Ok(())
    }

    async fn find_by_path_async(&self, path: &Path) -> DomainResult<Option<FilePathMapping>> {
        let row: Option<FileMappingRow> = sqlx::query_as(
            "SELECT file_path, page_id, file_modified_at, file_size_bytes, checksum, created_at, updated_at
             FROM file_page_mappings
             WHERE file_path = ?"
        )
        .bind(path.to_string_lossy().as_ref())
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DomainError::InvalidOperation(format!("Query error: {}", e)))?;

        row.map(Self::row_to_domain).transpose()
    }

    async fn find_by_page_id_async(&self, page_id: &PageId) -> DomainResult<Option<FilePathMapping>> {
        let row: Option<FileMappingRow> = sqlx::query_as(
            "SELECT file_path, page_id, file_modified_at, file_size_bytes, checksum, created_at, updated_at
             FROM file_page_mappings
             WHERE page_id = ?"
        )
        .bind(page_id.as_str())
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DomainError::InvalidOperation(format!("Query error: {}", e)))?;

        row.map(Self::row_to_domain).transpose()
    }

    async fn find_all_async(&self) -> DomainResult<Vec<FilePathMapping>> {
        let rows: Vec<FileMappingRow> = sqlx::query_as(
            "SELECT file_path, page_id, file_modified_at, file_size_bytes, checksum, created_at, updated_at
             FROM file_page_mappings
             ORDER BY file_path"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DomainError::InvalidOperation(format!("Query error: {}", e)))?;

        rows.into_iter()
            .map(Self::row_to_domain)
            .collect()
    }

    async fn delete_by_path_async(&mut self, path: &Path) -> DomainResult<bool> {
        let result = sqlx::query("DELETE FROM file_page_mappings WHERE file_path = ?")
            .bind(path.to_string_lossy().as_ref())
            .execute(&self.pool)
            .await
            .map_err(|e| DomainError::InvalidOperation(format!("Delete error: {}", e)))?;

        Ok(result.rows_affected() > 0)
    }

    async fn delete_by_page_id_async(&mut self, page_id: &PageId) -> DomainResult<bool> {
        let result = sqlx::query("DELETE FROM file_page_mappings WHERE page_id = ?")
            .bind(page_id.as_str())
            .execute(&self.pool)
            .await
            .map_err(|e| DomainError::InvalidOperation(format!("Delete error: {}", e)))?;

        Ok(result.rows_affected() > 0)
    }

    async fn find_modified_after_async(&self, timestamp: DateTime<Utc>) -> DomainResult<Vec<FilePathMapping>> {
        let rows: Vec<FileMappingRow> = sqlx::query_as(
            "SELECT file_path, page_id, file_modified_at, file_size_bytes, checksum, created_at, updated_at
             FROM file_page_mappings
             WHERE file_modified_at > ?
             ORDER BY file_modified_at DESC"
        )
        .bind(timestamp)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DomainError::InvalidOperation(format!("Query error: {}", e)))?;

        rows.into_iter()
            .map(Self::row_to_domain)
            .collect()
    }

    async fn save_batch_async(&mut self, mappings: Vec<FilePathMapping>) -> DomainResult<()> {
        let mut tx = self.pool.begin().await
            .map_err(|e| DomainError::InvalidOperation(format!("Transaction error: {}", e)))?;

        for mapping in mappings {
            sqlx::query(
                "INSERT INTO file_page_mappings
                 (file_path, page_id, file_modified_at, file_size_bytes, checksum, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?)
                 ON CONFLICT(file_path) DO UPDATE SET
                     page_id = excluded.page_id,
                     file_modified_at = excluded.file_modified_at,
                     file_size_bytes = excluded.file_size_bytes,
                     checksum = excluded.checksum,
                     updated_at = excluded.updated_at"
            )
            .bind(mapping.file_path().to_string_lossy().as_ref())
            .bind(mapping.page_id().as_str())
            .bind(mapping.file_modified_at())
            .bind(mapping.file_size_bytes() as i64)
            .bind(mapping.checksum())
            .bind(Utc::now())
            .bind(Utc::now())
            .execute(&mut *tx)
            .await
            .map_err(|e| DomainError::InvalidOperation(format!("Batch insert error: {}", e)))?;
        }

        tx.commit().await
            .map_err(|e| DomainError::InvalidOperation(format!("Commit error: {}", e)))?;

        Ok(())
    }
}

impl FileMappingRepository for SqliteFileMappingRepository {
    fn save(&mut self, mapping: FilePathMapping) -> DomainResult<()> {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                self.save_async(mapping).await
            })
        })
    }

    fn find_by_path(&self, path: &Path) -> DomainResult<Option<FilePathMapping>> {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                self.find_by_path_async(path).await
            })
        })
    }

    fn find_by_page_id(&self, page_id: &PageId) -> DomainResult<Option<FilePathMapping>> {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                self.find_by_page_id_async(page_id).await
            })
        })
    }

    fn find_all(&self) -> DomainResult<Vec<FilePathMapping>> {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                self.find_all_async().await
            })
        })
    }

    fn delete_by_path(&mut self, path: &Path) -> DomainResult<bool> {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                self.delete_by_path_async(path).await
            })
        })
    }

    fn delete_by_page_id(&mut self, page_id: &PageId) -> DomainResult<bool> {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                self.delete_by_page_id_async(page_id).await
            })
        })
    }

    fn find_modified_after(&self, timestamp: DateTime<Utc>) -> DomainResult<Vec<FilePathMapping>> {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                self.find_modified_after_async(timestamp).await
            })
        })
    }

    fn save_batch(&mut self, mappings: Vec<FilePathMapping>) -> DomainResult<()> {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                self.save_batch_async(mappings).await
            })
        })
    }
}
```

## Service Integration

### Updated ImportService

```rust
// backend/src/application/services/import_service.rs

pub struct ImportService<P: PageRepository, M: FileMappingRepository> {
    page_repository: P,
    mapping_repository: M,
    max_concurrent_files: usize,
}

impl<P: PageRepository, M: FileMappingRepository> ImportService<P, M> {
    pub fn new(page_repository: P, mapping_repository: M) -> Self {
        Self {
            page_repository,
            mapping_repository,
            max_concurrent_files: 4,
        }
    }

    async fn process_file(&mut self, path: PathBuf) -> ImportResult<()> {
        // Parse file
        let page = LogseqMarkdownParser::parse_file(&path).await?;
        let page_id = page.id().clone();

        // Get file metadata
        let metadata = tokio::fs::metadata(&path).await?;
        let modified_at = metadata.modified()?
            .duration_since(UNIX_EPOCH)?
            .as_secs();
        let file_size = metadata.len();

        // Save page
        self.page_repository.save(page)?;

        // Save file mapping
        let mapping = FilePathMapping::new(
            path,
            page_id,
            DateTime::from_timestamp(modified_at as i64, 0).unwrap(),
            file_size,
            None,  // Checksum optional for v1
        )?;
        self.mapping_repository.save(mapping)?;

        Ok(())
    }
}
```

### Updated SyncService

```rust
// backend/src/application/services/sync_service.rs

pub struct SyncService<P: PageRepository, M: FileMappingRepository> {
    page_repository: Arc<Mutex<P>>,
    mapping_repository: Arc<Mutex<M>>,
    directory_path: LogseqDirectoryPath,
    watcher: LogseqFileWatcher,
}

impl<P: PageRepository + Send + 'static, M: FileMappingRepository + Send + 'static>
    SyncService<P, M>
{
    pub fn new(
        page_repository: Arc<Mutex<P>>,
        mapping_repository: Arc<Mutex<M>>,
        directory_path: LogseqDirectoryPath,
    ) -> SyncResult<Self> {
        let watcher = LogseqFileWatcher::new(directory_path.as_path(), Duration::from_millis(500))?;

        Ok(Self {
            page_repository,
            mapping_repository,
            directory_path,
            watcher,
        })
    }

    async fn handle_file_deleted(&self, path: PathBuf) -> SyncResult<()> {
        let mut mapping_repo = self.mapping_repository.lock().await;
        let mut page_repo = self.page_repository.lock().await;

        // Find mapping for deleted file
        if let Some(mapping) = mapping_repo.find_by_path(&path)? {
            let page_id = mapping.page_id().clone();

            // Delete page from repository
            page_repo.delete(&page_id)?;

            // Delete mapping
            mapping_repo.delete_by_path(&path)?;

            tracing::info!("Deleted page {} for file {:?}", page_id.as_str(), path);
        } else {
            tracing::warn!("No mapping found for deleted file: {:?}", path);
        }

        Ok(())
    }

    async fn handle_file_created(&self, path: PathBuf) -> SyncResult<()> {
        // Check if mapping already exists (rename detection)
        let mapping_repo = self.mapping_repository.lock().await;
        if let Some(_existing) = mapping_repo.find_by_path(&path)? {
            drop(mapping_repo);
            // File already tracked - treat as update
            return self.handle_file_updated(path).await;
        }
        drop(mapping_repo);

        // Parse and save new file
        let page = LogseqMarkdownParser::parse_file(&path).await?;
        let page_id = page.id().clone();

        // Get file metadata
        let metadata = tokio::fs::metadata(&path).await?;
        let modified_at = DateTime::from_timestamp(
            metadata.modified()?.duration_since(UNIX_EPOCH)?.as_secs() as i64,
            0
        ).unwrap();

        // Save page
        let mut page_repo = self.page_repository.lock().await;
        page_repo.save(page)?;
        drop(page_repo);

        // Save mapping
        let mut mapping_repo = self.mapping_repository.lock().await;
        let mapping = FilePathMapping::new(
            path,
            page_id,
            modified_at,
            metadata.len(),
            None,
        )?;
        mapping_repo.save(mapping)?;

        Ok(())
    }

    async fn handle_file_updated(&self, path: PathBuf) -> SyncResult<()> {
        // Get existing mapping
        let mapping_repo = self.mapping_repository.lock().await;
        let existing_mapping = mapping_repo.find_by_path(&path)?;
        drop(mapping_repo);

        // Get current file metadata
        let metadata = tokio::fs::metadata(&path).await?;
        let current_modified = DateTime::from_timestamp(
            metadata.modified()?.duration_since(UNIX_EPOCH)?.as_secs() as i64,
            0
        ).unwrap();

        // Check if file actually changed
        if let Some(mapping) = &existing_mapping {
            if !mapping.is_stale(current_modified) {
                tracing::debug!("File not modified, skipping: {:?}", path);
                return Ok(());
            }
        }

        // Parse updated file
        let page = LogseqMarkdownParser::parse_file(&path).await?;
        let page_id = page.id().clone();

        // Save page
        let mut page_repo = self.page_repository.lock().await;
        page_repo.save(page)?;
        drop(page_repo);

        // Update mapping
        let mut mapping_repo = self.mapping_repository.lock().await;
        let mapping = FilePathMapping::new(
            path,
            page_id,
            current_modified,
            metadata.len(),
            None,
        )?;
        mapping_repo.save(mapping)?;

        Ok(())
    }
}
```

## Rename Detection (Advanced)

### Strategy

Detect file renames by comparing:
1. **File size** (unchanged for rename)
2. **Modification time** (unchanged for rename)
3. **Content checksum** (if enabled)

```rust
impl<P, M> SyncService<P, M>
where
    P: PageRepository + Send + 'static,
    M: FileMappingRepository + Send + 'static,
{
    async fn detect_rename(&self, new_path: PathBuf) -> SyncResult<Option<PageId>> {
        let metadata = tokio::fs::metadata(&new_path).await?;
        let size = metadata.len();
        let modified = DateTime::from_timestamp(
            metadata.modified()?.duration_since(UNIX_EPOCH)?.as_secs() as i64,
            0
        ).unwrap();

        // Find all mappings with same size and modification time
        let mapping_repo = self.mapping_repository.lock().await;
        let all_mappings = mapping_repo.find_all()?;

        for mapping in all_mappings {
            // Check if mapping's file no longer exists
            if !mapping.file_path().exists() {
                // Same size and modification time = likely a rename
                if mapping.file_size_bytes() == size
                    && mapping.file_modified_at() == modified
                {
                    tracing::info!(
                        "Detected rename: {:?} -> {:?}",
                        mapping.file_path(),
                        new_path
                    );
                    return Ok(Some(mapping.page_id().clone()));
                }
            }
        }

        Ok(None)
    }

    async fn handle_file_created_with_rename_detection(&self, path: PathBuf) -> SyncResult<()> {
        // Try to detect rename
        if let Some(page_id) = self.detect_rename(path.clone()).await? {
            // This is a rename - update mapping
            let mut mapping_repo = self.mapping_repository.lock().await;

            // Delete old mapping
            mapping_repo.delete_by_page_id(&page_id)?;

            // Create new mapping
            let metadata = tokio::fs::metadata(&path).await?;
            let mapping = FilePathMapping::new(
                path,
                page_id,
                DateTime::from_timestamp(
                    metadata.modified()?.duration_since(UNIX_EPOCH)?.as_secs() as i64,
                    0
                ).unwrap(),
                metadata.len(),
                None,
            )?;
            mapping_repo.save(mapping)?;

            tracing::info!("Updated mapping for renamed file");
            return Ok(());
        }

        // Not a rename - treat as new file
        self.handle_file_created(path).await
    }
}
```

## Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_save_and_find_mapping() {
        let pool = create_test_pool().await;
        let mut repo = SqliteFileMappingRepository::new(pool);

        let page_id = PageId::new("test-page").unwrap();
        let mapping = FilePathMapping::new(
            "/path/to/test.md",
            page_id.clone(),
            Utc::now(),
            1024,
            None,
        ).unwrap();

        repo.save(mapping.clone()).unwrap();

        let found = repo.find_by_path(Path::new("/path/to/test.md")).unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().page_id(), &page_id);
    }

    #[tokio::test]
    async fn test_delete_cascade() {
        // When page is deleted, mapping should also be deleted
        let pool = create_test_pool().await;
        let mut page_repo = SqlitePageRepository::new(pool.clone());
        let mut mapping_repo = SqliteFileMappingRepository::new(pool);

        let page_id = PageId::new("test").unwrap();
        let page = Page::new(page_id.clone(), "Test".to_string());
        page_repo.save(page).unwrap();

        let mapping = FilePathMapping::new(
            "/path/test.md",
            page_id.clone(),
            Utc::now(),
            100,
            None,
        ).unwrap();
        mapping_repo.save(mapping).unwrap();

        // Delete page
        page_repo.delete(&page_id).unwrap();

        // Mapping should be gone
        let found = mapping_repo.find_by_page_id(&page_id).unwrap();
        assert!(found.is_none());
    }
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_sync_handles_file_deletion() {
    let page_repo = Arc::new(Mutex::new(SqlitePageRepository::new_in_memory().await.unwrap()));
    let mapping_repo = Arc::new(Mutex::new(SqliteFileMappingRepository::new_in_memory().await.unwrap()));

    let logseq_dir = create_test_logseq_dir();
    let sync_service = SyncService::new(page_repo.clone(), mapping_repo.clone(), logseq_dir).unwrap();

    // Create and sync a file
    let test_file = logseq_dir.join("pages/test.md");
    create_test_file(&test_file, "# Test");
    sync_service.sync_once(None).await.unwrap();

    // Verify page and mapping exist
    let page_repo_lock = page_repo.lock().await;
    let pages = page_repo_lock.find_all().unwrap();
    assert_eq!(pages.len(), 1);
    drop(page_repo_lock);

    // Delete file
    fs::remove_file(&test_file).unwrap();
    sync_service.sync_once(None).await.unwrap();

    // Verify page and mapping are deleted
    let page_repo_lock = page_repo.lock().await;
    let pages = page_repo_lock.find_all().unwrap();
    assert_eq!(pages.len(), 0);
}
```

## Performance Considerations

### Optimizations

1. **Batch inserts during import:** Use `save_batch()` instead of individual saves
2. **Index on page_id:** Fast reverse lookups
3. **Index on file_modified_at:** Efficient "find stale files" queries
4. **Avoid full table scans:** Use targeted queries with WHERE clauses

### Expected Performance

- **Save mapping:** ~1-2ms
- **Find by path:** ~1ms (indexed)
- **Find by page_id:** ~1-2ms (indexed)
- **Batch save (1000 mappings):** ~100-200ms (transaction)

## Rollout Plan

### Phase 1: Foundation ✅
- [ ] Add `FilePathMapping` value object
- [ ] Create database migration for `file_page_mappings` table
- [ ] Define `FileMappingRepository` trait
- [ ] Implement `SqliteFileMappingRepository`

### Phase 2: Service Integration ✅
- [ ] Update `ImportService` to save mappings
- [ ] Update `SyncService` to use mappings for deletion
- [ ] Add mapping updates for file create/update events
- [ ] Add conflict detection using `is_stale()`

### Phase 3: Advanced Features 🚀
- [ ] Implement rename detection algorithm
- [ ] Add checksum calculation (SHA-256)
- [ ] Add checksum-based conflict resolution
- [ ] Add "orphan cleanup" job (mappings without files)

### Phase 4: Testing & Documentation ✅
- [ ] Unit tests for repository
- [ ] Integration tests for sync with deletions
- [ ] Integration tests for rename detection
- [ ] Update documentation

## Open Questions

1. **Checksum performance:** Should checksums be calculated on-demand or stored?
2. **Rename detection threshold:** How confident do we need to be before treating create as rename?
3. **Orphan cleanup:** Should we automatically delete mappings for missing files, or alert user?
4. **Conflict resolution UI:** How to present file vs. DB conflicts to user?
5. **Multi-device sync:** How will file mappings work across different machines?

## Future Enhancements

- **Content-based checksums:** SHA-256 hashing for conflict detection
- **Move detection:** Track directory renames (e.g., `pages/` → `archive/`)
- **Symbolic links:** Handle symlinks to files outside Logseq directory
- **Sync status tracking:** Add `sync_status` column (synced, conflict, deleted)
- **Conflict resolution UI:** Present conflicts to user with merge options
- **Audit log:** Track all file→page mapping changes for debugging

## References

- Git rename detection: https://github.com/git/git/blob/master/diffcore-rename.c
- SQLite foreign key constraints: https://www.sqlite.org/foreignkeys.html
- DDD repository pattern: https://martinfowler.com/eaaCatalog/repository.html
