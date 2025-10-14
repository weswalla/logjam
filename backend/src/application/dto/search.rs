use crate::domain::value_objects::{BlockId, PageId, PageReference, Url};

/// Type of search to perform
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchType {
    /// Keyword-based traditional search
    Traditional,
    /// Vector/embedding-based semantic search
    Semantic,
}

/// Type of results to return
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResultType {
    /// Return only pages
    PagesOnly,
    /// Return only blocks
    BlocksOnly,
    /// Return only URLs
    UrlsOnly,
    /// Return all types of results
    All,
}

/// Search request parameters
#[derive(Debug, Clone)]
pub struct SearchRequest {
    /// The search query text
    pub query: String,
    /// Type of search (traditional or semantic)
    pub search_type: SearchType,
    /// Type of results to return
    pub result_type: ResultType,
    /// Optional filter to limit results to specific pages
    pub page_filters: Option<Vec<PageId>>,
}

impl SearchRequest {
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            search_type: SearchType::Traditional,
            result_type: ResultType::All,
            page_filters: None,
        }
    }

    pub fn with_search_type(mut self, search_type: SearchType) -> Self {
        self.search_type = search_type;
        self
    }

    pub fn with_result_type(mut self, result_type: ResultType) -> Self {
        self.result_type = result_type;
        self
    }

    pub fn with_page_filters(mut self, page_filters: Vec<PageId>) -> Self {
        self.page_filters = Some(page_filters);
        self
    }
}

/// A search result with matched item and context
#[derive(Debug, Clone, PartialEq)]
pub struct SearchResult {
    /// The matched item (page, block, or URL)
    pub item: SearchItem,
    /// Relevance score (higher is more relevant)
    pub score: f64,
}

/// The type of item that was matched in a search
#[derive(Debug, Clone, PartialEq)]
pub enum SearchItem {
    Page(PageResult),
    Block(BlockResult),
    Url(UrlResult),
}

/// A page search result
#[derive(Debug, Clone, PartialEq)]
pub struct PageResult {
    pub page_id: PageId,
    pub title: String,
    /// Number of blocks in the page
    pub block_count: usize,
    /// URLs found in the page
    pub urls: Vec<Url>,
    /// Page references found in the page
    pub page_references: Vec<PageReference>,
}

/// A block search result with hierarchical context
#[derive(Debug, Clone, PartialEq)]
pub struct BlockResult {
    pub block_id: BlockId,
    pub content: String,
    pub page_id: PageId,
    pub page_title: String,
    /// Hierarchical path from root to this block (block contents)
    pub hierarchy_path: Vec<String>,
    /// Page references in ancestor and descendant blocks
    pub related_pages: Vec<PageReference>,
    /// URLs in ancestor and descendant blocks
    pub related_urls: Vec<Url>,
}

/// A URL search result with hierarchical context
#[derive(Debug, Clone, PartialEq)]
pub struct UrlResult {
    pub url: Url,
    pub containing_block_id: BlockId,
    pub containing_block_content: String,
    pub page_id: PageId,
    pub page_title: String,
    /// Page references in ancestor blocks
    pub ancestor_page_refs: Vec<PageReference>,
    /// Page references in descendant blocks
    pub descendant_page_refs: Vec<PageReference>,
}

/// Result for URL-to-pages connection query
#[derive(Debug, Clone, PartialEq)]
pub struct PageConnection {
    pub page_id: PageId,
    pub page_title: String,
    /// Blocks that contain the URL
    pub blocks_with_url: Vec<BlockId>,
}

/// Result for page-to-links query
#[derive(Debug, Clone, PartialEq)]
pub struct UrlWithContext {
    pub url: Url,
    pub block_id: BlockId,
    pub block_content: String,
    /// Hierarchical path from root to the block containing the URL
    pub hierarchy_path: Vec<String>,
    /// Page references related to this URL (from ancestors and descendants)
    pub related_page_refs: Vec<PageReference>,
}
