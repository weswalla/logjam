use crate::application::repositories::PageRepository;
use crate::domain::{aggregates::Page, DomainResult};

/// Use case for indexing a page
///
/// This use case handles the process of saving a page to the repository,
/// making it available for search and retrieval.
pub struct IndexPage<'a, R: PageRepository> {
    repository: &'a mut R,
}

impl<'a, R: PageRepository> IndexPage<'a, R> {
    pub fn new(repository: &'a mut R) -> Self {
        Self { repository }
    }

    /// Index a page, making it available for search
    ///
    /// This will save the page to the repository. If a page with the same ID
    /// already exists, it will be updated.
    pub fn execute(&mut self, page: Page) -> DomainResult<()> {
        self.repository.save(page)?;
        Ok(())
    }
}

/// Use case for batch indexing multiple pages
///
/// This use case handles the process of indexing multiple pages at once,
/// which is useful for initial imports or bulk updates.
pub struct BatchIndexPages<'a, R: PageRepository> {
    repository: &'a mut R,
}

impl<'a, R: PageRepository> BatchIndexPages<'a, R> {
    pub fn new(repository: &'a mut R) -> Self {
        Self { repository }
    }

    /// Index multiple pages in a batch
    ///
    /// Returns the number of pages successfully indexed.
    pub fn execute(&mut self, pages: Vec<Page>) -> DomainResult<usize> {
        let mut count = 0;
        for page in pages {
            self.repository.save(page)?;
            count += 1;
        }
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
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
    fn test_index_page() {
        let mut repo = InMemoryPageRepository::new();

        let page_id = PageId::new("page-1").unwrap();
        let page = Page::new(page_id.clone(), "Test Page".to_string());

        let mut use_case = IndexPage::new(&mut repo);
        use_case.execute(page).unwrap();

        // Verify the page was indexed
        let retrieved = repo.find_by_id(&page_id).unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().title(), "Test Page");
    }

    #[test]
    fn test_index_page_update_existing() {
        let mut repo = InMemoryPageRepository::new();

        let page_id = PageId::new("page-1").unwrap();
        let page1 = Page::new(page_id.clone(), "Original Title".to_string());

        let mut use_case = IndexPage::new(&mut repo);
        use_case.execute(page1).unwrap();

        // Update with same ID but different content
        let mut page2 = Page::new(page_id.clone(), "Updated Title".to_string());
        let block = Block::new_root(
            BlockId::new("block-1").unwrap(),
            BlockContent::new("New content"),
        );
        page2.add_block(block).unwrap();

        let mut use_case2 = IndexPage::new(&mut repo);
        use_case2.execute(page2).unwrap();

        // Verify the page was updated
        let retrieved = repo.find_by_id(&page_id).unwrap().unwrap();
        assert_eq!(retrieved.title(), "Updated Title");
        assert_eq!(retrieved.all_blocks().count(), 1);
    }

    #[test]
    fn test_batch_index_pages() {
        let mut repo = InMemoryPageRepository::new();

        let pages = vec![
            Page::new(PageId::new("page-1").unwrap(), "Page 1".to_string()),
            Page::new(PageId::new("page-2").unwrap(), "Page 2".to_string()),
            Page::new(PageId::new("page-3").unwrap(), "Page 3".to_string()),
        ];

        let mut use_case = BatchIndexPages::new(&mut repo);
        let count = use_case.execute(pages).unwrap();

        assert_eq!(count, 3);
        assert_eq!(repo.find_all().unwrap().len(), 3);
    }

    #[test]
    fn test_batch_index_empty() {
        let mut repo = InMemoryPageRepository::new();

        let mut use_case = BatchIndexPages::new(&mut repo);
        let count = use_case.execute(vec![]).unwrap();

        assert_eq!(count, 0);
    }
}
