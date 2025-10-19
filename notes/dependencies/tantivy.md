Tantivy and Qdrant can work together to create a powerful hybrid search system that combines the strengths of both keyword-based (sparse) and semantic (dense) search. Here's how they complement each other:

## Architecture Overview

**Tantivy** handles traditional keyword search using algorithms like **BM25**, which excels at exact term matching and keyword relevance.[^1]

**Qdrant** manages semantic similarity search using dense vector embeddings, which captures meaning and context beyond exact keyword matches.[^2]

## Implementation Approaches

### 1. **Parallel Search with Result Fusion**

The most common approach:

- **Index your documents in both systems:**

  - Use Tantivy for full-text indexing with BM25 scoring
  - Use Qdrant for vector embeddings of the same documents

- **Execute searches in parallel:**

  - Query Tantivy for keyword-based results
  - Query Qdrant for semantically similar results

- **Combine and rerank results:**
  - Merge results from both systems
  - Apply a fusion algorithm (like Reciprocal Rank Fusion)
  - Optionally use a reranking model for final ordering[^4]

### 2. **Qdrant's Native Hybrid Search**

Qdrant has built-in hybrid search capabilities that can work alongside external keyword search:[^2][^3]

- Qdrant supports both dense and sparse vectors natively
- You can store BM25-style sparse vectors directly in Qdrant
- Use Qdrant's Query API to combine multiple search strategies
- This reduces the need for external result merging

### 3. **Sparse Vector Integration**

For tighter integration:

- Generate sparse vectors (BM25-style) using Tantivy's preprocessing pipeline[^1]
- Store these sparse vectors in Qdrant alongside dense embeddings
- Leverage Qdrant's native hybrid query capabilities to search both vector types simultaneously
- This approach keeps all search logic within Qdrant

## Practical Implementation for Your Logseq App

```rust
// Pseudocode structure
async fn hybrid_search(query: &str) -> Vec<SearchResult> {
    // 1. Search with Tantivy (keyword)
    let keyword_results = tantivy_index.search(query);

    // 2. Search with Qdrant (semantic)
    let embedding = generate_embedding(query);
    let semantic_results = qdrant_client.search(embedding).await;

    // 3. Combine results
    let combined = merge_and_rerank(keyword_results, semantic_results);

    combined
}
```

## Benefits of This Hybrid Approach

- **Better recall**: Semantic search finds conceptually related content even without exact keyword matches
- **Better precision**: Keyword search ensures exact term matches aren't missed
- **Robustness**: Handles both specific queries ("find note about X") and conceptual queries ("notes about productivity")

For your Logseq markdown search, this hybrid approach would be particularly effective since users might search both for specific terms (tags, names) and conceptual topics (themes, ideas).

