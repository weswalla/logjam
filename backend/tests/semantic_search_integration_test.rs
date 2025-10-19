/// Integration tests for semantic search functionality
use backend::application::{
    dto::{SearchRequest, SearchType},
    repositories::PageRepository,
    services::{EmbeddingService, EmbeddingServiceConfig},
    use_cases::SearchPagesAndBlocks,
};
use backend::domain::{
    aggregates::Page,
    base::Entity,
    entities::Block,
    value_objects::{BlockContent, BlockId, IndentLevel, PageId},
    DomainResult,
};
use std::collections::HashMap;
use std::sync::Arc;

/// In-memory repository implementation for testing
struct InMemoryPageRepository {
    pages: HashMap<PageId, Page>,
}

impl InMemoryPageRepository {
    fn new() -> Self {
        Self {
            pages: HashMap::new(),
        }
    }
}

impl PageRepository for InMemoryPageRepository {
    fn save(&mut self, page: Page) -> DomainResult<()> {
        self.pages.insert(page.id().clone(), page);
        Ok(())
    }

    fn find_by_id(&self, id: &PageId) -> DomainResult<Option<Page>> {
        Ok(self.pages.get(id).cloned())
    }

    fn find_by_title(&self, title: &str) -> DomainResult<Option<Page>> {
        Ok(self.pages.values().find(|p| p.title() == title).cloned())
    }

    fn find_all(&self) -> DomainResult<Vec<Page>> {
        Ok(self.pages.values().cloned().collect())
    }

    fn delete(&mut self, id: &PageId) -> DomainResult<bool> {
        Ok(self.pages.remove(id).is_some())
    }
}

/// Create a sample knowledge base for semantic search testing
fn create_semantic_test_knowledge_base() -> InMemoryPageRepository {
    let mut repo = InMemoryPageRepository::new();

    // Page 1: Machine Learning
    let page1_id = PageId::new("ml").unwrap();
    let mut page1 = Page::new(page1_id.clone(), "Machine Learning".to_string());

    let block1_1 = Block::new_root(
        BlockId::new("ml-1").unwrap(),
        BlockContent::new("Machine learning is a subset of artificial intelligence that enables systems to learn and improve from experience without being explicitly programmed."),
    );
    page1.add_block(block1_1).unwrap();

    let block1_2 = Block::new_root(
        BlockId::new("ml-2").unwrap(),
        BlockContent::new("Neural networks are computing systems inspired by biological neural networks that process information through interconnected nodes."),
    );
    page1.add_block(block1_2).unwrap();

    repo.save(page1).unwrap();

    // Page 2: Deep Learning
    let page2_id = PageId::new("dl").unwrap();
    let mut page2 = Page::new(page2_id, "Deep Learning".to_string());

    let block2_1 = Block::new_root(
        BlockId::new("dl-1").unwrap(),
        BlockContent::new("Deep learning uses artificial neural networks with multiple layers to progressively extract higher-level features from raw input."),
    );
    page2.add_block(block2_1).unwrap();

    let block2_2 = Block::new_root(
        BlockId::new("dl-2").unwrap(),
        BlockContent::new("Convolutional neural networks are specialized for processing grid-like data such as images."),
    );
    page2.add_block(block2_2).unwrap();

    repo.save(page2).unwrap();

    // Page 3: Weather (unrelated topic for contrast)
    let page3_id = PageId::new("weather").unwrap();
    let mut page3 = Page::new(page3_id, "Weather".to_string());

    let block3_1 = Block::new_root(
        BlockId::new("weather-1").unwrap(),
        BlockContent::new("The weather today is sunny with clear skies and mild temperatures."),
    );
    page3.add_block(block3_1).unwrap();

    let block3_2 = Block::new_root(
        BlockId::new("weather-2").unwrap(),
        BlockContent::new("Meteorology is the study of atmospheric phenomena including weather patterns and climate."),
    );
    page3.add_block(block3_2).unwrap();

    repo.save(page3).unwrap();

    repo
}

