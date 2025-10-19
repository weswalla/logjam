use crate::application::{
    dto::{
        BlockResult, PageResult, ResultType, SearchItem, SearchRequest, SearchResult,
        SearchType, UrlResult,
    },
    repositories::PageRepository,
};
use crate::domain::{aggregates::Page, base::Entity, value_objects::PageId, DomainResult};

/// Use case for searching pages and blocks
///
/// This use case orchestrates the search functionality across pages and blocks,
/// applying filters and returning structured results with hierarchical context.
pub struct SearchPagesAndBlocks<'a, R: PageRepository> {
    repository: &'a R,
}

impl<'a, R: PageRepository> SearchPagesAndBlocks<'a, R> {
    pub fn new(repository: &'a R) -> Self {
        Self { repository }
    }

    /// Execute a search query and return matching results
    pub fn execute(&self, request: SearchRequest) -> DomainResult<Vec<SearchResult>> {
        // Get all pages (or filtered pages if specified)
        let pages = if let Some(ref page_filters) = request.page_filters {
            self.get_filtered_pages(page_filters)?
        } else {
            self.repository.find_all()?
        };

        // Perform search based on search type
        let results = match request.search_type {
            SearchType::Traditional => self.traditional_search(&pages, &request),
            SearchType::Semantic => {
                // For now, semantic search falls back to traditional
                // This will be implemented with vector embeddings in the infrastructure layer
                self.traditional_search(&pages, &request)
            }
        };

        Ok(results)
    }

    fn get_filtered_pages(&self, page_ids: &[PageId]) -> DomainResult<Vec<Page>> {
        let mut pages = Vec::new();
        for page_id in page_ids {
            if let Some(page) = self.repository.find_by_id(page_id)? {
                pages.push(page);
            }
        }
        Ok(pages)
    }

    fn traditional_search(&self, pages: &[Page], request: &SearchRequest) -> Vec<SearchResult> {
        let query_lower = request.query.to_lowercase();
        let mut results = Vec::new();

        for page in pages {
            // Search pages
            if matches!(
                request.result_type,
                ResultType::PagesOnly | ResultType::All
            ) {
                if let Some(result) = self.search_page(page, &query_lower) {
                    results.push(result);
                }
            }

            // Search blocks
            if matches!(
                request.result_type,
                ResultType::BlocksOnly | ResultType::All
            ) {
                results.extend(self.search_blocks(page, &query_lower));
            }

            // Search URLs
            if matches!(request.result_type, ResultType::UrlsOnly | ResultType::All) {
                results.extend(self.search_urls(page, &query_lower));
            }
        }

        // Sort by score (highest first)
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());