[^1]: [BM42: New Baseline for Hybrid Search](https://qdrant.tech/articles/bm42/) (42%)
[^2]: [Hybrid Search Revamped - Building with Qdrant's Query API](https://qdrant.tech/articles/hybrid-search/) (34%)
[^3]: [Hybrid Queries - Qdrant](https://qdrant.tech/documentation/concepts/hybrid-queries/) (13%)
[^4]: [Reranking in Hybrid Search](https://qdrant.tech/documentation/advanced-tutorials/reranking-hybrid-search/) (11%)

# Tantivy Basic Rust Implementation Guide

Tantivy is a full-text search engine library inspired by Apache Lucene, written in Rust. This guide covers the fundamentals of building a search engine with Tantivy.[^1]

## Setup

Add Tantivy to your `Cargo.toml`:

```toml
[dependencies]
tantivy = "0.22"
```

## Core Concepts

Tantivy works with these key components:

- **Schema**: Defines the structure of your documents
- **Index**: Stores the searchable data
- **IndexWriter**: Adds documents to the index
- **IndexReader**: Reads from the index for searching
- **QueryParser**: Parses search queries

## Complete Implementation Example

Here's a complete working example that demonstrates indexing and searching:

```rust
use tantivy::{
    collector::TopDocs,
    query::QueryParser,
    schema::*,
    Index, IndexWriter, ReloadPolicy,
};
use tempfile::TempDir;

fn main() -> tantivy::Result<()> {
    // 1. Define the schema
    let mut schema_builder = Schema::builder();

    // Add fields to the schema
    let title = schema_builder.add_text_field("title", TEXT | STORED);
    let body = schema_builder.add_text_field("body", TEXT);
    let id = schema_builder.add_u64_field("id", INDEXED | STORED);

    let schema = schema_builder.build();

    // 2. Create the index
    let index_path = TempDir::new()?;
    let index = Index::create_in_dir(&index_path, schema.clone())?;

    // 3. Create an index writer (with 50MB buffer)
    let mut index_writer: IndexWriter = index.writer(50_000_000)?;

    // 4. Add documents to the index
    index_writer.add_document(doc!(
        title => "The Old Man and the Sea",
        body => "He was an old man who fished alone in a skiff in the Gulf Stream.",
        id => 1u64
    ))?;

    index_writer.add_document(doc!(
        title => "Of Mice and Men",
        body => "A few miles south of Soledad, the Salinas River drops close to the hillside.",
        id => 2u64
    ))?;

    index_writer.add_document(doc!(
        title => "Frankenstein",
        body => "You will rejoice to hear that no disaster has accompanied the commencement of an enterprise.",
        id => 3u64
    ))?;

    // Commit changes
    index_writer.commit()?;

    // 5. Create a reader for searching
    let reader = index
        .reader_builder()
        .reload_policy(ReloadPolicy::OnCommitWithDelay)
        .try_into()?;

    let searcher = reader.searcher();

    // 6. Parse and execute a query
    let query_parser = QueryParser::for_index(&index, vec![title, body]);
    let query = query_parser.parse_query("sea whale")?;

    // 7. Search and collect top 10 results
    let top_docs = searcher.search(&query, &TopDocs::with_limit(10))?;

    // 8. Display results
    for (score, doc_address) in top_docs {
        let retrieved_doc = searcher.doc(doc_address)?;
        println!("Score: {}", score);
        println!("Document: {:?}", retrieved_doc);
    }

    Ok(())
}
```

## Step-by-Step Breakdown

### 1. Define Your Schema

The schema defines what fields your documents contain:[^3]

```rust
let mut schema_builder = Schema::builder();

// TEXT: tokenized and indexed for full-text search
// STORED: field value is stored and can be retrieved
let title = schema_builder.add_text_field("title", TEXT | STORED);
let body = schema_builder.add_text_field("body", TEXT);

// For numeric fields
let id = schema_builder.add_u64_field("id", INDEXED | STORED);

let schema = schema_builder.build();
```

**Field options:**

- `TEXT`: Tokenizes and indexes text for searching
- `STORED`: Stores the original value for retrieval
- `INDEXED`: Makes the field searchable
- `FAST`: Enables fast field access (for sorting/filtering)

### 2. Create an Index

```rust
use tantivy::Index;

// Create in-memory index
let index = Index::create_in_ram(schema.clone());

// Or create on disk
let index = Index::create_in_dir("./index_dir", schema.clone())?;
```

### 3. Index Documents

```rust
let mut index_writer = index.writer(50_000_000)?; // 50MB buffer

// Add documents using the doc! macro
index_writer.add_document(doc!(
    title => "Document Title",
    body => "Document content goes here",
    id => 1u64
))?;

// Commit to make documents searchable
index_writer.commit()?;
```

### 4. Search Documents

```rust
// Create a reader
let reader = index.reader()?;
let searcher = reader.searcher();

// Parse query
let query_parser = QueryParser::for_index(&index, vec![title, body]);
let query = query_parser.parse_query("search terms")?;

// Execute search
let top_docs = searcher.search(&query, &TopDocs::with_limit(10))?;

// Process results
for (score, doc_address) in top_docs {
    let doc = searcher.doc(doc_address)?;
    println!("{:?}", doc);
}
```

## For Your Logseq App

Here's how to adapt this for markdown file indexing:

```rust
use tantivy::schema::*;

let mut schema_builder = Schema::builder();

// Fields for markdown documents
let file_path = schema_builder.add_text_field("file_path", STRING | STORED);
let title = schema_builder.add_text_field("title", TEXT | STORED);
let content = schema_builder.add_text_field("content", TEXT);
let tags = schema_builder.add_text_field("tags", TEXT);
let urls = schema_builder.add_text_field("urls", TEXT | STORED);
let modified_time = schema_builder.add_date_field("modified", INDEXED | STORED);

let schema = schema_builder.build();
```

## Key Points

- Tantivy compiles on stable Rust and supports Linux, macOS, and Windows[^2]
- Use `commit()` to make indexed documents searchable
- The `IndexWriter` buffer size affects memory usage and indexing performance
- `ReloadPolicy` controls when the reader sees new commits
- Tantivy supports various query types: term queries, phrase queries, boolean queries, and more

This foundation will work well with the `notify` crate for file watching and can be combined with Qdrant for hybrid search capabilities.

[^1]: [tantivy - Rust](https://docs.rs/tantivy/) (57%)
[^2]: [Tantivy is a full-text search engine library inspired ...](https://github.com/quickwit-oss/tantivy) (25%)
[^3]: [tantivy::schema - Rust - Docs.rs](https://docs.rs/tantivy/latest/tantivy/schema/) (18%)
