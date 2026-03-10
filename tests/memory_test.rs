use rusqlite::Connection;
use maximous::db;
use maximous::tools;

fn setup() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    db::init_db(&conn).unwrap();
    conn
}

#[test]
fn test_memory_set_and_get() {
    let conn = setup();
    let result = tools::memory::set(
        &serde_json::json!({"namespace": "test", "key": "foo", "value": "{\"bar\":1}"}),
        &conn,
    );
    assert!(result.ok);
    let result = tools::memory::get(
        &serde_json::json!({"namespace": "test", "key": "foo"}),
        &conn,
    );
    assert!(result.ok);
    let data = result.data.unwrap();
    assert_eq!(data["value"], "{\"bar\":1}");
}

#[test]
fn test_memory_list_keys() {
    let conn = setup();
    tools::memory::set(&serde_json::json!({"namespace": "ns", "key": "a", "value": "1"}), &conn);
    tools::memory::set(&serde_json::json!({"namespace": "ns", "key": "b", "value": "2"}), &conn);
    let result = tools::memory::get(&serde_json::json!({"namespace": "ns"}), &conn);
    assert!(result.ok);
    let keys = result.data.unwrap();
    let keys = keys["keys"].as_array().unwrap();
    assert_eq!(keys.len(), 2);
}

#[test]
fn test_memory_search() {
    let conn = setup();
    tools::memory::set(&serde_json::json!({"namespace": "ns", "key": "k1", "value": "hello world"}), &conn);
    tools::memory::set(&serde_json::json!({"namespace": "ns", "key": "k2", "value": "goodbye"}), &conn);
    let result = tools::memory::search(&serde_json::json!({"query": "hello"}), &conn);
    assert!(result.ok);
    let matches = result.data.unwrap();
    let matches = matches["matches"].as_array().unwrap();
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0]["key"], "k1");
}

#[test]
fn test_memory_delete() {
    let conn = setup();
    tools::memory::set(&serde_json::json!({"namespace": "ns", "key": "k", "value": "v"}), &conn);
    let result = tools::memory::delete(&serde_json::json!({"namespace": "ns", "key": "k"}), &conn);
    assert!(result.ok);
    let result = tools::memory::get(&serde_json::json!({"namespace": "ns", "key": "k"}), &conn);
    assert!(result.ok);
    assert!(result.data.unwrap().get("value").unwrap().is_null());
}

#[test]
fn test_memory_ttl_lazy_expiry() {
    let conn = setup();
    conn.execute(
        "INSERT INTO memory (namespace, key, value, ttl_seconds, created_at, updated_at) VALUES ('ns', 'expired', 'v', 0, 0, 0)",
        [],
    ).unwrap();
    let result = tools::memory::get(&serde_json::json!({"namespace": "ns", "key": "expired"}), &conn);
    assert!(result.ok);
    assert!(result.data.unwrap().get("value").unwrap().is_null());
}

#[test]
fn test_memory_fts_search() {
    let conn = setup();
    tools::memory::set(
        &serde_json::json!({"namespace": "docs", "key": "rust-guide", "value": "Rust is a systems programming language focused on safety"}),
        &conn,
    );
    tools::memory::set(
        &serde_json::json!({"namespace": "docs", "key": "python-guide", "value": "Python is an interpreted high-level language"}),
        &conn,
    );
    tools::memory::set(
        &serde_json::json!({"namespace": "docs", "key": "rust-async", "value": "Async programming in Rust uses futures and tokio runtime"}),
        &conn,
    );
    let result = tools::memory::search(
        &serde_json::json!({"query": "rust programming"}),
        &conn,
    );
    assert!(result.ok);
    let data = result.data.unwrap();
    let matches = data["matches"].as_array().unwrap();
    assert!(matches.len() >= 2);
    assert!(matches[0].get("rank").is_some());
}

#[test]
fn test_memory_set_upsert() {
    let conn = setup();
    tools::memory::set(&serde_json::json!({"namespace": "ns", "key": "k", "value": "v1"}), &conn);
    tools::memory::set(&serde_json::json!({"namespace": "ns", "key": "k", "value": "v2"}), &conn);
    let result = tools::memory::get(&serde_json::json!({"namespace": "ns", "key": "k"}), &conn);
    assert_eq!(result.data.unwrap()["value"], "v2");
}
