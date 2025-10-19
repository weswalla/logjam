# How to Use Qdrant for Semantic Search in Rust

Here's a complete guide for building semantic search with Qdrant for your Logseq notes:

## Step 1: Set Up Your Project

Add the necessary dependencies to your `Cargo.toml`:

```toml
[dependencies]
qdrant-client = "1.11"
fastembed = "3.0"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
anyhow = "1.0"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

## Step 2: Initialize Qdrant in Embedded Mode

```rust
use qdrant_client::Qdrant;
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    // Create embedded Qdrant instance (stores data locally)
    let client = Qdrant::from_url("http://localhost:6334")
        .build()?;

    // For truly embedded mode without server:
    // let client = Qdrant::from_url("file://./qdrant_storage")
    //     .build()?;

    Ok(())
}
```

## Step 3: Create a Collection

```rust
use qdrant_client::qdrant::{
    CreateCollectionBuilder, Distance, VectorParamsBuilder
};

async fn create_collection(client: &Qdrant) -> Result<()> {
    let collection_name = "logseq_notes";

    // Create collection with 384-dimensional vectors (for all-MiniLM-L6-v2)
    client.create_collection(
        CreateCollectionBuilder::new(collection_name)
            .vectors_config(VectorParamsBuilder::new(384, Distance::Cosine))
    ).await?;

    Ok(())
}
```

## Step 4: Generate Embeddings

```rust
use fastembed::{TextEmbedding, InitOptions, EmbeddingModel};

fn setup_embedding_model() -> Result<TextEmbedding> {
    let model = TextEmbedding::try_new(
        InitOptions::new(EmbeddingModel::AllMiniLML6V2)
    )?;

    Ok(model)
}

fn generate_embeddings(
    model: &TextEmbedding,
    texts: Vec<String>
) -> Result<Vec<Vec<f32>>> {
    let embeddings = model.embed(texts, None)?;
    Ok(embeddings)
}
```

## Step 5: Insert Notes into Qdrant

```rust
use qdrant_client::qdrant::{PointStruct, UpsertPointsBuilder};
use serde_json::json;

async fn insert_notes(
    client: &Qdrant,
    model: &TextEmbedding,
    notes: Vec<(String, String)> // (note_id, note_content)
) -> Result<()> {
    let collection_name = "logseq_notes";

    // Extract content for embedding
    let contents: Vec<String> = notes.iter()
        .map(|(_, content)| content.clone())
        .collect();

    // Generate embeddings
    let embeddings = generate_embeddings(model, contents)?;

    // Create points with metadata
    let points: Vec<PointStruct> = notes.iter()
        .zip(embeddings.iter())
        .enumerate()
        .map(|(idx, ((note_id, content), embedding))| {
            PointStruct::new(
                idx as u64,
                embedding.clone(),
                json!({
                    "note_id": note_id,
                    "content": content,
                    "timestamp": chrono::Utc::now().to_rfc3339()
                })
            )
        })
        .collect();

    // Upsert points into collection
    client.upsert_points(
        UpsertPointsBuilder::new(collection_name, points)
    ).await?;

    Ok(())
}
```

## Step 6: Perform Semantic Search

```rust
use qdrant_client::qdrant::{SearchPointsBuilder, SearchParamsBuilder};

