# Tantivy Full-Text Search Implementation Plan

## Overview

Implement full-text search capabilities using Tantivy, a high-performance search engine library written in Rust. This will enable fast, typo-tolerant searches across pages, blocks, URLs, and page references following the existing DDD architecture.

## Goals

1. **Fast full-text search** across all content (pages, blocks, URLs, tags)
2. **Typo-tolerant fuzzy search** for better user experience
3. **Ranked results** with relevance scoring (BM25 algorithm)
4. **Filter by content type** (pages vs blocks, tags vs links)
5. **Incremental indexing** (update index as content changes)
6. **Real-time sync** with repository changes
7. **Performant queries** (<50ms for typical searches)

## Why Tantivy?

**Tantivy** is a modern full-text search library similar to Apache Lucene:

- **Pure Rust:** Memory-safe, fast, integrates seamlessly with our codebase
- **Battle-tested:** Used in production by Quickwit, Meilisearch
- **Full-featured:** BM25 ranking, fuzzy search, highlighting, faceting
- **Performant:** ~10-100x faster than SQLite FTS5 for complex queries
- **Embedded:** No separate server process needed (unlike Elasticsearch)

**Alternatives considered:**
- SQLite FTS5: Limited features, slower for fuzzy search
- Meilisearch: Requires separate server, heavier weight
- Typesense: Also requires server, adds deployment complexity

## Architecture Layer

**Infrastructure Layer** (`backend/src/infrastructure/search/`)

Search is infrastructure because:
- Domain layer remains search-agnostic
- Tantivy is an implementation detail (could swap with Meilisearch)
- Search index is a derived data structure (not source of truth)

## Dependencies

```toml
# backend/Cargo.toml

[dependencies]
tantivy = "0.22"  # Full-text search engine
```

## Index Schema Design

### Document Structure

Tantivy indexes "documents" with typed fields. We'll create a flattened search index:

```rust
// backend/src/infrastructure/search/schema.rs

use tantivy::schema::*;

pub struct SearchSchema {
    schema: Schema,

    // Document ID fields
    pub page_id: Field,
    pub block_id: Field,  // Empty for page-level documents

    // Content fields (searchable)
    pub page_title: Field,
    pub block_content: Field,
    pub urls: Field,
    pub page_references: Field,

    // Metadata fields (filterable, not searchable)
    pub document_type: Field,  // "page" or "block"
    pub reference_type: Field,  // "link", "tag", or empty
    pub indent_level: Field,

    // Ranking signals
    pub url_domains: Field,
}

impl SearchSchema {
    pub fn build() -> (Schema, Self) {
        let mut schema_builder = Schema::builder();

        // IDs (stored, not indexed for search)
        let page_id = schema_builder.add_text_field("page_id", STRING | STORED);
        let block_id = schema_builder.add_text_field("block_id", STRING | STORED);

        // Searchable text fields with different weights
        let page_title = schema_builder.add_text_field(
            "page_title",
            TextOptions::default()
                .set_indexing_options(
                    TextFieldIndexing::default()
                        .set_tokenizer("en_stem")  // English stemming
                        .set_index_option(IndexRecordOption::WithFreqsAndPositions)
                )
                .set_stored()
        );

        let block_content = schema_builder.add_text_field(
            "block_content",
            TextOptions::default()
                .set_indexing_options(
                    TextFieldIndexing::default()
                        .set_tokenizer("en_stem")
                        .set_index_option(IndexRecordOption::WithFreqsAndPositions)
                )
                .set_stored()
        );

        let urls = schema_builder.add_text_field(
            "urls",
            TextOptions::default()
                .set_indexing_options(
                    TextFieldIndexing::default()
                        .set_tokenizer("raw")  // Don't stem URLs
                        .set_index_option(IndexRecordOption::WithFreqsAndPositions)
                )
        );

        let page_references = schema_builder.add_text_field(
            "page_references",
            TextOptions::default()
                .set_indexing_options(
                    TextFieldIndexing::default()
                        .set_tokenizer("en_stem")
                        .set_index_option(IndexRecordOption::WithFreqsAndPositions)
                )
        );

        // Facet fields for filtering
        let document_type = schema_builder.add_facet_field("document_type", STORED);
        let reference_type = schema_builder.add_facet_field("reference_type", STORED);
        let indent_level = schema_builder.add_u64_field("indent_level", INDEXED | STORED);

        // Domain extraction for URL filtering
        let url_domains = schema_builder.add_facet_field("url_domains", INDEXED);

        let schema = schema_builder.build();

        (schema.clone(), Self {
            schema,
            page_id,
            block_id,
            page_title,
            block_content,
            urls,
            page_references,
            document_type,
            reference_type,
            indent_level,
            url_domains,
        })
    }
}
```

