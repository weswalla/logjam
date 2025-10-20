/// Service for managing semantic search embeddings
use anyhow::{Context, Result};
use std::sync::Arc;
use tracing::{debug, info, warn};

use crate::application::repositories::PageRepository;
use crate::domain::aggregates::Page;
use crate::domain::base::Entity;
use crate::domain::value_objects::{BlockId, ChunkId, EmbeddingModel, PageId};
use crate::infrastructure::embeddings::{
    ChunkMetadata, FastEmbedService, QdrantVectorStore, TextPreprocessor,
};

/// Configuration for the embedding service
#[derive(Debug, Clone)]
pub struct EmbeddingServiceConfig {
    /// Embedding model to use
    pub model: EmbeddingModel,
    /// Qdrant server URL
    pub qdrant_url: String,
    /// Collection name in Qdrant
    pub collection_name: String,
    /// Maximum words per chunk
    pub max_words_per_chunk: usize,
    /// Overlap words between chunks
    pub overlap_words: usize,
    /// Batch size for embedding generation
    pub batch_size: usize,
}

impl Default for EmbeddingServiceConfig {
    fn default() -> Self {
        EmbeddingServiceConfig {
            model: EmbeddingModel::default(),
            qdrant_url: "http://localhost:6334".to_string(),
            collection_name: "logseq_blocks".to_string(),
            max_words_per_chunk: 150, // ~512 tokens with margin
            overlap_words: 50,
            batch_size: 32,
        }
    }
}

/// Service that orchestrates embedding generation and storage
pub struct EmbeddingService {
    config: EmbeddingServiceConfig,
    embedding_service: Arc<FastEmbedService>,
    vector_store: Arc<QdrantVectorStore>,
    text_preprocessor: Arc<TextPreprocessor>,
}

impl EmbeddingService {
    /// Create a new embedding service
    pub async fn new(config: EmbeddingServiceConfig) -> Result<Self> {
        info!("Initializing EmbeddingService with config: {:?}", config);

        let embedding_service = FastEmbedService::new(config.model)
            .await
            .context("Failed to initialize FastEmbed service")?;

        let vector_store = QdrantVectorStore::new(
            &config.qdrant_url,
            &config.collection_name,
            config.model.dimension_count(),
        )
        .await
        .context("Failed to initialize Qdrant vector store")?;

        Ok(EmbeddingService {
            config,
            embedding_service: Arc::new(embedding_service),
            vector_store: Arc::new(vector_store),
            text_preprocessor: Arc::new(TextPreprocessor::new()),
        })
    }

    /// Create with default configuration
    pub async fn new_default() -> Result<Self> {
        Self::new(EmbeddingServiceConfig::default()).await
    }

    /// Embed a single page and store in vector database
    pub async fn embed_page<R: PageRepository>(
        &self,
        page: &Page,
        _repository: &R,
    ) -> Result<EmbeddingStats> {
        info!("Embedding page: {} ({})", page.title(), page.id());

        let mut stats = EmbeddingStats::default();
        let page_title = page.title();
        let page_id = page.id();

        // Process each block in the page
        let mut all_chunk_data = Vec::new();

        for block in page.all_blocks() {
            let block_id = block.id();
            let content = block.content().as_str();

            if content.trim().is_empty() {
                continue;
            }

            // Get hierarchy path for context
            let hierarchy_path = page
                .get_hierarchy_path(block_id)
                .iter()
                .map(|b| b.content().as_str().to_string())
                .collect::<Vec<_>>();

            // Preprocess the content
            let preprocessed = self.text_preprocessor.preprocess(
                content,
                page_title,
                &hierarchy_path,
            );

            // Chunk the text if needed
            let chunks = self.text_preprocessor.chunk_text(
                &preprocessed,
                self.config.max_words_per_chunk,
                self.config.overlap_words,
            );

            let total_chunks = chunks.len();

            // Create chunk metadata for each chunk
            for (chunk_index, chunk_text) in chunks.into_iter().enumerate() {
                let chunk_id = ChunkId::from_block(block_id, chunk_index);

                let chunk_metadata = ChunkMetadata {
                    chunk_id: chunk_id.as_str().to_string(),
                    block_id: block_id.as_str().to_string(),
                    page_id: page_id.as_str().to_string(),
                    page_title: page_title.to_string(),
                    chunk_index,
                    total_chunks,
                    original_content: content.to_string(),
                    preprocessed_content: chunk_text,
                    hierarchy_path: hierarchy_path.clone(),
                };

                all_chunk_data.push(chunk_metadata);
            }

            stats.blocks_processed += 1;
        }

        stats.chunks_created = all_chunk_data.len();

        // Generate embeddings in batches
        let mut chunk_batch = Vec::new();
        for chunk_metadata in all_chunk_data {
            chunk_batch.push(chunk_metadata);

            if chunk_batch.len() >= self.config.batch_size {
                self.process_chunk_batch(&mut chunk_batch, &mut stats).await?;
            }
        }

        // Process remaining chunks
        if !chunk_batch.is_empty() {
            self.process_chunk_batch(&mut chunk_batch, &mut stats).await?;
        }

        info!(
            "Completed embedding page '{}': {} blocks, {} chunks, {} stored",
            page_title, stats.blocks_processed, stats.chunks_created, stats.chunks_stored
        );

        Ok(stats)
    }