async fn search_notes(
    client: &Qdrant,
    model: &TextEmbedding,
    query: &str,
    limit: u64
) -> Result<Vec<(String, f32)>> {
    let collection_name = "logseq_notes";

    // Generate embedding for query
    let query_embedding = model.embed(vec![query.to_string()], None)?
        .into_iter()
        .next()
        .unwrap();

    // Search for similar vectors
    let search_result = client.search_points(
        SearchPointsBuilder::new(collection_name, query_embedding, limit)
            .with_payload(true)
    ).await?;

    // Extract results
    let results: Vec<(String, f32)> = search_result.result
        .into_iter()
        .map(|point| {
            let content = point.payload.get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let score = point.score;
            (content, score)
        })
        .collect();

    Ok(results)
}
```

## Step 7: Complete Example

```rust
#[tokio::main]
async fn main() -> Result<()> {
    // Initialize
    let client = Qdrant::from_url("http://localhost:6334").build()?;
    let model = setup_embedding_model()?;

    // Create collection
    create_collection(&client).await?;

    // Index your Logseq notes
    let notes = vec![
        ("note1".to_string(), "Rust is a systems programming language".to_string()),
        ("note2".to_string(), "Vector databases enable semantic search".to_string()),
        ("note3".to_string(), "Logseq is a knowledge management tool".to_string()),
    ];

    insert_notes(&client, &model, notes).await?;

    // Search
    let query = "programming languages";
    let results = search_notes(&client, &model, query, 5).await?;

    println!("Search results for: '{}'", query);
    for (content, score) in results {
        println!("Score: {:.4} - {}", score, content);
    }

    Ok(())
}
```

## Key Points for Your Logseq Tool

1. **Parse markdown files**: Read your Logseq notes from disk
2. **Batch processing**: Process notes in batches for efficiency
3. **Metadata storage**: Store file paths, dates, tags in the payload
4. **Incremental updates**: Only re-embed modified notes
5. **Filtering**: Use Qdrant's filtering capabilities to search by date, tags, etc.[^1]

This setup gives you a fully local, fast semantic search engine for your notes![^2][^3]

[^1]: [GitHub - bosun-ai/qdrant-rust-client: Rust client for Qdrant vector...](https://github.com/bosun-ai/qdrant-rust-client) (51%)
[^2]: [Build Your First Semantic Search Engine in 5 Minutes](https://qdrant.tech/documentation/beginner-tutorials/search-beginners/) (25%)
[^3]: [Semantic Search 101](https://qdrant.tech/documentation/tutorials/search-beginners/) (24%)

# Qdrant in a Tauri Desktop App

Great news: **No PostgreSQL needed!** Qdrant uses its own file-based storage system, making it perfect for local desktop apps.

## How Qdrant Persistence Works

### Storage System

Qdrant uses **file-based persistence** with two storage options:[^1]

1. **OnDisk**: Stores vectors and metadata on the filesystem
2. **InMemory**: Keeps data in RAM (with optional disk backup)

For your Tauri app, you'll use **OnDisk storage**, which writes everything to a local directory on the user's machine.[^1]

### No Database Server Required

- **No PostgreSQL, SQLite, or any external database needed**
- Qdrant manages its own storage files
- Data is stored in a directory you specify (e.g., `~/.logseq-search/qdrant_data`)

## Tauri + Qdrant Architecture

```
┌─────────────────────────────────────┐
│         Tauri Frontend              │
│      (HTML/JS/TypeScript)           │
└──────────────┬──────────────────────┘
               │ IPC Commands
┌──────────────▼──────────────────────┐
│         Rust Backend                │
│  ┌──────────────────────────────┐   │
│  │  fastembed-rs                │   │
│  │  (embedding generation)      │   │
│  └──────────────────────────────┘   │
│  ┌──────────────────────────────┐   │
│  │  Qdrant (embedded)           │   │
│  │  - Vector storage            │   │
│  │  - Similarity search         │   │
│  └──────────────────────────────┘   │
└─────────────────────────────────────┘
               │
               ▼
    ┌──────────────────────┐
    │  Local File System   │
    │  ~/app_data/         │
    │  ├── qdrant_storage/ │
    │  └── embeddings/     │
    └──────────────────────┘
