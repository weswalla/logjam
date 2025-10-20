use rusqlite::{Connection, Result};

/// Initialize the SQLite database with the required schema.
/// This function is idempotent and can be safely called multiple times.
pub fn initialize_database(conn: &Connection) -> Result<()> {
    // Enable foreign key constraints
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;

    // Create pages table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS pages (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_pages_title ON pages(title)",
        [],
    )?;

    // Create blocks table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS blocks (
            id TEXT PRIMARY KEY,
            page_id TEXT NOT NULL,
            parent_id TEXT,
            content TEXT NOT NULL,
            indent_level INTEGER NOT NULL,
            position INTEGER NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            FOREIGN KEY (page_id) REFERENCES pages(id) ON DELETE CASCADE,
            FOREIGN KEY (parent_id) REFERENCES blocks(id) ON DELETE CASCADE
        )",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_blocks_page ON blocks(page_id)",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_blocks_parent ON blocks(parent_id)",
        [],
    )?;

    // Create block_children junction table for maintaining child order
    conn.execute(
        "CREATE TABLE IF NOT EXISTS block_children (
            parent_id TEXT NOT NULL,
            child_id TEXT NOT NULL,
            position INTEGER NOT NULL,
            PRIMARY KEY (parent_id, child_id),
            FOREIGN KEY (parent_id) REFERENCES blocks(id) ON DELETE CASCADE,
            FOREIGN KEY (child_id) REFERENCES blocks(id) ON DELETE CASCADE
        )",
        [],
    )?;

    // Create URLs table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS urls (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            block_id TEXT NOT NULL,
            url TEXT NOT NULL,
            FOREIGN KEY (block_id) REFERENCES blocks(id) ON DELETE CASCADE
        )",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_urls_block ON urls(block_id)",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_urls_url ON urls(url)",
        [],
    )?;

    // Create page_references table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS page_references (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            block_id TEXT NOT NULL,
            title TEXT NOT NULL,
            is_tag INTEGER NOT NULL,
            FOREIGN KEY (block_id) REFERENCES blocks(id) ON DELETE CASCADE
        )",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_refs_block ON page_references(block_id)",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_refs_title ON page_references(title)",
        [],
    )?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initialize_database() {
        let conn = Connection::open_in_memory().unwrap();
        initialize_database(&conn).unwrap();

        // Verify all tables exist
        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<Vec<String>, _>>()
            .unwrap();

        assert!(tables.contains(&"pages".to_string()));
        assert!(tables.contains(&"blocks".to_string()));
        assert!(tables.contains(&"block_children".to_string()));
        assert!(tables.contains(&"urls".to_string()));
        assert!(tables.contains(&"page_references".to_string()));

        // Verify foreign keys are enabled
        let foreign_keys: i32 = conn
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .unwrap();
        assert_eq!(foreign_keys, 1);
    }

    #[test]
    fn test_initialize_database_idempotent() {
        let conn = Connection::open_in_memory().unwrap();

        // Call initialize multiple times
        initialize_database(&conn).unwrap();
        initialize_database(&conn).unwrap();
        initialize_database(&conn).unwrap();

        // Should not error and all our tables should exist
        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<Vec<String>, _>>()
            .unwrap();

        // Verify all 5 user tables exist
        assert_eq!(tables.len(), 5);
        assert!(tables.contains(&"pages".to_string()));
        assert!(tables.contains(&"blocks".to_string()));
        assert!(tables.contains(&"block_children".to_string()));
        assert!(tables.contains(&"urls".to_string()));
        assert!(tables.contains(&"page_references".to_string()));
    }
}
