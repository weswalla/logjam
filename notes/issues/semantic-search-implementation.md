# Title: Implement Semantic Search with fastembed-rs + Qdrant using Simplified DDD Architecture

## Description

Build the semantic search system for Logseq notes using local vector embeddings and similarity search. This extends our existing domain model with embedding capabilities while maintaining the pragmatic DDD approach established in the import/sync system.

## Core Requirements

### 1. Semantic Search Domain Extensions

**New Value Objects:**
- `EmbeddingVector`: Wrapper around Vec<f32> with validation (384 dimensions for all-MiniLM-L6-v2)
- `ChunkId`: Unique identifier for text chunks (may be 1:1 or 1:many with BlockId)  
- `SimilarityScore`: Normalized similarity score (0.0-1.0)
- `EmbeddingModel`: Enum of supported models (AllMiniLML6V2 as default)

**New Entities:**
- `TextChunk`: Represents preprocessed text ready for embedding
  - Contains cleaned text content
  - Maintains reference to source Block and Page
  - Handles chunking logic for long blocks (>512 tokens)
  - Stores metadata for context reconstruction

**New Aggregates:**
- `EmbeddedBlock`: Aggregate containing Block + its embeddings
  - Manages relationship between Block and its TextChunks
  - Handles embedding lifecycle (create, update, delete)
  - Ensures consistency between block content and embeddings

### 2. Application Layer Use Cases

**EmbedBlocks UseCase:**
- Convert blocks to embeddings and store in vector DB
- Batch processing (32 blocks per batch for efficiency)
- Text preprocessing pipeline (remove Logseq syntax, add context)
- Handle chunking for long blocks with overlap strategy
- Async processing to avoid blocking UI

**SemanticSearch UseCase:**
- Query vector DB and return ranked results with context
- Generate query embedding using same model
- Reconstruct full context (page title, hierarchy path, related refs)
- Combine with existing search DTOs for unified results

**UpdateEmbeddings UseCase:**
- Re-embed modified blocks (triggered by file sync events)
- Incremental updates to avoid full re-indexing
- Cleanup orphaned embeddings for deleted blocks

**DeleteEmbeddings UseCase:**
- Remove embeddings for deleted blocks/pages
- Maintain vector DB consistency with domain model

### 3. Infrastructure Layer

**Embedding Service (fastembed-rs):**
- `FastEmbedService`: Local embedding generation wrapper
- `EmbeddingModelManager`: Model loading and caching (~25MB download on first run)
- `TextPreprocessor`: Clean Logseq syntax while preserving context
  - Remove [[page references]] brackets but keep text
  - Remove #tags formatting but keep text  
  - Remove TODO/DONE markers
  - Add page title and parent block context
  - Handle chunking with 50-token overlap

**Vector Database (Qdrant embedded):**
- `QdrantVectorStore`: Local file-based vector storage
- `VectorCollectionManager`: Collection schema and lifecycle management
- Store in app data directory (no external database needed)
- Cosine similarity search with configurable parameters

### 4. Integration Points

**With Existing Domain:**
- Extend existing `Block` and `Page` aggregates with embedding methods
- Use existing `PageRepository` for context reconstruction
- Integrate with file sync system for incremental updates

**With Future Hybrid Search:**
- Semantic results combine with tantivy keyword results
- Shared result ranking and fusion logic (Reciprocal Rank Fusion)
- Common search result DTOs and interfaces

**With Tauri Frontend:**
- Async search commands with progress reporting
- Embedding status and model management UI
- Search result streaming for large result sets

## Technical Stack

- **Embedding Generation**: fastembed-rs (all-MiniLM-L6-v2 model, 384 dimensions)
- **Vector Storage**: Qdrant embedded mode (file-based, no server needed)
- **Text Processing**: Custom preprocessing pipeline for Logseq syntax
- **Persistence**: Qdrant's native file storage + existing SQLite for metadata
- **Integration**: Direct integration with existing PageRepository and sync system

## Architecture Notes

**Pragmatic DDD Approach:**
- Extend existing domain objects rather than creating parallel structures
- Use existing repositories and services where possible
- Keep embedding logic separate but integrated with core domain
- Focus on testability with mockable embedding and vector services