### Indexing Strategy

**Two document types:**

1. **Page documents:** For searching by page title
   - `page_id`: PageId
   - `page_title`: Searchable page title
   - `document_type`: "page"

2. **Block documents:** For searching block content
   - `page_id`: Parent page ID
   - `block_id`: Block ID
   - `block_content`: Searchable content
   - `urls`: Concatenated URLs
   - `page_references`: Concatenated page refs
   - `document_type`: "block"
   - `indent_level`: Hierarchy depth

**Rationale:** Separate document types allow:
- Title-only search ("find page titled X")
- Content-only search ("find blocks containing X")
- Combined search with different weights (title matches rank higher)

## Search Index Implementation

### TantivySearchIndex

```rust
// backend/src/infrastructure/search/tantivy_index.rs

use tantivy::{Index, IndexWriter, IndexReader, ReloadPolicy, TantivyDocument};
use tantivy::collector::TopDocs;
use tantivy::query::{QueryParser, FuzzyTermQuery, BooleanQuery, Occur};
use tantivy::schema::*;
use std::path::Path;
use crate::domain::{Page, Block, PageId, BlockId};
use super::schema::SearchSchema;

pub struct TantivySearchIndex {
    index: Index,
    schema: SearchSchema,
    writer: IndexWriter,
    reader: IndexReader,
}

impl TantivySearchIndex {
    /// Create new search index at the given directory
    pub fn new(index_dir: impl AsRef<Path>) -> tantivy::Result<Self> {
        let (schema_def, schema) = SearchSchema::build();

        let index = Index::create_in_dir(index_dir, schema_def.clone())?;

        let writer = index.writer(50_000_000)?;  // 50MB heap

        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()?;

        Ok(Self {
            index,
            schema,
            writer,
            reader,
        })
    }

    /// Create in-memory index (for testing)
    pub fn new_in_memory() -> tantivy::Result<Self> {
        let (schema_def, schema) = SearchSchema::build();

        let index = Index::create_in_ram(schema_def.clone());

        let writer = index.writer(50_000_000)?;

        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()?;

        Ok(Self {
            index,
            schema,
            writer,
            reader,
        })
    }

    /// Index a page (creates page document + block documents)
    pub fn index_page(&mut self, page: &Page) -> tantivy::Result<()> {
        // Create page document
        let mut page_doc = TantivyDocument::default();
        page_doc.add_text(self.schema.page_id, page.id().as_str());
        page_doc.add_text(self.schema.block_id, "");  // Empty for page docs
        page_doc.add_text(self.schema.page_title, page.title());
        page_doc.add_facet(self.schema.document_type, "/page");

        self.writer.add_document(page_doc)?;

        // Create block documents
        for block in page.all_blocks() {
            let mut block_doc = TantivyDocument::default();

            block_doc.add_text(self.schema.page_id, page.id().as_str());
            block_doc.add_text(self.schema.block_id, block.id().as_str());
            block_doc.add_text(self.schema.page_title, page.title());
            block_doc.add_text(self.schema.block_content, block.content().as_str());

            // Add URLs
            let urls_text = block.urls()
                .iter()
                .map(|u| u.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            if !urls_text.is_empty() {
                block_doc.add_text(self.schema.urls, &urls_text);
            }

            // Add URL domains as facets
            for url in block.urls() {
                if let Some(domain) = url.domain() {
                    block_doc.add_facet(
                        self.schema.url_domains,
                        &format!("/domain/{}", domain)
                    );
                }
            }

            // Add page references
            let refs_text = block.page_references()
                .iter()
                .map(|r| r.text())
                .collect::<Vec<_>>()
                .join(" ");
            if !refs_text.is_empty() {
                block_doc.add_text(self.schema.page_references, &refs_text);
            }

            // Add reference type facets
            for page_ref in block.page_references() {
                let facet_path = match page_ref.reference_type() {
                    ReferenceType::Link => "/reference/link",
                    ReferenceType::Tag => "/reference/tag",
                };
                block_doc.add_facet(self.schema.reference_type, facet_path);
            }

            block_doc.add_facet(self.schema.document_type, "/block");
            block_doc.add_u64(self.schema.indent_level, block.indent_level().level() as u64);

            self.writer.add_document(block_doc)?;
        }

        Ok(())
    }

    /// Remove all documents for a page
    pub fn delete_page(&mut self, page_id: &PageId) -> tantivy::Result<()> {
        let term = Term::from_field_text(self.schema.page_id, page_id.as_str());
        self.writer.delete_term(term);
        Ok(())
    }

    /// Update a page (delete + re-index)
    pub fn update_page(&mut self, page: &Page) -> tantivy::Result<()> {
        self.delete_page(page.id())?;
        self.index_page(page)?;
        Ok(())
    }

    /// Commit all pending changes
    pub fn commit(&mut self) -> tantivy::Result<()> {
        self.writer.commit()?;
        Ok(())
    }

    /// Search with query string
    pub fn search(&self, query_str: &str, limit: usize) -> tantivy::Result<Vec<SearchResult>> {
        let searcher = self.reader.searcher();

        // Parse query across multiple fields
        let query_parser = QueryParser::for_index(
            &self.index,
            vec![
                self.schema.page_title,
                self.schema.block_content,
                self.schema.urls,
                self.schema.page_references,
            ],
        );

        let query = query_parser.parse_query(query_str)?;

        // Execute search
        let top_docs = searcher.search(&query, &TopDocs::with_limit(limit))?;

        // Convert to SearchResult
        let mut results = Vec::new();
        for (_score, doc_address) in top_docs {
            let retrieved_doc: TantivyDocument = searcher.doc(doc_address)?;
            results.push(SearchResult::from_document(&retrieved_doc, &self.schema)?);
        }

        Ok(results)
    }

    /// Fuzzy search (typo-tolerant)
    pub fn fuzzy_search(&self, query_str: &str, limit: usize, max_distance: u8) -> tantivy::Result<Vec<SearchResult>> {
        let searcher = self.reader.searcher();

        // Build fuzzy query for each term
        let terms: Vec<_> = query_str.split_whitespace().collect();
        let mut queries: Vec<Box<dyn tantivy::query::Query>> = Vec::new();

        for term in terms {
            // Fuzzy search on title
            let title_term = Term::from_field_text(self.schema.page_title, term);
            queries.push(Box::new(FuzzyTermQuery::new(title_term, max_distance, true)));

            // Fuzzy search on content
            let content_term = Term::from_field_text(self.schema.block_content, term);
            queries.push(Box::new(FuzzyTermQuery::new(content_term, max_distance, true)));
        }

        let query = BooleanQuery::new(
            queries.into_iter().map(|q| (Occur::Should, q)).collect()
        );

        let top_docs = searcher.search(&query, &TopDocs::with_limit(limit))?;

        let mut results = Vec::new();
        for (_score, doc_address) in top_docs {
            let retrieved_doc: TantivyDocument = searcher.doc(doc_address)?;
            results.push(SearchResult::from_document(&retrieved_doc, &self.schema)?);
        }

        Ok(results)
    }

    /// Search with filters
    pub fn search_with_filters(
        &self,
        query_str: &str,
        limit: usize,
        filters: SearchFilters,
    ) -> tantivy::Result<Vec<SearchResult>> {
        let searcher = self.reader.searcher();

        let mut query_parser = QueryParser::for_index(
            &self.index,
            vec![
                self.schema.page_title,
                self.schema.block_content,
                self.schema.urls,
                self.schema.page_references,
            ],
        );

        let text_query = query_parser.parse_query(query_str)?;

        // Build filter queries
        let mut queries: Vec<(Occur, Box<dyn tantivy::query::Query>)> = vec![
            (Occur::Must, text_query),
        ];

        if let Some(doc_type) = filters.document_type {
            let facet = Facet::from(&format!("/{}", doc_type));
            let facet_term = Term::from_facet(self.schema.document_type, &facet);
            queries.push((
                Occur::Must,
                Box::new(tantivy::query::TermQuery::new(
                    facet_term,
                    IndexRecordOption::Basic,
                )),
            ));
        }

        if let Some(ref_type) = filters.reference_type {
            let facet = Facet::from(&format!("/reference/{}", ref_type));
            let facet_term = Term::from_facet(self.schema.reference_type, &facet);
            queries.push((
                Occur::Must,
                Box::new(tantivy::query::TermQuery::new(
                    facet_term,
                    IndexRecordOption::Basic,
                )),
            ));
        }

        let final_query = BooleanQuery::new(queries);

        let top_docs = searcher.search(&final_query, &TopDocs::with_limit(limit))?;

        let mut results = Vec::new();
        for (_score, doc_address) in top_docs {
            let retrieved_doc: TantivyDocument = searcher.doc(doc_address)?;
            results.push(SearchResult::from_document(&retrieved_doc, &self.schema)?);
        }

        Ok(results)
    }
}
```

