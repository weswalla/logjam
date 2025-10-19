Here's a comprehensive guide to using SQLite with Tauri for your search application:

## Setup

### 1. Install Dependencies

**Rust side** (`src-tauri/Cargo.toml`):

```toml
[dependencies]
tauri-plugin-sql = { version = "2", features = ["sqlite"] }
```

**Frontend** (JavaScript/TypeScript):

```bash
npm install @tauri-apps/plugin-sql
# or
pnpm add @tauri-apps/plugin-sql
```

### 2. Register Plugin with Migrations

**`src-tauri/src/main.rs`**: [^1][^4]

```rust
use tauri_plugin_sql::{Builder, Migration, MigrationKind};

fn main() {
    tauri::Builder::default()
        .plugin(
            Builder::default()
                .add_migrations(
                    "sqlite:search.db",
                    vec![
                        // Migration 1: Create documents table
                        Migration {
                            version: 1,
                            description: "create documents table",
                            sql: "CREATE TABLE documents (
                                id INTEGER PRIMARY KEY AUTOINCREMENT,
                                file_path TEXT NOT NULL UNIQUE,
                                content TEXT NOT NULL,
                                created_at INTEGER NOT NULL,
                                updated_at INTEGER NOT NULL
                            );",
                            kind: MigrationKind::Up,
                        },
                        // Migration 2: Create FTS5 virtual table for full-text search
                        Migration {
                            version: 2,
                            description: "create FTS5 index",
                            sql: "CREATE VIRTUAL TABLE documents_fts USING fts5(
                                content,
                                content=documents,
                                content_rowid=id
                            );",
                            kind: MigrationKind::Up,
                        },
                        // Migration 3: Create URLs table
                        Migration {
                            version: 3,
                            description: "create urls table",
                            sql: "CREATE TABLE urls (
                                id INTEGER PRIMARY KEY AUTOINCREMENT,
                                document_id INTEGER NOT NULL,
                                url TEXT NOT NULL,
                                title TEXT,
                                FOREIGN KEY (document_id) REFERENCES documents(id)
                            );
                            CREATE INDEX idx_urls_document ON urls(document_id);",
                            kind: MigrationKind::Up,
                        },
                        // Migration 4: Create embeddings table for semantic search
                        Migration {
                            version: 4,
                            description: "create embeddings table",
                            sql: "CREATE TABLE embeddings (
                                id INTEGER PRIMARY KEY AUTOINCREMENT,
                                document_id INTEGER NOT NULL,
                                chunk_text TEXT NOT NULL,
                                embedding BLOB NOT NULL,
                                FOREIGN KEY (document_id) REFERENCES documents(id)
                            );",
                            kind: MigrationKind::Up,
                        },
                    ],
                )
                .build(),
        )
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

## Frontend Usage

### Basic Database Operations

```typescript
import Database from "@tauri-apps/plugin-sql";

// Initialize database connection
const db = await Database.load("sqlite:search.db");

// Insert a document
async function indexDocument(filePath: string, content: string) {
  const now = Date.now();

  const result = await db.execute(
    "INSERT INTO documents (file_path, content, created_at, updated_at) VALUES (?, ?, ?, ?)",
    [filePath, content, now, now]
  );

  return result.lastInsertId;
}

// Update FTS index (triggers automatically with content table)
async function updateFTSIndex(documentId: number, content: string) {
  await db.execute("INSERT INTO documents_fts(rowid, content) VALUES (?, ?)", [
    documentId,
    content,
  ]);
}

// Index URLs from a document
async function indexUrls(
  documentId: number,
  urls: Array<{ url: string; title?: string }>
) {
  for (const { url, title } of urls) {
    await db.execute(
      "INSERT INTO urls (document_id, url, title) VALUES (?, ?, ?)",
      [documentId, url, title]
    );
  }
}
```

### Full-Text Search with FTS5

```typescript
// Search documents using FTS5
async function searchDocuments(query: string) {
  const results = await db.select(
    `SELECT d.id, d.file_path, d.content, 
            snippet(documents_fts, 0, '<mark>', '</mark>', '...', 32) as snippet,
            rank
     FROM documents_fts 
     JOIN documents d ON documents_fts.rowid = d.id
     WHERE documents_fts MATCH ?
     ORDER BY rank
     LIMIT 20`,
    [query]
  );

  return results;
}

