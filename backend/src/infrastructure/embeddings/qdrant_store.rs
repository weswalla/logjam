/// Qdrant vector store for semantic search
use anyhow::{Context, Result};
use qdrant_client::{
    Payload,
    Qdrant,
    qdrant::{
        CreateCollectionBuilder, DeletePointsBuilder, Distance, PointStruct,
        SearchPointsBuilder, UpsertPointsBuilder, VectorParamsBuilder,
    },
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::{debug, info, warn};

use crate::domain::value_objects::{BlockId, ChunkId, EmbeddingVector, PageId};

/// Vector store implementation using Qdrant
pub struct QdrantVectorStore {
    client: Qdrant,
    collection_name: String,
    dimension_count: usize,
}

impl QdrantVectorStore {
    /// Create a new Qdrant vector store
    ///
    /// # Arguments
    /// * `url` - Qdrant server URL (e.g., "http://localhost:6334")
    /// * `collection_name` - Name of the collection to use
    /// * `dimension_count` - Vector dimension count (384 for all-MiniLM-L6-v2)
    pub async fn new(
        url: &str,
        collection_name: impl Into<String>,
        dimension_count: usize,
    ) -> Result<Self> {
        info!("Connecting to Qdrant at {}", url);

        let client = Qdrant::from_url(url)
            .build()
            .context("Failed to connect to Qdrant")?;

        let collection_name = collection_name.into();
        let store = QdrantVectorStore {
            client,
            collection_name: collection_name.clone(),
            dimension_count,
        };

        // Ensure collection exists
        if !store.collection_exists().await? {
            info!("Creating collection: {}", collection_name);
            store.create_collection().await?;
        } else {
            info!("Collection '{}' already exists", collection_name);
        }

        Ok(store)
    }

    /// Create a new store with default local connection
    pub async fn new_local(collection_name: impl Into<String>, dimension_count: usize) -> Result<Self> {
        Self::new("http://localhost:6334", collection_name, dimension_count).await
    }

    /// Create collection with proper vector configuration
    async fn create_collection(&self) -> Result<()> {
        self.client
            .create_collection(
                CreateCollectionBuilder::new(&self.collection_name).vectors_config(
                    VectorParamsBuilder::new(self.dimension_count as u64, Distance::Cosine),
                ),
            )
            .await
            .context("Failed to create collection")?;

        info!(
            "Created collection '{}' with {} dimensions",
            self.collection_name, self.dimension_count
        );
        Ok(())
    }

    /// Check if collection exists
    async fn collection_exists(&self) -> Result<bool> {
        let collections = self.client.list_collections().await?;
        Ok(collections
            .collections
            .iter()
            .any(|c| c.name == self.collection_name))
    }

    /// Delete the collection (useful for testing)
    pub async fn delete_collection(&self) -> Result<()> {
        self.client
            .delete_collection(&self.collection_name)
            .await
            .context("Failed to delete collection")?;
        info!("Deleted collection: {}", self.collection_name);
        Ok(())
    }

    /// Insert a single chunk with its embedding
    pub async fn insert_chunk(
        &self,
        chunk: &ChunkMetadata,
        embedding: &EmbeddingVector,
    ) -> Result<()> {
        debug!("Inserting chunk: {}", chunk.chunk_id);

        let payload: Payload = json!({
            "chunk_id": chunk.chunk_id,
            "block_id": chunk.block_id,
            "page_id": chunk.page_id,
            "page_title": chunk.page_title,
            "chunk_index": chunk.chunk_index,
            "total_chunks": chunk.total_chunks,
            "original_content": chunk.original_content,
            "preprocessed_content": chunk.preprocessed_content,
            "hierarchy_path": chunk.hierarchy_path,
            "created_at": chrono::Utc::now().to_rfc3339(),
        })
        .try_into()
        .context("Failed to serialize payload")?;

        let point = PointStruct::new(
            chunk.chunk_id.clone(),
            embedding.dimensions().to_vec(),
            payload,
        );

        self.client
            .upsert_points(
                UpsertPointsBuilder::new(&self.collection_name, vec![point]).wait(true),
            )
            .await
            .context("Failed to insert chunk")?;

        Ok(())
    }

    /// Batch insert chunks (more efficient for multiple chunks)
    pub async fn insert_chunks_batch(
        &self,
        chunks: Vec<(ChunkMetadata, EmbeddingVector)>,
    ) -> Result<()> {
        if chunks.is_empty() {
            return Ok(());
        }

        debug!("Inserting batch of {} chunks", chunks.len());

        let points: Result<Vec<PointStruct>> = chunks
            .into_iter()
            .map(|(chunk, embedding)| {
                let payload: Payload = json!({
                    "chunk_id": chunk.chunk_id,
                    "block_id": chunk.block_id,
                    "page_id": chunk.page_id,
                    "page_title": chunk.page_title,
                    "chunk_index": chunk.chunk_index,
                    "total_chunks": chunk.total_chunks,
                    "original_content": chunk.original_content,
                    "preprocessed_content": chunk.preprocessed_content,
                    "hierarchy_path": chunk.hierarchy_path,
                    "created_at": chrono::Utc::now().to_rfc3339(),
                })
                .try_into()
                .context("Failed to serialize payload")?;

                Ok(PointStruct::new(
                    chunk.chunk_id.clone(),
                    embedding.dimensions().to_vec(),
                    payload,
                ))
            })
            .collect();

        self.client
            .upsert_points(
                UpsertPointsBuilder::new(&self.collection_name, points?).wait(true),
            )
            .await
            .context("Failed to insert batch")?;

        debug!("Batch insert completed");
        Ok(())
    }

    /// Search for similar chunks
    pub async fn search(
        &self,
        query_embedding: &EmbeddingVector,
        limit: u64,
    ) -> Result<Vec<SearchResult>> {
        debug!("Searching with limit: {}", limit);

        let search_result = self
            .client
            .search_points(
                SearchPointsBuilder::new(
                    &self.collection_name,
                    query_embedding.dimensions().to_vec(),
                    limit,
                )
                .with_payload(true),
            )
            .await
            .context("Search failed")?;

        let results: Vec<SearchResult> = search_result
            .result
            .into_iter()
            .map(|point| {
                let payload = point.payload;
                SearchResult {
                    chunk_id: payload
                        .get("chunk_id")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                        .unwrap_or_default(),
                    block_id: payload
                        .get("block_id")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                        .unwrap_or_default(),
                    page_id: payload
                        .get("page_id")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                        .unwrap_or_default(),
                    page_title: payload
                        .get("page_title")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                        .unwrap_or_default(),
                    original_content: payload
                        .get("original_content")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                        .unwrap_or_default(),
                    preprocessed_content: payload
                        .get("preprocessed_content")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                        .unwrap_or_default(),
                    hierarchy_path: payload
                        .get("hierarchy_path")
                        .and_then(|v| v.as_list())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|v| v.as_str().map(String::from))
                                .collect()
                        })
                        .unwrap_or_default(),
                    score: point.score,
                }
            })
            .collect();

        debug!("Found {} results", results.len());
        Ok(results)
    }

    /// Delete a specific chunk
    pub async fn delete_chunk(&self, chunk_id: &ChunkId) -> Result<()> {
        debug!("Deleting chunk: {}", chunk_id);

        use qdrant_client::qdrant::PointId;

        self.client
            .delete_points(
                DeletePointsBuilder::new(&self.collection_name)
                    .points(vec![PointId::from(chunk_id.as_str().to_string())])
                    .wait(true),
            )
            .await
            .context("Failed to delete chunk")?;

        Ok(())
    }

    /// Delete all chunks for a specific block
    pub async fn delete_block_chunks(&self, block_id: &BlockId) -> Result<()> {
        debug!("Deleting all chunks for block: {}", block_id);

        // Note: Qdrant doesn't support filter-based deletion in the same way
        // For now, we'll need to search for chunks and delete by ID
        // In production, consider using Qdrant's scroll API for large deletions
        warn!(
            "Block deletion not yet implemented. Block ID: {}",
            block_id
        );

        Ok(())
    }

    /// Delete all chunks for a specific page
    pub async fn delete_page_chunks(&self, page_id: &PageId) -> Result<()> {
        debug!("Deleting all chunks for page: {}", page_id);

        warn!("Page deletion not yet implemented. Page ID: {}", page_id);

        Ok(())
    }

    /// Get collection info
    pub async fn get_collection_info(&self) -> Result<CollectionInfo> {
        let collection = self
            .client
            .collection_info(&self.collection_name)
            .await
            .context("Failed to get collection info")?;

        let (vectors_count, points_count) = if let Some(result) = collection.result {
            (result.vectors_count, result.points_count)
        } else {
            (None, None)
        };

        Ok(CollectionInfo {
            name: self.collection_name.clone(),
            vectors_count,
            points_count,
        })
    }
}