### Search Result Types

```rust
// backend/src/infrastructure/search/result.rs

use crate::domain::{PageId, BlockId};
use tantivy::TantivyDocument;
use super::schema::SearchSchema;

#[derive(Debug, Clone)]
pub enum SearchResult {
    PageResult {
        page_id: PageId,
        page_title: String,
        score: f32,
    },
    BlockResult {
        page_id: PageId,
        block_id: BlockId,
        page_title: String,
        block_content: String,
        indent_level: usize,
        score: f32,
    },
}

impl SearchResult {
    pub fn from_document(doc: &TantivyDocument, schema: &SearchSchema) -> tantivy::Result<Self> {
        let page_id_str = doc.get_first(schema.page_id)
            .and_then(|v| v.as_str())
            .ok_or_else(|| tantivy::TantivyError::InvalidArgument("Missing page_id".into()))?;

        let page_id = PageId::new(page_id_str)
            .map_err(|e| tantivy::TantivyError::InvalidArgument(e.to_string()))?;

        let page_title = doc.get_first(schema.page_title)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let block_id_str = doc.get_first(schema.block_id)
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if block_id_str.is_empty() {
            // Page result
            Ok(SearchResult::PageResult {
                page_id,
                page_title,
                score: 0.0,  // Score set by caller
            })
        } else {
            // Block result
            let block_id = BlockId::new(block_id_str)
                .map_err(|e| tantivy::TantivyError::InvalidArgument(e.to_string()))?;

            let block_content = doc.get_first(schema.block_content)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let indent_level = doc.get_first(schema.indent_level)
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as usize;

            Ok(SearchResult::BlockResult {
                page_id,
                block_id,
                page_title,
                block_content,
                indent_level,
                score: 0.0,
            })
        }
    }

    pub fn page_id(&self) -> &PageId {
        match self {
            SearchResult::PageResult { page_id, .. } => page_id,
            SearchResult::BlockResult { page_id, .. } => page_id,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct SearchFilters {
    pub document_type: Option<String>,  // "page" or "block"
    pub reference_type: Option<String>,  // "link" or "tag"
    pub min_indent_level: Option<usize>,
    pub max_indent_level: Option<usize>,
}
```

