use rusqlite::Connection;
use maximous::db;
use maximous::tools;

fn setup() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    db::init_db(&conn).unwrap();
    conn
}

#[test]
fn test_private_tags_stripped_on_read() {
    let conn = setup();
    tools::memory::set(
        &serde_json::json!({
            "namespace": "notes",
            "key": "api-keys",
            "value": "Use the API at api.example.com. <private>API_KEY=sk-secret123</private> The rate limit is 100/min."
        }),
        &conn,
    );
    let result = tools::memory::get(
        &serde_json::json!({"namespace": "notes", "key": "api-keys"}),
        &conn,
    );
    assert!(result.ok);
    let data = result.data.unwrap();
    let value = data["value"].as_str().unwrap();
    assert!(!value.contains("sk-secret123"));
    assert!(!value.contains("<private>"));
    assert!(value.contains("api.example.com"));
    assert!(value.contains("[REDACTED]"));
    assert!(value.contains("The rate limit is 100/min."));
}

#[test]
fn test_private_tags_stripped_in_search() {
    let conn = setup();
    tools::memory::set(
        &serde_json::json!({
            "namespace": "ns",
            "key": "k",
            "value": "public info <private>secret stuff</private> more public"
        }),
        &conn,
    );
    let result = tools::memory::search(
        &serde_json::json!({"query": "public"}),
        &conn,
    );
    assert!(result.ok);
    let data = result.data.unwrap();
    let value = data["matches"][0]["value"].as_str().unwrap();
    assert!(!value.contains("secret stuff"));
    assert!(value.contains("[REDACTED]"));
}

#[test]
fn test_no_private_tags_unchanged() {
    let conn = setup();
    tools::memory::set(
        &serde_json::json!({"namespace": "ns", "key": "k", "value": "normal value"}),
        &conn,
    );
    let result = tools::memory::get(
        &serde_json::json!({"namespace": "ns", "key": "k"}),
        &conn,
    );
    assert!(result.ok);
    assert_eq!(result.data.unwrap()["value"], "normal value");
}

#[test]
fn test_multiple_private_tags() {
    let conn = setup();
    tools::memory::set(
        &serde_json::json!({
            "namespace": "ns",
            "key": "k",
            "value": "a <private>x</private> b <private>y</private> c"
        }),
        &conn,
    );
    let result = tools::memory::get(
        &serde_json::json!({"namespace": "ns", "key": "k"}),
        &conn,
    );
    let value = result.data.unwrap()["value"].as_str().unwrap().to_string();
    assert_eq!(value, "a [REDACTED] b [REDACTED] c");
}

#[test]
fn test_unclosed_private_tag() {
    let conn = setup();
    tools::memory::set(
        &serde_json::json!({
            "namespace": "ns",
            "key": "k",
            "value": "start <private>secret with no end"
        }),
        &conn,
    );
    let result = tools::memory::get(
        &serde_json::json!({"namespace": "ns", "key": "k"}),
        &conn,
    );
    let value = result.data.unwrap()["value"].as_str().unwrap().to_string();
    assert_eq!(value, "start [REDACTED]");
}