/// Metadata for a text chunk to be stored in the vector database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkMetadata {
    pub chunk_id: String,
    pub block_id: String,
    pub page_id: String,
    pub page_title: String,
    pub chunk_index: usize,
    pub total_chunks: usize,
    pub original_content: String,
    pub preprocessed_content: String,
    pub hierarchy_path: Vec<String>,
}

/// Search result from vector database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub chunk_id: String,
    pub block_id: String,
    pub page_id: String,
    pub page_title: String,
    pub original_content: String,
    pub preprocessed_content: String,
    pub hierarchy_path: Vec<String>,
    pub score: f32,
}

/// Collection information
#[derive(Debug, Clone)]
pub struct CollectionInfo {
    pub name: String,
    pub vectors_count: Option<u64>,
    pub points_count: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: These tests require a running Qdrant instance
    // Run with: docker run -p 6333:6333 -p 6334:6334 qdrant/qdrant

    async fn create_test_store() -> Result<QdrantVectorStore> {
        let collection_name = format!("test_collection_{}", uuid::Uuid::new_v4());
        QdrantVectorStore::new_local(collection_name, 384).await
    }

    #[tokio::test]
    #[ignore] // Requires running Qdrant instance
    async fn test_create_store() {
        let result = create_test_store().await;
        assert!(result.is_ok());

        let store = result.unwrap();
        let info = store.get_collection_info().await.unwrap();
        assert_eq!(info.points_count, Some(0));
    }