```

## Implementation in Tauri

### 1. Project Structure

```toml
# Cargo.toml (in src-tauri/)
[dependencies]
tauri = "1.5"
qdrant-client = "1.11"
fastembed = "3.0"
tokio = { version = "1", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

### 2. Initialize Qdrant with Local Storage

```rust
// src-tauri/src/main.rs
use qdrant_client::Qdrant;
use tauri::Manager;
use std::path::PathBuf;

struct AppState {
    qdrant: Qdrant,
    embedding_model: fastembed::TextEmbedding,
}

#[tokio::main]
async fn main() {
    tauri::Builder::default()
        .setup(|app| {
            // Get app data directory
            let app_data_dir = app.path_resolver()
                .app_data_dir()
                .expect("Failed to get app data dir");

            // Create Qdrant storage path
            let qdrant_path = app_data_dir.join("qdrant_storage");
            std::fs::create_dir_all(&qdrant_path)?;

            // Initialize embedded Qdrant
            let qdrant = Qdrant::from_url(&format!("file://{}", qdrant_path.display()))
                .build()
                .expect("Failed to initialize Qdrant");

            // Initialize embedding model
            let embedding_model = fastembed::TextEmbedding::try_new(
                fastembed::InitOptions::new(fastembed::EmbeddingModel::AllMiniLML6V2)
            ).expect("Failed to load embedding model");

            // Store in app state
            app.manage(AppState {
                qdrant,
                embedding_model,
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            index_notes,
            search_notes
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

### 3. Create Tauri Commands

```rust
use tauri::State;

#[tauri::command]
async fn index_notes(
    notes: Vec<(String, String)>, // (id, content)
    state: State<'_, AppState>
) -> Result<String, String> {
    // Generate embeddings
    let contents: Vec<String> = notes.iter()
        .map(|(_, content)| content.clone())
        .collect();

    let embeddings = state.embedding_model
        .embed(contents, None)
        .map_err(|e| e.to_string())?;

    // Insert into Qdrant
    // ... (similar to previous example)

    Ok("Indexed successfully".to_string())
}

#[tauri::command]
async fn search_notes(
    query: String,
    limit: u64,
    state: State<'_, AppState>
) -> Result<Vec<SearchResult>, String> {
    // Generate query embedding
    let query_embedding = state.embedding_model
        .embed(vec![query], None)
        .map_err(|e| e.to_string())?
        .into_iter()
        .next()
        .unwrap();

    // Search in Qdrant
    // ... (similar to previous example)

    Ok(results)
}

#[derive(serde::Serialize)]
struct SearchResult {
    content: String,
    score: f32,
    note_id: String,
}
```

### 4. Frontend Integration

```typescript
// src/App.tsx or similar
import { invoke } from "@tauri-apps/api/tauri";

async function indexNotes(notes: Array<[string, string]>) {
  await invoke("index_notes", { notes });
}

async function searchNotes(query: string, limit: number = 10) {
  const results = await invoke("search_notes", { query, limit });
  return results;
}

// Usage
const results = await searchNotes("rust programming");
console.log(results);
```

## Data Storage Location

Qdrant will store data in:

- **macOS**: `~/Library/Application Support/com.yourapp.name/qdrant_storage/`
- **Windows**: `C:\Users\{username}\AppData\Roaming\com.yourapp.name\qdrant_storage\`
- **Linux**: `~/.local/share/com.yourapp.name/qdrant_storage/`

## Key Benefits for Tauri

1. **Single binary**: Everything bundled in your app
2. **No external services**: Completely offline
3. **Fast startup**: No server to launch
4. **Simple deployment**: Just distribute the app
5. **User privacy**: All data stays on their machine
6. **Cross-platform**: Works on Windows, macOS, and Linux

## Performance Considerations

- **First launch**: Downloads embedding model (~25MB for MiniLM)
- **Indexing**: Process notes on app startup or in background
- **Search**: Near-instant for typical personal note collections
- **Storage**: ~4 bytes per dimension per vector (384 dimensions ≈ 1.5KB per note)

This setup gives you a **fully local, privacy-preserving semantic search** desktop app with no external dependencies![^2][^3]

[^1]: [Storage](https://qdrant.tech/documentation/concepts/storage/) (61%)
[^2]: [Configuration - Qdrant](https://qdrant.tech/documentation/guides/configuration/) (28%)
[^3]: [Local AI with Postgres, pgvector and llama2, inside a Tauri ...](https://electric-sql.com/blog/2024/02/05/local-first-ai-with-tauri-postgres-pgvector-llama) (11%)

# fastembed-rs + Qdrant Overview for Tauri Vector Search

Here's a comprehensive overview of how these components work together in your Tauri app:

## Architecture Overview

```
┌─────────────────────────────────────────────────────┐
│              Tauri Desktop App                      │
├─────────────────────────────────────────────────────┤
│  Frontend (HTML/JS/TS)                              │
│  └─ User Interface for search & results            │
├─────────────────────────────────────────────────────┤
│  Rust Backend                                       │
│  ┌───────────────────────────────────────────────┐ │
│  │  fastembed-rs                                 │ │
│  │  • Converts text → vectors (embeddings)      │ │
│  │  • Runs ML models locally (ONNX)             │ │
│  │  • ~25MB model download on first run         │ │
│  └───────────────────────────────────────────────┘ │
│  ┌───────────────────────────────────────────────┐ │
│  │  Qdrant (embedded mode)                       │ │
│  │  • Stores vectors on disk                    │ │
│  │  • Performs similarity search                │ │
│  │  • Manages metadata & payloads               │ │
│  └───────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────┘
                      │
                      ▼
        ┌─────────────────────────┐
        │  Local File System      │
        │  ~/AppData/yourapp/     │
        │  ├─ qdrant_storage/     │
        │  │  ├─ collections/     │
        │  │  ├─ segments/        │
        │  │  └─ metadata/        │
        │  └─ models/             │
        │     └─ fastembed/       │
        └─────────────────────────┘
```

## Component Breakdown

### **fastembed-rs: The Embedding Generator**

**Purpose**: Converts text into numerical vectors (embeddings)[^8][^5]

**Key Features**:

- **Local execution**: Runs ML models entirely on-device[^1]
- **ONNX-based**: Uses efficient ONNX runtime for fast inference[^4]
- **Multiple models**: Supports various embedding models (MiniLM, BGE, etc.)[^5]
- **No external APIs**: Zero network calls for embedding generation[^1]
- **Lightweight**: Minimal dependencies, no PyTorch/Libtorch required[^4]

**What it does**:

```
"Rust programming language"
    ↓ fastembed-rs
[0.023, -0.145, 0.891, ..., 0.234]  (384 dimensions)
```

### **Qdrant: The Vector Database**

**Purpose**: Stores and searches through vector embeddings[^9][^7]

**Key Features**:

- **Written in Rust**: High performance and memory safety[^6]
- **File-based storage**: No PostgreSQL or external DB needed[^2]
- **Embedded mode**: Runs directly in your app process
- **Fast search**: Optimized for similarity search at scale[^3]
- **Rich metadata**: Store additional data alongside vectors[^2]

**Storage Types**:[^2]

- **OnDisk**: Vectors stored on filesystem (recommended for your use case)
- **InMemory**: Vectors in RAM with optional disk backup

## How They Work Together

### **1. Indexing Flow** (Adding notes to search)

```
Logseq Note (markdown)
    ↓
Parse & extract text
    ↓
fastembed-rs: Generate embedding
    ↓
Qdrant: Store vector + metadata
    ↓
Saved to disk (persistent)
```

### **2. Search Flow** (Finding similar notes)

```
User query: "rust programming"
    ↓
fastembed-rs: Generate query embedding
    ↓
Qdrant: Find similar vectors (cosine similarity)
    ↓
Return ranked results with metadata
    ↓
Display in UI
```

## Data Persistence

### **No External Database Required**

Qdrant handles persistence internally:[^2]

- **Collections**: Organized groups of vectors
- **Segments**: Optimized storage units
- **Metadata**: JSON payloads stored alongside vectors
- **Indexes**: Efficient search structures

**Storage Location** (managed by Tauri):

- macOS: `~/Library/Application Support/com.yourapp/qdrant_storage/`
- Windows: `%APPDATA%\com.yourapp\qdrant_storage\`
- Linux: `~/.local/share/com.yourapp/qdrant_storage/`

### **What Gets Stored**:

```
qdrant_storage/
├── collection/
│   └── logseq_notes/
│       ├── segments/           # Vector data
│       ├── payload/            # Note content & metadata
│       └── config.json         # Collection settings
└── meta/                       # Database metadata
```

## Complete Integration Example

```rust
use fastembed::{TextEmbedding, InitOptions, EmbeddingModel};
use qdrant_client::Qdrant;
use tauri::State;

// Initialize both components
struct AppState {
    embedder: TextEmbedding,
    vector_db: Qdrant,
}

#[tauri::command]
async fn semantic_search(
    query: String,
    state: State<'_, AppState>
) -> Result<Vec<SearchResult>, String> {
    // 1. Generate embedding for query
    let query_vector = state.embedder
        .embed(vec![query], None)
        .map_err(|e| e.to_string())?
        .into_iter()
        .next()
        .unwrap();

    // 2. Search in Qdrant
    let results = state.vector_db
        .search_points(/* ... */)
        .await
        .map_err(|e| e.to_string())?;

    // 3. Return results
    Ok(results)
}
```

## Key Benefits for Your Use Case

### **Privacy & Offline**:

- ✅ All processing happens locally
- ✅ No data sent to external servers
- ✅ Works completely offline after initial model download

### **Performance**:

- ✅ Fast embedding generation (milliseconds)[^3]
- ✅ Near-instant search results[^3]
- ✅ Efficient disk usage

### **Simplicity**:

- ✅ No database server to manage
- ✅ No complex configuration
- ✅ Single binary deployment

### **Scalability**:

- ✅ Handles thousands of notes efficiently
- ✅ Incremental indexing (only process new/modified notes)
- ✅ Can grow with your knowledge base

## Typical Performance Metrics

For a personal knowledge base:

- **Embedding generation**: ~10-50ms per note
- **Search latency**: <100ms for 10,000 notes[^3]
- **Storage**: ~1.5KB per note (384-dim vectors)
- **Initial indexing**: ~1-5 seconds per 1,000 notes

This stack gives you a **production-ready, fully local semantic search engine** with no external dependencies beyond the initial model download![^1]

[^1]: [Local AI with Postgres, pgvector and llama2, inside a Tauri ...](https://electric-sql.com/blog/2024/02/05/local-first-ai-with-tauri-postgres-pgvector-llama) (26%)
[^2]: [Storage](https://qdrant.tech/documentation/concepts/storage/) (18%)
[^3]: [Blazing fast open-source Semantic Search-as-you-type ...](https://www.reddit.com/r/rust/comments/15qsd7m/blazing_fast_opensource_semantic_searchasyoutype/) (18%)
[^4]: [fastembed - crates.io: Rust Package Registry](https://crates.io/crates/fastembed) (13%)
[^5]: [fastembed - Rust](https://docs.rs/fastembed) (10%)
[^6]: [Best Vector Database for RAG : r/vectordatabase](https://www.reddit.com/r/vectordatabase/comments/1hzovpy/best_vector_database_for_rag/) (6%)
[^7]: [Understanding Vector Search in Qdrant - Qdrant](https://qdrant.tech/documentation/overview/vector-search/) (4%)
[^8]: [GitHub - Anush008/fastembed-rs: Rust library for generating vector ...](https://github.com/Anush008/fastembed-rs) (4%)
[^9]: [Qdrant - Vector Database - Qdrant](https://qdrant.tech/) (1%)
