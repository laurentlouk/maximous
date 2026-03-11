use rusqlite::Connection;
use maximous::db;
use maximous::tools;

fn setup() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    db::init_db(&conn).unwrap();
    conn
}

fn cache_ticket(conn: &Connection, id: &str, source: &str, external_id: &str, title: &str, status: &str) {
    let result = tools::tickets::cache(
        &serde_json::json!({
            "id": id,
            "source": source,
            "external_id": external_id,
            "title": title,
            "status": status,
        }),
        conn,
    );
    assert!(result.ok, "cache_ticket failed: {:?}", result.error);
}

#[test]
fn test_ticket_cache_basic() {
    let conn = setup();
    let result = tools::tickets::cache(
        &serde_json::json!({
            "id": "t-1",
            "source": "linear",
            "external_id": "ENG-100",
            "title": "Fix login bug",
            "status": "todo",
        }),
        &conn,
    );
    assert!(result.ok);
    let data = result.data.unwrap();
    assert_eq!(data["cached"], true);
    assert_eq!(data["id"], "t-1");
}

#[test]
fn test_ticket_cache_with_optional_fields() {
    let conn = setup();
    let result = tools::tickets::cache(
        &serde_json::json!({
            "id": "t-2",
            "source": "jira",
            "external_id": "PROJ-42",
            "title": "Add dark mode",
            "status": "backlog",
            "description": "Support dark mode for all UI components",
            "priority": 1,
            "url": "https://jira.example.com/PROJ-42",
            "labels": ["ui", "ux"],
            "metadata": {"sprint": "Q1"}
        }),
        &conn,
    );
    assert!(result.ok);

    let list_result = tools::tickets::list(&serde_json::json!({}), &conn);
    assert!(list_result.ok);
    let tickets = list_result.data.unwrap();
    let t = &tickets["tickets"][0];
    assert_eq!(t["description"], "Support dark mode for all UI components");
    assert_eq!(t["priority"], 1);
    assert_eq!(t["url"], "https://jira.example.com/PROJ-42");
}

#[test]
fn test_ticket_cache_upsert() {
    let conn = setup();
    // Insert initial
    cache_ticket(&conn, "t-3", "linear", "ENG-200", "Original title", "todo");

    // Upsert with same id but updated title
    let result = tools::tickets::cache(
        &serde_json::json!({
            "id": "t-3",
            "source": "linear",
            "external_id": "ENG-200",
            "title": "Updated title",
            "status": "backlog",
        }),
        &conn,
    );
    assert!(result.ok);

    let list_result = tools::tickets::list(&serde_json::json!({}), &conn);
    let tickets = list_result.data.unwrap();
    let arr = tickets["tickets"].as_array().unwrap();
    assert_eq!(arr.len(), 1, "Should only have one ticket after upsert");
    assert_eq!(arr[0]["title"], "Updated title");
    assert_eq!(arr[0]["status"], "backlog");
}

#[test]
fn test_ticket_list_all() {
    let conn = setup();
    cache_ticket(&conn, "t-10", "linear", "ENG-10", "Task A", "todo");
    cache_ticket(&conn, "t-11", "jira", "PROJ-10", "Task B", "backlog");
    cache_ticket(&conn, "t-12", "linear", "ENG-11", "Task C", "todo");

    let result = tools::tickets::list(&serde_json::json!({}), &conn);
    assert!(result.ok);
    let data = result.data.unwrap();
    assert_eq!(data["count"], 3);
    assert_eq!(data["tickets"].as_array().unwrap().len(), 3);
}

#[test]
fn test_ticket_list_filter_by_source() {
    let conn = setup();
    cache_ticket(&conn, "t-20", "linear", "ENG-20", "Linear task 1", "todo");
    cache_ticket(&conn, "t-21", "linear", "ENG-21", "Linear task 2", "todo");
    cache_ticket(&conn, "t-22", "jira", "PROJ-20", "Jira task", "backlog");

    let result = tools::tickets::list(&serde_json::json!({"source": "linear"}), &conn);
    assert!(result.ok);
    let data = result.data.unwrap();
    assert_eq!(data["count"], 2);
    for ticket in data["tickets"].as_array().unwrap() {
        assert_eq!(ticket["source"], "linear");
    }
}

