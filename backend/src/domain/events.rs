/// Domain events
use super::base::DomainEvent;
use super::value_objects::{BlockId, PageId};
use std::path::PathBuf;

/// Event emitted when a new page is created
#[derive(Debug, Clone)]
pub struct PageCreated {
    pub page_id: PageId,
    pub title: String,
}

impl DomainEvent for PageCreated {
    fn event_type(&self) -> &'static str {
        "PageCreated"
    }

    fn aggregate_id(&self) -> String {
        self.page_id.as_str().to_string()
    }
}

/// Event emitted when a page is updated
#[derive(Debug, Clone)]
pub struct PageUpdated {
    pub page_id: PageId,
    pub title: Option<String>,
}

impl DomainEvent for PageUpdated {
    fn event_type(&self) -> &'static str {
        "PageUpdated"
    }

    fn aggregate_id(&self) -> String {
        self.page_id.as_str().to_string()
    }
}

/// Event emitted when a page is deleted
#[derive(Debug, Clone)]
pub struct PageDeleted {
    pub page_id: PageId,
}

impl DomainEvent for PageDeleted {
    fn event_type(&self) -> &'static str {
        "PageDeleted"
    }

    fn aggregate_id(&self) -> String {
        self.page_id.as_str().to_string()
    }
}

/// Event emitted when a block is added to a page
#[derive(Debug, Clone)]
pub struct BlockAdded {
    pub page_id: PageId,
    pub block_id: BlockId,
    pub parent_block_id: Option<BlockId>,
}

impl DomainEvent for BlockAdded {
    fn event_type(&self) -> &'static str {
        "BlockAdded"
    }

    fn aggregate_id(&self) -> String {
        self.page_id.as_str().to_string()
    }
}

/// Event emitted when a block is updated
#[derive(Debug, Clone)]
pub struct BlockUpdated {
    pub page_id: PageId,
    pub block_id: BlockId,
}

impl DomainEvent for BlockUpdated {
    fn event_type(&self) -> &'static str {
        "BlockUpdated"
    }

    fn aggregate_id(&self) -> String {
        self.page_id.as_str().to_string()
    }
}

/// Event emitted when a block is removed from a page
#[derive(Debug, Clone)]
pub struct BlockRemoved {
    pub page_id: PageId,
    pub block_id: BlockId,
}

impl DomainEvent for BlockRemoved {
    fn event_type(&self) -> &'static str {
        "BlockRemoved"
    }

    fn aggregate_id(&self) -> String {
        self.page_id.as_str().to_string()
    }
}

/// Event emitted when an import operation starts
#[derive(Debug, Clone)]
pub struct ImportStarted {
    pub directory_path: PathBuf,
    pub total_files: usize,
}

impl DomainEvent for ImportStarted {
    fn event_type(&self) -> &'static str {
        "ImportStarted"
    }

    fn aggregate_id(&self) -> String {
        self.directory_path.to_string_lossy().to_string()
    }
}

/// Event emitted when a file is processed during import
#[derive(Debug, Clone)]
pub struct FileProcessed {
    pub directory_path: PathBuf,
    pub file_path: PathBuf,
    pub page_id: PageId,
    pub files_processed: usize,
    pub total_files: usize,
}

impl DomainEvent for FileProcessed {
    fn event_type(&self) -> &'static str {
        "FileProcessed"
    }

    fn aggregate_id(&self) -> String {
        self.directory_path.to_string_lossy().to_string()
    }
}

/// Event emitted when import completes successfully
#[derive(Debug, Clone)]
pub struct ImportCompleted {
    pub directory_path: PathBuf,
    pub pages_imported: usize,
    pub duration_ms: u64,
}

impl DomainEvent for ImportCompleted {
    fn event_type(&self) -> &'static str {
        "ImportCompleted"
    }

    fn aggregate_id(&self) -> String {
        self.directory_path.to_string_lossy().to_string()
    }
}

/// Event emitted when import fails
#[derive(Debug, Clone)]
pub struct ImportFailed {
    pub directory_path: PathBuf,
    pub error: String,
    pub files_processed: usize,
}

impl DomainEvent for ImportFailed {
    fn event_type(&self) -> &'static str {
        "ImportFailed"
    }

    fn aggregate_id(&self) -> String {
        self.directory_path.to_string_lossy().to_string()
    }
}

/// Event emitted when file sync starts
#[derive(Debug, Clone)]
pub struct SyncStarted {
    pub directory_path: PathBuf,
}

impl DomainEvent for SyncStarted {
    fn event_type(&self) -> &'static str {
        "SyncStarted"
    }

    fn aggregate_id(&self) -> String {
        self.directory_path.to_string_lossy().to_string()
    }
}

/// Event emitted when a file is created and synced
#[derive(Debug, Clone)]
pub struct FileCreatedEvent {
    pub directory_path: PathBuf,
    pub file_path: PathBuf,
    pub page_id: PageId,
}

