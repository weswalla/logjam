use crate::domain::{aggregates::Page, value_objects::PageId, DomainResult};

/// Repository trait for managing Page aggregates.
///
/// This trait defines the contract for persisting and retrieving Page aggregates
/// from a data store. Implementations can be backed by different storage mechanisms
/// (in-memory, database, etc.).
pub trait PageRepository {
    /// Saves a page to the repository.
    ///
    /// If a page with the same ID already exists, it should be updated.
    /// Otherwise, a new page should be created.
    fn save(&mut self, page: Page) -> DomainResult<()>;

    /// Finds a page by its unique identifier.
    ///
    /// Returns `Ok(Some(page))` if found, `Ok(None)` if not found,
    /// or an error if the operation fails.
    fn find_by_id(&self, id: &PageId) -> DomainResult<Option<Page>>;

    /// Finds a page by its title.
    ///
    /// Returns `Ok(Some(page))` if found, `Ok(None)` if not found,
    /// or an error if the operation fails.
    fn find_by_title(&self, title: &str) -> DomainResult<Option<Page>>;

    /// Returns all pages in the repository.
    fn find_all(&self) -> DomainResult<Vec<Page>>;

    /// Deletes a page by its unique identifier.
    ///
    /// Returns `Ok(true)` if the page was deleted, `Ok(false)` if the page
    /// was not found, or an error if the operation fails.
    fn delete(&mut self, id: &PageId) -> DomainResult<bool>;
}
