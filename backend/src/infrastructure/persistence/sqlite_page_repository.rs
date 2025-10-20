use crate::application::repositories::PageRepository;
use crate::domain::aggregates::Page;
use crate::domain::base::{DomainError, Entity};
use crate::domain::entities::Block;
use crate::domain::value_objects::{
    BlockContent, BlockId, IndentLevel, PageId, PageReference, Url,
};
use crate::domain::DomainResult;
use rusqlite::{params, Connection, Result as SqliteResult};
use std::collections::HashMap;

/// SQLite-based implementation of the PageRepository trait
pub struct SqlitePageRepository {
    conn: Connection,
}

impl SqlitePageRepository {
    /// Create a new SQLite repository with the given connection
    pub fn new(conn: Connection) -> Self {
        SqlitePageRepository { conn }
    }

    /// Create a new in-memory SQLite repository (useful for testing)
    pub fn new_in_memory() -> SqliteResult<Self> {
        let conn = Connection::open_in_memory()?;
        super::schema::initialize_database(&conn)?;
        Ok(SqlitePageRepository { conn })
    }

    /// Create a new file-based SQLite repository
    pub fn new_with_path(path: impl AsRef<std::path::Path>) -> SqliteResult<Self> {
        let conn = Connection::open(path)?;
        super::schema::initialize_database(&conn)?;
        Ok(SqlitePageRepository { conn })
    }

    /// Save a page and all its blocks in a single transaction
    fn save_page_transaction(&mut self, page: Page) -> SqliteResult<()> {
        let tx = self.conn.transaction()?;

        // Insert or update page
        tx.execute(
            "INSERT OR REPLACE INTO pages (id, title, created_at, updated_at)
             VALUES (?1, ?2, datetime('now'), datetime('now'))",
            params![page.id().as_str(), page.title()],
        )?;

        // Delete existing blocks for this page
        tx.execute(
            "DELETE FROM blocks WHERE page_id = ?1",
            params![page.id().as_str()],
        )?;

        // Collect all blocks into a Vec and sort by indent level
        // This ensures parent blocks (lower indent) are inserted before children (higher indent)
        // which satisfies the FOREIGN KEY constraint on parent_id
        let mut blocks: Vec<&Block> = page.all_blocks().collect();
        blocks.sort_by_key(|b| b.indent_level().value());

        // First pass: Insert all blocks (without child relationships yet)
        for block in &blocks {
            // Insert block
            tx.execute(
                "INSERT INTO blocks (id, page_id, parent_id, content, indent_level, position, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, 0, datetime('now'), datetime('now'))",
                params![
                    block.id().as_str(),
                    page.id().as_str(),
                    block.parent_id().map(|id| id.as_str()),
                    block.content().as_str(),
                    block.indent_level().value() as i64,
                ],
            )?;

            // Insert URLs
            for url in block.urls() {
                tx.execute(
                    "INSERT INTO urls (block_id, url) VALUES (?1, ?2)",
                    params![block.id().as_str(), url.as_str()],
                )?;
            }

            // Insert page references
            for page_ref in block.page_references() {
                tx.execute(
                    "INSERT INTO page_references (block_id, title, is_tag)
                     VALUES (?1, ?2, ?3)",
                    params![
                        block.id().as_str(),
                        page_ref.title(),
                        if page_ref.is_tag() { 1 } else { 0 }
                    ],
                )?;
            }
        }

        // Second pass: Insert block_children relationships
        // Now all blocks exist, so foreign key constraints will be satisfied
        for block in &blocks {
            for (idx, child_id) in block.child_ids().iter().enumerate() {
                tx.execute(
                    "INSERT INTO block_children (parent_id, child_id, position)
                     VALUES (?1, ?2, ?3)",
                    params![block.id().as_str(), child_id.as_str(), idx as i64],
                )?;
            }
        }

        tx.commit()?;
        Ok(())
    }

