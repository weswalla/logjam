# Workflow 3: Full-Text Search

**User Action:** Type "algorithm" in search box

## Flow Diagram

```
┌──────────────────────────────────────────────────────────────────┐
│                        FRONTEND                                   │
│  <input onChange={query => search(query)} />                     │
│  User types: "algorithm"                                         │
└───────────────────────────┬──────────────────────────────────────┘
                            │ TauriApi.search({ query: "algorithm", ... })
                            ↓
┌──────────────────────────────────────────────────────────────────┐
│                    TAURI COMMAND                                  │
│  search(state, request) → SearchResultDto[]                      │
└───────────────────────────┬──────────────────────────────────────┘
                            │
                            ↓
┌──────────────────────────────────────────────────────────────────┐
│                  APPLICATION - SEARCH SERVICE                     │
│  SearchService::search(query, limit)                             │
└───────────────────────────┬──────────────────────────────────────┘
                            │
                            ↓
┌──────────────────────────────────────────────────────────────────┐
│              INFRASTRUCTURE - TANTIVY INDEX                       │
│  TantivySearchIndex::search("algorithm", 20)                     │
│                                                                   │
│  1. Parse query into Tantivy Query object                        │
│     ├─ QueryParser for fields: [page_title, block_content, ...]  │
│     └─ Parse "algorithm" into terms                              │
│                                                                   │
│  2. Execute search with BM25 ranking                             │
│     ├─ Searcher scans inverted index                             │
│     ├─ Calculate relevance scores                                │
│     └─ Return top 20 documents                                   │
│                                                                   │
│  3. Convert Tantivy documents → SearchResult                     │
│     └─ Extract page_id, block_id, content from stored fields     │
└───────────────────────────┬──────────────────────────────────────┘
                            │ Vec<SearchResult>
                            ↓
┌──────────────────────────────────────────────────────────────────┐
│  Return to frontend:                                             │
│  [                                                               │
│    BlockResult {                                                 │
│      page_id: "data-structures",                                 │
│      block_id: "block-42",                                       │
│      block_content: "Binary search algorithm is O(log n)",       │
│      score: 8.7                                                  │
│    },                                                            │
│    PageResult {                                                  │
│      page_id: "algorithms",                                      │
│      page_title: "Algorithms & Complexity",                      │
│      score: 6.2                                                  │
│    }                                                             │
│  ]                                                               │
└──────────────────────────────────────────────────────────────────┘
```

## Tantivy Index Structure

```
┌─────────────────────────────────────────────────────────────────┐
│                      TANTIVY INDEX                               │
│                                                                  │
│  Document Type 1: PAGE DOCUMENTS                                │
│  ┌────────────────────────────────────────────────────────┐     │
│  │ page_id: "algorithms"                                  │     │
│  │ page_title: "Algorithms & Complexity"  [SEARCHABLE]    │     │
│  │ document_type: "/page"                 [FACET]         │     │
│  └────────────────────────────────────────────────────────┘     │
│                                                                  │
│  Document Type 2: BLOCK DOCUMENTS                               │
│  ┌────────────────────────────────────────────────────────┐     │
│  │ page_id: "data-structures"                             │     │
│  │ block_id: "block-42"                                   │     │
│  │ page_title: "Data Structures"                          │     │
│  │ block_content: "Binary search algorithm..."[SEARCHABLE]│     │
│  │ urls: "https://en.wikipedia.org/wiki/Binary_search"    │     │
│  │ page_references: "algorithms complexity"               │     │
│  │ document_type: "/block"                [FACET]         │     │
│  │ indent_level: 1                        [INDEXED]       │     │
│  │ url_domains: "/domain/en.wikipedia.org"[FACET]         │     │
│  └────────────────────────────────────────────────────────┘     │
│                                                                  │
│  Inverted Index (for fast term lookup):                         │
│  ┌────────────────────────────────────────────────────────┐     │
│  │ "algorithm" → [doc_1, doc_5, doc_42, ...]             │     │
│  │ "binary"    → [doc_42, doc_103, ...]                  │     │
│  │ "search"    → [doc_42, doc_55, ...]                   │     │
│  └────────────────────────────────────────────────────────┘     │
└─────────────────────────────────────────────────────────────────┘
```

