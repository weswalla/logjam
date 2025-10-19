use crate::application::{dto::PageConnection, repositories::PageRepository};
use crate::domain::{base::Entity, value_objects::Url, DomainResult};

/// Use case for finding all pages connected to a URL
///
/// Given a URL, this use case finds all pages that contain the URL in any of their blocks,
/// providing the context of which blocks contain the URL.
pub struct GetPagesForUrl<'a, R: PageRepository> {
    repository: &'a R,
}

impl<'a, R: PageRepository> GetPagesForUrl<'a, R> {
    pub fn new(repository: &'a R) -> Self {
        Self { repository }
    }

    /// Find all pages that contain the given URL
    pub fn execute(&self, url: &Url) -> DomainResult<Vec<PageConnection>> {
        let all_pages = self.repository.find_all()?;
        let mut connections = Vec::new();

        for page in all_pages {
            let mut blocks_with_url = Vec::new();

            // Find all blocks in this page that contain the URL
            for block in page.all_blocks() {
                if block.urls().iter().any(|u| u == url) {
                    blocks_with_url.push(block.id().clone());
                }
            }

            // If we found any blocks with this URL, add the page connection
            if !blocks_with_url.is_empty() {
                connections.push(PageConnection {
                    page_id: page.id().clone(),
                    page_title: page.title().to_string(),
                    blocks_with_url,
                });
            }
        }

        Ok(connections)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        aggregates::Page,
        base::Entity,
        entities::Block,
        value_objects::{BlockContent, BlockId, PageId},
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
    fn test_get_pages_for_url_single_page() {
        let mut repo = InMemoryPageRepository::new();

        let page_id = PageId::new("page-1").unwrap();
        let mut page = Page::new(page_id, "Page 1".to_string());

        let mut block = Block::new_root(
            BlockId::new("block-1").unwrap(),
            BlockContent::new("Check this out"),
        );
        let url = Url::new("https://example.com").unwrap();
        block.add_url(url.clone());
        page.add_block(block).unwrap();

        repo.save(page).unwrap();

        let use_case = GetPagesForUrl::new(&repo);
        let connections = use_case.execute(&url).unwrap();

        assert_eq!(connections.len(), 1);
        assert_eq!(connections[0].page_title, "Page 1");
        assert_eq!(connections[0].blocks_with_url.len(), 1);
    }

    #[test]
    fn test_get_pages_for_url_multiple_pages() {
        let mut repo = InMemoryPageRepository::new();
        let url = Url::new("https://example.com").unwrap();

        // Create two pages with the same URL
        for i in 1..=2 {
            let page_id = PageId::new(format!("page-{}", i)).unwrap();
            let mut page = Page::new(page_id, format!("Page {}", i));

            let mut block = Block::new_root(
                BlockId::new(format!("block-{}", i)).unwrap(),
                BlockContent::new("Link here"),
            );
            block.add_url(url.clone());
            page.add_block(block).unwrap();

            repo.save(page).unwrap();
        }

        let use_case = GetPagesForUrl::new(&repo);
        let connections = use_case.execute(&url).unwrap();

        assert_eq!(connections.len(), 2);
    }

    #[test]
    fn test_get_pages_for_url_multiple_blocks_same_page() {
        let mut repo = InMemoryPageRepository::new();

        let page_id = PageId::new("page-1").unwrap();
        let mut page = Page::new(page_id, "Page 1".to_string());

        let url = Url::new("https://example.com").unwrap();

        // Add two blocks with the same URL
        for i in 1..=2 {
            let mut block = Block::new_root(
                BlockId::new(format!("block-{}", i)).unwrap(),
                BlockContent::new(format!("Block {}", i)),
            );
            block.add_url(url.clone());
            page.add_block(block).unwrap();
        }

        repo.save(page).unwrap();

        let use_case = GetPagesForUrl::new(&repo);
        let connections = use_case.execute(&url).unwrap();

        assert_eq!(connections.len(), 1);
        assert_eq!(connections[0].blocks_with_url.len(), 2);
    }

    #[test]
    fn test_get_pages_for_url_not_found() {
        let repo = InMemoryPageRepository::new();
        let url = Url::new("https://notfound.com").unwrap();

        let use_case = GetPagesForUrl::new(&repo);
        let connections = use_case.execute(&url).unwrap();

        assert_eq!(connections.len(), 0);
    }
}