    /// Load a page with all its blocks and relationships
    fn load_page(&self, page_id: &PageId) -> SqliteResult<Option<Page>> {
        // Load page metadata
        let page_result: Result<(String, String), _> = self.conn.query_row(
            "SELECT id, title FROM pages WHERE id = ?1",
            params![page_id.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        );

        let (_, title) = match page_result {
            Ok(data) => data,
            Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
            Err(e) => return Err(e),
        };

        // Create page
        let mut page = Page::new(page_id.clone(), title);

        // Load all blocks for this page
        let mut stmt = self.conn.prepare(
            "SELECT id, parent_id, content, indent_level
             FROM blocks
             WHERE page_id = ?1
             ORDER BY position",
        )?;

        let blocks_data: Vec<(String, Option<String>, String, i64)> = stmt
            .query_map(params![page_id.as_str()], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })?
            .collect::<SqliteResult<Vec<_>>>()?;

        // Load URLs for all blocks in this page
        let mut url_map: HashMap<BlockId, Vec<Url>> = HashMap::new();
        let mut url_stmt = self
            .conn
            .prepare("SELECT block_id, url FROM urls WHERE block_id IN (SELECT id FROM blocks WHERE page_id = ?1)")?;

        let urls_data: Vec<(String, String)> = url_stmt
            .query_map(params![page_id.as_str()], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })?
            .collect::<SqliteResult<Vec<_>>>()?;

        for (block_id_str, url_str) in urls_data {
            let block_id = BlockId::new(block_id_str).map_err(|_| {
                rusqlite::Error::InvalidQuery // Convert domain error to sqlite error
            })?;
            let url = Url::new(url_str).map_err(|_| rusqlite::Error::InvalidQuery)?;

            url_map.entry(block_id).or_insert_with(Vec::new).push(url);
        }

        // Load page references for all blocks in this page
        let mut ref_map: HashMap<BlockId, Vec<PageReference>> = HashMap::new();
        let mut ref_stmt = self.conn.prepare(
            "SELECT block_id, title, is_tag FROM page_references WHERE block_id IN (SELECT id FROM blocks WHERE page_id = ?1)",
        )?;

        let refs_data: Vec<(String, String, i32)> = ref_stmt
            .query_map(params![page_id.as_str()], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?
            .collect::<SqliteResult<Vec<_>>>()?;

        for (block_id_str, title, is_tag) in refs_data {
            let block_id = BlockId::new(block_id_str).map_err(|_| rusqlite::Error::InvalidQuery)?;
            let page_ref = if is_tag != 0 {
                PageReference::from_tag(title)
            } else {
                PageReference::from_brackets(title)
            }
            .map_err(|_| rusqlite::Error::InvalidQuery)?;

            ref_map
                .entry(block_id)
                .or_insert_with(Vec::new)
                .push(page_ref);
        }

        // Load child relationships
        let mut child_map: HashMap<BlockId, Vec<BlockId>> = HashMap::new();
        let mut child_stmt = self.conn.prepare(
            "SELECT parent_id, child_id FROM block_children
             WHERE parent_id IN (SELECT id FROM blocks WHERE page_id = ?1)
             ORDER BY position",
        )?;

        let children_data: Vec<(String, String)> = child_stmt
            .query_map(params![page_id.as_str()], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })?
            .collect::<SqliteResult<Vec<_>>>()?;

        for (parent_id_str, child_id_str) in children_data {
            let parent_id =
                BlockId::new(parent_id_str).map_err(|_| rusqlite::Error::InvalidQuery)?;
            let child_id = BlockId::new(child_id_str).map_err(|_| rusqlite::Error::InvalidQuery)?;

            child_map
                .entry(parent_id)
                .or_insert_with(Vec::new)
                .push(child_id);
        }

        // Build blocks in correct order (parents before children)
        // We need to build root blocks first, then children
        let mut blocks_to_add: Vec<Block> = Vec::new();

        for (id_str, parent_id_opt, content_str, indent_level) in blocks_data {
            let block_id = BlockId::new(id_str).map_err(|_| rusqlite::Error::InvalidQuery)?;
            let content = BlockContent::new(content_str);
            let indent = IndentLevel::new(indent_level as usize);

            let mut block = if let Some(parent_id_str) = parent_id_opt {
                let parent_id =
                    BlockId::new(parent_id_str).map_err(|_| rusqlite::Error::InvalidQuery)?;
                Block::new_child(block_id.clone(), content, parent_id, indent)
            } else {
                Block::new_root(block_id.clone(), content)
            };

            // Add URLs
            if let Some(urls) = url_map.get(&block_id) {
                for url in urls {
                    block.add_url(url.clone());
                }
            }

            // Add page references
            if let Some(refs) = ref_map.get(&block_id) {
                for page_ref in refs {
                    block.add_page_reference(page_ref.clone());
                }
            }

            blocks_to_add.push(block);
        }

        // Add blocks to page (roots first, then children)
        // Sort blocks so parents come before children
        blocks_to_add.sort_by_key(|b| b.indent_level().value());

        for block in blocks_to_add {
            page.add_block(block)
                .map_err(|_| rusqlite::Error::InvalidQuery)?;
        }

