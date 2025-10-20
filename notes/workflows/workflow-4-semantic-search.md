# Workflow 4: Semantic Search with Embeddings

**User Action:** Ask natural language question: "How do I optimize database queries?"

**Purpose:** Unlike keyword search (Tantivy), semantic search understands *meaning*. It finds conceptually similar content even without exact keyword matches.

## Flow Diagram

```
┌──────────────────────────────────────────────────────────────────┐
│                        FRONTEND                                   │
│  User types: "How do I optimize database queries?"               │
│  (No exact keywords like "SQL" or "index" in query)              │
└───────────────────────────┬──────────────────────────────────────┘
                            │ TauriApi.semanticSearch({ query: "..." })
                            ↓
┌──────────────────────────────────────────────────────────────────┐
│                    TAURI COMMAND                                  │
│  semantic_search(state, request) → SemanticResultDto[]           │
└───────────────────────────┬──────────────────────────────────────┘
                            │
                            ↓
┌──────────────────────────────────────────────────────────────────┐
│              APPLICATION - SEMANTIC SEARCH USE CASE               │
│  SemanticSearch::execute(request)                                │
│                                                                   │
│  Step 1: Generate query embedding                                │
│    query_vector = fastembed_service.generate_embeddings([query]) │
│    → [0.12, -0.45, 0.89, ..., 0.34]  (384 dimensions)            │
└───────────────────────────┬──────────────────────────────────────┘
                            │ EmbeddingVector (query)
                            ↓
┌──────────────────────────────────────────────────────────────────┐
│          INFRASTRUCTURE - QDRANT VECTOR STORE                     │
│  QdrantVectorStore::similarity_search(query_vector, limit)       │
│                                                                   │
│  Step 2: Similarity search (cosine similarity)                   │
│    ├─ Compare query_vector to all chunk embeddings               │
│    ├─ Calculate cosine similarity scores                         │
│    └─ Return top K most similar chunks                           │
│                                                                   │
│  Vector Index (HNSW - Hierarchical Navigable Small World):       │
│    • Approximate nearest neighbor (ANN) search                   │
│    • O(log n) complexity instead of O(n)                         │
│    • Trade-off: 95%+ accuracy with 100x speedup                  │
└───────────────────────────┬──────────────────────────────────────┘
                            │ Vec<ScoredChunk>
                            ↓
┌──────────────────────────────────────────────────────────────────┐
│  Results (ranked by semantic similarity):                        │
│  [                                                               │
│    ScoredChunk {                                                 │
│      chunk_id: "chunk-147",                                      │
│      page_id: "database-performance",                            │
│      block_id: "block-89",                                       │
│      content: "Adding indexes on foreign keys dramatically       │
│                improves JOIN performance. Use EXPLAIN to..."     │
│      similarity_score: 0.87  ← High semantic match!              │
│    },                                                            │
│    ScoredChunk {                                                 │
│      chunk_id: "chunk-203",                                      │
│      page_id: "sql-tips",                                        │
│      content: "Query planning: PostgreSQL query planner uses     │
│                statistics to optimize execution..."              │
│      similarity_score: 0.82                                      │
│    }                                                             │
│  ]                                                               │
│                                                                   │
│  Note: Neither result contains "optimize database queries"       │
│        but both are semantically related!                        │
└──────────────────────────────────────────────────────────────────┘
```

## Chunking Strategy

**Problem:** Embeddings have token limits (usually 512 tokens). We need to split pages into chunks.

**Chunking Approaches:**

