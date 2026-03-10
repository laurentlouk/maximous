use rusqlite::Connection;

use maximous::db;

#[test]
fn test_init_db_creates_all_tables() {
    let conn = Connection::open_in_memory().unwrap();
    db::init_db(&conn).unwrap();

    let tables: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();

    assert!(tables.contains(&"memory".to_string()));
    assert!(tables.contains(&"messages".to_string()));
    assert!(tables.contains(&"tasks".to_string()));
    assert!(tables.contains(&"agents".to_string()));
    assert!(tables.contains(&"changes".to_string()));
    assert!(tables.contains(&"config".to_string()));
}

#[test]
fn test_wal_mode_enabled() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let conn = Connection::open(&db_path).unwrap();
    db::init_db(&conn).unwrap();

    let mode: String = conn
        .pragma_query_value(None, "journal_mode", |row| row.get(0))
        .unwrap();
    assert_eq!(mode, "wal");
}

#[test]
fn test_trigger_populates_changes_on_memory_insert() {
    let conn = Connection::open_in_memory().unwrap();
    db::init_db(&conn).unwrap();

    conn.execute(
        "INSERT INTO memory (namespace, key, value, created_at, updated_at) VALUES (?1, ?2, ?3, strftime('%s','now'), strftime('%s','now'))",
        rusqlite::params!["test-ns", "test-key", r#"{"hello":"world"}"#],
    ).unwrap();

    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM changes WHERE table_name = 'memory'", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 1);
}
