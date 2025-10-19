# Semantic Search Feature

## Overview

Semantic search system that converts Logseq blocks into vector embeddings for similarity-based retrieval. Uses fastembed-rs for local embedding generation and Qdrant for vector storage and search. Designed for hybrid search compatibility with future tantivy integration.

## Core Components

### Domain Layer

#### Value Objects
- `EmbeddingVector`: Wrapper around Vec<f32> with validation
- `ChunkId`: Unique identifier for text chunks (may be 1:1 or 1:many with BlockId)
- `SimilarityScore`: Normalized similarity score (0.0-1.0)
- `EmbeddingModel`: Enum of supported models (AllMiniLML6V2 as default)

#### Entities
- `TextChunk`: Represents a piece of text ready for embedding
  - Contains preprocessed text content
  - Maintains reference to source Block and Page
  - Handles text chunking logic for long blocks
  - Stores metadata for context reconstruction

#### Aggregates
- `EmbeddedBlock`: Aggregate containing Block + its embeddings
  - Manages relationship between Block and its TextChunks
  - Handles embedding lifecycle (create, update, delete)
  - Ensures consistency between block content and embeddings

### Application Layer

#### Use Cases
- `EmbedBlocks`: Convert blocks to embeddings and store in vector DB
- `SemanticSearch`: Query vector DB and return ranked results
- `UpdateEmbeddings`: Re-embed modified blocks
- `DeleteEmbeddings`: Remove embeddings for deleted blocks

#### DTOs
- `EmbeddingRequest`: Block content + metadata for embedding
- `SemanticSearchRequest`: Query text + search parameters
- `SemanticSearchResult`: Ranked results with similarity scores and context

#### Repositories
- `EmbeddingRepository`: Interface for vector storage operations
- `EmbeddingModelRepository`: Interface for embedding model management

### Infrastructure Layer

#### Embedding Service
- `FastEmbedService`: Wraps fastembed-rs for local embedding generation
- `EmbeddingModelManager`: Handles model loading and caching
- `TextPreprocessor`: Cleans and prepares text for embedding

#### Vector Database
- `QdrantVectorStore`: Qdrant client wrapper for vector operations
- `VectorCollectionManager`: Manages Qdrant collections and schemas

## Implementation Approach

### Text Preprocessing Pipeline
```rust
impl TextPreprocessor {
    pub fn preprocess_block(&self, block: &Block) -> Vec<String> {
        let content = block.content().as_str();
        
        // 1. Remove Logseq-specific syntax
        let cleaned = self.remove_logseq_syntax(content);
        
        // 2. Extract and preserve important context
        let with_context = self.add_context_markers(&cleaned, block);
        
        // 3. Handle chunking for long blocks
        self.chunk_if_needed(with_context)
    }
    
    fn remove_logseq_syntax(&self, text: &str) -> String {
        // Remove [[page references]] but keep the text
        // Remove #tags but keep the text
        // Remove TODO/DONE markers
        // Clean up markdown formatting for better embedding
    }
    
    fn add_context_markers(&self, text: &str, block: &Block) -> String {
        // Add page title as context
        // Add parent block context for nested blocks
        // Preserve important structural information
    }
}
```

### Chunking Strategy
```rust
impl TextChunk {
    const MAX_CHUNK_SIZE: usize = 512; // tokens, roughly 400 words
    const OVERLAP_SIZE: usize = 50;    // token overlap between chunks
    
    pub fn from_block(block: &Block, page_title: &str) -> Vec<Self> {
        let preprocessed = TextPreprocessor::new().preprocess_block(block);
        
        if preprocessed.len() <= Self::MAX_CHUNK_SIZE {
            // Single chunk
            vec![Self::new_single(block, page_title, preprocessed)]
        } else {
            // Multiple chunks with overlap
            Self::create_overlapping_chunks(block, page_title, preprocessed)
        }
    }
}
```

### Embedding Generation
```rust
impl EmbedBlocks {
    pub async fn execute(&self, blocks: Vec<Block>) -> DomainResult<()> {
        // 1. Preprocess blocks into chunks
        let chunks = self.create_chunks_from_blocks(blocks);
        
        // 2. Generate embeddings in batches
        let batch_size = 32; // Optimize for fastembed performance
        for chunk_batch in chunks.chunks(batch_size) {
            let texts: Vec<String> = chunk_batch.iter()
                .map(|c| c.preprocessed_text().to_string())
                .collect();
                
            let embeddings = self.embedding_service
                .generate_embeddings(texts)
                .await?;
                
            // 3. Store in vector database
            self.store_embeddings(chunk_batch, embeddings).await?;
        }
        
        Ok(())
    }
}
```