impl DomainEvent for FileCreatedEvent {
    fn event_type(&self) -> &'static str {
        "FileCreated"
    }

    fn aggregate_id(&self) -> String {
        self.directory_path.to_string_lossy().to_string()
    }
}

/// Event emitted when a file is updated and synced
#[derive(Debug, Clone)]
pub struct FileUpdatedEvent {
    pub directory_path: PathBuf,
    pub file_path: PathBuf,
    pub page_id: PageId,
}

impl DomainEvent for FileUpdatedEvent {
    fn event_type(&self) -> &'static str {
        "FileUpdated"
    }

    fn aggregate_id(&self) -> String {
        self.directory_path.to_string_lossy().to_string()
    }
}

/// Event emitted when a file is deleted and synced
#[derive(Debug, Clone)]
pub struct FileDeletedEvent {
    pub directory_path: PathBuf,
    pub file_path: PathBuf,
    pub page_id: PageId,
}

impl DomainEvent for FileDeletedEvent {
    fn event_type(&self) -> &'static str {
        "FileDeleted"
    }

    fn aggregate_id(&self) -> String {
        self.directory_path.to_string_lossy().to_string()
    }
}

/// Event emitted when sync completes
#[derive(Debug, Clone)]
pub struct SyncCompleted {
    pub directory_path: PathBuf,
    pub files_created: usize,
    pub files_updated: usize,
    pub files_deleted: usize,
}

impl DomainEvent for SyncCompleted {
    fn event_type(&self) -> &'static str {
        "SyncCompleted"
    }

    fn aggregate_id(&self) -> String {
        self.directory_path.to_string_lossy().to_string()
    }
}

/// Enum wrapper for all domain events to make them object-safe
#[derive(Debug, Clone)]
pub enum DomainEventEnum {
    PageCreated(PageCreated),
    PageUpdated(PageUpdated),
    PageDeleted(PageDeleted),
    BlockAdded(BlockAdded),
    BlockUpdated(BlockUpdated),
    BlockRemoved(BlockRemoved),
    ImportStarted(ImportStarted),
    FileProcessed(FileProcessed),
    ImportCompleted(ImportCompleted),
    ImportFailed(ImportFailed),
    SyncStarted(SyncStarted),
    FileCreated(FileCreatedEvent),
    FileUpdated(FileUpdatedEvent),
    FileDeleted(FileDeletedEvent),
    SyncCompleted(SyncCompleted),
}