    /// Process a batch of chunks: generate embeddings and store
    async fn process_chunk_batch(
        &self,
        chunk_batch: &mut Vec<ChunkMetadata>,
        stats: &mut EmbeddingStats,
    ) -> Result<()> {
        if chunk_batch.is_empty() {
            return Ok(());
        }

        debug!("Processing batch of {} chunks", chunk_batch.len());

        // Extract preprocessed content for embedding
        let texts: Vec<&str> = chunk_batch
            .iter()
            .map(|c| c.preprocessed_content.as_str())
            .collect();

        // Generate embeddings
        let embeddings = self
            .embedding_service
            .embed_batch(texts)
            .await
            .context("Failed to generate embeddings")?;

        // Pair chunks with embeddings
        let chunk_embedding_pairs: Vec<(ChunkMetadata, _)> = chunk_batch
            .drain(..)
            .zip(embeddings.into_iter())
            .collect();

        // Store in vector database
        self.vector_store
            .insert_chunks_batch(chunk_embedding_pairs)
            .await
            .context("Failed to store chunks in vector database")?;

        stats.chunks_stored += chunk_batch.len();

        Ok(())
    }

    /// Embed multiple pages in batch
    pub async fn embed_pages<R: PageRepository>(
        &self,
        pages: Vec<&Page>,
        repository: &R,
    ) -> Result<EmbeddingStats> {
        let page_count = pages.len();
        info!("Embedding {} pages", page_count);

        let mut total_stats = EmbeddingStats::default();

        for page in pages {
            match self.embed_page(page, repository).await {
                Ok(stats) => {
                    total_stats.blocks_processed += stats.blocks_processed;
                    total_stats.chunks_created += stats.chunks_created;
                    total_stats.chunks_stored += stats.chunks_stored;
                }
                Err(e) => {
                    warn!("Failed to embed page '{}': {}", page.title(), e);
                    total_stats.errors += 1;
                }
            }
        }

        info!(
            "Completed embedding {} pages: {} total chunks stored, {} errors",
            page_count,
            total_stats.chunks_stored,
            total_stats.errors
        );

        Ok(total_stats)
    }

    /// Search for similar content
    pub async fn search(&self, query: &str, limit: usize) -> Result<Vec<crate::infrastructure::embeddings::SearchResult>> {
        debug!("Searching for: '{}' (limit: {})", query, limit);

        // Generate query embedding
        let query_embedding = self
            .embedding_service
            .embed_text(query)
            .await
            .context("Failed to generate query embedding")?;

        // Search vector database
        let results = self
            .vector_store
            .search(&query_embedding, limit as u64)
            .await
            .context("Vector search failed")?;

        debug!("Found {} results", results.len());

        Ok(results)
    }

    /// Delete embeddings for a specific page
    pub async fn delete_page_embeddings(&self, page_id: &PageId) -> Result<()> {
        info!("Deleting embeddings for page: {}", page_id);

        self.vector_store
            .delete_page_chunks(page_id)
            .await
            .context("Failed to delete page embeddings")?;

        Ok(())
    }

    /// Delete embeddings for a specific block
    pub async fn delete_block_embeddings(&self, block_id: &BlockId) -> Result<()> {
        info!("Deleting embeddings for block: {}", block_id);

        self.vector_store
            .delete_block_chunks(block_id)
            .await
            .context("Failed to delete block embeddings")?;

        Ok(())
    }

    /// Get statistics about the vector store
    pub async fn get_stats(&self) -> Result<crate::infrastructure::embeddings::CollectionInfo> {
        self.vector_store
            .get_collection_info()
            .await
            .context("Failed to get vector store stats")
    }
}

/// Statistics from embedding operations
#[derive(Debug, Default, Clone)]
pub struct EmbeddingStats {
    pub blocks_processed: usize,
    pub chunks_created: usize,
    pub chunks_stored: usize,
    pub errors: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::value_objects::{BlockContent, BlockId, PageId};

    #[tokio::test]
    #[ignore] // Requires running Qdrant instance
    async fn test_create_embedding_service() {
        let config = EmbeddingServiceConfig {
            collection_name: format!("test_{}", uuid::Uuid::new_v4()),
            ..Default::default()
        };

        let service = EmbeddingService::new(config).await;
        assert!(service.is_ok());
    }

    #[tokio::test]
    #[ignore] // Requires running Qdrant instance
    async fn test_search() {
        let config = EmbeddingServiceConfig {
            collection_name: format!("test_{}", uuid::Uuid::new_v4()),
            ..Default::default()
        };

        let service = EmbeddingService::new(config).await.unwrap();

        // Search (should return empty on new collection)
        let results = service.search("test query", 5).await;
        assert!(results.is_ok());
        assert_eq!(results.unwrap().len(), 0);
    }
}