## Search Service (Application Layer)

### SearchService

```rust
// backend/src/application/services/search_service.rs

use crate::infrastructure::search::{TantivySearchIndex, SearchResult, SearchFilters};
use crate::domain::{PageId, DomainResult};

pub struct SearchService {
    index: TantivySearchIndex,
}

impl SearchService {
    pub fn new(index: TantivySearchIndex) -> Self {
        Self { index }
    }

    /// Full-text search
    pub fn search(&self, query: &str, limit: usize) -> DomainResult<Vec<SearchResult>> {
        self.index.search(query, limit)
            .map_err(|e| DomainError::InvalidOperation(format!("Search error: {}", e)))
    }

    /// Fuzzy search (typo-tolerant, max edit distance = 2)
    pub fn fuzzy_search(&self, query: &str, limit: usize) -> DomainResult<Vec<SearchResult>> {
        self.index.fuzzy_search(query, limit, 2)
            .map_err(|e| DomainError::InvalidOperation(format!("Fuzzy search error: {}", e)))
    }

    /// Search with filters
    pub fn search_with_filters(
        &self,
        query: &str,
        limit: usize,
        filters: SearchFilters,
    ) -> DomainResult<Vec<SearchResult>> {
        self.index.search_with_filters(query, limit, filters)
            .map_err(|e| DomainError::InvalidOperation(format!("Filtered search error: {}", e)))
    }

    /// Search only page titles
    pub fn search_pages(&self, query: &str, limit: usize) -> DomainResult<Vec<SearchResult>> {
        let filters = SearchFilters {
            document_type: Some("page".to_string()),
            ..Default::default()
        };
        self.search_with_filters(query, limit, filters)
    }

    /// Search only block content
    pub fn search_blocks(&self, query: &str, limit: usize) -> DomainResult<Vec<SearchResult>> {
        let filters = SearchFilters {
            document_type: Some("block".to_string()),
            ..Default::default()
        };
        self.search_with_filters(query, limit, filters)
    }

    /// Search only tags
    pub fn search_tags(&self, query: &str, limit: usize) -> DomainResult<Vec<SearchResult>> {
        let filters = SearchFilters {
            reference_type: Some("tag".to_string()),
            ..Default::default()
        };
        self.search_with_filters(query, limit, filters)
    }
}
```

