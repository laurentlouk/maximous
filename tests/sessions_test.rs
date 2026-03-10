use rusqlite::Connection;
use maximous::db;
use maximous::tools;

fn setup() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    db::init_db(&conn).unwrap();
    conn
}

#[test]
fn test_session_start_and_end() {
    let conn = setup();
    let result = tools::sessions::start(
        &serde_json::json!({"agent_id": "agent-1", "metadata": "{\"project\": \"maximous\"}"}),
        &conn,
    );
    assert!(result.ok);
    let data = result.data.unwrap();
    let session_id = data["id"].as_str().unwrap().to_string();

    let result = tools::sessions::end(
        &serde_json::json!({"id": session_id, "summary": "Implemented FTS5 search"}),
        &conn,
    );
    assert!(result.ok);
    assert_eq!(result.data.unwrap()["ended"], true);
}

#[test]
fn test_session_list() {
    let conn = setup();
    tools::sessions::start(&serde_json::json!({"agent_id": "agent-1"}), &conn);
    tools::sessions::start(&serde_json::json!({"agent_id": "agent-2"}), &conn);
    let result = tools::sessions::list(&serde_json::json!({}), &conn);
    assert!(result.ok);
    let data = result.data.unwrap();
    assert_eq!(data["sessions"].as_array().unwrap().len(), 2);
}

#[test]
fn test_session_list_filter_by_agent() {
    let conn = setup();
    tools::sessions::start(&serde_json::json!({"agent_id": "agent-1"}), &conn);
    tools::sessions::start(&serde_json::json!({"agent_id": "agent-2"}), &conn);
    let result = tools::sessions::list(&serde_json::json!({"agent_id": "agent-1"}), &conn);
    assert!(result.ok);
    let data = result.data.unwrap();
    assert_eq!(data["sessions"].as_array().unwrap().len(), 1);
}
