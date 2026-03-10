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

    // Run migrations for existing databases
    migrate(conn)?;

    Ok(())
}

fn migrate(conn: &Connection) -> Result<()> {
    let has_obs_type: bool = conn
        .prepare("SELECT observation_type FROM memory LIMIT 0")
        .is_ok();
    if !has_obs_type {
        conn.execute_batch(
            "ALTER TABLE memory ADD COLUMN observation_type TEXT;
             ALTER TABLE memory ADD COLUMN category TEXT;",
        )?;
    }
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
