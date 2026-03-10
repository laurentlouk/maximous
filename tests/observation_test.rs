use rusqlite::Connection;
use maximous::db;
use maximous::tools;

fn setup() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    db::init_db(&conn).unwrap();
    conn
}

#[test]
fn test_memory_set_with_observation_type() {
    let conn = setup();
    let result = tools::memory::set(
        &serde_json::json!({
            "namespace": "project",
            "key": "use-axum",
            "value": "Decided to use axum for the web server",
            "observation_type": "decision",
            "category": "architecture"
        }),
        &conn,
    );
    assert!(result.ok);
    let result = tools::memory::get(
        &serde_json::json!({"namespace": "project", "key": "use-axum"}),
        &conn,
    );
    assert!(result.ok);
    let data = result.data.unwrap();
    assert_eq!(data["observation_type"], "decision");
    assert_eq!(data["category"], "architecture");
}

#[test]
fn test_memory_set_without_observation_type() {
    let conn = setup();
    let result = tools::memory::set(
        &serde_json::json!({"namespace": "ns", "key": "k", "value": "plain value"}),
        &conn,
    );
    assert!(result.ok);
    let result = tools::memory::get(
        &serde_json::json!({"namespace": "ns", "key": "k"}),
        &conn,
    );
    assert!(result.ok);
    let data = result.data.unwrap();
    assert!(data["observation_type"].is_null());
}
