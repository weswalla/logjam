pub mod dto;
pub mod repositories;
pub mod services;
pub mod use_cases;

// Re-export key types to avoid naming conflicts
pub use dto::{
    PageConnection, SearchItem, SearchRequest, SearchResult, SearchType, UrlWithContext,
};
pub use repositories::PageRepository;
pub use services::{
    ImportError, ImportProgressEvent, ImportResult, ImportService, ImportSummary,
    ProgressCallback, SyncCallback, SyncError, SyncEvent, SyncResult, SyncService,
};
pub use use_cases::{
    BatchIndexPages, GetLinksForPage, GetPagesForUrl, IndexPage, SearchPagesAndBlocks,
};