    #[tokio::test]
    #[ignore] // Requires running Qdrant instance
    async fn test_insert_and_search() {
        let store = create_test_store().await.unwrap();

        // Create test data
        let chunk = ChunkMetadata {
            chunk_id: "test-chunk-1".to_string(),
            block_id: "test-block-1".to_string(),
            page_id: "test-page-1".to_string(),
            page_title: "Test Page".to_string(),
            chunk_index: 0,
            total_chunks: 1,
            original_content: "This is test content about Rust programming".to_string(),
            preprocessed_content: "test content Rust programming".to_string(),
            hierarchy_path: vec![],
        };

        let embedding = EmbeddingVector::new(vec![0.1; 384]).unwrap();

        // Insert
        let insert_result = store.insert_chunk(&chunk, &embedding).await;
        assert!(insert_result.is_ok());

        // Search
        let query_embedding = EmbeddingVector::new(vec![0.1; 384]).unwrap();
        let results = store.search(&query_embedding, 5).await.unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].chunk_id, "test-chunk-1");
        assert_eq!(results[0].block_id, "test-block-1");

        // Cleanup
        let _ = store.delete_collection().await;
    }

    #[tokio::test]
    #[ignore] // Requires running Qdrant instance
    async fn test_batch_insert() {
        let store = create_test_store().await.unwrap();

        let chunks: Vec<(ChunkMetadata, EmbeddingVector)> = (0..5)
            .map(|i| {
                let chunk = ChunkMetadata {
                    chunk_id: format!("chunk-{}", i),
                    block_id: format!("block-{}", i),
                    page_id: "page-1".to_string(),
                    page_title: "Test Page".to_string(),
                    chunk_index: 0,
                    total_chunks: 1,
                    original_content: format!("Content {}", i),
                    preprocessed_content: format!("content {}", i),
                    hierarchy_path: vec![],
                };
                let embedding = EmbeddingVector::new(vec![i as f32 * 0.1; 384]).unwrap();
                (chunk, embedding)
            })
            .collect();

        let result = store.insert_chunks_batch(chunks).await;
        assert!(result.is_ok());

        // Verify count
        let info = store.get_collection_info().await.unwrap();
        assert_eq!(info.points_count, Some(5));

        // Cleanup
        let _ = store.delete_collection().await;
    }
}