## Integration with Existing Services

### Update ImportService

```rust
// backend/src/application/services/import_service.rs

pub struct ImportService<P: PageRepository, M: FileMappingRepository> {
    page_repository: P,
    mapping_repository: M,
    search_index: Option<Arc<Mutex<TantivySearchIndex>>>,  // Optional for now
    max_concurrent_files: usize,
}

impl<P: PageRepository, M: FileMappingRepository> ImportService<P, M> {
    pub fn with_search_index(mut self, index: Arc<Mutex<TantivySearchIndex>>) -> Self {
        self.search_index = Some(index);
        self
    }

    async fn process_file(&mut self, path: PathBuf) -> ImportResult<()> {
        // ... existing parsing and repository save logic ...

        // Index page in search index
        if let Some(ref index) = self.search_index {
            let mut index_lock = index.lock().await;
            index_lock.index_page(&page)
                .map_err(|e| ImportError::SearchIndex(e.to_string()))?;
        }

        Ok(())
    }

    pub async fn import_directory(/* ... */) -> ImportResult<ImportSummary> {
        // ... existing import logic ...

        // Commit search index
        if let Some(ref index) = self.search_index {
            let mut index_lock = index.lock().await;
            index_lock.commit()
                .map_err(|e| ImportError::SearchIndex(e.to_string()))?;
        }

        Ok(summary)
    }
}
```

### Update SyncService

```rust
// backend/src/application/services/sync_service.rs

pub struct SyncService<P: PageRepository, M: FileMappingRepository> {
    page_repository: Arc<Mutex<P>>,
    mapping_repository: Arc<Mutex<M>>,
    search_index: Option<Arc<Mutex<TantivySearchIndex>>>,
    directory_path: LogseqDirectoryPath,
    watcher: LogseqFileWatcher,
}

impl<P, M> SyncService<P, M>
where
    P: PageRepository + Send + 'static,
    M: FileMappingRepository + Send + 'static,
{
    pub fn with_search_index(mut self, index: Arc<Mutex<TantivySearchIndex>>) -> Self {
        self.search_index = Some(index);
        self
    }

    async fn handle_file_created(&self, path: PathBuf) -> SyncResult<()> {
        // ... existing parsing and repository save logic ...

        // Index in search
        if let Some(ref index) = self.search_index {
            let mut index_lock = index.lock().await;
            index_lock.index_page(&page)?;
            index_lock.commit()?;
        }

        Ok(())
    }

    async fn handle_file_updated(&self, path: PathBuf) -> SyncResult<()> {
        // ... existing update logic ...

        // Update search index
        if let Some(ref index) = self.search_index {
            let mut index_lock = index.lock().await;
            index_lock.update_page(&page)?;
            index_lock.commit()?;
        }

        Ok(())
    }

    async fn handle_file_deleted(&self, path: PathBuf) -> SyncResult<()> {
        // ... existing deletion logic ...

        // Delete from search index
        if let Some(ref index) = self.search_index {
            let mut index_lock = index.lock().await;
            index_lock.delete_page(&page_id)?;
            index_lock.commit()?;
        }

        Ok(())
    }
}
```

