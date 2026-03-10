use rusqlite::Connection;
use maximous::db;
use maximous::tools;

fn setup() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    db::init_db(&conn).unwrap();
    conn
}

#[test]
fn test_send_and_read_message() {
    let conn = setup();
    let result = tools::messages::send(
        &serde_json::json!({"channel": "general", "sender": "agent-a", "content": "{\"text\":\"hello\"}"}),
        &conn,
    );
    assert!(result.ok);
    let result = tools::messages::read(&serde_json::json!({"channel": "general"}), &conn);
    assert!(result.ok);
    let msgs = result.data.unwrap();
    let msgs = msgs["messages"].as_array().unwrap();
    assert_eq!(msgs.len(), 1);
    assert_eq!(msgs[0]["sender"], "agent-a");
}

#[test]
fn test_message_priority_ordering() {
    let conn = setup();
    tools::messages::send(&serde_json::json!({"channel": "ch", "sender": "a", "content": "low", "priority": 3}), &conn);
    tools::messages::send(&serde_json::json!({"channel": "ch", "sender": "a", "content": "critical", "priority": 0}), &conn);
    tools::messages::send(&serde_json::json!({"channel": "ch", "sender": "a", "content": "normal", "priority": 2}), &conn);
    let result = tools::messages::read(&serde_json::json!({"channel": "ch"}), &conn);
    let msgs = result.data.unwrap();
    let msgs = msgs["messages"].as_array().unwrap();
    assert_eq!(msgs[0]["content"], "critical");
    assert_eq!(msgs[1]["content"], "normal");
    assert_eq!(msgs[2]["content"], "low");
}

#[test]
fn test_message_acknowledge() {
    let conn = setup();
    tools::messages::send(&serde_json::json!({"channel": "ch", "sender": "a", "content": "msg"}), &conn);
    let result = tools::messages::read(&serde_json::json!({"channel": "ch"}), &conn);
    let msg_id = result.data.unwrap()["messages"][0]["id"].as_i64().unwrap();
    let result = tools::messages::ack(&serde_json::json!({"id": msg_id}), &conn);
    assert!(result.ok);
    let result = tools::messages::read(&serde_json::json!({"channel": "ch", "unacknowledged_only": true}), &conn);
    let msgs = result.data.unwrap();
    assert_eq!(msgs["messages"].as_array().unwrap().len(), 0);
}

#[test]
fn test_message_read_limit() {
    let conn = setup();
    for i in 0..10 {
        tools::messages::send(&serde_json::json!({"channel": "ch", "sender": "a", "content": format!("msg-{}", i)}), &conn);
    }
    let result = tools::messages::read(&serde_json::json!({"channel": "ch", "limit": 3}), &conn);
    let msgs = result.data.unwrap();
    assert_eq!(msgs["messages"].as_array().unwrap().len(), 3);
}
