use rusqlite::Connection;
use maximous::db;
use maximous::tools;

fn setup() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    db::init_db(&conn).unwrap();
    conn
}

fn seed_agent(conn: &Connection, id: &str) {
    tools::definitions::define(
        &serde_json::json!({
            "id": id,
            "name": format!("Agent {}", id),
            "model": "sonnet",
            "capabilities": ["coding"],
            "prompt_hint": "You are a helpful agent",
        }),
        conn,
    );
}

fn seed_team(conn: &Connection, name: &str) -> String {
    let result = tools::teams::create(
        &serde_json::json!({"name": name}),
        conn,
    );
    assert!(result.ok, "team create failed: {:?}", result.error);
    result.data.unwrap()["team"]["id"].as_str().unwrap().to_string()
}

fn seed_ticket(conn: &Connection, id: &str, title: &str) {
    let result = tools::tickets::cache(
        &serde_json::json!({
            "id": id,
            "source": "linear",
            "external_id": id,
            "title": title,
            "status": "todo",
        }),
        conn,
    );
    assert!(result.ok, "ticket cache failed: {:?}", result.error);
}

#[test]
fn test_launch_create() {
    let conn = setup();
    seed_agent(&conn, "agent-1");
    let team_id = seed_team(&conn, "backend");
    seed_ticket(&conn, "ticket-1", "Fix the bug");

    let result = tools::launches::create(
        &serde_json::json!({
            "ticket_id": "ticket-1",
            "team_id": team_id,
            "branch": "fix/bug-123",
        }),
        &conn,
    );
    assert!(result.ok, "expected ok, got: {:?}", result.error);
    let data = result.data.unwrap();
    let launch = &data["launch"];
    assert!(!launch["id"].as_str().unwrap().is_empty());
    assert_eq!(launch["ticket_id"], "ticket-1");
    assert_eq!(launch["team_id"], team_id);
    assert_eq!(launch["branch"], "fix/bug-123");
    assert_eq!(launch["status"], "pending");
}

#[test]
fn test_launch_create_with_worktree_path() {
    let conn = setup();
    seed_agent(&conn, "agent-1");
    let team_id = seed_team(&conn, "backend");
    seed_ticket(&conn, "ticket-1", "Fix the bug");

    let result = tools::launches::create(
        &serde_json::json!({
            "ticket_id": "ticket-1",
            "team_id": team_id,
            "branch": "fix/bug-123",
            "worktree_path": "/tmp/worktrees/bug-123",
        }),
        &conn,
    );
    assert!(result.ok);
}

#[test]
fn test_launch_create_missing_required_fields() {
    let conn = setup();

    let result = tools::launches::create(&serde_json::json!({}), &conn);
    assert!(!result.ok);
    assert!(result.error.unwrap().contains("ticket_id"));

    let result = tools::launches::create(
        &serde_json::json!({"ticket_id": "t1"}),
        &conn,
    );
    assert!(!result.ok);
    assert!(result.error.unwrap().contains("team_id"));

    let result = tools::launches::create(
        &serde_json::json!({"ticket_id": "t1", "team_id": "tm1"}),
        &conn,
    );
    assert!(!result.ok);
    assert!(result.error.unwrap().contains("branch"));
}

#[test]
fn test_launch_update_status() {
    let conn = setup();
    seed_agent(&conn, "agent-1");
    let team_id = seed_team(&conn, "backend");
    seed_ticket(&conn, "ticket-1", "Fix the bug");

    let create_result = tools::launches::create(
        &serde_json::json!({
            "ticket_id": "ticket-1",
            "team_id": team_id,
            "branch": "fix/bug-123",
        }),
        &conn,
    );
    let launch_id = create_result.data.unwrap()["launch"]["id"].as_str().unwrap().to_string();

    let result = tools::launches::update(
        &serde_json::json!({"id": launch_id, "status": "running"}),
        &conn,
    );
    assert!(result.ok, "expected ok, got: {:?}", result.error);
    let data = result.data.unwrap();
    let launch = &data["launch"];
    assert_eq!(launch["status"], "running");
    assert_eq!(launch["id"], launch_id);
}