        Ok(Some(page))
    }
}

impl PageRepository for SqlitePageRepository {
    fn save(&mut self, page: Page) -> DomainResult<()> {
        self.save_page_transaction(page)
            .map_err(|e| DomainError::InvalidOperation(format!("Database error: {}", e)))
    }

    fn find_by_id(&self, id: &PageId) -> DomainResult<Option<Page>> {
        self.load_page(id)
            .map_err(|e| DomainError::InvalidOperation(format!("Database error: {}", e)))
    }

    fn find_by_title(&self, title: &str) -> DomainResult<Option<Page>> {
        // First, find the page ID by title
        let page_id_result: Result<String, _> = self.conn.query_row(
            "SELECT id FROM pages WHERE title = ?1",
            params![title],
            |row| row.get(0),
        );

        let page_id_str = match page_id_result {
            Ok(id) => id,
            Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
            Err(e) => {
                return Err(DomainError::InvalidOperation(format!(
                    "Database error: {}",
                    e
                )))
            }
        };

        let page_id = PageId::new(page_id_str)?;
        self.find_by_id(&page_id)
    }

    fn find_all(&self) -> DomainResult<Vec<Page>> {
        // Get all page IDs
        let mut stmt = self
            .conn
            .prepare("SELECT id FROM pages")
            .map_err(|e| DomainError::InvalidOperation(format!("Database error: {}", e)))?;

        let page_ids: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .map_err(|e| DomainError::InvalidOperation(format!("Database error: {}", e)))?
            .collect::<SqliteResult<Vec<_>>>()
            .map_err(|e| DomainError::InvalidOperation(format!("Database error: {}", e)))?;

        // Load each page
        let mut pages = Vec::new();
        for id_str in page_ids {
            let page_id = PageId::new(id_str)?;
            if let Some(page) = self.find_by_id(&page_id)? {
                pages.push(page);
            }
        }

        Ok(pages)
    }

