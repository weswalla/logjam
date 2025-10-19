/// Domain entities
use super::base::Entity;
use super::value_objects::{BlockContent, BlockId, IndentLevel, PageReference, Url};

/// A Block represents a single bullet point in Logseq
/// Blocks form a tree structure where each block can have a parent and children
#[derive(Debug, Clone)]
pub struct Block {
    id: BlockId,
    content: BlockContent,
    indent_level: IndentLevel,
    parent_id: Option<BlockId>,
    child_ids: Vec<BlockId>,
    urls: Vec<Url>,
    page_references: Vec<PageReference>,
}

impl Block {
    /// Create a new root-level block
    pub fn new_root(id: BlockId, content: BlockContent) -> Self {
        Block {
            id,
            content,
            indent_level: IndentLevel::root(),
            parent_id: None,
            child_ids: Vec::new(),
            urls: Vec::new(),
            page_references: Vec::new(),
        }
    }

    /// Create a new child block
    pub fn new_child(
        id: BlockId,
        content: BlockContent,
        parent_id: BlockId,
        indent_level: IndentLevel,
    ) -> Self {
        Block {
            id,
            content,
            indent_level,
            parent_id: Some(parent_id),
            child_ids: Vec::new(),
            urls: Vec::new(),
            page_references: Vec::new(),
        }
    }

    /// Get the block's ID
    pub fn id(&self) -> &BlockId {
        &self.id
    }

    /// Get the block's content
    pub fn content(&self) -> &BlockContent {
        &self.content
    }

    /// Get the indent level
    pub fn indent_level(&self) -> IndentLevel {
        self.indent_level
    }

    /// Get the parent block ID, if any
    pub fn parent_id(&self) -> Option<&BlockId> {
        self.parent_id.as_ref()
    }

    /// Get the child block IDs
    pub fn child_ids(&self) -> &[BlockId] {
        &self.child_ids
    }

    /// Check if this is a root-level block
    pub fn is_root(&self) -> bool {
        self.parent_id.is_none()
    }

    /// Check if this block has children
    pub fn has_children(&self) -> bool {
        !self.child_ids.is_empty()
    }

    /// Add a child block ID
    pub fn add_child(&mut self, child_id: BlockId) {
        if !self.child_ids.contains(&child_id) {
            self.child_ids.push(child_id);
        }
    }

    /// Remove a child block ID
    pub fn remove_child(&mut self, child_id: &BlockId) {
        self.child_ids.retain(|id| id != child_id);
    }

    /// Get all URLs in this block
    pub fn urls(&self) -> &[Url] {
        &self.urls
    }

    /// Add a URL to this block
    pub fn add_url(&mut self, url: Url) {
        if !self.urls.contains(&url) {
            self.urls.push(url);
        }
    }

    /// Get all page references in this block
    pub fn page_references(&self) -> &[PageReference] {
        &self.page_references
    }

    /// Add a page reference to this block
    pub fn add_page_reference(&mut self, reference: PageReference) {
        if !self.page_references.contains(&reference) {
            self.page_references.push(reference);
        }
    }

    /// Update the block's content
    pub fn update_content(&mut self, content: BlockContent) {
        self.content = content;
    }

    /// Set the parent block ID
    pub fn set_parent(&mut self, parent_id: Option<BlockId>) {
        self.parent_id = parent_id;
    }
}

impl Entity for Block {
    type Id = BlockId;

    fn id(&self) -> &Self::Id {
        &self.id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_root_block() {
        let id = BlockId::new("block-1").unwrap();
        let content = BlockContent::new("Root block content");
        let block = Block::new_root(id.clone(), content);

        assert_eq!(block.id().as_str(), "block-1");
        assert!(block.is_root());
        assert_eq!(block.indent_level(), IndentLevel::root());
        assert!(block.parent_id().is_none());
        assert!(!block.has_children());
    }

    #[test]
    fn test_create_child_block() {
        let parent_id = BlockId::new("block-1").unwrap();
        let child_id = BlockId::new("block-2").unwrap();
        let content = BlockContent::new("Child block content");

        let block = Block::new_child(
            child_id.clone(),
            content,
            parent_id.clone(),
            IndentLevel::new(1),
        );

        assert_eq!(block.id().as_str(), "block-2");
        assert!(!block.is_root());
        assert_eq!(block.indent_level(), IndentLevel::new(1));
        assert_eq!(block.parent_id(), Some(&parent_id));
    }

    #[test]
    fn test_add_child_to_block() {
        let id = BlockId::new("block-1").unwrap();
        let content = BlockContent::new("Parent block");
        let mut block = Block::new_root(id, content);

        let child_id = BlockId::new("block-2").unwrap();
        block.add_child(child_id.clone());

        assert!(block.has_children());
        assert_eq!(block.child_ids().len(), 1);
        assert_eq!(block.child_ids()[0], child_id);
    }

    #[test]
    fn test_add_duplicate_child_ignored() {
        let id = BlockId::new("block-1").unwrap();
        let content = BlockContent::new("Parent block");
        let mut block = Block::new_root(id, content);

        let child_id = BlockId::new("block-2").unwrap();
        block.add_child(child_id.clone());
        block.add_child(child_id.clone());

        assert_eq!(block.child_ids().len(), 1);
    }

    #[test]
    fn test_remove_child_from_block() {
        let id = BlockId::new("block-1").unwrap();
        let content = BlockContent::new("Parent block");
        let mut block = Block::new_root(id, content);

        let child_id = BlockId::new("block-2").unwrap();
        block.add_child(child_id.clone());
        assert!(block.has_children());

        block.remove_child(&child_id);
        assert!(!block.has_children());
    }

    #[test]
    fn test_add_url_to_block() {
        let id = BlockId::new("block-1").unwrap();
        let content = BlockContent::new("Block with URL");
        let mut block = Block::new_root(id, content);

        let url = Url::new("https://example.com").unwrap();
        block.add_url(url.clone());

        assert_eq!(block.urls().len(), 1);
        assert_eq!(block.urls()[0], url);
    }

    #[test]
    fn test_add_page_reference_to_block() {
        let id = BlockId::new("block-1").unwrap();
        let content = BlockContent::new("Block with reference");
        let mut block = Block::new_root(id, content);

        let reference = PageReference::from_brackets("referenced-page").unwrap();
        block.add_page_reference(reference.clone());

        assert_eq!(block.page_references().len(), 1);
        assert_eq!(block.page_references()[0], reference);
    }

    #[test]
    fn test_update_block_content() {
        let id = BlockId::new("block-1").unwrap();
        let content = BlockContent::new("Original content");
        let mut block = Block::new_root(id, content);

        let new_content = BlockContent::new("Updated content");
        block.update_content(new_content.clone());

        assert_eq!(block.content(), &new_content);
    }
}