#[test]
fn test_ticket_list_filter_by_status() {
    let conn = setup();
    cache_ticket(&conn, "t-30", "linear", "ENG-30", "Todo task 1", "todo");
    cache_ticket(&conn, "t-31", "linear", "ENG-31", "Todo task 2", "todo");
    cache_ticket(&conn, "t-32", "jira", "PROJ-30", "Backlog task", "backlog");

    let result = tools::tickets::list(&serde_json::json!({"status": "todo"}), &conn);
    assert!(result.ok);
    let data = result.data.unwrap();
    assert_eq!(data["count"], 2);
    for ticket in data["tickets"].as_array().unwrap() {
        assert_eq!(ticket["status"], "todo");
    }
}

#[test]
fn test_ticket_list_filter_by_source_and_status() {
    let conn = setup();
    cache_ticket(&conn, "t-40", "linear", "ENG-40", "Linear todo", "todo");
    cache_ticket(&conn, "t-41", "linear", "ENG-41", "Linear backlog", "backlog");
    cache_ticket(&conn, "t-42", "jira", "PROJ-40", "Jira todo", "todo");

    let result = tools::tickets::list(&serde_json::json!({"source": "linear", "status": "todo"}), &conn);
    assert!(result.ok);
    let data = result.data.unwrap();
    assert_eq!(data["count"], 1);
    assert_eq!(data["tickets"][0]["id"], "t-40");
}

#[test]
fn test_ticket_clear_by_source() {
    let conn = setup();
    cache_ticket(&conn, "t-50", "linear", "ENG-50", "Linear task", "todo");
    cache_ticket(&conn, "t-51", "linear", "ENG-51", "Linear task 2", "todo");
    cache_ticket(&conn, "t-52", "jira", "PROJ-50", "Jira task", "backlog");

    let result = tools::tickets::clear(&serde_json::json!({"source": "linear"}), &conn);
    assert!(result.ok);
    let data = result.data.unwrap();
    assert_eq!(data["cleared"], 2);

    let list_result = tools::tickets::list(&serde_json::json!({}), &conn);
    let tickets = list_result.data.unwrap();
    assert_eq!(tickets["count"], 1);
    assert_eq!(tickets["tickets"][0]["source"], "jira");
}

#[test]
fn test_ticket_clear_all() {
    let conn = setup();
    cache_ticket(&conn, "t-60", "linear", "ENG-60", "Task 1", "todo");
    cache_ticket(&conn, "t-61", "jira", "PROJ-60", "Task 2", "backlog");
    cache_ticket(&conn, "t-62", "linear", "ENG-61", "Task 3", "todo");

    let result = tools::tickets::clear(&serde_json::json!({}), &conn);
    assert!(result.ok);
    let data = result.data.unwrap();
    assert_eq!(data["cleared"], 3);

    let list_result = tools::tickets::list(&serde_json::json!({}), &conn);
    let tickets = list_result.data.unwrap();
    assert_eq!(tickets["count"], 0);
}

#[test]
fn test_ticket_cache_missing_id() {
    let conn = setup();
    let result = tools::tickets::cache(
        &serde_json::json!({
            "source": "linear",
            "external_id": "ENG-999",
            "title": "No id",
            "status": "todo",
        }),
        &conn,
    );
    assert!(!result.ok);
    assert!(result.error.unwrap().contains("id"));
}

#[test]
fn test_ticket_cache_missing_source() {
    let conn = setup();
    let result = tools::tickets::cache(
        &serde_json::json!({
            "id": "t-err-1",
            "external_id": "ENG-999",
            "title": "No source",
            "status": "todo",
        }),
        &conn,
    );
    assert!(!result.ok);
    assert!(result.error.unwrap().contains("source"));
}