## Tauri Integration

### Search DTOs

```rust
// backend/src/tauri/dto.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchRequest {
    pub query: String,
    pub limit: usize,
    pub fuzzy: bool,
    pub filters: Option<SearchFiltersDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchFiltersDto {
    pub document_type: Option<String>,
    pub reference_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SearchResultDto {
    PageResult {
        page_id: String,
        page_title: String,
        score: f32,
    },
    BlockResult {
        page_id: String,
        block_id: String,
        page_title: String,
        block_content: String,
        indent_level: usize,
        score: f32,
    },
}
```

### Search Commands

```rust
// backend/src/tauri/commands/search.rs

#[tauri::command]
pub async fn search(
    state: State<'_, AppState>,
    request: SearchRequest,
) -> Result<Vec<SearchResultDto>, ErrorResponse> {
    let search_service = state.search_service.lock().await;

    let results = if request.fuzzy {
        search_service.fuzzy_search(&request.query, request.limit)?
    } else if let Some(filters) = request.filters {
        let search_filters = SearchFilters {
            document_type: filters.document_type,
            reference_type: filters.reference_type,
            ..Default::default()
        };
        search_service.search_with_filters(&request.query, request.limit, search_filters)?
    } else {
        search_service.search(&request.query, request.limit)?
    };

    Ok(results.into_iter().map(DtoMapper::search_result_to_dto).collect())
}

#[tauri::command]
pub async fn search_pages(
    state: State<'_, AppState>,
    query: String,
    limit: usize,
) -> Result<Vec<SearchResultDto>, ErrorResponse> {
    let search_service = state.search_service.lock().await;
    let results = search_service.search_pages(&query, limit)?;
    Ok(results.into_iter().map(DtoMapper::search_result_to_dto).collect())
}

#[tauri::command]
pub async fn search_tags(
    state: State<'_, AppState>,
    query: String,
    limit: usize,
) -> Result<Vec<SearchResultDto>, ErrorResponse> {
    let search_service = state.search_service.lock().await;
    let results = search_service.search_tags(&query, limit)?;
    Ok(results.into_iter().map(DtoMapper::search_result_to_dto).collect())
}
```

## Frontend Integration

### TypeScript API

```typescript
// frontend/src/lib/tauri-api.ts

export interface SearchRequest {
  query: string;
  limit: number;
  fuzzy: boolean;
  filters?: SearchFilters;
}

export interface SearchFilters {
  document_type?: 'page' | 'block';
  reference_type?: 'link' | 'tag';
}

export type SearchResultDto =
  | {
      type: 'PageResult';
      page_id: string;
      page_title: string;
      score: number;
    }
  | {
      type: 'BlockResult';
      page_id: string;
      block_id: string;
      page_title: string;
      block_content: string;
      indent_level: number;
      score: number;
    };

export class TauriApi {
  static async search(request: SearchRequest): Promise<SearchResultDto[]> {
    return await invoke<SearchResultDto[]>('search', { request });
  }

  static async searchPages(query: string, limit: number): Promise<SearchResultDto[]> {
    return await invoke<SearchResultDto[]>('search_pages', { query, limit });
  }

  static async searchTags(query: string, limit: number): Promise<SearchResultDto[]> {
    return await invoke<SearchResultDto[]>('search_tags', { query, limit });
  }
}
```

### React Hook

```typescript
// frontend/src/hooks/useSearch.ts

import { useState, useCallback } from 'react';
import { TauriApi, SearchRequest, SearchResultDto } from '../lib/tauri-api';

export function useSearch() {
  const [results, setResults] = useState<SearchResultDto[]>([]);
  const [isSearching, setIsSearching] = useState(false);
  const [error, setError] = useState<Error | null>(null);

  const search = useCallback(async (request: SearchRequest) => {
    setIsSearching(true);
    setError(null);

    try {
      const searchResults = await TauriApi.search(request);
      setResults(searchResults);
    } catch (err) {
      setError(err as Error);
    } finally {
      setIsSearching(false);
    }
  }, []);

  const searchPages = useCallback(async (query: string, limit = 20) => {
    setIsSearching(true);
    setError(null);

    try {
      const searchResults = await TauriApi.searchPages(query, limit);
      setResults(searchResults);
    } catch (err) {
      setError(err as Error);
    } finally {
      setIsSearching(false);
    }
  }, []);

  return {
    results,
    isSearching,
    error,
    search,
    searchPages,
  };
}
```