#[tokio::test]
#[ignore] // Requires running Qdrant instance
async fn test_semantic_search_finds_similar_content() {
    // Create unique collection for this test
    let collection_name = format!("test_semantic_{}", uuid::Uuid::new_v4());
    let config = EmbeddingServiceConfig {
        collection_name: collection_name.clone(),
        ..Default::default()
    };

    let embedding_service = Arc::new(EmbeddingService::new(config).await.unwrap());
    let repo = create_semantic_test_knowledge_base();

    // Embed all pages
    let pages = repo.find_all().unwrap();
    let pages_refs: Vec<&Page> = pages.iter().collect();
    embedding_service.embed_pages(pages_refs, &repo).await.unwrap();

    // Search for AI-related content
    let search_use_case = SearchPagesAndBlocks::with_embedding_service(&repo, embedding_service.clone());
    let request = SearchRequest::new("artificial intelligence and neural networks")
        .with_search_type(SearchType::Semantic);

    let results = search_use_case.execute(request).await.unwrap();

    // Should find ML and DL pages (semantically similar)
    // Should NOT rank weather page highly
    assert!(results.len() > 0, "Should find semantic matches");

    // Verify ML/DL content ranks higher than weather
    let top_results: Vec<_> = results.iter().take(3).collect();
    let has_ml_or_dl = top_results.iter().any(|r| {
        if let backend::application::dto::SearchItem::Block(block) = &r.item {
            block.page_title == "Machine Learning" || block.page_title == "Deep Learning"
        } else {
            false
        }
    });

    assert!(has_ml_or_dl, "Top results should include ML or DL content");
}

#[tokio::test]
#[ignore] // Requires running Qdrant instance
async fn test_semantic_search_with_page_filter() {
    let collection_name = format!("test_semantic_filter_{}", uuid::Uuid::new_v4());
    let config = EmbeddingServiceConfig {
        collection_name: collection_name.clone(),
        ..Default::default()
    };

    let embedding_service = Arc::new(EmbeddingService::new(config).await.unwrap());
    let repo = create_semantic_test_knowledge_base();

    // Embed all pages
    let pages = repo.find_all().unwrap();
    let pages_refs: Vec<&Page> = pages.iter().collect();
    embedding_service.embed_pages(pages_refs, &repo).await.unwrap();

    // Search with page filter
    let search_use_case = SearchPagesAndBlocks::with_embedding_service(&repo, embedding_service.clone());
    let page_id = PageId::new("ml").unwrap();
    let request = SearchRequest::new("neural networks")
        .with_search_type(SearchType::Semantic)
        .with_page_filters(vec![page_id]);

    let results = search_use_case.execute(request).await.unwrap();

    // Should only find results from Machine Learning page
    for result in &results {
        if let backend::application::dto::SearchItem::Block(block) = &result.item {
            assert_eq!(block.page_title, "Machine Learning", "Should only return ML page results");
        }
    }
}

#[tokio::test]
#[ignore] // Requires running Qdrant instance
async fn test_embedding_stats() {
    let collection_name = format!("test_stats_{}", uuid::Uuid::new_v4());
    let config = EmbeddingServiceConfig {
        collection_name: collection_name.clone(),
        ..Default::default()
    };

    let embedding_service = EmbeddingService::new(config).await.unwrap();
    let repo = create_semantic_test_knowledge_base();

    // Embed a single page
    let page = repo.find_by_title("Machine Learning").unwrap().unwrap();
    let stats = embedding_service.embed_page(&page, &repo).await.unwrap();

    // Verify stats
    assert_eq!(stats.blocks_processed, 2, "Should process 2 blocks");
    assert!(stats.chunks_created > 0, "Should create chunks");
    assert!(stats.chunks_stored > 0, "Should store chunks");
    assert_eq!(stats.errors, 0, "Should have no errors");

    // Check vector store stats
    let collection_info = embedding_service.get_stats().await.unwrap();
    assert!(collection_info.vectors_count.unwrap_or(0) > 0, "Should have vectors in collection");
}

#[tokio::test]
#[ignore] // Requires running Qdrant instance
async fn test_delete_page_embeddings() {
    let collection_name = format!("test_delete_{}", uuid::Uuid::new_v4());
    let config = EmbeddingServiceConfig {
        collection_name: collection_name.clone(),
        ..Default::default()
    };

    let embedding_service = Arc::new(EmbeddingService::new(config).await.unwrap());
    let repo = create_semantic_test_knowledge_base();

    // Embed all pages
    let pages = repo.find_all().unwrap();
    let pages_refs: Vec<&Page> = pages.iter().collect();
    embedding_service.embed_pages(pages_refs, &repo).await.unwrap();

    // Get initial stats
    let initial_stats = embedding_service.get_stats().await.unwrap();
    let initial_count = initial_stats.points_count;

    // Delete one page's embeddings
    let page_id = PageId::new("ml").unwrap();
    embedding_service.delete_page_embeddings(&page_id).await.unwrap();

    // Verify deletion
    let final_stats = embedding_service.get_stats().await.unwrap();
    let final_count = final_stats.points_count;

    assert!(final_count < initial_count, "Point count should decrease after deletion");
}

