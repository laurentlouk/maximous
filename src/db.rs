use rusqlite::{Connection, Result};

const SCHEMA: &str = include_str!("schema.sql");

pub fn init_db(conn: &Connection) -> Result<()> {
    // Enable WAL mode for concurrent access
    conn.pragma_update(None, "journal_mode", "wal")?;

    // Reasonable defaults for multi-process access
    conn.pragma_update(None, "busy_timeout", 5000)?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;

    // Run schema
    conn.execute_batch(SCHEMA)?;

    Ok(())
}

pub fn open_db(path: &str) -> Result<Connection> {
    // Ensure parent directory exists
    if let Some(parent) = std::path::Path::new(path).parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let conn = Connection::open(path)?;
    init_db(&conn)?;
    Ok(conn)
}