## Search Query Types

```rust
// 1. BASIC SEARCH (exact terms)
search_service.search("machine learning", 20)
// → Finds documents with "machine" AND/OR "learning"

// 2. FUZZY SEARCH (typo-tolerant, Levenshtein distance ≤ 2)
search_service.fuzzy_search("algoritm", 20)
// → Matches "algorithm" (edit distance = 1)

// 3. FILTERED SEARCH (facets)
search_service.search_with_filters("rust", 20, SearchFilters {
    document_type: Some("block"),      // Only search blocks
    reference_type: Some("tag"),       // Only blocks with tags
})

// 4. SPECIALIZED SEARCHES
search_service.search_pages("rust", 20)      // Only page titles
search_service.search_blocks("rust", 20)     // Only block content
search_service.search_tags("programming", 20) // Only tagged blocks
```

## BM25 Ranking Algorithm

Tantivy uses BM25 (Best Matching 25) for relevance scoring:

```
BM25(q,d) = Σ IDF(qi) × (f(qi,d) × (k1 + 1)) / (f(qi,d) + k1 × (1 - b + b × |d|/avgdl))

Where:
- q = query terms
- d = document
- f(qi,d) = frequency of term qi in document d
- |d| = document length
- avgdl = average document length
- k1 = term frequency saturation parameter (typically 1.2)
- b = field length normalization parameter (typically 0.75)
- IDF(qi) = inverse document frequency of term qi
```

**Key Properties:**
- **Term Frequency:** More occurrences = higher score
- **Document Length:** Longer documents penalized
- **Inverse Document Frequency:** Rare terms weighted higher
- **Saturation:** Diminishing returns for high term frequency

## Index Schema Definition

```rust
// backend/src/infrastructure/search/tantivy_schema.rs

pub fn create_schema() -> Schema {
    let mut schema_builder = Schema::builder();

    // Common fields
    let page_id = schema_builder.add_text_field("page_id", STORED);
    let page_title = schema_builder.add_text_field("page_title", TEXT | STORED);
    let document_type = schema_builder.add_facet_field("document_type", INDEXED);

    // Block-specific fields
    let block_id = schema_builder.add_text_field("block_id", STORED);
    let block_content = schema_builder.add_text_field("block_content", TEXT | STORED);
    let urls = schema_builder.add_text_field("urls", TEXT);
    let page_references = schema_builder.add_text_field("page_references", TEXT);
    let indent_level = schema_builder.add_u64_field("indent_level", INDEXED);
    let url_domains = schema_builder.add_facet_field("url_domains", INDEXED);

    schema_builder.build()
}
```

## Indexing Process

```rust
// backend/src/infrastructure/search/tantivy_index.rs

impl TantivySearchIndex {
    pub fn index_page(&mut self, page: &Page) -> Result<()> {
        let mut index_writer = self.index.writer(50_000_000)?; // 50MB heap

        // 1. Index page document
        let mut page_doc = Document::new();
        page_doc.add_text(self.schema.page_id, page.id().as_str());
        page_doc.add_text(self.schema.page_title, page.title());
        page_doc.add_facet(self.schema.document_type, Facet::from("/page"));
        index_writer.add_document(page_doc)?;

        // 2. Index each block as separate document
        for block in page.all_blocks() {
            let mut block_doc = Document::new();
            
            // Basic fields
            block_doc.add_text(self.schema.page_id, page.id().as_str());
            block_doc.add_text(self.schema.block_id, block.id().as_str());
            block_doc.add_text(self.schema.page_title, page.title());
            block_doc.add_text(self.schema.block_content, block.content().as_str());
            block_doc.add_u64(self.schema.indent_level, block.indent_level().as_u64());
            block_doc.add_facet(self.schema.document_type, Facet::from("/block"));

            // URLs
            for url in block.urls() {
                block_doc.add_text(self.schema.urls, url.as_str());
                if let Some(domain) = url.domain() {
                    block_doc.add_facet(
                        self.schema.url_domains, 
                        Facet::from(&format!("/domain/{}", domain))
                    );
                }
            }

            // Page references
            for page_ref in block.page_references() {
                block_doc.add_text(self.schema.page_references, page_ref.text());
                let ref_type = if page_ref.is_tag() { "tag" } else { "link" };
                block_doc.add_facet(
                    self.schema.reference_type,
                    Facet::from(&format!("/reference/{}", ref_type))
                );
            }

            index_writer.add_document(block_doc)?;
        }

        index_writer.commit()?;
        Ok(())
    }
}
```