### Semantic Search
```rust
impl SemanticSearch {
    pub async fn execute(&self, request: SemanticSearchRequest) -> DomainResult<Vec<SemanticSearchResult>> {
        // 1. Generate query embedding
        let query_embedding = self.embedding_service
            .generate_embeddings(vec![request.query])
            .await?
            .into_iter()
            .next()
            .unwrap();
            
        // 2. Search vector database
        let vector_results = self.vector_store
            .similarity_search(query_embedding, request.limit)
            .await?;
            
        // 3. Reconstruct context and rank results
        let results = self.build_search_results(vector_results).await?;
        
        Ok(results)
    }
    
    async fn build_search_results(&self, vector_results: Vec<VectorSearchResult>) -> DomainResult<Vec<SemanticSearchResult>> {
        let mut results = Vec::new();
        
        for vector_result in vector_results {
            // Get original block and page context
            let chunk = self.embedding_repository
                .get_chunk_by_id(&vector_result.chunk_id)
                .await?;
                
            let block = self.page_repository
                .find_block_by_id(&chunk.block_id)
                .await?;
                
            let page = self.page_repository
                .find_by_id(&chunk.page_id)
                .await?;
                
            results.push(SemanticSearchResult {
                block_id: chunk.block_id.clone(),
                page_id: chunk.page_id.clone(),
                page_title: page.title().to_string(),
                block_content: block.content().as_str().to_string(),
                chunk_text: chunk.preprocessed_text().to_string(),
                similarity_score: SimilarityScore::new(vector_result.score),
                hierarchy_path: page.get_hierarchy_path(&chunk.block_id)
                    .iter()
                    .map(|b| b.content().as_str().to_string())
                    .collect(),
            });
        }
        
        Ok(results)
    }
}
```

## Data Storage Strategy

### Qdrant Collection Schema
```rust
// Collection: "logseq_blocks"
// Vector dimension: 384 (for all-MiniLM-L6-v2)
// Distance metric: Cosine similarity

// Payload structure:
{
    "chunk_id": "block-123-chunk-0",
    "block_id": "block-123", 
    "page_id": "page-456",
    "page_title": "Programming Notes",
    "chunk_index": 0,  // For multi-chunk blocks
    "total_chunks": 1,
    "original_content": "Original block text...",
    "preprocessed_content": "Cleaned text for embedding...",
    "hierarchy_path": ["Parent block", "Current block"],
    "created_at": "2025-10-18T10:00:00Z",
    "updated_at": "2025-10-18T10:00:00Z"
}
```

### Embedding Model Configuration
- **Default Model**: `all-MiniLM-L6-v2` (384 dimensions)
  - Good balance of quality and speed
  - Suitable for personal knowledge bases
  - ~25MB model size
- **Alternative Models**: Support for BGE-small-en-v1.5, nomic-embed-text-v1
- **Model Selection**: Configurable via application settings

## Integration Points

### With Existing Domain
- Extends existing `Block` and `Page` aggregates
- Uses existing `PageRepository` for context reconstruction
- Integrates with file sync system for incremental updates

### With Future Hybrid Search
- Semantic results can be combined with tantivy keyword results
- Shared result ranking and fusion logic
- Common search result DTOs

### With Tauri Frontend
- Async search commands with progress reporting
- Embedding status and model management
- Search result streaming for large result sets

## Performance Considerations

### Embedding Generation
- Batch processing for efficiency (32 blocks per batch)
- Async processing to avoid blocking UI
- Model caching to avoid repeated loading

### Vector Search
- Qdrant's HNSW index for fast similarity search
- Configurable search parameters (ef, m values)
- Result caching for repeated queries

### Storage Optimization
- Quantization options for reduced storage
- Periodic index optimization
- Cleanup of orphaned embeddings

## Error Handling

### Embedding Failures
- Continue processing other blocks if one fails
- Retry logic for transient failures
- Fallback to keyword search if embedding unavailable

### Vector Database Issues
- Graceful degradation to traditional search
- Connection retry with exponential backoff
- Data consistency checks and repair

## Testing Strategy

### Unit Tests
- Text preprocessing logic
- Chunking algorithms
- Embedding generation (with mock models)
- Search result reconstruction

### Integration Tests
- End-to-end embedding and search flow
- Performance benchmarks with realistic data
- Error scenario handling

### Test Data
- Sample Logseq blocks with various content types
- Long blocks requiring chunking
- Blocks with complex Logseq syntax

## Key Simplifications

**Removed:**
- Complex embedding model management
- Sophisticated chunking strategies
- Advanced vector database optimizations
- Detailed analytics and monitoring
- Multi-language support

**Kept:**
- Clean text preprocessing
- Efficient batching and async processing
- Context preservation for search results
- Integration with existing domain model
- Hybrid search compatibility

This approach provides high-quality semantic search with reasonable complexity for a personal knowledge management tool.
