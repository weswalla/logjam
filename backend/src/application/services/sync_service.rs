/// Sync service for keeping Logseq directory in sync with changes
use crate::application::repositories::PageRepository;
use crate::domain::value_objects::LogseqDirectoryPath;
use crate::infrastructure::file_system::{FileEvent, FileEventKind, LogseqFileWatcher};
use crate::infrastructure::parsers::LogseqMarkdownParser;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
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

/// Operation to perform during sync
#[derive(Debug)]
enum SyncOperation {
    Create(PathBuf),
    Update(PathBuf),
    Delete(PathBuf),
}

/// Service for syncing Logseq directory changes
pub struct SyncService<R: PageRepository> {
    repository: Arc<Mutex<R>>,
    directory_path: LogseqDirectoryPath,
    watcher: LogseqFileWatcher,
    debounce_duration: Duration,
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
        })
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
        match operation {
            SyncOperation::Create(path) | SyncOperation::Update(path) => {
                // Parse the file
                let page = LogseqMarkdownParser::parse_file(&path).await?;

                // Save to repository
                let mut repo = self.repository.lock().await;
                repo.save(page)?;

                // Emit event
                if let Some(cb) = callback {
                    if matches!(operation, SyncOperation::Create(_)) {
                        cb(SyncEvent::FileCreated { file_path: path });
                        Ok(FileEventKind::Created)
                    } else {
                        cb(SyncEvent::FileUpdated { file_path: path });
                        Ok(FileEventKind::Modified)
                    }
                } else {
                    if matches!(operation, SyncOperation::Create(_)) {
                        Ok(FileEventKind::Created)
                    } else {
                        Ok(FileEventKind::Modified)
                    }
                }
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
                    cb(SyncEvent::FileDeleted { file_path: path });
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

    #[test]
    fn test_sync_stats() {
        let stats = SyncStats::default();
        assert_eq!(stats.files_created, 0);
        assert_eq!(stats.files_updated, 0);
        assert_eq!(stats.files_deleted, 0);
    }
}