    fn delete(&mut self, id: &PageId) -> DomainResult<bool> {
        let rows_affected = self
            .conn
            .execute("DELETE FROM pages WHERE id = ?1", params![id.as_str()])
            .map_err(|e| DomainError::InvalidOperation(format!("Database error: {}", e)))?;

        Ok(rows_affected > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::value_objects::IndentLevel;

    fn create_test_page() -> Page {
        let page_id = PageId::new("test-page-1").unwrap();
        let mut page = Page::new(page_id, "Test Page".to_string());

        // Add root block
        let root_id = BlockId::new("block-1").unwrap();
        let mut root_block = Block::new_root(root_id.clone(), BlockContent::new("Root block"));
        root_block.add_url(Url::new("https://example.com").unwrap());
        root_block.add_page_reference(PageReference::from_brackets("referenced-page").unwrap());
        page.add_block(root_block).unwrap();

        // Add child block
        let child_id = BlockId::new("block-2").unwrap();
        let mut child_block = Block::new_child(
            child_id,
            BlockContent::new("Child block"),
            root_id,
            IndentLevel::new(1),
        );
        child_block.add_page_reference(PageReference::from_tag("tag").unwrap());
        page.add_block(child_block).unwrap();

        page
    }

    #[test]
    fn test_save_and_find_by_id() {
        let mut repo = SqlitePageRepository::new_in_memory().unwrap();
        let page = create_test_page();
        let page_id = page.id().clone();

        // Save page
        repo.save(page.clone()).unwrap();

        // Load page
        let loaded_page = repo.find_by_id(&page_id).unwrap().unwrap();

        assert_eq!(loaded_page.id(), page.id());
        assert_eq!(loaded_page.title(), page.title());
        assert_eq!(loaded_page.root_blocks().len(), page.root_blocks().len());
    }

    #[test]
    fn test_find_by_title() {
        let mut repo = SqlitePageRepository::new_in_memory().unwrap();
        let page = create_test_page();
        let title = page.title().to_string();

        repo.save(page.clone()).unwrap();

        let loaded_page = repo.find_by_title(&title).unwrap().unwrap();
        assert_eq!(loaded_page.id(), page.id());
        assert_eq!(loaded_page.title(), title);
    }

    #[test]
    fn test_find_by_title_not_found() {
        let repo = SqlitePageRepository::new_in_memory().unwrap();
        let result = repo.find_by_title("nonexistent").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_find_all() {
        let mut repo = SqlitePageRepository::new_in_memory().unwrap();

        // Create and save multiple pages
        let page1 = create_test_page();
        let page2_id = PageId::new("test-page-2").unwrap();
        let page2 = Page::new(page2_id, "Test Page 2".to_string());

        repo.save(page1).unwrap();
        repo.save(page2).unwrap();

        // Load all pages
        let pages = repo.find_all().unwrap();
        assert_eq!(pages.len(), 2);
    }

    #[test]
    fn test_delete() {
        let mut repo = SqlitePageRepository::new_in_memory().unwrap();
        let page = create_test_page();
        let page_id = page.id().clone();

        repo.save(page).unwrap();

        // Delete page
        let deleted = repo.delete(&page_id).unwrap();
        assert!(deleted);

        // Verify page is gone
        let result = repo.find_by_id(&page_id).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_delete_nonexistent() {
        let mut repo = SqlitePageRepository::new_in_memory().unwrap();
        let page_id = PageId::new("nonexistent").unwrap();

        let deleted = repo.delete(&page_id).unwrap();
        assert!(!deleted);
    }

    #[test]
    fn test_update_page() {
        let mut repo = SqlitePageRepository::new_in_memory().unwrap();
        let page = create_test_page();
        let page_id = page.id().clone();

        // Save original page
        repo.save(page).unwrap();

        // Modify and save again
        let mut updated_page = Page::new(page_id.clone(), "Updated Title".to_string());
        let new_block = Block::new_root(
            BlockId::new("block-3").unwrap(),
            BlockContent::new("New block"),
        );
        updated_page.add_block(new_block).unwrap();

        repo.save(updated_page.clone()).unwrap();

        // Load and verify
        let loaded_page = repo.find_by_id(&page_id).unwrap().unwrap();
        assert_eq!(loaded_page.title(), "Updated Title");
        assert_eq!(loaded_page.root_blocks().len(), 1);
    }

    #[test]
    fn test_block_hierarchy_preserved() {
        let mut repo = SqlitePageRepository::new_in_memory().unwrap();

        // Create page with deep hierarchy
        let page_id = PageId::new("hierarchy-test").unwrap();
        let mut page = Page::new(page_id.clone(), "Hierarchy Test".to_string());

        let root_id = BlockId::new("root").unwrap();
        let root = Block::new_root(root_id.clone(), BlockContent::new("Root"));
        page.add_block(root).unwrap();

        let child1_id = BlockId::new("child1").unwrap();
        let child1 = Block::new_child(
            child1_id.clone(),
            BlockContent::new("Child 1"),
            root_id.clone(),
            IndentLevel::new(1),
        );
        page.add_block(child1).unwrap();

        let child2_id = BlockId::new("child2").unwrap();
        let child2 = Block::new_child(
            child2_id.clone(),
            BlockContent::new("Child 2"),
            child1_id.clone(),
            IndentLevel::new(2),
        );
        page.add_block(child2).unwrap();

        // Save and load
        repo.save(page).unwrap();
        let loaded_page = repo.find_by_id(&page_id).unwrap().unwrap();

        // Verify hierarchy
        assert_eq!(loaded_page.root_blocks().len(), 1);
        let root = loaded_page.get_block(&root_id).unwrap();
        assert_eq!(root.child_ids().len(), 1);

        let child1 = loaded_page.get_block(&child1_id).unwrap();
        assert_eq!(child1.parent_id(), Some(&root_id));
        assert_eq!(child1.child_ids().len(), 1);

        let child2 = loaded_page.get_block(&child2_id).unwrap();
        assert_eq!(child2.parent_id(), Some(&child1_id));
    }

    #[test]
    fn test_urls_and_references_preserved() {
        let mut repo = SqlitePageRepository::new_in_memory().unwrap();
        let page = create_test_page();
        let page_id = page.id().clone();

        repo.save(page).unwrap();
        let loaded_page = repo.find_by_id(&page_id).unwrap().unwrap();

        // Check URLs
        let root_block = &loaded_page.root_blocks()[0];
        assert_eq!(root_block.urls().len(), 1);
        assert_eq!(root_block.urls()[0].as_str(), "https://example.com");

        // Check page references
        assert_eq!(root_block.page_references().len(), 1);
        assert_eq!(root_block.page_references()[0].title(), "referenced-page");
        assert!(!root_block.page_references()[0].is_tag());

        // Check child block tags
        let child_id = BlockId::new("block-2").unwrap();
        let child_block = loaded_page.get_block(&child_id).unwrap();
        assert_eq!(child_block.page_references().len(), 1);
        assert_eq!(child_block.page_references()[0].title(), "tag");
        assert!(child_block.page_references()[0].is_tag());
    }
}