## Search Implementation

```rust
impl TantivySearchIndex {
    pub fn search(&self, query_str: &str, limit: usize) -> Result<Vec<SearchResult>> {
        let reader = self.index.reader()?;
        let searcher = reader.searcher();

        // 1. Parse query
        let query_parser = QueryParser::for_index(
            &self.index,
            vec![self.schema.page_title, self.schema.block_content]
        );
        let query = query_parser.parse_query(query_str)?;

        // 2. Execute search
        let top_docs = searcher.search(&query, &TopDocs::with_limit(limit))?;

        // 3. Convert results
        let mut results = Vec::new();
        for (score, doc_address) in top_docs {
            let doc = searcher.doc(doc_address)?;
            
            let page_id = doc.get_first(self.schema.page_id)
                .and_then(|v| v.as_text())
                .ok_or("Missing page_id")?;

            if let Some(block_id_value) = doc.get_first(self.schema.block_id) {
                // Block result
                let block_id = block_id_value.as_text().ok_or("Invalid block_id")?;
                let content = doc.get_first(self.schema.block_content)
                    .and_then(|v| v.as_text())
                    .unwrap_or("");

                results.push(SearchResult::Block {
                    page_id: PageId::new(page_id)?,
                    block_id: BlockId::new(block_id)?,
                    content: content.to_string(),
                    score,
                });
            } else {
                // Page result
                let title = doc.get_first(self.schema.page_title)
                    .and_then(|v| v.as_text())
                    .unwrap_or("");

                results.push(SearchResult::Page {
                    page_id: PageId::new(page_id)?,
                    title: title.to_string(),
                    score,
                });
            }
        }

        Ok(results)
    }

    pub fn fuzzy_search(&self, query_str: &str, limit: usize) -> Result<Vec<SearchResult>> {
        let reader = self.index.reader()?;
        let searcher = reader.searcher();

        // Create fuzzy query (edit distance ≤ 2)
        let terms: Vec<_> = query_str.split_whitespace()
            .map(|term| {
                let page_title_term = Term::from_field_text(self.schema.page_title, term);
                let block_content_term = Term::from_field_text(self.schema.block_content, term);
                
                BooleanQuery::new(vec![
                    (Occur::Should, Box::new(FuzzyTermQuery::new(page_title_term, 2, true))),
                    (Occur::Should, Box::new(FuzzyTermQuery::new(block_content_term, 2, true))),
                ])
            })
            .collect();

        let query = BooleanQuery::new(
            terms.into_iter()
                .map(|q| (Occur::Should, Box::new(q) as Box<dyn Query>))
                .collect()
        );

        let top_docs = searcher.search(&query, &TopDocs::with_limit(limit))?;
        // ... convert results same as regular search
    }
}
```

## Faceted Search

Facets allow filtering search results by categories:

```rust
pub fn search_with_facets(
    &self, 
    query_str: &str, 
    filters: SearchFilters,
    limit: usize
) -> Result<Vec<SearchResult>> {
    let reader = self.index.reader()?;
    let searcher = reader.searcher();

    // Build base query
    let query_parser = QueryParser::for_index(&self.index, vec![...]);
    let mut base_query = query_parser.parse_query(query_str)?;

    // Add facet filters
    let mut filter_queries = Vec::new();

    if let Some(doc_type) = filters.document_type {
        let facet = Facet::from(&format!("/{}", doc_type));
        filter_queries.push(Box::new(TermQuery::new(
            Term::from_facet(self.schema.document_type, &facet),
            IndexRecordOption::Basic,
        )) as Box<dyn Query>);
    }

    if let Some(ref_type) = filters.reference_type {
        let facet = Facet::from(&format!("/reference/{}", ref_type));
        filter_queries.push(Box::new(TermQuery::new(
            Term::from_facet(self.schema.reference_type, &facet),
            IndexRecordOption::Basic,
        )) as Box<dyn Query>);
    }

    // Combine with AND logic
    if !filter_queries.is_empty() {
        filter_queries.push(base_query);
        base_query = Box::new(BooleanQuery::new(
            filter_queries.into_iter()
                .map(|q| (Occur::Must, q))
                .collect()
        ));
    }

    let top_docs = searcher.search(&base_query, &TopDocs::with_limit(limit))?;
    // ... convert results
}
```

## Performance Characteristics

### Index Size
- **Pages:** ~100 bytes per page (title + metadata)
- **Blocks:** ~200-500 bytes per block (content + references)
- **Total:** ~1-5MB per 1000 pages (depending on content density)

### Search Speed
- **Simple queries:** 1-10ms for 10K documents
- **Complex queries:** 10-50ms for 10K documents
- **Fuzzy queries:** 50-200ms for 10K documents

### Memory Usage
- **Index reader:** ~10-50MB for 10K documents
- **Search:** ~1-10MB per concurrent search
- **Indexing:** ~50MB writer buffer (configurable)

## Integration with Other Components

### With Sync Service
```rust
// Update index when files change
async fn handle_file_updated(&self, path: PathBuf) -> SyncResult<()> {
    let page = LogseqMarkdownParser::parse_file(&path).await?;
    
    // Update database
    self.page_repository.save(page.clone())?;
    
    // Update search index
    if let Some(ref index) = self.search_index {
        index.lock().await.update_page(&page)?;
        index.lock().await.commit()?;
    }
    
    Ok(())
}
```

### With Import Service
```rust
// Index pages during bulk import
for file in files {
    let page = LogseqMarkdownParser::parse_file(&file).await?;
    
    // Save to database
    self.page_repository.save(page.clone())?;
    
    // Add to search index (batch commit later)
    if let Some(ref index) = self.search_index {
        index.lock().await.index_page(&page)?;
    }
}

// Commit all changes at once
if let Some(ref index) = self.search_index {
    index.lock().await.commit()?;
}
```

## Error Handling

### Index Corruption
- Detect corruption on startup
- Rebuild index from database if needed
- Graceful degradation (disable search temporarily)

### Query Parsing Errors
- Invalid syntax → return empty results
- Log malformed queries for debugging
- Suggest corrections for common mistakes

### Resource Exhaustion
- Limit concurrent searches
- Timeout long-running queries
- Monitor memory usage during indexing

## Future Enhancements

### Query Features
- **Phrase queries:** `"exact phrase"`
- **Field-specific:** `title:algorithm`
- **Boolean operators:** `rust AND (web OR cli)`
- **Date ranges:** `created:2023-01-01..2023-12-31`

### Performance
- **Incremental indexing:** Only reindex changed blocks
- **Parallel search:** Multi-threaded query execution
- **Caching:** Cache frequent queries
- **Compression:** Reduce index size

### Analytics
- **Query logging:** Track popular searches
- **Performance metrics:** Search latency, index size
- **Usage patterns:** Most searched terms, result click-through
