/// FastEmbed service for local embedding generation
use anyhow::{Context, Result};
use fastembed::{EmbeddingModel as FastEmbedModel, InitOptions, TextEmbedding};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, info};

use crate::domain::value_objects::{EmbeddingModel, EmbeddingVector};

/// Service for generating embeddings using fastembed
pub struct FastEmbedService {
    model: Arc<Mutex<TextEmbedding>>,
    model_type: EmbeddingModel,
}

impl FastEmbedService {
    /// Create a new FastEmbed service with the specified model
    pub async fn new(model_type: EmbeddingModel) -> Result<Self> {
        info!("Initializing FastEmbed service with model: {}", model_type);

        let fastembed_model = match model_type {
            EmbeddingModel::AllMiniLML6V2 => FastEmbedModel::AllMiniLML6V2,
        };

        let model = TextEmbedding::try_new(
            InitOptions::new(fastembed_model).with_show_download_progress(true),
        )
        .context("Failed to initialize FastEmbed model")?;

        info!("FastEmbed model initialized successfully");

        Ok(FastEmbedService {
            model: Arc::new(Mutex::new(model)),
            model_type,
        })
    }

    /// Create a new FastEmbed service with the default model
    pub async fn new_default() -> Result<Self> {
        Self::new(EmbeddingModel::default()).await
    }

    /// Generate embedding for a single text
    pub async fn embed_text(&self, text: &str) -> Result<EmbeddingVector> {
        debug!("Generating embedding for text (length: {})", text.len());

        let mut model = self.model.lock().await;
        let embeddings = model
            .embed(vec![text], None)
            .context("Failed to generate embedding")?;

        let embedding_vec = embeddings
            .into_iter()
            .next()
            .context("No embedding returned")?;

        EmbeddingVector::new(embedding_vec)
            .map_err(|e| anyhow::anyhow!("Invalid embedding vector: {}", e))
    }

    /// Generate embeddings for multiple texts in a batch
    /// Returns embeddings in the same order as input texts
    pub async fn embed_batch(&self, texts: Vec<&str>) -> Result<Vec<EmbeddingVector>> {
        debug!("Generating embeddings for batch of {} texts", texts.len());

        if texts.is_empty() {
            return Ok(Vec::new());
        }

        let mut model = self.model.lock().await;
        let embeddings = model
            .embed(texts, None)
            .context("Failed to generate batch embeddings")?;

        let mut result = Vec::with_capacity(embeddings.len());
        for embedding_vec in embeddings {
            let embedding = EmbeddingVector::new(embedding_vec)
                .map_err(|e| anyhow::anyhow!("Invalid embedding vector: {}", e))?;
            result.push(embedding);
        }

        debug!("Generated {} embeddings successfully", result.len());
        Ok(result)
    }

    /// Get the model type being used
    pub fn model_type(&self) -> EmbeddingModel {
        self.model_type
    }

    /// Get the expected dimension count for embeddings
    pub fn dimension_count(&self) -> usize {
        self.model_type.dimension_count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_service() {
        let service = FastEmbedService::new_default().await;
        assert!(service.is_ok());

        let service = service.unwrap();
        assert_eq!(service.model_type(), EmbeddingModel::AllMiniLML6V2);
        assert_eq!(service.dimension_count(), 384);
    }

    #[tokio::test]
    async fn test_embed_single_text() {
        let service = FastEmbedService::new_default().await.unwrap();

        let text = "This is a test sentence for embedding generation.";
        let result = service.embed_text(text).await;

        assert!(result.is_ok());
        let embedding = result.unwrap();
        assert_eq!(embedding.dimension_count(), 384);
    }

    #[tokio::test]
    async fn test_embed_batch() {
        let service = FastEmbedService::new_default().await.unwrap();

        let texts = vec![
            "First sentence for embedding.",
            "Second sentence about different topic.",
            "Third sentence with more content.",
        ];

        let result = service.embed_batch(texts).await;

        assert!(result.is_ok());
        let embeddings = result.unwrap();
        assert_eq!(embeddings.len(), 3);
        for embedding in embeddings {
            assert_eq!(embedding.dimension_count(), 384);
        }
    }

    #[tokio::test]
    async fn test_embed_empty_batch() {
        let service = FastEmbedService::new_default().await.unwrap();

        let texts: Vec<&str> = vec![];
        let result = service.embed_batch(texts).await;

        assert!(result.is_ok());
        let embeddings = result.unwrap();
        assert_eq!(embeddings.len(), 0);
    }

    #[tokio::test]
    async fn test_embedding_similarity() {
        let service = FastEmbedService::new_default().await.unwrap();

        let text1 = "Machine learning is a subset of artificial intelligence.";
        let text2 = "AI and machine learning are related fields.";
        let text3 = "The weather is nice today.";

        let embedding1 = service.embed_text(text1).await.unwrap();
        let embedding2 = service.embed_text(text2).await.unwrap();
        let embedding3 = service.embed_text(text3).await.unwrap();

        // Similar texts should have higher similarity
        let sim_1_2 = embedding1.cosine_similarity(&embedding2).unwrap();
        let sim_1_3 = embedding1.cosine_similarity(&embedding3).unwrap();

        // Semantically similar texts should have higher similarity score
        assert!(sim_1_2 > sim_1_3, "Similar texts should have higher similarity");
    }
}
