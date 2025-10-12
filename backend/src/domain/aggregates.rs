/// Domain aggregates
use super::base::{AggregateRoot, DomainError, DomainEvent, DomainResult, Entity};
use super::entities::Block;
use super::events::DomainEventEnum;
use super::value_objects::{BlockId, PageId, PageReference, Url};
use std::collections::HashMap;

/// A Page is an aggregate root that represents a Logseq page (markdown file)
/// It contains a tree of blocks and manages the relationships between them
#[derive(Debug, Clone)]
pub struct Page {
    id: PageId,
    title: String,
    blocks: HashMap<BlockId, Block>,
    root_block_ids: Vec<BlockId>,
}

impl Page {
    /// Create a new empty page
    pub fn new(id: PageId, title: String) -> Self {
        Page {
            id,
            title,
            blocks: HashMap::new(),
            root_block_ids: Vec::new(),
        }
    }

    /// Get the page title
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Update the page title
    pub fn set_title(&mut self, title: String) {
        self.title = title;
    }

    /// Add a block to the page
    pub fn add_block(&mut self, block: Block) -> DomainResult<()> {
        let block_id = block.id().clone();
        let parent_id = block.parent_id().cloned();

        // If block has a parent, verify the parent exists
        if let Some(ref pid) = parent_id {
            if !self.blocks.contains_key(pid) {
                return Err(DomainError::InvalidOperation(format!(
                    "Parent block {} does not exist",
                    pid
                )));
            }
        }

        // Insert the block first
        self.blocks.insert(block_id.clone(), block);

        // Then update parent-child relationships
        if let Some(pid) = parent_id {
            if let Some(parent) = self.blocks.get_mut(&pid) {
                parent.add_child(block_id.clone());
            }
        } else {
            // Root-level block
            if !self.root_block_ids.contains(&block_id) {
                self.root_block_ids.push(block_id);
            }
        }

        Ok(())
    }

    /// Get a block by ID
    pub fn get_block(&self, id: &BlockId) -> Option<&Block> {
        self.blocks.get(id)
    }

    /// Get a mutable reference to a block by ID
    pub fn get_block_mut(&mut self, id: &BlockId) -> Option<&mut Block> {
        self.blocks.get_mut(id)
    }

    /// Remove a block from the page
    pub fn remove_block(&mut self, id: &BlockId) -> DomainResult<()> {
        let block = self
            .blocks
            .get(id)
            .ok_or_else(|| DomainError::NotFound(format!("Block {} not found", id)))?;

        // Clone the data we need before mutable operations
        let parent_id = block.parent_id().cloned();
        let child_ids: Vec<BlockId> = block.child_ids().to_vec();

        // Remove from parent's children list
        if let Some(parent_id) = parent_id {
            if let Some(parent) = self.blocks.get_mut(&parent_id) {
                parent.remove_child(id);
            }
        } else {
            // Remove from root blocks
            self.root_block_ids.retain(|bid| bid != id);
        }

        // Remove all children recursively
        for child_id in child_ids {
            self.remove_block(&child_id)?;
        }

        self.blocks.remove(id);
        Ok(())
    }

    /// Get all root-level blocks
    pub fn root_blocks(&self) -> Vec<&Block> {
        self.root_block_ids
            .iter()
            .filter_map(|id| self.blocks.get(id))
            .collect()
    }

    /// Get all blocks in the page
    pub fn all_blocks(&self) -> impl Iterator<Item = &Block> {
        self.blocks.values()
    }

    /// Get all URLs in the page
    pub fn all_urls(&self) -> Vec<&Url> {
        self.blocks
            .values()
            .flat_map(|block| block.urls())
            .collect()
    }

    /// Get all page references in the page
    pub fn all_page_references(&self) -> Vec<&PageReference> {
        self.blocks
            .values()
            .flat_map(|block| block.page_references())
            .collect()
    }