// Search URLs
async function searchUrls(query: string) {
  const results = await db.select(
    `SELECT u.url, u.title, d.file_path
     FROM urls u
     JOIN documents d ON u.document_id = d.id
     WHERE u.url LIKE ? OR u.title LIKE ?`,
    [`%${query}%`, `%${query}%`]
  );

  return results;
}
```

### Semantic Search with Embeddings

```typescript
// Store embeddings (embedding should be Float32Array converted to bytes)
async function storeEmbedding(
  documentId: number,
  chunkText: string,
  embedding: Float32Array
) {
  // Convert Float32Array to bytes
  const buffer = new Uint8Array(embedding.buffer);

  await db.execute(
    "INSERT INTO embeddings (document_id, chunk_text, embedding) VALUES (?, ?, ?)",
    [documentId, chunkText, Array.from(buffer)]
  );
}

// Retrieve all embeddings for similarity calculation
async function getAllEmbeddings() {
  const results = await db.select(
    "SELECT id, document_id, chunk_text, embedding FROM embeddings"
  );

  return results.map((row) => ({
    id: row.id,
    documentId: row.document_id,
    chunkText: row.chunk_text,
    embedding: new Float32Array(new Uint8Array(row.embedding).buffer),
  }));
}

// Semantic search (cosine similarity calculated in JS)
async function semanticSearch(queryEmbedding: Float32Array, topK: number = 10) {
  const allEmbeddings = await getAllEmbeddings();

  // Calculate cosine similarity
  const similarities = allEmbeddings.map((item) => ({
    ...item,
    similarity: cosineSimilarity(queryEmbedding, item.embedding),
  }));

  // Sort by similarity and return top K
  return similarities
    .sort((a, b) => b.similarity - a.similarity)
    .slice(0, topK);
}

function cosineSimilarity(a: Float32Array, b: Float32Array): number {
  let dotProduct = 0;
  let normA = 0;
  let normB = 0;

  for (let i = 0; i < a.length; i++) {
    dotProduct += a[i] * b[i];
    normA += a[i] * a[i];
    normB += b[i] * b[i];
  }

  return dotProduct / (Math.sqrt(normA) * Math.sqrt(normB));
}
```

## Performance Optimization

For better semantic search performance with large datasets, consider: [^2]

1. **Use SQLite extensions** like `sqlite-vss` or `sqlite-vec` for native vector operations
2. **Implement HNSW indexing** in Rust for faster approximate nearest neighbor search
3. **Batch operations** when indexing multiple documents
4. **Create appropriate indexes** on frequently queried columns

```typescript
// Batch insert example
async function batchIndexDocuments(
  documents: Array<{ path: string; content: string }>
) {
  await db.execute("BEGIN TRANSACTION");

  try {
    for (const doc of documents) {
      await indexDocument(doc.path, doc.content);
    }
    await db.execute("COMMIT");
  } catch (error) {
    await db.execute("ROLLBACK");
    throw error;
  }
}
```

This setup gives you a robust foundation for implementing all three search types in your Logseq search application. [^3][^1]

[^1]: [GitHub - CodeEditorLand/TauriPluginSQL: [READ ONLY] This...](https://github.com/CodeEditorLand/TauriPluginSQL) (38%)
[^2]: [Local AI with Postgres, pgvector and llama2, inside a Tauri app](https://electric-sql.com/blog/2024/02/05/local-first-ai-with-tauri-postgres-pgvector-llama) (33%)
[^3]: [GitHub - tauri-apps/tauri-plugin-sql: [READ ONLY] This ...](https://github.com/tauri-apps/tauri-plugin-sql) (22%)
[^4]: [SQL | Tauri](https://v2.tauri.app/fr/plugin/sql/) (7%)