#[tokio::test]
#[ignore] // Requires running Qdrant instance
async fn test_chunking_for_long_content() {
    let collection_name = format!("test_chunking_{}", uuid::Uuid::new_v4());
    let config = EmbeddingServiceConfig {
        collection_name: collection_name.clone(),
        max_words_per_chunk: 20, // Small chunks for testing
        overlap_words: 5,
        ..Default::default()
    };

    let embedding_service = EmbeddingService::new(config).await.unwrap();
    let mut repo = InMemoryPageRepository::new();

    // Create a page with long content
    let page_id = PageId::new("long-page").unwrap();
    let mut page = Page::new(page_id.clone(), "Long Content".to_string());

    let long_content = "This is a very long piece of content that will be split into multiple chunks. \
                        Each chunk should have some overlap with the previous chunk to maintain context. \
                        The chunking algorithm needs to handle word boundaries properly. \
                        This ensures that semantic meaning is preserved across chunk boundaries. \
                        Testing this functionality is important for the semantic search system.";

    let block = Block::new_root(
        BlockId::new("long-1").unwrap(),
        BlockContent::new(long_content),
    );
    page.add_block(block).unwrap();
    repo.save(page.clone()).unwrap();

    // Embed the page
    let stats = embedding_service.embed_page(&page, &repo).await.unwrap();

    // Should create multiple chunks
    assert!(stats.chunks_created > 1, "Long content should be split into multiple chunks");
    assert_eq!(stats.blocks_processed, 1, "Should process 1 block");
}

#[tokio::test]
#[ignore] // Requires running Qdrant instance
async fn test_semantic_vs_traditional_search() {
    let collection_name = format!("test_comparison_{}", uuid::Uuid::new_v4());
    let config = EmbeddingServiceConfig {
        collection_name: collection_name.clone(),
        ..Default::default()
    };

    let embedding_service = Arc::new(EmbeddingService::new(config).await.unwrap());
    let repo = create_semantic_test_knowledge_base();

    // Embed all pages
    let pages = repo.find_all().unwrap();
    let pages_refs: Vec<&Page> = pages.iter().collect();
    embedding_service.embed_pages(pages_refs, &repo).await.unwrap();

    let search_use_case = SearchPagesAndBlocks::with_embedding_service(&repo, embedding_service.clone());

    // Query: "AI systems" (not exact match for any content)
    let semantic_request = SearchRequest::new("AI systems")
        .with_search_type(SearchType::Semantic);
    let semantic_results = search_use_case.execute(semantic_request).await.unwrap();

    let traditional_request = SearchRequest::new("AI systems")
        .with_search_type(SearchType::Traditional);
    let traditional_results = search_use_case.execute(traditional_request).await.unwrap();

    // Semantic search should find ML content (AI is related to artificial intelligence)
    // Traditional search might not find exact matches
    assert!(semantic_results.len() > 0, "Semantic search should find related content");

    // Both should work but may have different results
    println!("Semantic results: {}", semantic_results.len());
    println!("Traditional results: {}", traditional_results.len());
}

#[tokio::test]
#[ignore] // Requires running Qdrant instance
async fn test_hierarchical_context_in_embeddings() {
    let collection_name = format!("test_hierarchy_{}", uuid::Uuid::new_v4());
    let config = EmbeddingServiceConfig {
        collection_name: collection_name.clone(),
        ..Default::default()
    };

    let embedding_service = EmbeddingService::new(config).await.unwrap();
    let mut repo = InMemoryPageRepository::new();

    // Create a page with nested structure
    let page_id = PageId::new("nested").unwrap();
    let mut page = Page::new(page_id.clone(), "Programming Concepts".to_string());

    let parent_block = Block::new_root(
        BlockId::new("parent").unwrap(),
        BlockContent::new("Data structures are ways to organize data"),
    );
    page.add_block(parent_block.clone()).unwrap();

    let child_block = Block::new_child(
        BlockId::new("child").unwrap(),
        BlockContent::new("Arrays store elements in contiguous memory"),
        BlockId::new("parent").unwrap(),
        IndentLevel::new(1),
    );

    // Update parent's children
    if let Some(parent) = page.get_block_mut(&BlockId::new("parent").unwrap()) {
        parent.add_child(child_block.id().clone());
    }
    page.add_block(child_block).unwrap();

    repo.save(page.clone()).unwrap();

    // Embed the page
    let stats = embedding_service.embed_page(&page, &repo).await.unwrap();

    assert_eq!(stats.blocks_processed, 2, "Should process parent and child blocks");
    assert!(stats.chunks_stored > 0, "Should store chunks with hierarchical context");
}
