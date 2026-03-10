use rusqlite::Connection;
use maximous::db;
use maximous::tools;

fn setup() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    db::init_db(&conn).unwrap();
    conn
}

#[test]
fn test_memory_search_index_returns_compact() {
    let conn = setup();
    let long_value = format!("This is a long document about programming {}", "and more content ".repeat(300));
    tools::memory::set(
        &serde_json::json!({"namespace": "docs", "key": "big-doc", "value": long_value}),
        &conn,
    );
    let result = tools::memory::search_index(
        &serde_json::json!({"query": "programming"}),
        &conn,
    );
    assert!(result.ok);
    let data = result.data.unwrap();
    let matches = data["matches"].as_array().unwrap();
    assert_eq!(matches.len(), 1);
    let snippet = matches[0]["snippet"].as_str().unwrap();
    assert!(snippet.len() <= 200); // 150 + "..."
    assert!(matches[0]["estimated_tokens"].as_i64().unwrap() > 0);
    assert!(matches[0].get("value").is_none());
    assert!(data["hint"].as_str().is_some());
}

#[test]
fn test_memory_search_index_with_type_filter() {
    let conn = setup();
    tools::memory::set(
        &serde_json::json!({"namespace": "ns", "key": "k1", "value": "some error occurred", "observation_type": "error"}),
        &conn,
    );
    tools::memory::set(
        &serde_json::json!({"namespace": "ns", "key": "k2", "value": "user prefers dark mode", "observation_type": "preference"}),
        &conn,
    );
    let result = tools::memory::search_index(
        &serde_json::json!({"query": "mode", "observation_type": "preference"}),
        &conn,
    );
    assert!(result.ok);
    let data = result.data.unwrap();
    let matches = data["matches"].as_array().unwrap();
    assert_eq!(matches.len(), 1);
}
