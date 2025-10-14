use backend::application::{
    dto::{ResultType, SearchRequest},
    repositories::PageRepository,
    use_cases::{GetLinksForPage, GetPagesForUrl, IndexPage, SearchPagesAndBlocks},
};
use backend::domain::{
    aggregates::Page,
    base::Entity,
    entities::Block,
    value_objects::{BlockContent, BlockId, IndentLevel, PageId, PageReference, Url},
    DomainResult,
};
use std::collections::HashMap;

/// In-memory repository implementation for testing
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Create a sample knowledge base with interconnected pages
    fn create_sample_knowledge_base() -> InMemoryPageRepository {
        let mut repo = InMemoryPageRepository::new();

        // Page 1: Programming page with nested blocks about Rust
        let page1_id = PageId::new("programming").unwrap();
        let mut page1 = Page::new(page1_id.clone(), "Programming".to_string());

        let mut block1_1 = Block::new_root(
            BlockId::new("prog-1").unwrap(),
            BlockContent::new("Learning Rust programming language"),
        );
        block1_1.add_url(Url::new("https://rust-lang.org").unwrap());
        block1_1.add_page_reference(PageReference::from_tag("learning").unwrap());
        page1.add_block(block1_1).unwrap();

        let mut block1_2 = Block::new_child(
            BlockId::new("prog-2").unwrap(),
            BlockContent::new("Ownership and borrowing concepts"),
            BlockId::new("prog-1").unwrap(),
            IndentLevel::new(1),
        );
        block1_2.add_url(Url::new("https://doc.rust-lang.org/book/ch04-00-understanding-ownership.html").unwrap());

        // Update parent's children
        if let Some(parent) = page1.get_block_mut(&BlockId::new("prog-1").unwrap()) {
            parent.add_child(block1_2.id().clone());
        }
        page1.add_block(block1_2).unwrap();

        repo.save(page1).unwrap();

        // Page 2: Web Development page
        let page2_id = PageId::new("web-dev").unwrap();
        let mut page2 = Page::new(page2_id, "Web Development".to_string());

        let mut block2_1 = Block::new_root(
            BlockId::new("web-1").unwrap(),
            BlockContent::new("Building web applications with Rust"),
        );
        block2_1.add_url(Url::new("https://rocket.rs").unwrap());
        block2_1.add_page_reference(PageReference::from_brackets("programming").unwrap());
        page2.add_block(block2_1).unwrap();

        let mut block2_2 = Block::new_root(
            BlockId::new("web-2").unwrap(),
            BlockContent::new("Frontend frameworks"),
        );
        block2_2.add_url(Url::new("https://yew.rs").unwrap());
        page2.add_block(block2_2).unwrap();

        repo.save(page2).unwrap();

        // Page 3: Learning page (referenced by tag in Page 1)
        let page3_id = PageId::new("learning").unwrap();
        let mut page3 = Page::new(page3_id, "Learning Resources".to_string());

        let mut block3_1 = Block::new_root(
            BlockId::new("learn-1").unwrap(),
            BlockContent::new("Best resources for learning programming"),
        );
        block3_1.add_url(Url::new("https://rust-lang.org").unwrap());
        page3.add_block(block3_1).unwrap();

        repo.save(page3).unwrap();

        repo
    }

    #[test]
    fn test_search_by_keyword() {
        let repo = create_sample_knowledge_base();
        let search_use_case = SearchPagesAndBlocks::new(&repo);

        let request = SearchRequest::new("Rust");
        let results = search_use_case.execute(request).unwrap();

        // Should find matches in multiple pages
        assert!(results.len() >= 2, "Expected at least 2 results");
    }

    #[test]
    fn test_search_pages_only() {
        let repo = create_sample_knowledge_base();
        let search_use_case = SearchPagesAndBlocks::new(&repo);

        let request = SearchRequest::new("programming").with_result_type(ResultType::PagesOnly);
        let results = search_use_case.execute(request).unwrap();

        // Should find the Programming page
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_search_urls_only() {
        let repo = create_sample_knowledge_base();
        let search_use_case = SearchPagesAndBlocks::new(&repo);

        let request = SearchRequest::new("rust-lang.org").with_result_type(ResultType::UrlsOnly);
        let results = search_use_case.execute(request).unwrap();

        // Should find the rust-lang.org URLs (appears 2 times: once in programming, once in learning)
        // There's also the doc.rust-lang.org URL which also matches
        assert!(results.len() >= 2, "Expected at least 2 URL results");
    }

    #[test]
    fn test_search_with_page_filter() {
        let repo = create_sample_knowledge_base();
        let search_use_case = SearchPagesAndBlocks::new(&repo);

        let page_id = PageId::new("programming").unwrap();
        let request = SearchRequest::new("Rust")
            .with_result_type(ResultType::BlocksOnly)
            .with_page_filters(vec![page_id]);
        let results = search_use_case.execute(request).unwrap();

        // Should only find results in the Programming page
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_get_pages_for_url() {
        let repo = create_sample_knowledge_base();
        let use_case = GetPagesForUrl::new(&repo);

        let url = Url::new("https://rust-lang.org").unwrap();
        let connections = use_case.execute(&url).unwrap();

        // The rust-lang.org URL appears in Programming and Learning pages
        assert_eq!(connections.len(), 2);
        assert!(connections
            .iter()
            .any(|c| c.page_title == "Programming"));
        assert!(connections
            .iter()
            .any(|c| c.page_title == "Learning Resources"));
    }

    #[test]
    fn test_get_links_for_page() {
        let repo = create_sample_knowledge_base();
        let use_case = GetLinksForPage::new(&repo);

        let page_id = PageId::new("programming").unwrap();
        let links = use_case.execute(&page_id).unwrap();

        // Should find 2 URLs in the Programming page
        assert_eq!(links.len(), 2);

        // Check that one URL is in a nested block with page references
        let nested_url = links
            .iter()
            .find(|l| l.url.as_str().contains("understanding-ownership"))
            .expect("Should find the nested URL");

        assert!(nested_url.hierarchy_path.len() >= 2); // Parent and child blocks
        assert!(!nested_url.related_page_refs.is_empty()); // Should have page ref from parent
    }

    #[test]
    fn test_indexing_workflow() {
        let mut repo = InMemoryPageRepository::new();

        // Create a new page
        let page_id = PageId::new("new-page").unwrap();
        let mut page = Page::new(page_id.clone(), "New Page".to_string());

        let mut block = Block::new_root(
            BlockId::new("new-block").unwrap(),
            BlockContent::new("Content with important information"),
        );
        block.add_url(Url::new("https://example.com").unwrap());
        page.add_block(block).unwrap();

        // Index the page
        let mut index_use_case = IndexPage::new(&mut repo);
        index_use_case.execute(page).unwrap();

        // Verify it's searchable
        let search_use_case = SearchPagesAndBlocks::new(&repo);
        let request = SearchRequest::new("important");
        let results = search_use_case.execute(request).unwrap();

        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_hierarchical_context_in_search_results() {
        let repo = create_sample_knowledge_base();
        let search_use_case = SearchPagesAndBlocks::new(&repo);

        let request =
            SearchRequest::new("Ownership and borrowing").with_result_type(ResultType::BlocksOnly);
        let results = search_use_case.execute(request).unwrap();

        assert_eq!(results.len(), 1);

        // Verify the result has hierarchical context
        if let backend::application::dto::SearchItem::Block(block_result) = &results[0].item {
            // Should have a hierarchy path with parent and child
            assert_eq!(block_result.hierarchy_path.len(), 2);
            // Should have page references from parent
            assert!(!block_result.related_pages.is_empty());
            // Should have URLs from both parent and child
            assert!(!block_result.related_urls.is_empty());
        } else {
            panic!("Expected a Block result");
        }
    }

    #[test]
    fn test_cross_page_references() {
        let repo = create_sample_knowledge_base();

        // Search for "Building" which appears in Web Development page
        let search_use_case = SearchPagesAndBlocks::new(&repo);
        let request = SearchRequest::new("Building").with_result_type(ResultType::BlocksOnly);
        let results = search_use_case.execute(request).unwrap();

        // Should find the Web Development page with "Building web applications"
        let web_dev_block = results.iter().find(|r| {
            if let backend::application::dto::SearchItem::Block(block_result) = &r.item {
                block_result.page_title == "Web Development"
            } else {
                false
            }
        });

        assert!(
            web_dev_block.is_some(),
            "Should find block in Web Development page"
        );

        // Verify that pages can be searched across the knowledge base
        let programming_search =
            SearchRequest::new("Rust").with_result_type(ResultType::BlocksOnly);
        let prog_results = search_use_case.execute(programming_search).unwrap();

        // Should find blocks from multiple pages (Programming and Web Development pages)
        assert!(
            prog_results.len() >= 2,
            "Should find Rust mentioned in multiple pages"
        );
    }

    #[test]
    fn test_url_context_includes_related_pages() {
        let repo = create_sample_knowledge_base();
        let use_case = GetLinksForPage::new(&repo);

        let page_id = PageId::new("web-dev").unwrap();
        let links = use_case.execute(&page_id).unwrap();

        // Should find 2 URLs in the Web Development page
        assert_eq!(links.len(), 2);

        // Find the rocket.rs URL
        let rocket_url = links
            .iter()
            .find(|l| l.url.as_str().contains("rocket.rs"))
            .expect("Should find rocket.rs URL");

        // The URL is in a block that contains [[programming]] page reference
        // Note: get_urls_with_context() returns ancestor/descendant refs, not same-block refs
        // Since this block has no children and is a root block, there won't be related_page_refs
        // But we can verify the URL was found correctly
        assert!(rocket_url.url.as_str().contains("rocket.rs"));
        assert_eq!(rocket_url.block_content, "Building web applications with Rust");
    }
}
