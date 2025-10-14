use crate::application::{dto::UrlWithContext, repositories::PageRepository};
use crate::domain::{value_objects::PageId, DomainResult};

/// Use case for getting all links associated with a page
///
/// Given a page, this use case retrieves all URLs in the page along with their
/// hierarchical context (path to the block, related page references).
pub struct GetLinksForPage<'a, R: PageRepository> {
    repository: &'a R,
}

impl<'a, R: PageRepository> GetLinksForPage<'a, R> {
    pub fn new(repository: &'a R) -> Self {
        Self { repository }
    }

    /// Get all URLs in the page with their context
    pub fn execute(&self, page_id: &PageId) -> DomainResult<Vec<UrlWithContext>> {
        let page = self
            .repository
            .find_by_id(page_id)?
            .ok_or_else(|| {
                crate::domain::DomainError::NotFound(format!("Page with id {:?} not found", page_id))
            })?;

        let mut results = Vec::new();

        // Get all URLs with their hierarchical context
        let urls_with_refs = page.get_urls_with_context();

        for (url, ancestor_refs, descendant_refs) in urls_with_refs {
            // Find the block containing this URL
            if let Some(block) = page
                .all_blocks()
                .find(|b| b.urls().iter().any(|u| u == url))
            {
                // Get the hierarchy path to this block
                let hierarchy_path = page
                    .get_hierarchy_path(block.id())
                    .iter()
                    .map(|b| b.content().as_str().to_string())
                    .collect();

                // Combine ancestor and descendant page references
                let mut related_page_refs = Vec::new();
                related_page_refs.extend(ancestor_refs.iter().map(|r| (*r).clone()));
                related_page_refs.extend(descendant_refs.iter().map(|r| (*r).clone()));

                results.push(UrlWithContext {
                    url: url.clone(),
                    block_id: block.id().clone(),
                    block_content: block.content().as_str().to_string(),
                    hierarchy_path,
                    related_page_refs,
                });
            }
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        aggregates::Page,
        base::Entity,
        entities::Block,
        value_objects::{BlockContent, BlockId, IndentLevel, PageReference, Url},
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

    #[test]
    fn test_get_links_for_page_single_url() {
        let mut repo = InMemoryPageRepository::new();

        let page_id = PageId::new("page-1").unwrap();
        let mut page = Page::new(page_id.clone(), "Page 1".to_string());

        let mut block = Block::new_root(
            BlockId::new("block-1").unwrap(),
            BlockContent::new("Check this link"),
        );
        block.add_url(Url::new("https://example.com").unwrap());
        page.add_block(block).unwrap();

        repo.save(page).unwrap();

        let use_case = GetLinksForPage::new(&repo);
        let links = use_case.execute(&page_id).unwrap();

        assert_eq!(links.len(), 1);
        assert_eq!(links[0].url.as_str(), "https://example.com");
        assert_eq!(links[0].block_content, "Check this link");
    }

    #[test]
    fn test_get_links_for_page_multiple_urls() {
        let mut repo = InMemoryPageRepository::new();

        let page_id = PageId::new("page-1").unwrap();
        let mut page = Page::new(page_id.clone(), "Page 1".to_string());

        // Add two blocks with different URLs
        let mut block1 = Block::new_root(
            BlockId::new("block-1").unwrap(),
            BlockContent::new("First link"),
        );
        block1.add_url(Url::new("https://example.com").unwrap());
        page.add_block(block1).unwrap();

        let mut block2 = Block::new_root(
            BlockId::new("block-2").unwrap(),
            BlockContent::new("Second link"),
        );
        block2.add_url(Url::new("https://test.com").unwrap());
        page.add_block(block2).unwrap();

        repo.save(page).unwrap();

        let use_case = GetLinksForPage::new(&repo);
        let links = use_case.execute(&page_id).unwrap();

        assert_eq!(links.len(), 2);
    }

    #[test]
    fn test_get_links_for_page_with_hierarchy() {
        let mut repo = InMemoryPageRepository::new();

        let page_id = PageId::new("page-1").unwrap();
        let mut page = Page::new(page_id.clone(), "Page 1".to_string());

        // Create parent block with page reference
        let mut parent = Block::new_root(
            BlockId::new("parent").unwrap(),
            BlockContent::new("Parent block"),
        );
        parent.add_page_reference(PageReference::from_brackets("topic").unwrap());
        page.add_block(parent).unwrap();

        // Create child block with URL
        let parent_id = BlockId::new("parent").unwrap();
        let mut child = Block::new_child(
            BlockId::new("child").unwrap(),
            BlockContent::new("Child block with link"),
            parent_id.clone(),
            IndentLevel::new(1),
        );
        child.add_url(Url::new("https://example.com").unwrap());

        // Update parent's children
        if let Some(parent_block) = page.get_block_mut(&parent_id) {
            parent_block.add_child(child.id().clone());
        }
        page.add_block(child).unwrap();

        repo.save(page).unwrap();

        let use_case = GetLinksForPage::new(&repo);
        let links = use_case.execute(&page_id).unwrap();

        assert_eq!(links.len(), 1);
        assert_eq!(links[0].hierarchy_path.len(), 2); // Parent and child
        assert!(!links[0].related_page_refs.is_empty()); // Should have the page ref from parent
    }

    #[test]
    fn test_get_links_for_page_not_found() {
        let repo = InMemoryPageRepository::new();
        let page_id = PageId::new("nonexistent").unwrap();

        let use_case = GetLinksForPage::new(&repo);
        let result = use_case.execute(&page_id);

        assert!(result.is_err());
    }

    #[test]
    fn test_get_links_for_page_no_urls() {
        let mut repo = InMemoryPageRepository::new();

        let page_id = PageId::new("page-1").unwrap();
        let mut page = Page::new(page_id.clone(), "Page 1".to_string());

        let block = Block::new_root(
            BlockId::new("block-1").unwrap(),
            BlockContent::new("No links here"),
        );
        page.add_block(block).unwrap();

        repo.save(page).unwrap();

        let use_case = GetLinksForPage::new(&repo);
        let links = use_case.execute(&page_id).unwrap();

        assert_eq!(links.len(), 0);
    }
}