#[test]
fn test_ticket_cache_missing_external_id() {
    let conn = setup();
    let result = tools::tickets::cache(
        &serde_json::json!({
            "id": "t-err-2",
            "source": "linear",
            "title": "No external_id",
            "status": "todo",
        }),
        &conn,
    );
    assert!(!result.ok);
    assert!(result.error.unwrap().contains("external_id"));
}

#[test]
fn test_ticket_cache_missing_title() {
    let conn = setup();
    let result = tools::tickets::cache(
        &serde_json::json!({
            "id": "t-err-3",
            "source": "linear",
            "external_id": "ENG-999",
            "status": "todo",
        }),
        &conn,
    );
    assert!(!result.ok);
    assert!(result.error.unwrap().contains("title"));
}

#[test]
fn test_ticket_cache_missing_status() {
    let conn = setup();
    let result = tools::tickets::cache(
        &serde_json::json!({
            "id": "t-err-4",
            "source": "linear",
            "external_id": "ENG-999",
            "title": "Missing status",
        }),
        &conn,
    );
    assert!(!result.ok);
    assert!(result.error.unwrap().contains("status"));
}

#[test]
fn test_ticket_get() {
    let conn = setup();
    cache_ticket(&conn, "t-get-1", "linear", "ENG-500", "Fetch me", "in_progress");

    let result = tools::tickets::get(
        &serde_json::json!({"id": "t-get-1"}),
        &conn,
    );
    assert!(result.ok, "expected ok, got: {:?}", result.error);
    let data = result.data.unwrap();
    let ticket = &data["ticket"];
    assert_eq!(ticket["id"], "t-get-1");
    assert_eq!(ticket["source"], "linear");
    assert_eq!(ticket["external_id"], "ENG-500");
    assert_eq!(ticket["title"], "Fetch me");
    assert_eq!(ticket["status"], "in_progress");
}

#[test]
fn test_ticket_get_not_found() {
    let conn = setup();

    let result = tools::tickets::get(
        &serde_json::json!({"id": "nonexistent-ticket"}),
        &conn,
    );
    assert!(!result.ok);
    assert!(result.error.unwrap().contains("not found"));
}

#[test]
fn test_ticket_cache_with_assignee() {
    let conn = setup();
    let result = tools::tickets::cache(
        &serde_json::json!({
            "id": "t-assignee-1",
            "source": "linear",
            "external_id": "ENG-700",
            "title": "Assigned ticket",
            "status": "in_progress",
            "assignee": "alice@example.com",
        }),
        &conn,
    );
    assert!(result.ok, "cache_ticket failed: {:?}", result.error);

    let list_result = tools::tickets::list(&serde_json::json!({}), &conn);
    assert!(list_result.ok);
    let data = list_result.data.unwrap();
    let tickets = data["tickets"].as_array().unwrap();
    assert_eq!(tickets.len(), 1);
    assert_eq!(tickets[0]["assignee"], "alice@example.com");
}

#[test]
fn test_ticket_list_priority_ordering() {
    let conn = setup();
    // Insert with different priorities
    tools::tickets::cache(
        &serde_json::json!({"id": "t-ord-1", "source": "linear", "external_id": "ENG-ord-1", "title": "Low priority", "status": "todo", "priority": 3}),
        &conn,
    ).ok;
    tools::tickets::cache(
        &serde_json::json!({"id": "t-ord-2", "source": "linear", "external_id": "ENG-ord-2", "title": "High priority", "status": "todo", "priority": 1}),
        &conn,
    ).ok;
    tools::tickets::cache(
        &serde_json::json!({"id": "t-ord-3", "source": "linear", "external_id": "ENG-ord-3", "title": "Medium priority", "status": "todo", "priority": 2}),
        &conn,
    ).ok;

    let result = tools::tickets::list(&serde_json::json!({}), &conn);
    assert!(result.ok);
    let data = result.data.unwrap();
    let tickets = data["tickets"].as_array().unwrap();
    assert_eq!(tickets[0]["priority"], 1, "First should be highest priority");
    assert_eq!(tickets[1]["priority"], 2);
    assert_eq!(tickets[2]["priority"], 3);
}
