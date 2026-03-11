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

    let has_agent_definitions: bool = conn
        .prepare("SELECT id FROM agent_definitions LIMIT 0")
        .is_ok();
    if !has_agent_definitions {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS agent_definitions (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                capabilities TEXT NOT NULL DEFAULT '[]',
                model TEXT NOT NULL DEFAULT 'sonnet',
                prompt_hint TEXT NOT NULL DEFAULT '',
                created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
                updated_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
            );
            CREATE TABLE IF NOT EXISTS teams (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                description TEXT NOT NULL DEFAULT '',
                created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
                updated_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
            );
            CREATE TABLE IF NOT EXISTS team_members (
                team_id TEXT NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
                agent_id TEXT NOT NULL REFERENCES agent_definitions(id) ON DELETE CASCADE,
                role TEXT NOT NULL DEFAULT '',
                PRIMARY KEY (team_id, agent_id)
            );
            CREATE TABLE IF NOT EXISTS tickets (
                id TEXT PRIMARY KEY,
                source TEXT NOT NULL,
                external_id TEXT NOT NULL,
                title TEXT NOT NULL,
                description TEXT NOT NULL DEFAULT '',
                status TEXT NOT NULL,
                priority INTEGER NOT NULL DEFAULT 2,
                url TEXT NOT NULL DEFAULT '',
                labels TEXT NOT NULL DEFAULT '[]',
                metadata TEXT NOT NULL DEFAULT '{}',
                fetched_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
                created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
                updated_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
                UNIQUE(source, external_id)
            );
            CREATE TABLE IF NOT EXISTS launches (
                id TEXT PRIMARY KEY,
                ticket_id TEXT NOT NULL REFERENCES tickets(id),
                team_id TEXT NOT NULL REFERENCES teams(id),
                branch TEXT NOT NULL,
                worktree_path TEXT NOT NULL DEFAULT '',
                status TEXT NOT NULL DEFAULT 'pending',
                pr_url TEXT NOT NULL DEFAULT '',
                error TEXT NOT NULL DEFAULT '',
                created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
                updated_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
            );
            CREATE INDEX IF NOT EXISTS idx_agent_definitions_name ON agent_definitions(name);
            CREATE INDEX IF NOT EXISTS idx_teams_name ON teams(name);
            CREATE INDEX IF NOT EXISTS idx_team_members_agent ON team_members(agent_id);
            CREATE INDEX IF NOT EXISTS idx_tickets_source ON tickets(source, status);
            CREATE INDEX IF NOT EXISTS idx_launches_status ON launches(status);
            CREATE INDEX IF NOT EXISTS idx_launches_ticket ON launches(ticket_id);",
        )?;
    }

    let has_assignee: bool = conn
        .prepare("SELECT assignee FROM tickets LIMIT 0")
        .is_ok();
    if !has_assignee {
        conn.execute_batch("ALTER TABLE tickets ADD COLUMN assignee TEXT NOT NULL DEFAULT '';")?;
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