**Performance Considerations:**
- Batch embedding generation (32 blocks per batch)
- Async processing with progress reporting
- Model caching to avoid repeated loading
- Incremental updates for file changes
- Local storage optimization (quantization options)

**Error Handling Strategy:**
- Continue processing other blocks if one embedding fails
- Graceful degradation to keyword search if embeddings unavailable
- Retry logic for transient failures
- Connection retry with exponential backoff for vector DB

## Data Storage Strategy

**Qdrant Collection Schema:**
```json
{
  "collection": "logseq_blocks",
  "vector_size": 384,
  "distance": "Cosine",
  "payload": {
    "chunk_id": "block-123-chunk-0",
    "block_id": "block-123", 
    "page_id": "page-456",
    "page_title": "Programming Notes",
    "chunk_index": 0,
    "total_chunks": 1,
    "original_content": "Original block text...",
    "preprocessed_content": "Cleaned text for embedding...",
    "hierarchy_path": ["Parent block", "Current block"],
    "created_at": "2025-10-18T10:00:00Z",
    "updated_at": "2025-10-18T10:00:00Z"
  }
}
```

**Storage Locations:**
- **macOS**: `~/Library/Application Support/com.logseq-search/qdrant_storage/`
- **Windows**: `%APPDATA%\com.logseq-search\qdrant_storage\`
- **Linux**: `~/.local/share/com.logseq-search/qdrant_storage/`

## Testing Requirements

**Unit Tests:**
- Text preprocessing logic (Logseq syntax removal, context addition)
- Chunking algorithms (long blocks, overlap strategy)
- Embedding generation (with mock models)
- Search result reconstruction and ranking
- Vector storage operations (with mock Qdrant)

**Integration Tests:**
- End-to-end embedding and search flow
- Performance benchmarks with realistic data (1000+ blocks)
- Error scenario handling (model loading failures, vector DB issues)
- Incremental update workflows
- Memory usage and storage optimization

**Test Data:**
- Sample Logseq blocks with various content types
- Long blocks requiring chunking (>512 tokens)
- Blocks with complex Logseq syntax ([[refs]], #tags, URLs)
- Hierarchical block structures with context

## Implementation Phases

**Phase 1: Core Infrastructure**
- Set up fastembed-rs integration and model management
- Implement Qdrant embedded storage
- Create basic text preprocessing pipeline
- Build embedding generation service

**Phase 2: Domain Integration**
- Extend existing domain objects with embedding capabilities
- Implement EmbedBlocks and SemanticSearch use cases
- Create vector repository interfaces
- Add embedding lifecycle management

**Phase 3: Search Integration**
- Integrate semantic search with existing search system
- Implement result fusion and ranking
- Add search result DTOs and context reconstruction
- Build Tauri command interfaces

**Phase 4: Optimization & Polish**
- Performance optimization and benchmarking
- Error handling and recovery mechanisms
- Progress reporting and status management
- Documentation and testing completion

## Success Criteria

- **Functionality**: Users can perform semantic search across their Logseq notes
- **Performance**: Search results return in <200ms for typical personal knowledge bases
- **Accuracy**: Semantic search finds conceptually related content beyond keyword matching
- **Integration**: Seamless integration with existing import/sync system
- **Reliability**: Robust error handling and graceful degradation
- **Maintainability**: Clean, testable code following established DDD patterns

## Dependencies

- `fastembed = "3.0"` - Local embedding generation
- `qdrant-client = "1.11"` - Vector database client
- `tokio` - Async runtime (already in project)
- `serde` - Serialization (already in project)

## Notes for Implementation

This builds directly on the existing domain model and repository patterns established in the import/sync system. The semantic search capabilities are designed as extensions rather than replacements, allowing for future hybrid search implementations.

The focus remains on pragmatic DDD - we want the benefits of clean architecture without over-engineering for a personal project. All components should be testable, maintainable, and performant for typical personal knowledge base sizes (1K-10K notes).

See `./notes/features/SemanticSearch.md` for detailed architectural guidance and `./notes/dependencies/fastembed-ts.md` and `./notes/dependencies/qdrant.md` for technical implementation details.
