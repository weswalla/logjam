/// Domain entities
use super::base::Entity;
use super::value_objects::{
    BlockContent, BlockId, ChunkId, EmbeddingVector, IndentLevel, PageId, PageReference, Url,
};

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

/// A TextChunk represents preprocessed text ready for embedding
/// Chunks may be 1:1 with blocks or a block may be split into multiple chunks
#[derive(Debug, Clone)]
pub struct TextChunk {
    id: ChunkId,
    block_id: BlockId,
    page_id: PageId,
    chunk_index: usize,
    total_chunks: usize,
    original_content: BlockContent,
    preprocessed_content: String,
    embedding: Option<EmbeddingVector>,
    page_title: String,
    hierarchy_path: Vec<String>,
}

impl TextChunk {
    /// Create a new text chunk
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: ChunkId,
        block_id: BlockId,
        page_id: PageId,
        chunk_index: usize,
        total_chunks: usize,
        original_content: BlockContent,
        preprocessed_content: String,
        page_title: String,
        hierarchy_path: Vec<String>,
    ) -> Self {
        TextChunk {
            id,
            block_id,
            page_id,
            chunk_index,
            total_chunks,
            original_content,
            preprocessed_content,
            embedding: None,
            page_title,
            hierarchy_path,
        }
    }

    /// Get the chunk ID
    pub fn id(&self) -> &ChunkId {
        &self.id
    }

    /// Get the source block ID
    pub fn block_id(&self) -> &BlockId {
        &self.block_id
    }

    /// Get the page ID
    pub fn page_id(&self) -> &PageId {
        &self.page_id
    }

    /// Get the chunk index (0-based)
    pub fn chunk_index(&self) -> usize {
        self.chunk_index
    }

    /// Get total number of chunks for this block
    pub fn total_chunks(&self) -> usize {
        self.total_chunks
    }

    /// Get the original block content
    pub fn original_content(&self) -> &BlockContent {
        &self.original_content
    }

    /// Get the preprocessed content ready for embedding
    pub fn preprocessed_content(&self) -> &str {
        &self.preprocessed_content
    }

    /// Get the embedding vector, if set
    pub fn embedding(&self) -> Option<&EmbeddingVector> {
        self.embedding.as_ref()
    }

    /// Set the embedding vector
    pub fn set_embedding(&mut self, embedding: EmbeddingVector) {
        self.embedding = Some(embedding);
    }

    /// Check if this chunk has an embedding
    pub fn has_embedding(&self) -> bool {
        self.embedding.is_some()
    }

    /// Get the page title
    pub fn page_title(&self) -> &str {
        &self.page_title
    }

    /// Get the hierarchy path (ancestor blocks)
    pub fn hierarchy_path(&self) -> &[String] {
        &self.hierarchy_path
    }

    /// Check if this is the only chunk for its block
    pub fn is_single_chunk(&self) -> bool {
        self.total_chunks == 1
    }
}

impl Entity for TextChunk {
    type Id = ChunkId;

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

    #[test]
    fn test_create_text_chunk() {
        let chunk_id = ChunkId::new("chunk-1").unwrap();
        let block_id = BlockId::new("block-1").unwrap();
        let page_id = PageId::new("page-1").unwrap();
        let content = BlockContent::new("Original block content");
        let preprocessed = "Preprocessed text for embedding".to_string();
        let page_title = "My Page".to_string();
        let hierarchy = vec!["Parent block".to_string()];

        let chunk = TextChunk::new(
            chunk_id.clone(),
            block_id.clone(),
            page_id.clone(),
            0,
            1,
            content.clone(),
            preprocessed.clone(),
            page_title.clone(),
            hierarchy.clone(),
        );

        assert_eq!(chunk.id(), &chunk_id);
        assert_eq!(chunk.block_id(), &block_id);
        assert_eq!(chunk.page_id(), &page_id);
        assert_eq!(chunk.chunk_index(), 0);
        assert_eq!(chunk.total_chunks(), 1);
        assert_eq!(chunk.original_content(), &content);
        assert_eq!(chunk.preprocessed_content(), &preprocessed);
        assert_eq!(chunk.page_title(), &page_title);
        assert_eq!(chunk.hierarchy_path(), &hierarchy[..]);
        assert!(!chunk.has_embedding());
        assert!(chunk.is_single_chunk());
    }

    #[test]
    fn test_set_embedding_on_chunk() {
        let chunk_id = ChunkId::new("chunk-1").unwrap();
        let block_id = BlockId::new("block-1").unwrap();
        let page_id = PageId::new("page-1").unwrap();
        let content = BlockContent::new("Content");
        let preprocessed = "Preprocessed".to_string();

        let mut chunk = TextChunk::new(
            chunk_id,
            block_id,
            page_id,
            0,
            1,
            content,
            preprocessed,
            "Page".to_string(),
            vec![],
        );

        assert!(!chunk.has_embedding());

        let embedding = EmbeddingVector::new(vec![0.1, 0.2, 0.3]).unwrap();
        chunk.set_embedding(embedding.clone());

        assert!(chunk.has_embedding());
        assert_eq!(chunk.embedding(), Some(&embedding));
    }

    #[test]
    fn test_multi_chunk_block() {
        let chunk_id = ChunkId::new("chunk-1").unwrap();
        let block_id = BlockId::new("block-1").unwrap();
        let page_id = PageId::new("page-1").unwrap();
        let content = BlockContent::new("Long content that needs multiple chunks");

        let chunk1 = TextChunk::new(
            chunk_id.clone(),
            block_id.clone(),
            page_id.clone(),
            0,
            3,
            content.clone(),
            "First chunk".to_string(),
            "Page".to_string(),
            vec![],
        );

        let chunk2_id = ChunkId::new("chunk-2").unwrap();
        let chunk2 = TextChunk::new(
            chunk2_id,
            block_id,
            page_id,
            1,
            3,
            content,
            "Second chunk".to_string(),
            "Page".to_string(),
            vec![],
        );

        assert_eq!(chunk1.chunk_index(), 0);
        assert_eq!(chunk1.total_chunks(), 3);
        assert!(!chunk1.is_single_chunk());

        assert_eq!(chunk2.chunk_index(), 1);
        assert_eq!(chunk2.total_chunks(), 3);
        assert!(!chunk2.is_single_chunk());
    }
}
