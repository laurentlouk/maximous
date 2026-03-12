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
fn test_launch_delete() {
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
    assert!(create_result.ok);
    let launch_id = create_result.data.unwrap()["launch"]["id"].as_str().unwrap().to_string();

    let result = tools::launches::delete(
        &serde_json::json!({"id": launch_id}),
        &conn,
    );
    assert!(result.ok, "expected ok, got: {:?}", result.error);
    assert_eq!(result.data.unwrap()["removed"], true);

    // Verify it's gone
    let list_result = tools::launches::list(&serde_json::json!({}), &conn);
    assert_eq!(list_result.data.unwrap()["count"], 0);
}

#[test]
fn test_launch_delete_not_found() {
    let conn = setup();

    let result = tools::launches::delete(
        &serde_json::json!({"id": "nonexistent-id"}),
        &conn,
    );
    assert!(result.ok, "expected ok, got: {:?}", result.error);
    assert_eq!(result.data.unwrap()["removed"], false);
}

#[test]
fn test_launch_create_with_invalid_ticket_id() {
    let conn = setup();
    // Create team but no ticket — launch should fail due to FK constraint on ticket_id
    seed_agent(&conn, "dev");
    let team = tools::teams::create(&serde_json::json!({"name": "squad", "members": [{"agent_id": "dev", "role": "impl"}]}), &conn);
    let team_id = team.data.unwrap()["team"]["id"].as_str().unwrap().to_string();

    let result = tools::launches::create(
        &serde_json::json!({"ticket_id": "nonexistent", "team_id": team_id, "branch": "feat/x"}),
        &conn,
    );
    // SQLite enforces FK constraints when foreign_keys pragma is ON (set in init_db)
    // The insert should fail because "nonexistent" is not in the tickets table
    assert!(!result.ok, "expected FK violation, got: {:?}", result.data);
}

#[test]
fn test_launch_create_with_invalid_team_id() {
    let conn = setup();
    // Create ticket but no team — launch should fail due to FK constraint on team_id
    tools::tickets::cache(&serde_json::json!({"id": "t1", "source": "linear", "external_id": "L1", "title": "Test", "status": "todo"}), &conn);

    let result = tools::launches::create(
        &serde_json::json!({"ticket_id": "t1", "team_id": "nonexistent", "branch": "feat/x"}),
        &conn,
    );
    // SQLite enforces FK constraints when foreign_keys pragma is ON (set in init_db)
    // The insert should fail because "nonexistent" is not in the teams table
    assert!(!result.ok, "expected FK violation, got: {:?}", result.data);
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

#[test]
fn test_launch_wait_timeout() {
    let conn = setup();

    // No launches exist — should timeout
    let result = tools::launches::wait(
        &serde_json::json!({"timeout": 1}),
        &conn,
    );
    assert!(result.ok, "expected ok, got: {:?}", result.error);
    let data = result.data.unwrap();
    assert_eq!(data["timed_out"], true);
    assert_eq!(data["launches"].as_array().unwrap().len(), 0);
    assert!(data["cursor"].as_i64().is_some());
}

#[test]
fn test_launch_wait_finds_pending() {
    let conn = setup();
    seed_agent(&conn, "agent-1");
    let team_id = seed_team(&conn, "backend");
    seed_ticket(&conn, "ticket-1", "Fix the bug");

    // Snapshot the current change cursor before creating a launch
    let cursor: i64 = conn
        .query_row("SELECT COALESCE(MAX(id), 0) FROM changes", [], |row| row.get(0))
        .unwrap();

    // Create a pending launch (triggers a change entry)
    let create_result = tools::launches::create(
        &serde_json::json!({
            "ticket_id": "ticket-1",
            "team_id": team_id,
            "branch": "fix/bug-123",
        }),
        &conn,
    );
    assert!(create_result.ok);

    // Wait should find it immediately
    let result = tools::launches::wait(
        &serde_json::json!({"timeout": 2, "since_id": cursor}),
        &conn,
    );
    assert!(result.ok, "expected ok, got: {:?}", result.error);
    let data = result.data.unwrap();
    assert_eq!(data["timed_out"], false);
    let launches = data["launches"].as_array().unwrap();
    assert_eq!(launches.len(), 1);
    assert_eq!(launches[0]["ticket_title"], "Fix the bug");
    assert_eq!(launches[0]["team_name"], "backend");
    assert_eq!(launches[0]["status"], "pending");
    assert!(data["cursor"].as_i64().unwrap() > cursor);
}

#[test]
fn test_launch_wait_ignores_stale_when_since_id_zero() {
    let conn = setup();
    seed_agent(&conn, "agent-1");
    let team_id = seed_team(&conn, "backend");
    seed_ticket(&conn, "ticket-1", "Old launch");

    // Create a launch before calling wait — this is "stale"
    tools::launches::create(
        &serde_json::json!({
            "ticket_id": "ticket-1",
            "team_id": team_id,
            "branch": "old/branch",
        }),
        &conn,
    );

    // since_id=0 defaults to current max change ID, so it should NOT pick up the old launch
    let result = tools::launches::wait(
        &serde_json::json!({"timeout": 1, "since_id": 0}),
        &conn,
    );
    assert!(result.ok);
    let data = result.data.unwrap();
    assert_eq!(data["timed_out"], true);
    assert_eq!(data["launches"].as_array().unwrap().len(), 0);
}

#[test]
fn test_launch_wait_dispatch() {
    let conn = setup();

    // Verify launch_wait is routable via dispatch_tool
    let result = tools::dispatch_tool(
        "launch_wait",
        &serde_json::json!({"timeout": 1}),
        &conn,
    );
    assert!(result.ok);
    assert_eq!(result.data.unwrap()["timed_out"], true);
}