```
┌─────────────────────────────────────────────────────────────────┐
│                   CHUNKING STRATEGIES                            │
│                                                                  │
│  1. BLOCK-BASED WITH PREPROCESSING (Logseq-aware)               │
│  ┌────────────────────────────────────────────────────────┐     │
│  │ Step 1: Remove Logseq syntax                           │     │
│  │   "Check [[Page Reference]] and #tag"                  │     │
│  │   → "Check Page Reference and tag"                     │     │
│  │                                                        │     │
│  │ Step 2: Add context markers                           │     │
│  │   Block: "Neural networks..."                         │     │
│  │   → "Page: Machine Learning. Neural networks..."      │     │
│  │                                                        │     │
│  │ Step 3: Create chunks (1 block = 1 chunk if ≤512 tok) │     │
│  │                                                        │     │
│  │ ✅ Preserves hierarchical context                      │     │
│  │ ✅ Clean text for better embeddings                    │     │
│  │ ❌ Blocks can still be too small or too large         │     │
│  └────────────────────────────────────────────────────────┘     │
│                                                                  │
│  2. ROLLING WINDOW CHUNKING (Overlapping)                       │
│  ┌────────────────────────────────────────────────────────┐     │
│  │ Fixed-size chunks with overlap                         │     │
│  │                                                        │     │
│  │ Text: "ABCDEFGHIJ"                                    │     │
│  │ Chunk 1: [ABC]                                        │     │
│  │ Chunk 2:   [CDE]  ← 1 token overlap                   │     │
│  │ Chunk 3:     [EFG]                                    │     │
│  │ Chunk 4:       [GHI]                                  │     │
│  │                                                        │     │
│  │ ✅ Ensures context isn't lost at boundaries            │     │
│  │ ❌ More chunks = more storage + compute               │     │
│  └────────────────────────────────────────────────────────┘     │
│                                                                  │
│  3. SEMANTIC CHUNKING (Context-aware)                           │
│  ┌────────────────────────────────────────────────────────┐     │
│  │ Split at topic boundaries (sentence similarity)        │     │
│  │                                                        │     │
│  │ Paragraph 1: "Rust ownership rules..."               │     │
│  │ Paragraph 2: "Borrowing prevents data races..."      │     │
│  │ ↓ High similarity → same chunk                        │     │
│  │                                                        │     │
│  │ Paragraph 3: "JavaScript async/await..."             │     │
│  │ ↓ Low similarity → new chunk                          │     │
│  │                                                        │     │
│  │ ✅ Chunks are topically coherent                       │     │
│  │ ❌ Computationally expensive                          │     │
│  └────────────────────────────────────────────────────────┘     │
│                                                                  │
│  RECOMMENDED: Block-based with preprocessing                    │
│    • Preprocess: Remove Logseq syntax, add context markers      │
│    • 1 block = 1 chunk if ≤ 512 tokens                          │
│    • Split large blocks with 50-token overlap                   │
│    • Batch processing: 32 blocks per batch for efficiency       │
└─────────────────────────────────────────────────────────────────┘
```

## Embedding Generation Pipeline

**Full workflow from import to embedding:**

```rust
// 1. IMPORT/SYNC: Page is saved to database
page_repository.save(page)?;

// 2. PREPROCESSING & CHUNKING: Create TextChunks from blocks
let text_preprocessor = TextPreprocessor::new();
let chunks: Vec<TextChunk> = page.all_blocks()
    .flat_map(|block| {
        // Preprocess: Remove [[links]], #tags, clean markdown
        let preprocessed = text_preprocessor.preprocess_block(block);

        // Add context: page title, parent hierarchy
        let with_context = text_preprocessor.add_context_markers(&preprocessed, block);

        // Chunk if needed (512 token limit, 50 token overlap)
        TextChunk::from_block(block, page.title(), with_context)
    })
    .collect();

// Example chunk:
// TextChunk {
//   chunk_id: "block-1-chunk-0",
//   block_id: "block-1",
//   page_id: "machine-learning",
//   original_content: "Check [[Neural Networks]] for #deep-learning info",
//   preprocessed_content: "Page: Machine Learning. Check Neural Networks for deep-learning info",
//   chunk_index: 0,
//   total_chunks: 1
// }

// 3. BATCH EMBEDDING: Generate vectors for chunks (32 at a time)
let batch_size = 32;
for chunk_batch in chunks.chunks(batch_size) {
    let texts: Vec<String> = chunk_batch.iter()
        .map(|c| c.preprocessed_text().to_string())
        .collect();

    // Use fastembed-rs for local embedding generation
    let embeddings = fastembed_service.generate_embeddings(texts).await?;
    // embeddings = Vec<Vec<f32>> with 384 dimensions (all-MiniLM-L6-v2)

    // 4. STORAGE: Save to Qdrant vector database
    qdrant_store.upsert_embeddings(chunk_batch, embeddings).await?;
}

// 5. INDEX: Qdrant builds HNSW index automatically (no manual commit needed)
```

## Code Example: Text Preprocessing