impl DomainEvent for DomainEventEnum {
    fn event_type(&self) -> &'static str {
        match self {
            DomainEventEnum::PageCreated(e) => e.event_type(),
            DomainEventEnum::PageUpdated(e) => e.event_type(),
            DomainEventEnum::PageDeleted(e) => e.event_type(),
            DomainEventEnum::BlockAdded(e) => e.event_type(),
            DomainEventEnum::BlockUpdated(e) => e.event_type(),
            DomainEventEnum::BlockRemoved(e) => e.event_type(),
            DomainEventEnum::ImportStarted(e) => e.event_type(),
            DomainEventEnum::FileProcessed(e) => e.event_type(),
            DomainEventEnum::ImportCompleted(e) => e.event_type(),
            DomainEventEnum::ImportFailed(e) => e.event_type(),
            DomainEventEnum::SyncStarted(e) => e.event_type(),
            DomainEventEnum::FileCreated(e) => e.event_type(),
            DomainEventEnum::FileUpdated(e) => e.event_type(),
            DomainEventEnum::FileDeleted(e) => e.event_type(),
            DomainEventEnum::SyncCompleted(e) => e.event_type(),
        }
    }

    fn aggregate_id(&self) -> String {
        match self {
            DomainEventEnum::PageCreated(e) => e.aggregate_id(),
            DomainEventEnum::PageUpdated(e) => e.aggregate_id(),
            DomainEventEnum::PageDeleted(e) => e.aggregate_id(),
            DomainEventEnum::BlockAdded(e) => e.aggregate_id(),
            DomainEventEnum::BlockUpdated(e) => e.aggregate_id(),
            DomainEventEnum::BlockRemoved(e) => e.aggregate_id(),
            DomainEventEnum::ImportStarted(e) => e.aggregate_id(),
            DomainEventEnum::FileProcessed(e) => e.aggregate_id(),
            DomainEventEnum::ImportCompleted(e) => e.aggregate_id(),
            DomainEventEnum::ImportFailed(e) => e.aggregate_id(),
            DomainEventEnum::SyncStarted(e) => e.aggregate_id(),
            DomainEventEnum::FileCreated(e) => e.aggregate_id(),
            DomainEventEnum::FileUpdated(e) => e.aggregate_id(),
            DomainEventEnum::FileDeleted(e) => e.aggregate_id(),
            DomainEventEnum::SyncCompleted(e) => e.aggregate_id(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_page_created_event() {
        let page_id = PageId::new("page-1").unwrap();
        let event = PageCreated {
            page_id: page_id.clone(),
            title: "Test Page".to_string(),
        };

        assert_eq!(event.event_type(), "PageCreated");
        assert_eq!(event.aggregate_id(), "page-1");
    }

    #[test]
    fn test_page_updated_event() {
        let page_id = PageId::new("page-1").unwrap();
        let event = PageUpdated {
            page_id: page_id.clone(),
            title: Some("Updated Title".to_string()),
        };

        assert_eq!(event.event_type(), "PageUpdated");
        assert_eq!(event.aggregate_id(), "page-1");
    }

    #[test]
    fn test_page_deleted_event() {
        let page_id = PageId::new("page-1").unwrap();
        let event = PageDeleted {
            page_id: page_id.clone(),
        };

        assert_eq!(event.event_type(), "PageDeleted");
        assert_eq!(event.aggregate_id(), "page-1");
    }

    #[test]
    fn test_block_added_event() {
        let page_id = PageId::new("page-1").unwrap();
        let block_id = BlockId::new("block-1").unwrap();
        let event = BlockAdded {
            page_id: page_id.clone(),
            block_id: block_id.clone(),
            parent_block_id: None,
        };

        assert_eq!(event.event_type(), "BlockAdded");
        assert_eq!(event.aggregate_id(), "page-1");
    }

    #[test]
    fn test_block_updated_event() {
        let page_id = PageId::new("page-1").unwrap();
        let block_id = BlockId::new("block-1").unwrap();
        let event = BlockUpdated {
            page_id: page_id.clone(),
            block_id: block_id.clone(),
        };

        assert_eq!(event.event_type(), "BlockUpdated");
        assert_eq!(event.aggregate_id(), "page-1");
    }

    #[test]
    fn test_block_removed_event() {
        let page_id = PageId::new("page-1").unwrap();
        let block_id = BlockId::new("block-1").unwrap();
        let event = BlockRemoved {
            page_id: page_id.clone(),
            block_id: block_id.clone(),
        };

        assert_eq!(event.event_type(), "BlockRemoved");
        assert_eq!(event.aggregate_id(), "page-1");
    }

    #[test]
    fn test_import_started_event() {
        let event = ImportStarted {
            directory_path: PathBuf::from("/test/directory"),
            total_files: 10,
        };

        assert_eq!(event.event_type(), "ImportStarted");
        assert_eq!(event.aggregate_id(), "/test/directory");
    }

    #[test]
    fn test_file_processed_event() {
        let page_id = PageId::new("page-1").unwrap();
        let event = FileProcessed {
            directory_path: PathBuf::from("/test/directory"),
            file_path: PathBuf::from("/test/directory/pages/test.md"),
            page_id,
            files_processed: 5,
            total_files: 10,
        };

        assert_eq!(event.event_type(), "FileProcessed");
        assert_eq!(event.aggregate_id(), "/test/directory");
    }

    #[test]
    fn test_import_completed_event() {
        let event = ImportCompleted {
            directory_path: PathBuf::from("/test/directory"),
            pages_imported: 10,
            duration_ms: 5000,
        };

        assert_eq!(event.event_type(), "ImportCompleted");
        assert_eq!(event.aggregate_id(), "/test/directory");
    }

    #[test]
    fn test_sync_events() {
        let page_id = PageId::new("page-1").unwrap();

        let sync_started = SyncStarted {
            directory_path: PathBuf::from("/test/directory"),
        };
        assert_eq!(sync_started.event_type(), "SyncStarted");

        let file_created = FileCreatedEvent {
            directory_path: PathBuf::from("/test/directory"),
            file_path: PathBuf::from("/test/directory/pages/new.md"),
            page_id: page_id.clone(),
        };
        assert_eq!(file_created.event_type(), "FileCreated");

        let file_updated = FileUpdatedEvent {
            directory_path: PathBuf::from("/test/directory"),
            file_path: PathBuf::from("/test/directory/pages/updated.md"),
            page_id: page_id.clone(),
        };
        assert_eq!(file_updated.event_type(), "FileUpdated");

        let file_deleted = FileDeletedEvent {
            directory_path: PathBuf::from("/test/directory"),
            file_path: PathBuf::from("/test/directory/pages/deleted.md"),
            page_id,
        };
        assert_eq!(file_deleted.event_type(), "FileDeleted");

        let sync_completed = SyncCompleted {
            directory_path: PathBuf::from("/test/directory"),
            files_created: 1,
            files_updated: 2,
            files_deleted: 1,
        };
        assert_eq!(sync_completed.event_type(), "SyncCompleted");
    }
}