    /// Get all ancestor blocks for a given block (from parent to root)
    pub fn get_ancestors(&self, block_id: &BlockId) -> Vec<&Block> {
        let mut ancestors = Vec::new();
        let mut current_id = block_id.clone();

        while let Some(block) = self.blocks.get(&current_id) {
            if let Some(parent_id) = block.parent_id() {
                if let Some(parent) = self.blocks.get(parent_id) {
                    ancestors.push(parent);
                    current_id = parent_id.clone();
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        ancestors
    }

    /// Get all descendant blocks for a given block (recursive)
    pub fn get_descendants(&self, block_id: &BlockId) -> Vec<&Block> {
        let mut descendants = Vec::new();

        if let Some(block) = self.blocks.get(block_id) {
            for child_id in block.child_ids() {
                if let Some(child) = self.blocks.get(child_id) {
                    descendants.push(child);
                    // Recursively get descendants
                    descendants.extend(self.get_descendants(child_id));
                }
            }
        }

        descendants
    }

    /// Get all URLs with their ancestor and descendant page references
    /// Returns tuples of (url, ancestor_refs, descendant_refs)
    pub fn get_urls_with_context(&self) -> Vec<(&Url, Vec<&PageReference>, Vec<&PageReference>)> {
        let mut results = Vec::new();

        for block in self.blocks.values() {
            for url in block.urls() {
                let ancestor_refs = self
                    .get_ancestors(block.id())
                    .into_iter()
                    .flat_map(|b| b.page_references())
                    .collect();

                let descendant_refs = self
                    .get_descendants(block.id())
                    .into_iter()
                    .flat_map(|b| b.page_references())
                    .collect();

                results.push((url, ancestor_refs, descendant_refs));
            }
        }

        results
    }

    /// Get all page references with their ancestor and descendant URLs
    /// Returns tuples of (page_ref, ancestor_urls, descendant_urls)
    pub fn get_page_references_with_context(&self) -> Vec<(&PageReference, Vec<&Url>, Vec<&Url>)> {
        let mut results = Vec::new();

        for block in self.blocks.values() {
            for page_ref in block.page_references() {
                let ancestor_urls = self
                    .get_ancestors(block.id())
                    .into_iter()
                    .flat_map(|b| b.urls())
                    .collect();

                let descendant_urls = self
                    .get_descendants(block.id())
                    .into_iter()
                    .flat_map(|b| b.urls())
                    .collect();

                results.push((page_ref, ancestor_urls, descendant_urls));
            }
        }

        results
    }

    /// Get the full hierarchy path from root to a specific block
    pub fn get_hierarchy_path(&self, block_id: &BlockId) -> Vec<&Block> {
        let mut path = self.get_ancestors(block_id);
        path.reverse(); // Reverse to go from root to target

        // Add the target block itself
        if let Some(block) = self.blocks.get(block_id) {
            path.push(block);
        }

        path
    }
}

impl Entity for Page {
    type Id = PageId;

    fn id(&self) -> &Self::Id {
        &self.id
    }
}

impl AggregateRoot for Page {
    fn apply_event(&mut self, _event: &DomainEventEnum) {
        // Event handling will be implemented when we add domain events
        // For now, this is a placeholder
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::value_objects::{BlockContent, IndentLevel};

    #[test]
    fn test_create_page() {
        let page_id = PageId::new("page-1").unwrap();
        let page = Page::new(page_id.clone(), "Test Page".to_string());

        assert_eq!(page.id().as_str(), "page-1");
        assert_eq!(page.title(), "Test Page");
        assert_eq!(page.root_blocks().len(), 0);
    }

    #[test]
    fn test_add_root_block() {
        let page_id = PageId::new("page-1").unwrap();
        let mut page = Page::new(page_id, "Test Page".to_string());

        let block_id = BlockId::new("block-1").unwrap();
        let content = BlockContent::new("Root block");
        let block = Block::new_root(block_id.clone(), content);

        page.add_block(block).unwrap();

        assert_eq!(page.root_blocks().len(), 1);
        assert!(page.get_block(&block_id).is_some());
    }

    #[test]
    fn test_add_child_block() {
        let page_id = PageId::new("page-1").unwrap();
        let mut page = Page::new(page_id, "Test Page".to_string());

        // Add parent block
        let parent_id = BlockId::new("block-1").unwrap();
        let parent_content = BlockContent::new("Parent block");
        let parent = Block::new_root(parent_id.clone(), parent_content);
        page.add_block(parent).unwrap();

        // Add child block
        let child_id = BlockId::new("block-2").unwrap();
        let child_content = BlockContent::new("Child block");
        let child = Block::new_child(
            child_id.clone(),
            child_content,
            parent_id.clone(),
            IndentLevel::new(1),
        );
        page.add_block(child).unwrap();

        // Verify structure
        assert_eq!(page.root_blocks().len(), 1);
        let parent = page.get_block(&parent_id).unwrap();
        assert_eq!(parent.child_ids().len(), 1);
        assert_eq!(parent.child_ids()[0], child_id);
    }

    #[test]
    fn test_add_child_without_parent_fails() {
        let page_id = PageId::new("page-1").unwrap();
        let mut page = Page::new(page_id, "Test Page".to_string());

        let parent_id = BlockId::new("nonexistent").unwrap();
        let child_id = BlockId::new("block-1").unwrap();
        let content = BlockContent::new("Child block");
        let child = Block::new_child(child_id, content, parent_id, IndentLevel::new(1));

        let result = page.add_block(child);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_ancestors() {
        let page_id = PageId::new("page-1").unwrap();
        let mut page = Page::new(page_id, "Test Page".to_string());

        // Create hierarchy: root -> child1 -> child2
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

        // Get ancestors of child2
        let ancestors = page.get_ancestors(&child2_id);
        assert_eq!(ancestors.len(), 2);
        assert_eq!(ancestors[0].id(), &child1_id);
        assert_eq!(ancestors[1].id(), &root_id);
    }

    #[test]
    fn test_get_descendants() {
        let page_id = PageId::new("page-1").unwrap();
        let mut page = Page::new(page_id, "Test Page".to_string());

        // Create hierarchy: root -> child1 -> child2
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

        // Get descendants of root
        let descendants = page.get_descendants(&root_id);
        assert_eq!(descendants.len(), 2);
    }

    #[test]
    fn test_get_hierarchy_path() {
        let page_id = PageId::new("page-1").unwrap();
        let mut page = Page::new(page_id, "Test Page".to_string());

        // Create hierarchy: root -> child1 -> child2
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

        // Get path from root to child2
        let path = page.get_hierarchy_path(&child2_id);
        assert_eq!(path.len(), 3);
        assert_eq!(path[0].id(), &root_id);
        assert_eq!(path[1].id(), &child1_id);
        assert_eq!(path[2].id(), &child2_id);
    }

    #[test]
    fn test_get_urls_with_context() {
        let page_id = PageId::new("page-1").unwrap();
        let mut page = Page::new(page_id, "Test Page".to_string());

        // Create hierarchy with URLs and refs
        let root_id = BlockId::new("root").unwrap();
        let mut root = Block::new_root(root_id.clone(), BlockContent::new("Root"));
        root.add_page_reference(PageReference::from_brackets("parent-ref").unwrap());
        page.add_block(root).unwrap();

        let child_id = BlockId::new("child").unwrap();
        let mut child = Block::new_child(
            child_id.clone(),
            BlockContent::new("Child"),
            root_id.clone(),
            IndentLevel::new(1),
        );
        child.add_url(Url::new("https://example.com").unwrap());
        page.add_block(child).unwrap();

        let grandchild_id = BlockId::new("grandchild").unwrap();
        let mut grandchild = Block::new_child(
            grandchild_id.clone(),
            BlockContent::new("Grandchild"),
            child_id.clone(),
            IndentLevel::new(2),
        );
        grandchild.add_page_reference(PageReference::from_brackets("child-ref").unwrap());
        page.add_block(grandchild).unwrap();

        // Get URLs with context
        let urls_with_context = page.get_urls_with_context();
        assert_eq!(urls_with_context.len(), 1);

        let (url, ancestor_refs, descendant_refs) = &urls_with_context[0];
        assert_eq!(url.as_str(), "https://example.com");
        assert_eq!(ancestor_refs.len(), 1); // parent-ref from root
        assert_eq!(descendant_refs.len(), 1); // child-ref from grandchild
    }

    #[test]
    fn test_remove_block() {
        let page_id = PageId::new("page-1").unwrap();
        let mut page = Page::new(page_id, "Test Page".to_string());

        let root_id = BlockId::new("root").unwrap();
        let root = Block::new_root(root_id.clone(), BlockContent::new("Root"));
        page.add_block(root).unwrap();

        let child_id = BlockId::new("child").unwrap();
        let child = Block::new_child(
            child_id.clone(),
            BlockContent::new("Child"),
            root_id.clone(),
            IndentLevel::new(1),
        );
        page.add_block(child).unwrap();

        // Remove child
        page.remove_block(&child_id).unwrap();

        assert!(page.get_block(&child_id).is_none());
        let root = page.get_block(&root_id).unwrap();
        assert_eq!(root.child_ids().len(), 0);
    }
}