```rust
// backend/src/infrastructure/embeddings/text_preprocessor.rs

pub struct TextPreprocessor;

impl TextPreprocessor {
    pub fn preprocess_block(&self, block: &Block) -> String {
        let content = block.content().as_str();

        // 1. Remove Logseq-specific syntax
        let cleaned = self.remove_logseq_syntax(content);

        // 2. Clean markdown formatting
        let cleaned = self.clean_markdown(&cleaned);

        cleaned
    }

    fn remove_logseq_syntax(&self, text: &str) -> String {
        let mut result = text.to_string();

        // Remove [[page references]] but keep the text
        // "Check [[Neural Networks]]" → "Check Neural Networks"
        result = Regex::new(r"\[\[([^\]]+)\]\]")
            .unwrap()
            .replace_all(&result, "$1")
            .to_string();

        // Remove #tags but keep the text
        // "Learn #machine-learning" → "Learn machine-learning"
        result = Regex::new(r"#(\S+)")
            .unwrap()
            .replace_all(&result, "$1")
            .to_string();

        // Remove TODO/DONE markers
        result = Regex::new(r"(TODO|DONE|LATER|NOW|WAITING)\s+")
            .unwrap()
            .replace_all(&result, "")
            .to_string();

        result
    }

    fn clean_markdown(&self, text: &str) -> String {
        // Remove bold/italic markers but keep text
        // Remove code block markers
        // Keep content readable for embeddings
        // ... implementation
    }

    pub fn add_context_markers(&self, text: &str, block: &Block, page: &Page) -> String {
        let mut contextualized = String::new();

        // Add page title as context
        contextualized.push_str(&format!("Page: {}. ", page.title()));

        // Add parent block context for nested blocks
        if let Some(parent_id) = block.parent_id() {
            if let Some(parent) = page.get_block(parent_id) {
                contextualized.push_str(&format!("Parent: {}. ", parent.content().as_str()));
            }
        }

        // Add the actual content
        contextualized.push_str(text);

        contextualized
    }
}
```

## Code Example: EmbedBlocks Use Case

```rust
// backend/src/application/use_cases/embed_blocks.rs

pub struct EmbedBlocks {
    embedding_service: Arc<FastEmbedService>,
    vector_store: Arc<QdrantVectorStore>,
    embedding_repository: Arc<dyn EmbeddingRepository>,
    preprocessor: TextPreprocessor,
}

impl EmbedBlocks {
    pub async fn execute(&self, blocks: Vec<Block>, page: &Page) -> DomainResult<()> {
        // 1. Preprocess blocks into TextChunks
        let chunks = self.create_chunks_from_blocks(blocks, page)?;

        // 2. Generate embeddings in batches (32 at a time for efficiency)
        let batch_size = 32;
        for chunk_batch in chunks.chunks(batch_size) {
            let texts: Vec<String> = chunk_batch.iter()
                .map(|c| c.preprocessed_text().to_string())
                .collect();

            // Generate embeddings using fastembed-rs
            let embeddings = self.embedding_service
                .generate_embeddings(texts)
                .await?;

            // 3. Store in vector database with metadata
            self.store_embeddings(chunk_batch, embeddings).await?;
        }

        Ok(())
    }

    fn create_chunks_from_blocks(&self, blocks: Vec<Block>, page: &Page) -> DomainResult<Vec<TextChunk>> {
        let mut chunks = Vec::new();

        for block in blocks {
            // Preprocess: remove Logseq syntax, clean markdown
            let cleaned = self.preprocessor.preprocess_block(&block);

            // Add context: page title, parent hierarchy
            let with_context = self.preprocessor.add_context_markers(&cleaned, &block, page);

            // Create chunks (split if > 512 tokens, 50 token overlap)
            let block_chunks = TextChunk::from_block(&block, page.title(), with_context);
            chunks.extend(block_chunks);
        }

        Ok(chunks)
    }

    async fn store_embeddings(&self, chunks: &[TextChunk], embeddings: Vec<Vec<f32>>) -> DomainResult<()> {
        for (chunk, embedding) in chunks.iter().zip(embeddings.iter()) {
            // Create EmbeddingVector value object
            let embedding_vector = EmbeddingVector::new(embedding.clone())?;

            // Create EmbeddedBlock aggregate
            let embedded_block = EmbeddedBlock::new(
                chunk.block_id().clone(),
                chunk.page_id().clone(),
                embedding_vector,
                chunk.clone(),
            );

            // Store in Qdrant with full payload
            self.vector_store.upsert_point(
                chunk.chunk_id(),
                embedding.clone(),
                Payload {
                    chunk_id: chunk.chunk_id().as_str(),
                    block_id: chunk.block_id().as_str(),
                    page_id: chunk.page_id().as_str(),
                    page_title: chunk.page_title(),
                    chunk_index: chunk.chunk_index(),
                    total_chunks: chunk.total_chunks(),
                    original_content: chunk.original_content(),
                    preprocessed_content: chunk.preprocessed_text(),
                    hierarchy_path: chunk.hierarchy_path(),
                }
            ).await?;

            // Track in repository
            self.embedding_repository.save(embedded_block).await?;
        }

        Ok(())
    }
}
```

