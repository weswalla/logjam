/// Sync service for keeping Logseq directory in sync with changes
use crate::application::repositories::PageRepository;
use crate::domain::base::Entity;
use crate::domain::value_objects::LogseqDirectoryPath;
use crate::infrastructure::file_system::{discover_logseq_files, FileEvent, FileEventKind, LogseqFileWatcher};
use crate::infrastructure::parsers::LogseqMarkdownParser;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use thiserror::Error;
use tokio::sync::Mutex;

#[derive(Error, Debug)]
pub enum SyncError {
    #[error("File system error: {0}")]
    FileSystem(#[from] std::io::Error),

    #[error("Parse error: {0}")]
    Parse(#[from] crate::infrastructure::parsers::ParseError),

    #[error("Repository error: {0}")]
    Repository(#[from] crate::domain::base::DomainError),

    #[error("Watcher error: {0}")]
    Watcher(#[from] crate::infrastructure::file_system::WatcherError),
}

pub type SyncResult<T> = Result<T, SyncError>;

/// Callback type for sync events
pub type SyncCallback = Arc<dyn Fn(SyncEvent) + Send + Sync>;

/// Sync event types
#[derive(Debug, Clone)]
pub enum SyncEvent {
    SyncStarted,
    FileCreated { file_path: PathBuf },
    FileUpdated { file_path: PathBuf },
    FileDeleted { file_path: PathBuf },
    SyncCompleted { files_created: usize, files_updated: usize, files_deleted: usize },
    Error { file_path: PathBuf, error: String },
}

/// Summary of a one-time sync operation
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncSummary {
    pub files_created: usize,
    pub files_updated: usize,
    pub files_deleted: usize,
    pub files_unchanged: usize,
    pub errors: Vec<(PathBuf, String)>,
}

/// Operation to perform during sync
#[derive(Debug)]
enum SyncOperation {
    Create(PathBuf),
    Update(PathBuf),
    Delete(PathBuf),
}

/// Metadata about a synced file
#[derive(Debug, Clone)]
struct FileMetadata {
    title: String,
    last_modified: SystemTime,
}

/// Service for syncing Logseq directory changes
pub struct SyncService<R: PageRepository> {
    repository: Arc<Mutex<R>>,
    directory_path: LogseqDirectoryPath,
    watcher: LogseqFileWatcher,
    debounce_duration: Duration,
    /// Tracks files that have been synced with their metadata
    sync_registry: Arc<Mutex<HashMap<PathBuf, FileMetadata>>>,
}

impl<R: PageRepository + Send + 'static> SyncService<R> {
    /// Create a new sync service
    pub fn new(
        repository: R,
        directory_path: LogseqDirectoryPath,
        debounce_duration: Option<Duration>,
    ) -> SyncResult<Self> {
        let debounce = debounce_duration.unwrap_or(Duration::from_millis(500));

        let watcher = LogseqFileWatcher::new(directory_path.as_path(), debounce)?;

        Ok(SyncService {
            repository: Arc::new(Mutex::new(repository)),
            directory_path,
            watcher,
            debounce_duration: debounce,
            sync_registry: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    /// Perform a one-time sync of the directory
    ///
    /// This method:
    /// 1. Discovers all markdown files in pages/ and journals/
    /// 2. Detects new files, updated files (by comparing modification time), and deleted files
    /// 3. Syncs changes to the repository
    /// 4. Returns a summary of the sync operation
    pub async fn sync_once(&self, callback: Option<SyncCallback>) -> SyncResult<SyncSummary> {
        tracing::info!("Starting one-time sync for {:?}", self.directory_path);

        if let Some(ref cb) = callback {
            cb(SyncEvent::SyncStarted);
        }

        let mut summary = SyncSummary {
            files_created: 0,
            files_updated: 0,
            files_deleted: 0,
            files_unchanged: 0,
            errors: Vec::new(),
        };

        // Discover all current files in the directory
        let current_files = discover_logseq_files(self.directory_path.as_path()).await?;
        let current_files_set: HashSet<PathBuf> = current_files.iter().cloned().collect();

        // Process each discovered file
        for file_path in current_files {
            match self.sync_file(&file_path, &mut summary, callback.as_ref()).await {
                Ok(_) => {}
                Err(e) => {
                    let error_msg = e.to_string();
                    tracing::error!("Failed to sync {}: {}", file_path.display(), error_msg);
                    summary.errors.push((file_path.clone(), error_msg.clone()));

                    if let Some(ref cb) = callback {
                        cb(SyncEvent::Error {
                            file_path,
                            error: error_msg,
                        });
                    }
                }
            }
        }

        // Handle deletions: files in registry but not in current_files
        let deleted_count = self.handle_deletions(&current_files_set, callback.as_ref()).await?;
        summary.files_deleted = deleted_count;

        // Emit completion event
        if let Some(ref cb) = callback {
            cb(SyncEvent::SyncCompleted {
                files_created: summary.files_created,
                files_updated: summary.files_updated,
                files_deleted: summary.files_deleted,
            });
        }

        tracing::info!(
            "One-time sync completed: {} created, {} updated, {} deleted, {} unchanged, {} errors",
            summary.files_created,
            summary.files_updated,
            summary.files_deleted,
            summary.files_unchanged,
            summary.errors.len()
        );

        Ok(summary)
    }

    /// Sync a single file, determining if it's new, updated, or unchanged
    async fn sync_file(
        &self,
        file_path: &PathBuf,
        summary: &mut SyncSummary,
        callback: Option<&SyncCallback>,
    ) -> SyncResult<()> {
        // Get file metadata
        let file_meta = tokio::fs::metadata(file_path).await?;
        let modified = file_meta.modified()?;

        // Extract title from filename
        let title = file_path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("Invalid filename: {}", file_path.display())
            ))?
            .to_string();

        // Check sync registry to determine if file needs syncing
        let mut registry = self.sync_registry.lock().await;
        let needs_sync = if let Some(metadata) = registry.get(file_path) {
            // File was previously synced, check if it changed
            modified > metadata.last_modified
        } else {
            // New file
            true
        };

        if needs_sync {
            // Check if page already exists in repository (for determining create vs update)
            let repo = self.repository.lock().await;
            let existing_page = repo.find_by_title(&title)?;
            drop(repo); // Release lock before parsing

            // Parse the file
            let page = LogseqMarkdownParser::parse_file(file_path).await?;

            // Save to repository
            let mut repo = self.repository.lock().await;
            repo.save(page)?;
            drop(repo); // Release lock

            // Update registry
            registry.insert(file_path.clone(), FileMetadata {
                title: title.clone(),
                last_modified: modified,
            });

            // Update summary and emit event
            if existing_page.is_some() {
                summary.files_updated += 1;
                if let Some(cb) = callback {
                    cb(SyncEvent::FileUpdated { file_path: file_path.clone() });
                }
            } else {
                summary.files_created += 1;
                if let Some(cb) = callback {
                    cb(SyncEvent::FileCreated { file_path: file_path.clone() });
                }
            }
        } else {
            summary.files_unchanged += 1;
        }

        Ok(())
    }

    /// Handle deleted files by removing them from repository and registry
    async fn handle_deletions(
        &self,
        current_files: &HashSet<PathBuf>,
        callback: Option<&SyncCallback>,
    ) -> SyncResult<usize> {
        let mut deleted_count = 0;
        let mut registry = self.sync_registry.lock().await;

        // Find files in registry that are no longer in the directory
        let to_delete: Vec<PathBuf> = registry
            .keys()
            .filter(|path| !current_files.contains(*path))
            .cloned()
            .collect();

        for file_path in to_delete {
            if let Some(metadata) = registry.remove(&file_path) {
                // Try to delete from repository using the title
                let mut repo = self.repository.lock().await;
                if let Ok(Some(page)) = repo.find_by_title(&metadata.title) {
                    let page_id = page.id().clone();
                    if repo.delete(&page_id).is_ok() {
                        deleted_count += 1;

                        if let Some(cb) = callback {
                            cb(SyncEvent::FileDeleted { file_path: file_path.clone() });
                        }

                        tracing::info!("Deleted page '{}' (file: {})", metadata.title, file_path.display());
                    }
                }
                drop(repo); // Release lock
            }
        }

        Ok(deleted_count)
    }

    /// Start watching for file changes and sync them
    /// This runs indefinitely until cancelled
    pub async fn start_watching(
        &self,
        callback: Option<SyncCallback>,
    ) -> SyncResult<()> {
        tracing::info!("Starting file watcher for {:?}", self.directory_path);

        if let Some(ref cb) = callback {
            cb(SyncEvent::SyncStarted);
        }

        loop {
            // Wait for file events (blocking)
            if let Some(events) = self.watcher.recv() {
                self.process_events(events, callback.clone()).await?;
            }

            // Small delay to prevent busy waiting
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    /// Process a batch of file events
    async fn process_events(
        &self,
        events: Vec<FileEvent>,
        callback: Option<SyncCallback>,
    ) -> SyncResult<()> {
        let mut stats = SyncStats::default();

        for event in events {
            let operation = match event.kind {
                FileEventKind::Created => SyncOperation::Create(event.path.clone()),
                FileEventKind::Modified => SyncOperation::Update(event.path.clone()),
                FileEventKind::Deleted => SyncOperation::Delete(event.path.clone()),
            };

            match self.process_operation(operation, callback.as_ref()).await {
                Ok(op_type) => {
                    match op_type {
                        FileEventKind::Created => stats.files_created += 1,
                        FileEventKind::Modified => stats.files_updated += 1,
                        FileEventKind::Deleted => stats.files_deleted += 1,
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to sync {}: {}", event.path.display(), e);
                    if let Some(ref cb) = callback {
                        cb(SyncEvent::Error {
                            file_path: event.path.clone(),
                            error: e.to_string(),
                        });
                    }
                }
            }
        }

        // Emit completion event
        if let Some(ref cb) = callback {
            cb(SyncEvent::SyncCompleted {
                files_created: stats.files_created,
                files_updated: stats.files_updated,
                files_deleted: stats.files_deleted,
            });
        }

        Ok(())
    }

    /// Process a single sync operation
    async fn process_operation(
        &self,
        operation: SyncOperation,
        callback: Option<&SyncCallback>,
    ) -> SyncResult<FileEventKind> {
        match &operation {
            SyncOperation::Create(path) | SyncOperation::Update(path) => {
                // Parse the file
                let page = LogseqMarkdownParser::parse_file(path).await?;

                // Save to repository
                let mut repo = self.repository.lock().await;
                repo.save(page)?;

                // Emit event and determine result based on operation type
                let is_create = matches!(operation, SyncOperation::Create(_));

                if let Some(cb) = callback {
                    if is_create {
                        cb(SyncEvent::FileCreated { file_path: path.clone() });
                    } else {
                        cb(SyncEvent::FileUpdated { file_path: path.clone() });
                    }
                }

                Ok(if is_create {
                    FileEventKind::Created
                } else {
                    FileEventKind::Modified
                })
            }

            SyncOperation::Delete(path) => {
                // For deletion, we'd need to maintain a mapping from file paths to page IDs
                // For now, we'll just log it
                tracing::info!("File deleted: {}", path.display());

                // In a full implementation, you'd:
                // 1. Look up the page ID from the file path (requires a file->page mapping)
                // 2. Delete from repository
                // For now, we just emit the event

                if let Some(cb) = callback {
                    cb(SyncEvent::FileDeleted { file_path: path.clone() });
                }

                Ok(FileEventKind::Deleted)
            }
        }
    }
}

#[derive(Default)]
struct SyncStats {
    files_created: usize,
    files_updated: usize,
    files_deleted: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::repositories::PageRepository;
    use crate::domain::aggregates::Page;
    use crate::domain::base::DomainResult;
    use crate::domain::value_objects::PageId;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};
    use tempfile::TempDir;

    #[test]
    fn test_sync_stats() {
        let stats = SyncStats::default();
        assert_eq!(stats.files_created, 0);
        assert_eq!(stats.files_updated, 0);
        assert_eq!(stats.files_deleted, 0);
    }

    // Mock repository for testing
    #[derive(Clone)]
    struct MockRepository {
        pages: Arc<std::sync::Mutex<HashMap<String, Page>>>,
    }

    impl MockRepository {
        fn new() -> Self {
            Self {
                pages: Arc::new(std::sync::Mutex::new(HashMap::new())),
            }
        }
    }

    impl PageRepository for MockRepository {
        fn save(&mut self, page: Page) -> DomainResult<()> {
            let title = page.title().to_string();
            let mut pages = self.pages.lock().unwrap();
            pages.insert(title, page);
            Ok(())
        }

        fn find_by_id(&self, id: &PageId) -> DomainResult<Option<Page>> {
            let pages = self.pages.lock().unwrap();
            Ok(pages.values().find(|p| p.id() == id).cloned())
        }

        fn find_by_title(&self, title: &str) -> DomainResult<Option<Page>> {
            let pages = self.pages.lock().unwrap();
            Ok(pages.get(title).cloned())
        }

        fn find_all(&self) -> DomainResult<Vec<Page>> {
            let pages = self.pages.lock().unwrap();
            Ok(pages.values().cloned().collect())
        }

        fn delete(&mut self, id: &PageId) -> DomainResult<bool> {
            let mut pages = self.pages.lock().unwrap();
            let initial_len = pages.len();
            pages.retain(|_, page| page.id() != id);
            Ok(pages.len() < initial_len)
        }
    }

    #[tokio::test]
    async fn test_sync_once_new_files() {
        // Create a temporary Logseq directory
        let temp_dir = TempDir::new().unwrap();
        let logseq_dir = temp_dir.path();

        // Create pages and journals directories
        let pages_dir = logseq_dir.join("pages");
        let journals_dir = logseq_dir.join("journals");
        std::fs::create_dir(&pages_dir).unwrap();
        std::fs::create_dir(&journals_dir).unwrap();

        // Create some test files
        std::fs::write(pages_dir.join("page1.md"), "- First block\n- Second block").unwrap();
        std::fs::write(pages_dir.join("page2.md"), "- Another page").unwrap();

        // Create sync service
        let repo = MockRepository::new();
        let dir_path = LogseqDirectoryPath::new(logseq_dir).unwrap();
        let service = SyncService::new(repo, dir_path, None).unwrap();

        // Perform sync
        let summary = service.sync_once(None).await.unwrap();

        // Verify results
        assert_eq!(summary.files_created, 2);
        assert_eq!(summary.files_updated, 0);
        assert_eq!(summary.files_deleted, 0);
        assert_eq!(summary.files_unchanged, 0);
        assert_eq!(summary.errors.len(), 0);
    }

    #[tokio::test]
    async fn test_sync_once_updated_files() {
        // Create a temporary Logseq directory
        let temp_dir = TempDir::new().unwrap();
        let logseq_dir = temp_dir.path();

        // Create pages and journals directories
        let pages_dir = logseq_dir.join("pages");
        let journals_dir = logseq_dir.join("journals");
        std::fs::create_dir(&pages_dir).unwrap();
        std::fs::create_dir(&journals_dir).unwrap();

        // Create a test file
        let file_path = pages_dir.join("page1.md");
        std::fs::write(&file_path, "- First block").unwrap();

        // Create sync service
        let repo = MockRepository::new();
        let dir_path = LogseqDirectoryPath::new(logseq_dir).unwrap();
        let service = SyncService::new(repo, dir_path, None).unwrap();

        // First sync
        let summary1 = service.sync_once(None).await.unwrap();
        assert_eq!(summary1.files_created, 1);

        // Wait a bit to ensure different modification time
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Modify the file
        std::fs::write(&file_path, "- First block\n- Second block").unwrap();

        // Second sync
        let summary2 = service.sync_once(None).await.unwrap();
        assert_eq!(summary2.files_created, 0);
        assert_eq!(summary2.files_updated, 1);
        assert_eq!(summary2.files_unchanged, 0);
    }

    #[tokio::test]
    async fn test_sync_once_unchanged_files() {
        // Create a temporary Logseq directory
        let temp_dir = TempDir::new().unwrap();
        let logseq_dir = temp_dir.path();

        // Create pages and journals directories
        let pages_dir = logseq_dir.join("pages");
        let journals_dir = logseq_dir.join("journals");
        std::fs::create_dir(&pages_dir).unwrap();
        std::fs::create_dir(&journals_dir).unwrap();

        // Create a test file
        std::fs::write(pages_dir.join("page1.md"), "- First block").unwrap();

        // Create sync service
        let repo = MockRepository::new();
        let dir_path = LogseqDirectoryPath::new(logseq_dir).unwrap();
        let service = SyncService::new(repo, dir_path, None).unwrap();

        // First sync
        let summary1 = service.sync_once(None).await.unwrap();
        assert_eq!(summary1.files_created, 1);

        // Second sync without modifications
        let summary2 = service.sync_once(None).await.unwrap();
        assert_eq!(summary2.files_created, 0);
        assert_eq!(summary2.files_updated, 0);
        assert_eq!(summary2.files_unchanged, 1);
    }

    #[tokio::test]
    async fn test_sync_once_deleted_files() {
        // Create a temporary Logseq directory
        let temp_dir = TempDir::new().unwrap();
        let logseq_dir = temp_dir.path();

        // Create pages and journals directories
        let pages_dir = logseq_dir.join("pages");
        let journals_dir = logseq_dir.join("journals");
        std::fs::create_dir(&pages_dir).unwrap();
        std::fs::create_dir(&journals_dir).unwrap();

        // Create test files
        let file1 = pages_dir.join("page1.md");
        let file2 = pages_dir.join("page2.md");
        std::fs::write(&file1, "- First page").unwrap();
        std::fs::write(&file2, "- Second page").unwrap();

        // Create sync service
        let repo = MockRepository::new();
        let dir_path = LogseqDirectoryPath::new(logseq_dir).unwrap();
        let service = SyncService::new(repo, dir_path, None).unwrap();

        // First sync
        let summary1 = service.sync_once(None).await.unwrap();
        assert_eq!(summary1.files_created, 2);

        // Delete one file
        std::fs::remove_file(&file1).unwrap();

        // Second sync
        let summary2 = service.sync_once(None).await.unwrap();
        assert_eq!(summary2.files_created, 0);
        assert_eq!(summary2.files_deleted, 1);
        assert_eq!(summary2.files_unchanged, 1);
    }

    #[tokio::test]
    async fn test_sync_once_mixed_operations() {
        // Create a temporary Logseq directory
        let temp_dir = TempDir::new().unwrap();
        let logseq_dir = temp_dir.path();

        // Create pages and journals directories
        let pages_dir = logseq_dir.join("pages");
        let journals_dir = logseq_dir.join("journals");
        std::fs::create_dir(&pages_dir).unwrap();
        std::fs::create_dir(&journals_dir).unwrap();

        // Create initial files
        let file1 = pages_dir.join("page1.md");
        let file2 = pages_dir.join("page2.md");
        std::fs::write(&file1, "- First page").unwrap();
        std::fs::write(&file2, "- Second page").unwrap();

        // Create sync service
        let repo = MockRepository::new();
        let dir_path = LogseqDirectoryPath::new(logseq_dir).unwrap();
        let service = SyncService::new(repo, dir_path, None).unwrap();

        // First sync
        let summary1 = service.sync_once(None).await.unwrap();
        assert_eq!(summary1.files_created, 2);

        // Wait to ensure different modification time
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Create a new file, modify an existing one, and delete one
        std::fs::write(pages_dir.join("page3.md"), "- Third page").unwrap();
        std::fs::write(&file2, "- Second page updated").unwrap();
        std::fs::remove_file(&file1).unwrap();

        // Second sync
        let summary2 = service.sync_once(None).await.unwrap();
        assert_eq!(summary2.files_created, 1); // page3
        assert_eq!(summary2.files_updated, 1); // page2
        assert_eq!(summary2.files_deleted, 1); // page1
        assert_eq!(summary2.files_unchanged, 0);
    }

    #[tokio::test]
    async fn test_sync_once_with_journals() {
        // Create a temporary Logseq directory
        let temp_dir = TempDir::new().unwrap();
        let logseq_dir = temp_dir.path();

        // Create pages and journals directories
        let pages_dir = logseq_dir.join("pages");
        let journals_dir = logseq_dir.join("journals");
        std::fs::create_dir(&pages_dir).unwrap();
        std::fs::create_dir(&journals_dir).unwrap();

        // Create files in both directories
        std::fs::write(pages_dir.join("page1.md"), "- Page content").unwrap();
        std::fs::write(journals_dir.join("2025_10_19.md"), "- Journal entry").unwrap();

        // Create sync service
        let repo = MockRepository::new();
        let dir_path = LogseqDirectoryPath::new(logseq_dir).unwrap();
        let service = SyncService::new(repo, dir_path, None).unwrap();

        // Perform sync
        let summary = service.sync_once(None).await.unwrap();

        // Verify both files were synced
        assert_eq!(summary.files_created, 2);
        assert_eq!(summary.files_updated, 0);
        assert_eq!(summary.files_deleted, 0);
    }

    #[tokio::test]
    async fn test_sync_once_with_callback() {
        // Create a temporary Logseq directory
        let temp_dir = TempDir::new().unwrap();
        let logseq_dir = temp_dir.path();

        // Create pages and journals directories
        let pages_dir = logseq_dir.join("pages");
        let journals_dir = logseq_dir.join("journals");
        std::fs::create_dir(&pages_dir).unwrap();
        std::fs::create_dir(&journals_dir).unwrap();

        // Create a test file
        std::fs::write(pages_dir.join("page1.md"), "- First block").unwrap();

        // Create sync service
        let repo = MockRepository::new();
        let dir_path = LogseqDirectoryPath::new(logseq_dir).unwrap();
        let service = SyncService::new(repo, dir_path, None).unwrap();

        // Track events
        let events = Arc::new(Mutex::new(Vec::new()));
        let events_clone = events.clone();

        let callback: SyncCallback = Arc::new(move |event| {
            let mut evts = events_clone.lock().unwrap();
            evts.push(event);
        });

        // Perform sync with callback
        let summary = service.sync_once(Some(callback)).await.unwrap();
        assert_eq!(summary.files_created, 1);

        // Verify events were emitted
        let evts = events.lock().unwrap();
        assert!(evts.len() >= 3); // SyncStarted, FileCreated, SyncCompleted

        // Check for SyncStarted
        assert!(matches!(evts[0], SyncEvent::SyncStarted));
    }
}