#[test]
fn test_launch_update_pr_url() {
    let conn = setup();
    seed_agent(&conn, "agent-1");
    let team_id = seed_team(&conn, "backend");
    seed_ticket(&conn, "ticket-1", "Fix the bug");

    let create_result = tools::launches::create(
        &serde_json::json!({
            "ticket_id": "ticket-1",
            "team_id": team_id,
            "branch": "fix/bug-123",
        }),
        &conn,
    );
    let launch_id = create_result.data.unwrap()["launch"]["id"].as_str().unwrap().to_string();

    let result = tools::launches::update(
        &serde_json::json!({
            "id": launch_id,
            "status": "pr_created",
            "pr_url": "https://github.com/org/repo/pull/42",
        }),
        &conn,
    );
    assert!(result.ok, "expected ok, got: {:?}", result.error);
    let data = result.data.unwrap();
    let launch = &data["launch"];
    assert_eq!(launch["status"], "pr_created");
    assert_eq!(launch["pr_url"], "https://github.com/org/repo/pull/42");
}

#[test]
fn test_launch_update_not_found() {
    let conn = setup();

    let result = tools::launches::update(
        &serde_json::json!({"id": "nonexistent-id", "status": "running"}),
        &conn,
    );
    assert!(!result.ok);
    assert!(result.error.unwrap().contains("not found"));
}

#[test]
fn test_launch_update_returns_full_record() {
    let conn = setup();
    seed_agent(&conn, "agent-1");
    let team_id = seed_team(&conn, "backend");
    seed_ticket(&conn, "ticket-1", "Fix the bug");

    let create_result = tools::launches::create(
        &serde_json::json!({
            "ticket_id": "ticket-1",
            "team_id": team_id,
            "branch": "fix/bug-123",
            "worktree_path": "/tmp/worktrees/bug-123",
        }),
        &conn,
    );
    let launch_id = create_result.data.unwrap()["launch"]["id"].as_str().unwrap().to_string();

    let result = tools::launches::update(
        &serde_json::json!({"id": launch_id, "status": "completed"}),
        &conn,
    );
    assert!(result.ok);
    let data = result.data.unwrap();
    let launch = &data["launch"];
    // Full record should include all fields
    assert!(launch.get("id").is_some());
    assert!(launch.get("ticket_id").is_some());
    assert!(launch.get("team_id").is_some());
    assert!(launch.get("branch").is_some());
    assert!(launch.get("worktree_path").is_some());
    assert!(launch.get("status").is_some());
    assert!(launch.get("pr_url").is_some());
    assert!(launch.get("error").is_some());
    assert!(launch.get("created_at").is_some());
    assert!(launch.get("updated_at").is_some());
    assert_eq!(launch["worktree_path"], "/tmp/worktrees/bug-123");
}

#[test]
fn test_launch_list_all() {
    let conn = setup();
    seed_agent(&conn, "agent-1");
    let team_id = seed_team(&conn, "backend");
    seed_ticket(&conn, "ticket-1", "Fix the bug");
    seed_ticket(&conn, "ticket-2", "Add feature");

    tools::launches::create(
        &serde_json::json!({"ticket_id": "ticket-1", "team_id": team_id, "branch": "fix/bug"}),
        &conn,
    );
    tools::launches::create(
        &serde_json::json!({"ticket_id": "ticket-2", "team_id": team_id, "branch": "feat/new"}),
        &conn,
    );

    let result = tools::launches::list(&serde_json::json!({}), &conn);
    assert!(result.ok, "expected ok, got: {:?}", result.error);
    let data = result.data.unwrap();
    assert_eq!(data["count"], 2);
    assert_eq!(data["launches"].as_array().unwrap().len(), 2);
}