## Performance Optimization

### Indexing Strategies

**Incremental indexing:**
- Commit after each page during import (slower but progress visible)
- Batch commits during bulk import (faster, commit every 100 pages)

**Commit frequency trade-offs:**
```rust
// Option 1: Commit after every page (real-time, slower)
for page in pages {
    index.index_page(&page)?;
    index.commit()?;  // Slow: ~10-50ms per commit
}

// Option 2: Batch commits (faster, less real-time)
for page in pages {
    index.index_page(&page)?;
}
index.commit()?;  // Fast: Single commit for all pages
```

**Recommendation:** Batch commits during import, real-time commits during sync.

### Query Optimization

**Limit result set:**
```rust
// Always specify reasonable limits
search_service.search(query, 100)  // Max 100 results
```

**Use filters to narrow scope:**
```rust
// Faster: Search only blocks
search_service.search_blocks(query, 20)

// Slower: Search everything
search_service.search(query, 20)
```

### Expected Performance

- **Index build:** ~1000-5000 pages/second (depends on page size)
- **Search latency:** ~5-50ms for typical queries
- **Fuzzy search:** ~10-100ms (more expensive than exact search)
- **Index size:** ~10-20% of original markdown size

## Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_index_and_search_page() {
        let mut index = TantivySearchIndex::new_in_memory().unwrap();

        let page = Page::new(
            PageId::new("test-page").unwrap(),
            "Test Page Title".to_string(),
        );

        index.index_page(&page).unwrap();
        index.commit().unwrap();

        let results = index.search("Test", 10).unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_fuzzy_search() {
        let mut index = TantivySearchIndex::new_in_memory().unwrap();

        // Index page with "algorithm"
        let page = create_test_page("My Algorithm Notes");
        index.index_page(&page).unwrap();
        index.commit().unwrap();

        // Search with typo "algoritm"
        let results = index.fuzzy_search("algoritm", 10, 2).unwrap();
        assert!(results.len() > 0);
    }
}
```

## Rollout Plan

### Phase 1: Foundation ✅
- [ ] Add Tantivy dependency
- [ ] Define search schema
- [ ] Implement TantivySearchIndex
- [ ] Create SearchResult types

### Phase 2: Service Integration ✅
- [ ] Create SearchService
- [ ] Integrate with ImportService
- [ ] Integrate with SyncService
- [ ] Add incremental indexing

### Phase 3: Tauri Integration ✅
- [ ] Create search DTOs
- [ ] Implement search commands
- [ ] Add frontend TypeScript types
- [ ] Build React hooks

### Phase 4: Advanced Features 🚀
- [ ] Add highlighting (show matched snippets)
- [ ] Implement search suggestions (autocomplete)
- [ ] Add search history
- [ ] Add search analytics

### Phase 5: Testing & Optimization ✅
- [ ] Unit tests for search index
- [ ] Integration tests for search service
- [ ] Performance benchmarks
- [ ] Optimize commit strategy

## Open Questions

1. **Highlighting:** Should we return matched snippets with highlighting?
2. **Autocomplete:** Should we implement search-as-you-type suggestions?
3. **Ranking tuning:** Should we allow users to customize ranking weights?
4. **Index size limits:** Should we warn users if index grows too large?
5. **Reindexing:** How to handle schema changes (require full reindex)?

## Future Enhancements

- **Snippet highlighting:** Return matched text with HTML highlighting
- **Query suggestions:** "Did you mean..." for misspelled queries
- **Related pages:** "Pages similar to this one" using TF-IDF similarity
- **Search analytics:** Track popular searches, no-result queries
- **Advanced syntax:** Support boolean operators (AND, OR, NOT, quotes)
- **Faceted search:** "Filter by tag", "Filter by date range"
- **Graph search:** "Find pages that link to this page"

## References

- Tantivy documentation: https://docs.rs/tantivy/
- BM25 algorithm: https://en.wikipedia.org/wiki/Okapi_BM25
- Fuzzy search: https://en.wikipedia.org/wiki/Levenshtein_distance
- Search UX patterns: https://www.nngroup.com/articles/search-interface/