        results
    }

    fn search_page(&self, page: &Page, query: &str) -> Option<SearchResult> {
        let title_lower = page.title().to_lowercase();
        if title_lower.contains(query) {
            // Calculate score based on match quality
            let score = if title_lower == query {
                1.0 // Exact match
            } else if title_lower.starts_with(query) {
                0.9 // Prefix match
            } else {
                0.7 // Contains match
            };

            Some(SearchResult {
                item: SearchItem::Page(PageResult {
                    page_id: page.id().clone(),
                    title: page.title().to_string(),
                    block_count: page.all_blocks().count(),
                    urls: page.all_urls().into_iter().cloned().collect(),
                    page_references: page.all_page_references().into_iter().cloned().collect(),
                }),
                score,
            })
        } else {
            None
        }
    }

    fn search_blocks(&self, page: &Page, query: &str) -> Vec<SearchResult> {
        let mut results = Vec::new();

        for block in page.all_blocks() {
            let content_lower = block.content().as_str().to_lowercase();
            if content_lower.contains(query) {
                let score = if content_lower == query {
                    1.0
                } else if content_lower.starts_with(query) {
                    0.9
                } else {
                    0.7
                };

                // Get hierarchy path for context
                let hierarchy_path = page
                    .get_hierarchy_path(block.id())
                    .iter()
                    .map(|b| b.content().as_str().to_string())
                    .collect();

                // Collect related pages and URLs from ancestors and descendants
                let mut related_pages = Vec::new();
                let mut related_urls = Vec::new();

                for ancestor in page.get_ancestors(block.id()) {
                    related_pages.extend(ancestor.page_references().iter().cloned());
                    related_urls.extend(ancestor.urls().iter().cloned());
                }

                for descendant in page.get_descendants(block.id()) {
                    related_pages.extend(descendant.page_references().iter().cloned());
                    related_urls.extend(descendant.urls().iter().cloned());
                }

                results.push(SearchResult {
                    item: SearchItem::Block(BlockResult {
                        block_id: block.id().clone(),
                        content: block.content().as_str().to_string(),
                        page_id: page.id().clone(),
                        page_title: page.title().to_string(),
                        hierarchy_path,
                        related_pages,
                        related_urls,
                    }),
                    score,
                });
            }
        }

        results
    }

    fn search_urls(&self, page: &Page, query: &str) -> Vec<SearchResult> {
        let mut results = Vec::new();

        // Get all URLs with their context
        let urls_with_context = page.get_urls_with_context();

        for (url, ancestor_refs, descendant_refs) in urls_with_context {
            let url_str = url.as_str().to_lowercase();
            if url_str.contains(query) {
                let score = if url_str == query {
                    1.0
                } else {
                    0.8
                };

                // Find the block containing this URL
                if let Some(block) = page
                    .all_blocks()
                    .find(|b| b.urls().iter().any(|u| u == url))
                {
                    results.push(SearchResult {
                        item: SearchItem::Url(UrlResult {
                            url: url.clone(),
                            containing_block_id: block.id().clone(),
                            containing_block_content: block.content().as_str().to_string(),
                            page_id: page.id().clone(),
                            page_title: page.title().to_string(),
                            ancestor_page_refs: ancestor_refs.into_iter().cloned().collect(),
                            descendant_page_refs: descendant_refs.into_iter().cloned().collect(),
                        }),
                        score,
                    });
                }
            }
        }

        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        base::Entity,
        entities::Block,
        value_objects::{BlockContent, BlockId, Url},
    };
    use std::collections::HashMap;

    struct InMemoryPageRepository {
        pages: HashMap<PageId, Page>,
    }

    impl InMemoryPageRepository {
        fn new() -> Self {
            Self {
                pages: HashMap::new(),
            }
        }
    }

    impl PageRepository for InMemoryPageRepository {
        fn save(&mut self, page: Page) -> DomainResult<()> {
            self.pages.insert(page.id().clone(), page);
            Ok(())
        }

        fn find_by_id(&self, id: &PageId) -> DomainResult<Option<Page>> {
            Ok(self.pages.get(id).cloned())
        }

        fn find_by_title(&self, title: &str) -> DomainResult<Option<Page>> {
            Ok(self.pages.values().find(|p| p.title() == title).cloned())
        }

        fn find_all(&self) -> DomainResult<Vec<Page>> {
            Ok(self.pages.values().cloned().collect())
        }

        fn delete(&mut self, id: &PageId) -> DomainResult<bool> {
            Ok(self.pages.remove(id).is_some())
        }
    }

    fn create_test_page() -> Page {
        let page_id = PageId::new("test-page").unwrap();
        let mut page = Page::new(page_id, "Test Page".to_string());

        // Create a simple block structure
        let block1 = Block::new_root(
            BlockId::new("block-1").unwrap(),
            BlockContent::new("First block with test content"),
        );
        page.add_block(block1).unwrap();

        let block2 = Block::new_root(
            BlockId::new("block-2").unwrap(),
            BlockContent::new("Second block with different text"),
        );
        page.add_block(block2).unwrap();

        page
    }

    #[test]
    fn test_search_pages_by_title() {
        let mut repo = InMemoryPageRepository::new();
        let page = create_test_page();
        repo.save(page).unwrap();

        let use_case = SearchPagesAndBlocks::new(&repo);
        let request = SearchRequest::new("Test Page").with_result_type(ResultType::PagesOnly);
        let results = use_case.execute(request).unwrap();

        assert_eq!(results.len(), 1);
        assert!(matches!(results[0].item, SearchItem::Page(_)));
    }

    #[test]
    fn test_search_blocks_by_content() {
        let mut repo = InMemoryPageRepository::new();
        let page = create_test_page();
        repo.save(page).unwrap();

        let use_case = SearchPagesAndBlocks::new(&repo);
        let request = SearchRequest::new("test content").with_result_type(ResultType::BlocksOnly);
        let results = use_case.execute(request).unwrap();

        assert_eq!(results.len(), 1);
        if let SearchItem::Block(block_result) = &results[0].item {
            assert!(block_result.content.contains("test content"));
        } else {
            panic!("Expected Block result");
        }
    }

    #[test]
    fn test_search_with_page_filter() {
        let mut repo = InMemoryPageRepository::new();
        let page1 = create_test_page();
        let page1_id = page1.id().clone();

        let page2_id = PageId::new("other-page").unwrap();
        let page2 = Page::new(page2_id.clone(), "Other Page".to_string());

        repo.save(page1).unwrap();
        repo.save(page2).unwrap();

        let use_case = SearchPagesAndBlocks::new(&repo);
        let request = SearchRequest::new("page")
            .with_result_type(ResultType::PagesOnly)
            .with_page_filters(vec![page1_id]);

        let results = use_case.execute(request).unwrap();

        assert_eq!(results.len(), 1);
        if let SearchItem::Page(page_result) = &results[0].item {
            assert_eq!(page_result.title, "Test Page");
        }
    }

    #[test]
    fn test_search_all_types() {
        let mut repo = InMemoryPageRepository::new();
        let page = create_test_page();
        repo.save(page).unwrap();

        let use_case = SearchPagesAndBlocks::new(&repo);
        let request = SearchRequest::new("test").with_result_type(ResultType::All);
        let results = use_case.execute(request).unwrap();

        // Should find page and block matches
        assert!(results.len() >= 2);
    }

    #[test]
    fn test_search_urls() {
        let mut repo = InMemoryPageRepository::new();
        let page_id = PageId::new("url-page").unwrap();
        let mut page = Page::new(page_id, "URL Page".to_string());

        let mut block = Block::new_root(
            BlockId::new("url-block").unwrap(),
            BlockContent::new("Check out this link"),
        );
        block.add_url(Url::new("https://example.com").unwrap());
        page.add_block(block).unwrap();

        repo.save(page).unwrap();

        let use_case = SearchPagesAndBlocks::new(&repo);
        let request = SearchRequest::new("example.com").with_result_type(ResultType::UrlsOnly);
        let results = use_case.execute(request).unwrap();

        assert_eq!(results.len(), 1);
        assert!(matches!(results[0].item, SearchItem::Url(_)));
    }
}
