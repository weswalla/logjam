/// Import service for importing Logseq directories
use crate::application::repositories::PageRepository;
use crate::domain::events::{FileProcessed, ImportCompleted, ImportFailed, ImportStarted};
use crate::domain::value_objects::{ImportProgress, LogseqDirectoryPath};
use crate::infrastructure::file_system::discover_logseq_files;
use crate::infrastructure::parsers::LogseqMarkdownParser;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use thiserror::Error;
use tokio::sync::{mpsc, Semaphore};

#[derive(Error, Debug)]
pub enum ImportError {
    #[error("Invalid directory: {0}")]
    InvalidDirectory(String),

    #[error("File system error: {0}")]
    FileSystem(#[from] std::io::Error),

    #[error("Parse error: {0}")]
    Parse(#[from] crate::infrastructure::parsers::ParseError),

    #[error("Repository error: {0}")]
    Repository(#[from] crate::domain::base::DomainError),

    #[error("Domain error: {0}")]
    Domain(String),
}

pub type ImportResult<T> = Result<T, ImportError>;

/// Callback type for progress events
pub type ProgressCallback = Arc<dyn Fn(ImportProgressEvent) + Send + Sync>;

/// Progress event for the import process
#[derive(Debug, Clone)]
pub enum ImportProgressEvent {
    Started { total_files: usize },
    FileProcessed { file_path: PathBuf, progress: ImportProgress },
    Completed { pages_imported: usize, duration_ms: u64 },
    Failed { error: String, files_processed: usize },
}

/// Service for importing Logseq directories
pub struct ImportService<R: PageRepository> {
    repository: R,
    max_concurrent_files: usize,
}

impl<R: PageRepository> ImportService<R> {
    pub fn new(repository: R) -> Self {
        ImportService {
            repository,
            max_concurrent_files: 4, // Default bounded concurrency
        }
    }

    pub fn with_concurrency(mut self, max_concurrent: usize) -> Self {
        self.max_concurrent_files = max_concurrent;
        self
    }

    /// Import a Logseq directory with progress tracking
    pub async fn import_directory(
        &mut self,
        directory_path: LogseqDirectoryPath,
        progress_callback: Option<ProgressCallback>,
    ) -> ImportResult<ImportSummary> {
        let start_time = Instant::now();
        let path_buf = directory_path.as_path().to_path_buf();

        // Discover all markdown files
        let files = discover_logseq_files(directory_path.as_path()).await?;
        let total_files = files.len();

        // Emit started event
        if let Some(ref callback) = progress_callback {
            callback(ImportProgressEvent::Started { total_files });
        }

        // Track progress
        let mut progress = ImportProgress::new(total_files);
        let mut errors = Vec::new();
        let mut pages_imported = 0;

        // Use bounded concurrency with a semaphore
        let semaphore = Arc::new(Semaphore::new(self.max_concurrent_files));
        let (tx, mut rx) = mpsc::channel(100);

        // Spawn tasks for each file
        for file_path in files {
            let semaphore = Arc::clone(&semaphore);
            let tx = tx.clone();

            tokio::spawn(async move {
                let _permit = semaphore.acquire().await.unwrap();
                let result = LogseqMarkdownParser::parse_file(&file_path).await;
                tx.send((file_path, result)).await.ok();
            });
        }

        // Drop the original sender so the channel closes when all tasks complete
        drop(tx);

        // Collect results
        while let Some((file_path, result)) = rx.recv().await {
            match result {
                Ok(page) => {
                    // Save page to repository
                    if let Err(e) = self.repository.save(page.clone()) {
                        tracing::error!("Failed to save page from {}: {}", file_path.display(), e);
                        errors.push((file_path.clone(), e.to_string()));
                    } else {
                        pages_imported += 1;
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to parse {}: {}", file_path.display(), e);
                    errors.push((file_path.clone(), e.to_string()));
                }
            }

            // Update progress
            progress.increment();
            progress.set_current_file(None);

            // Emit progress event
            if let Some(ref callback) = progress_callback {
                callback(ImportProgressEvent::FileProcessed {
                    file_path: file_path.clone(),
                    progress: progress.clone(),
                });
            }
        }

        let duration_ms = start_time.elapsed().as_millis() as u64;

        // Emit completion or failure event
        if let Some(ref callback) = progress_callback {
            if errors.is_empty() {
                callback(ImportProgressEvent::Completed {
                    pages_imported,
                    duration_ms,
                });
            } else {
                callback(ImportProgressEvent::Failed {
                    error: format!("{} files failed to import", errors.len()),
                    files_processed: progress.files_processed(),
                });
            }
        }

        Ok(ImportSummary {
            total_files,
            pages_imported,
            errors,
            duration_ms,
        })
    }
}

/// Summary of an import operation
#[derive(Debug)]
pub struct ImportSummary {
    pub total_files: usize,
    pub pages_imported: usize,
    pub errors: Vec<(PathBuf, String)>,
    pub duration_ms: u64,
}

impl ImportSummary {
    pub fn success_rate(&self) -> f64 {
        if self.total_files == 0 {
            return 100.0;
        }
        (self.pages_imported as f64 / self.total_files as f64) * 100.0
    }

    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::aggregates::Page;
    use crate::domain::base::{DomainResult, Entity};
    use crate::domain::value_objects::PageId;
    use std::collections::HashMap;

    // Mock repository for testing
    struct MockPageRepository {
        pages: HashMap<String, Page>,
    }

    impl MockPageRepository {
        fn new() -> Self {
            MockPageRepository {
                pages: HashMap::new(),
            }
        }
    }

    impl PageRepository for MockPageRepository {
        fn save(&mut self, page: Page) -> DomainResult<()> {
            self.pages.insert(page.id().as_str().to_string(), page);
            Ok(())
        }

        fn find_by_id(&self, id: &PageId) -> DomainResult<Option<Page>> {
            Ok(self.pages.get(id.as_str()).cloned())
        }

        fn find_by_title(&self, title: &str) -> DomainResult<Option<Page>> {
            Ok(self.pages.values().find(|p| p.title() == title).cloned())
        }

        fn find_all(&self) -> DomainResult<Vec<Page>> {
            Ok(self.pages.values().cloned().collect())
        }

        fn delete(&mut self, id: &PageId) -> DomainResult<bool> {
            Ok(self.pages.remove(id.as_str()).is_some())
        }
    }

    #[test]
    fn test_import_summary() {
        let summary = ImportSummary {
            total_files: 10,
            pages_imported: 8,
            errors: vec![
                (PathBuf::from("file1.md"), "error 1".to_string()),
                (PathBuf::from("file2.md"), "error 2".to_string()),
            ],
            duration_ms: 1000,
        };

        assert_eq!(summary.success_rate(), 80.0);
        assert!(summary.has_errors());
    }
}