#[test]
fn test_launch_list_filter_by_status() {
    let conn = setup();
    seed_agent(&conn, "agent-1");
    let team_id = seed_team(&conn, "backend");
    seed_ticket(&conn, "ticket-1", "Fix the bug");
    seed_ticket(&conn, "ticket-2", "Add feature");

    let r1 = tools::launches::create(
        &serde_json::json!({"ticket_id": "ticket-1", "team_id": team_id, "branch": "fix/bug"}),
        &conn,
    );
    let id1 = r1.data.unwrap()["launch"]["id"].as_str().unwrap().to_string();

    tools::launches::create(
        &serde_json::json!({"ticket_id": "ticket-2", "team_id": team_id, "branch": "feat/new"}),
        &conn,
    );

    // Update first launch to running
    tools::launches::update(
        &serde_json::json!({"id": id1, "status": "running"}),
        &conn,
    );

    let result = tools::launches::list(&serde_json::json!({"status": "running"}), &conn);
    assert!(result.ok);
    let data = result.data.unwrap();
    assert_eq!(data["count"], 1);
    let launches = data["launches"].as_array().unwrap();
    assert_eq!(launches[0]["status"], "running");

    let result = tools::launches::list(&serde_json::json!({"status": "pending"}), &conn);
    assert!(result.ok);
    let data = result.data.unwrap();
    assert_eq!(data["count"], 1);
}

#[test]
fn test_launch_list_includes_ticket_title_and_team_name() {
    let conn = setup();
    seed_agent(&conn, "agent-1");
    let team_id = seed_team(&conn, "backend");
    seed_ticket(&conn, "ticket-1", "Fix the critical bug");

    tools::launches::create(
        &serde_json::json!({"ticket_id": "ticket-1", "team_id": team_id, "branch": "fix/bug"}),
        &conn,
    );

    let result = tools::launches::list(&serde_json::json!({}), &conn);
    assert!(result.ok, "expected ok, got: {:?}", result.error);
    let data = result.data.unwrap();
    let launches = data["launches"].as_array().unwrap();
    assert_eq!(launches.len(), 1);
    let launch = &launches[0];
    assert_eq!(launch["ticket_title"], "Fix the critical bug");
    assert_eq!(launch["team_name"], "backend");
}

#[test]
fn test_launch_list_pagination() {
    let conn = setup();
    seed_agent(&conn, "agent-1");
    let team_id = seed_team(&conn, "backend");
    seed_ticket(&conn, "ticket-1", "Ticket 1");
    seed_ticket(&conn, "ticket-2", "Ticket 2");
    seed_ticket(&conn, "ticket-3", "Ticket 3");

    tools::launches::create(
        &serde_json::json!({"ticket_id": "ticket-1", "team_id": team_id, "branch": "branch-1"}),
        &conn,
    );
    tools::launches::create(
        &serde_json::json!({"ticket_id": "ticket-2", "team_id": team_id, "branch": "branch-2"}),
        &conn,
    );
    tools::launches::create(
        &serde_json::json!({"ticket_id": "ticket-3", "team_id": team_id, "branch": "branch-3"}),
        &conn,
    );

    let result = tools::launches::list(&serde_json::json!({"limit": 2, "offset": 0}), &conn);
    assert!(result.ok);
    assert_eq!(result.data.unwrap()["launches"].as_array().unwrap().len(), 2);

    let result = tools::launches::list(&serde_json::json!({"limit": 2, "offset": 2}), &conn);
    assert!(result.ok);
    assert_eq!(result.data.unwrap()["launches"].as_array().unwrap().len(), 1);
}

#[test]
fn test_dispatch_launch_tools() {
    let conn = setup();
    seed_agent(&conn, "agent-1");
    let team_id = seed_team(&conn, "backend");
    seed_ticket(&conn, "ticket-1", "Test dispatch");

    let create_result = tools::dispatch_tool(
        "launch_create",
        &serde_json::json!({
            "ticket_id": "ticket-1",
            "team_id": team_id,
            "branch": "test/dispatch",
        }),
        &conn,
    );
    assert!(create_result.ok);
    let launch_id = create_result.data.unwrap()["launch"]["id"].as_str().unwrap().to_string();

    let update_result = tools::dispatch_tool(
        "launch_update",
        &serde_json::json!({"id": launch_id, "status": "running"}),
        &conn,
    );
    assert!(update_result.ok);

    let list_result = tools::dispatch_tool("launch_list", &serde_json::json!({}), &conn);
    assert!(list_result.ok);
    assert_eq!(list_result.data.unwrap()["count"], 1);
}
