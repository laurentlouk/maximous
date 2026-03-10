use rusqlite::Connection;
use maximous::db;
use maximous::tools;

fn setup() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    db::init_db(&conn).unwrap();
    conn
}

#[test]
fn test_task_create_and_list() {
    let conn = setup();
    let result = tools::tasks::create(&serde_json::json!({"title": "Build API"}), &conn);
    assert!(result.ok);
    let id = result.data.unwrap()["id"].as_str().unwrap().to_string();
    assert!(!id.is_empty());
    let result = tools::tasks::list(&serde_json::json!({}), &conn);
    assert!(result.ok);
    let tasks = result.data.unwrap();
    assert_eq!(tasks["tasks"].as_array().unwrap().len(), 1);
}

#[test]
fn test_task_with_dependencies() {
    let conn = setup();
    let r1 = tools::tasks::create(&serde_json::json!({"title": "Step 1"}), &conn);
    let id1 = r1.data.unwrap()["id"].as_str().unwrap().to_string();
    let r2 = tools::tasks::create(&serde_json::json!({"title": "Step 2", "dependencies": [id1]}), &conn);
    assert!(r2.ok);
    let data = r2.data.unwrap();
    assert_eq!(data["dependencies"].as_array().unwrap().len(), 1);
}

#[test]
fn test_task_update_status() {
    let conn = setup();
    let r = tools::tasks::create(&serde_json::json!({"title": "Task"}), &conn);
    let id = r.data.unwrap()["id"].as_str().unwrap().to_string();
    let result = tools::tasks::update(&serde_json::json!({"id": id, "status": "running", "assigned_to": "agent-1"}), &conn);
    assert!(result.ok);
    let result = tools::tasks::list(&serde_json::json!({"status": "running"}), &conn);
    let tasks = result.data.unwrap();
    assert_eq!(tasks["tasks"].as_array().unwrap().len(), 1);
    assert_eq!(tasks["tasks"][0]["assigned_to"], "agent-1");
}

#[test]
fn test_task_ready_checks_dependencies() {
    let conn = setup();
    let r1 = tools::tasks::create(&serde_json::json!({"title": "Dep"}), &conn);
    let dep_id = r1.data.unwrap()["id"].as_str().unwrap().to_string();
    let r2 = tools::tasks::create(&serde_json::json!({"title": "Main", "dependencies": [dep_id]}), &conn);
    let main_id = r2.data.unwrap()["id"].as_str().unwrap().to_string();
    // Should fail — dep not done
    let result = tools::tasks::update(&serde_json::json!({"id": main_id, "status": "ready"}), &conn);
    assert!(!result.ok);
    // Complete dep
    tools::tasks::update(&serde_json::json!({"id": dep_id, "status": "done"}), &conn);
    // Now ready should work
    let result = tools::tasks::update(&serde_json::json!({"id": main_id, "status": "ready"}), &conn);
    assert!(result.ok);
}

#[test]
fn test_task_list_filter_by_assignee() {
    let conn = setup();
    let r1 = tools::tasks::create(&serde_json::json!({"title": "T1"}), &conn);
    let id1 = r1.data.unwrap()["id"].as_str().unwrap().to_string();
    tools::tasks::update(&serde_json::json!({"id": id1, "assigned_to": "agent-a"}), &conn);
    let r2 = tools::tasks::create(&serde_json::json!({"title": "T2"}), &conn);
    let id2 = r2.data.unwrap()["id"].as_str().unwrap().to_string();
    tools::tasks::update(&serde_json::json!({"id": id2, "assigned_to": "agent-b"}), &conn);
    let result = tools::tasks::list(&serde_json::json!({"assigned_to": "agent-a"}), &conn);
    let tasks = result.data.unwrap();
    assert_eq!(tasks["tasks"].as_array().unwrap().len(), 1);
    assert_eq!(tasks["tasks"][0]["title"], "T1");
}

#[test]
fn test_task_priority_ordering() {
    let conn = setup();
    tools::tasks::create(&serde_json::json!({"title": "Low", "priority": 3}), &conn);
    tools::tasks::create(&serde_json::json!({"title": "Critical", "priority": 0}), &conn);
    tools::tasks::create(&serde_json::json!({"title": "Normal", "priority": 2}), &conn);
    let result = tools::tasks::list(&serde_json::json!({}), &conn);
    let tasks = result.data.unwrap();
    let tasks = tasks["tasks"].as_array().unwrap();
    assert_eq!(tasks[0]["title"], "Critical");
    assert_eq!(tasks[1]["title"], "Normal");
    assert_eq!(tasks[2]["title"], "Low");
}