## Infrastructure: Qdrant Vector Store

```rust
// backend/src/infrastructure/vector_store/qdrant_store.rs

use qdrant_client::{client::QdrantClient, qdrant::*};

pub struct QdrantVectorStore {
    client: QdrantClient,
    collection_name: String,
}

impl QdrantVectorStore {
    pub async fn new_embedded() -> Result<Self> {
        // Embedded mode - no separate Qdrant server needed
        let client = QdrantClient::from_url("http://localhost:6334").build()?;

        let collection_name = "logseq_blocks".to_string();

        // Create collection: 384 dimensions, cosine similarity
        client.create_collection(&CreateCollection {
            collection_name: collection_name.clone(),
            vectors_config: Some(VectorsConfig {
                config: Some(Config::Params(VectorParams {
                    size: 384,  // all-MiniLM-L6-v2
                    distance: Distance::Cosine.into(),
                    hnsw_config: Some(HnswConfigDiff {
                        m: Some(16),           // connections per layer
                        ef_construct: Some(100), // build-time accuracy
                        ..Default::default()
                    }),
                    ..Default::default()
                })),
            }),
            ..Default::default()
        }).await?;

        Ok(Self { client, collection_name })
    }

    pub async fn upsert_point(
        &self,
        chunk_id: &ChunkId,
        embedding: Vec<f32>,
        payload: Payload,
    ) -> Result<()> {
        let point = PointStruct {
            id: Some(PointId::from(chunk_id.as_str())),
            vectors: Some(Vectors::from(embedding)),
            payload: payload.into_map(),
        };

        self.client.upsert_points(
            &self.collection_name,
            None,
            vec![point],
            None,
        ).await?;

        Ok(())
    }

    pub async fn similarity_search(
        &self,
        query_embedding: EmbeddingVector,
        limit: usize,
    ) -> Result<Vec<ScoredChunk>> {
        let search_result = self.client.search_points(&SearchPoints {
            collection_name: self.collection_name.clone(),
            vector: query_embedding.as_vec(),
            limit: limit as u64,
            with_payload: Some(WithPayloadSelector::from(true)),
            score_threshold: Some(0.5),  // Minimum similarity
            ..Default::default()
        }).await?;

        Ok(search_result.result.into_iter()
            .map(|scored_point| ScoredChunk {
                chunk_id: ChunkId::new(scored_point.id.unwrap().to_string()).unwrap(),
                block_id: BlockId::new(
                    scored_point.payload.get("block_id").unwrap().as_str().unwrap()
                ).unwrap(),
                page_id: PageId::new(
                    scored_point.payload.get("page_id").unwrap().as_str().unwrap()
                ).unwrap(),
                similarity_score: SimilarityScore::new(scored_point.score),
                content: scored_point.payload.get("original_content")
                    .unwrap().as_str().unwrap().to_string(),
            })
            .collect())
    }
}
```

## Hybrid Search: Combining Keyword + Semantic

**Best results come from combining both approaches:**

```rust
// backend/src/application/services/hybrid_search_service.rs

pub struct HybridSearchService {
    text_search: SearchService,           // Tantivy
    semantic_search: EmbeddingService,    // Qdrant
}

impl HybridSearchService {
    pub async fn hybrid_search(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<HybridResult>> {
        // 1. Parallel search (both at once)
        let (text_results, semantic_results) = tokio::join!(
            self.text_search.search(query, limit),
            self.semantic_search.semantic_search(query, limit),
        );

        // 2. Reciprocal Rank Fusion (RRF) for score combination
        // Formula: score = Σ(1 / (k + rank_i)) where k = 60
        let mut combined_scores: HashMap<String, f32> = HashMap::new();

        for (rank, result) in text_results?.iter().enumerate() {
            let key = format!("{}:{}", result.page_id(), result.block_id());
            let rrf_score = 1.0 / (60.0 + rank as f32);
            *combined_scores.entry(key).or_insert(0.0) += rrf_score * 0.7; // 70% weight
        }

        for (rank, result) in semantic_results?.iter().enumerate() {
            let key = format!("{}:{}", result.page_id, result.chunk_id);
            let rrf_score = 1.0 / (60.0 + rank as f32);
            *combined_scores.entry(key).or_insert(0.0) += rrf_score * 0.3; // 30% weight
        }

        // 3. Sort by combined score
        let mut results: Vec<_> = combined_scores.into_iter().collect();
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        // 4. Return top K
        Ok(results.into_iter()
            .take(limit)
            .map(|(id, score)| HybridResult { id, score })
            .collect())
    }
}
```

**Why Hybrid?**

```
Keyword Search (Tantivy):
✅ Exact matches (code, filenames, specific terms)
✅ Very fast (milliseconds)
❌ Misses synonyms ("car" won't find "automobile")
❌ No semantic understanding

Semantic Search (Embeddings):
✅ Understands meaning ("fast car" finds "quick vehicle")
✅ Handles paraphrasing
❌ Slower (tens of milliseconds)
❌ Can miss exact technical terms

Hybrid:
✅ Best of both worlds
✅ Technical terms + conceptual understanding
```

## Integration with Import/Sync

**Automatic embedding during import:**

```rust
// backend/src/application/services/import_service.rs

impl ImportService {
    pub async fn import_directory(&mut self, dir: LogseqDirectoryPath) -> Result<ImportSummary> {
        for file in files {
            // 1. Parse file
            let page = LogseqMarkdownParser::parse_file(&file).await?;

            // 2. Save to database
            self.page_repository.save(page.clone())?;

            // 3. Index in Tantivy (keyword search)
            if let Some(ref tantivy_index) = self.tantivy_index {
                tantivy_index.lock().await.index_page(&page)?;
            }

            // 4. Generate embeddings and index (semantic search)
            if let Some(ref embedding_service) = self.embedding_service {
                embedding_service.embed_and_index_page(&page).await?;
            }

            // 5. Save file mapping
            self.mapping_repository.save(mapping)?;
        }

        // Commit both indexes
        self.tantivy_index.lock().await.commit()?;
        // Qdrant commits automatically

        Ok(summary)
    }
}
```

**Automatic re-embedding on sync:**

```rust
// backend/src/application/services/sync_service.rs

async fn handle_file_updated(&self, path: PathBuf) -> SyncResult<()> {
    let page = LogseqMarkdownParser::parse_file(&path).await?;

    // Update database
    self.page_repository.lock().await.save(page.clone())?;

    // Update Tantivy index
    if let Some(ref index) = self.tantivy_index {
        index.lock().await.update_page(&page)?;
        index.lock().await.commit()?;
    }

    // Update embeddings
    if let Some(ref embedding_service) = self.embedding_service {
        // Delete old chunks for this page
        embedding_service.delete_page_chunks(&page.id()).await?;

        // Re-embed and index
        embedding_service.embed_and_index_page(&page).await?;
    }

    Ok(())
}
```

## Performance Considerations

### Embedding Generation
- **Model loading:** ~100-500MB memory (one-time cost)
- **Batch processing:** 32 texts per batch for optimal throughput
- **Speed:** ~10-50ms per batch (depending on text length)
- **Caching:** Cache embeddings to avoid regeneration

### Vector Storage
- **Index building:** HNSW index builds incrementally
- **Memory usage:** ~4 bytes per dimension per vector (384 * 4 = 1.5KB per embedding)
- **Search speed:** ~1-10ms for similarity search (depending on collection size)
- **Disk usage:** ~2-3x vector size (including index overhead)

### Scaling Considerations
- **10K blocks:** ~15MB embeddings, ~30MB index, <10ms search
- **100K blocks:** ~150MB embeddings, ~300MB index, ~20ms search
- **1M blocks:** ~1.5GB embeddings, ~3GB index, ~50ms search

## Error Handling

### Embedding Generation Failures
- **Model loading errors:** Fallback to keyword-only search
- **Out of memory:** Reduce batch size, process sequentially
- **Invalid text:** Skip problematic chunks, log errors

### Vector Store Issues
- **Connection failures:** Retry with exponential backoff
- **Index corruption:** Rebuild from stored embeddings
- **Disk full:** Clean up old embeddings, notify user

### Search Failures
- **Query too long:** Truncate to token limit
- **No results:** Fall back to keyword search
- **Timeout:** Return partial results, log performance issue

## Future Enhancements

### Advanced Chunking
- **Semantic chunking:** Split at topic boundaries
- **Hierarchical chunking:** Multi-level chunk sizes
- **Adaptive chunking:** Adjust size based on content type

### Model Improvements
- **Multiple models:** Support different embedding models
- **Fine-tuning:** Train on user's specific domain
- **Multilingual:** Support non-English content

### Search Features
- **Filters:** Combine semantic search with facets
- **Reranking:** Use cross-encoder for final ranking
- **Explanation:** Show why results were matched
- **Feedback:** Learn from user interactions
