use rusqlite::Connection;
use maximous::db;
use maximous::tools;

fn setup() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    db::init_db(&conn).unwrap();
    conn
}

fn define_agent(conn: &Connection, id: &str, name: &str, model: &str, capabilities: &[&str]) {
    let caps: Vec<String> = capabilities.iter().map(|s| s.to_string()).collect();
    tools::definitions::define(
        &serde_json::json!({
            "id": id,
            "name": name,
            "model": model,
            "capabilities": caps,
            "prompt_hint": format!("You are {}", name),
        }),
        conn,
    );
}

#[test]
fn test_create_empty_team() {
    let conn = setup();
    let result = tools::teams::create(
        &serde_json::json!({"name": "backend"}),
        &conn,
    );
    assert!(result.ok, "expected ok, got: {:?}", result.error);
    let data = result.data.unwrap();
    assert_eq!(data["team"]["name"], "backend");
    assert_eq!(data["team"]["description"], "");
    assert!(data["team"]["id"].as_str().unwrap().len() > 0);
    let members = data["team"]["members"].as_array().unwrap();
    assert_eq!(members.len(), 0);
}

#[test]
fn test_create_team_with_description() {
    let conn = setup();
    let result = tools::teams::create(
        &serde_json::json!({"name": "frontend", "description": "Frontend engineering"}),
        &conn,
    );
    assert!(result.ok);
    let data = result.data.unwrap();
    assert_eq!(data["team"]["description"], "Frontend engineering");
}

#[test]
fn test_create_team_with_members() {
    let conn = setup();
    define_agent(&conn, "agent-1", "CodeBot", "sonnet", &["coding", "review"]);
    define_agent(&conn, "agent-2", "TestBot", "haiku", &["testing"]);

    let result = tools::teams::create(
        &serde_json::json!({
            "name": "fullstack",
            "description": "Full stack team",
            "members": [
                {"agent_id": "agent-1", "role": "lead"},
                {"agent_id": "agent-2", "role": "tester"},
            ]
        }),
        &conn,
    );
    assert!(result.ok, "expected ok, got: {:?}", result.error);
    let data = result.data.unwrap();
    let members = data["team"]["members"].as_array().unwrap();
    assert_eq!(members.len(), 2);
    let roles: Vec<&str> = members.iter().map(|m| m["role"].as_str().unwrap()).collect();
    assert!(roles.contains(&"lead"));
    assert!(roles.contains(&"tester"));
}

#[test]
fn test_list_teams() {
    let conn = setup();
    tools::teams::create(&serde_json::json!({"name": "alpha"}), &conn);
    tools::teams::create(&serde_json::json!({"name": "beta"}), &conn);

    let result = tools::teams::list(&serde_json::json!({}), &conn);
    assert!(result.ok);
    let data = result.data.unwrap();
    assert_eq!(data["count"], 2);
    let teams = data["teams"].as_array().unwrap();
    assert_eq!(teams.len(), 2);
    // sorted by name
    assert_eq!(teams[0]["name"], "alpha");
    assert_eq!(teams[1]["name"], "beta");
}

#[test]
fn test_list_includes_members_with_agent_details() {
    let conn = setup();
    define_agent(&conn, "a1", "SeniorDev", "sonnet", &["coding", "architecture"]);

    tools::teams::create(
        &serde_json::json!({
            "name": "platform",
            "members": [{"agent_id": "a1", "role": "architect"}]
        }),
        &conn,
    );

    let result = tools::teams::list(&serde_json::json!({}), &conn);
    assert!(result.ok);
    let data = result.data.unwrap();
    let teams = data["teams"].as_array().unwrap();
    assert_eq!(teams.len(), 1);
    let members = teams[0]["members"].as_array().unwrap();
    assert_eq!(members.len(), 1);
    let member = &members[0];
    assert_eq!(member["agent_id"], "a1");
    assert_eq!(member["role"], "architect");
    assert_eq!(member["name"], "SeniorDev");
    assert_eq!(member["model"], "sonnet");
    // capabilities should be a JSON string
    let caps_str = member["capabilities"].as_str().unwrap();
    assert!(caps_str.contains("coding"));
}

#[test]
fn test_list_pagination() {
    let conn = setup();
    tools::teams::create(&serde_json::json!({"name": "team-a"}), &conn);
    tools::teams::create(&serde_json::json!({"name": "team-b"}), &conn);
    tools::teams::create(&serde_json::json!({"name": "team-c"}), &conn);

    let result = tools::teams::list(&serde_json::json!({"limit": 2, "offset": 0}), &conn);
    assert!(result.ok);
    let data = result.data.unwrap();
    assert_eq!(data["teams"].as_array().unwrap().len(), 2);

    let result = tools::teams::list(&serde_json::json!({"limit": 2, "offset": 2}), &conn);
    assert!(result.ok);
    let data = result.data.unwrap();
    assert_eq!(data["teams"].as_array().unwrap().len(), 1);
}

#[test]
fn test_delete_team() {
    let conn = setup();
    tools::teams::create(&serde_json::json!({"name": "to-delete"}), &conn);

    let result = tools::teams::delete(&serde_json::json!({"name": "to-delete"}), &conn);
    assert!(result.ok);
    let data = result.data.unwrap();
    assert_eq!(data["removed"], true);

    let list_result = tools::teams::list(&serde_json::json!({}), &conn);
    let list_data = list_result.data.unwrap();
    assert_eq!(list_data["count"], 0);
}

#[test]
fn test_delete_nonexistent_team() {
    let conn = setup();
    let result = tools::teams::delete(&serde_json::json!({"name": "ghost"}), &conn);
    assert!(result.ok);
    let data = result.data.unwrap();
    assert_eq!(data["removed"], false);
}

#[test]
fn test_delete_cascades_to_members() {
    let conn = setup();
    define_agent(&conn, "cascade-agent", "TempAgent", "haiku", &[]);
    tools::teams::create(
        &serde_json::json!({
            "name": "temp-team",
            "members": [{"agent_id": "cascade-agent", "role": "member"}]
        }),
        &conn,
    );

    // verify member exists
    let count_before: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM team_members tm JOIN teams t ON tm.team_id = t.id WHERE t.name = 'temp-team'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count_before, 1);

    tools::teams::delete(&serde_json::json!({"name": "temp-team"}), &conn);

    let count_after: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM team_members",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count_after, 0);
}

#[test]
fn test_duplicate_name_fails() {
    let conn = setup();
    let result1 = tools::teams::create(&serde_json::json!({"name": "unique-team"}), &conn);
    assert!(result1.ok);

    let result2 = tools::teams::create(&serde_json::json!({"name": "unique-team"}), &conn);
    assert!(!result2.ok);
    let err = result2.error.unwrap();
    assert!(err.contains("unique-team"), "error should mention team name: {}", err);
}

#[test]
fn test_create_missing_name() {
    let conn = setup();
    let result = tools::teams::create(&serde_json::json!({}), &conn);
    assert!(!result.ok);
    assert!(result.error.unwrap().contains("name"));
}

#[test]
fn test_delete_missing_name() {
    let conn = setup();
    let result = tools::teams::delete(&serde_json::json!({}), &conn);
    assert!(!result.ok);
    assert!(result.error.unwrap().contains("name"));
}

#[test]
fn test_dispatch_team_create() {
    let conn = setup();
    let result = tools::dispatch_tool("team_create", &serde_json::json!({"name": "dispatched"}), &conn);
    assert!(result.ok);
}

#[test]
fn test_dispatch_team_list() {
    let conn = setup();
    let result = tools::dispatch_tool("team_list", &serde_json::json!({}), &conn);
    assert!(result.ok);
}

#[test]
fn test_dispatch_team_delete() {
    let conn = setup();
    tools::dispatch_tool("team_create", &serde_json::json!({"name": "del-me"}), &conn);
    let result = tools::dispatch_tool("team_delete", &serde_json::json!({"name": "del-me"}), &conn);
    assert!(result.ok);
    assert_eq!(result.data.unwrap()["removed"], true);
}

#[test]
fn test_add_member_to_existing_team() {
    let conn = setup();
    define_agent(&conn, "dev-1", "DevBot", "sonnet", &["coding"]);
    tools::teams::create(&serde_json::json!({"name": "dev-team"}), &conn);

    let result = tools::teams::add_member(
        &serde_json::json!({"team_name": "dev-team", "agent_id": "dev-1", "role": "developer"}),
        &conn,
    );
    assert!(result.ok, "expected ok, got: {:?}", result.error);
    let data = result.data.unwrap();
    assert_eq!(data["added"], true);
    assert_eq!(data["team_name"], "dev-team");
    assert_eq!(data["agent_id"], "dev-1");
    assert_eq!(data["role"], "developer");
}

#[test]
fn test_add_member_team_not_found() {
    let conn = setup();
    define_agent(&conn, "dev-2", "DevBot2", "sonnet", &["coding"]);

    let result = tools::teams::add_member(
        &serde_json::json!({"team_name": "nonexistent-team", "agent_id": "dev-2"}),
        &conn,
    );
    assert!(!result.ok);
    let err = result.error.unwrap();
    assert!(err.contains("team not found"), "expected 'team not found' in: {}", err);
}

#[test]
fn test_add_member_already_exists() {
    let conn = setup();
    define_agent(&conn, "dev-3", "DevBot3", "sonnet", &["coding"]);
    tools::teams::create(&serde_json::json!({"name": "dup-team"}), &conn);

    // Add once - should succeed
    let r1 = tools::teams::add_member(
        &serde_json::json!({"team_name": "dup-team", "agent_id": "dev-3", "role": "member"}),
        &conn,
    );
    assert!(r1.ok, "first add should succeed: {:?}", r1.error);

    // Add again - should fail
    let r2 = tools::teams::add_member(
        &serde_json::json!({"team_name": "dup-team", "agent_id": "dev-3", "role": "member"}),
        &conn,
    );
    assert!(!r2.ok);
    let err = r2.error.unwrap();
    assert!(err.contains("already a member"), "expected 'already a member' in: {}", err);
}

#[test]
fn test_remove_member() {
    let conn = setup();
    define_agent(&conn, "dev-4", "DevBot4", "sonnet", &["coding"]);
    tools::teams::create(
        &serde_json::json!({
            "name": "removable-team",
            "members": [{"agent_id": "dev-4", "role": "member"}]
        }),
        &conn,
    );

    let result = tools::teams::remove_member(
        &serde_json::json!({"team_name": "removable-team", "agent_id": "dev-4"}),
        &conn,
    );
    assert!(result.ok, "expected ok, got: {:?}", result.error);
    let data = result.data.unwrap();
    assert_eq!(data["removed"], true);

    // Verify member is gone
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM team_members tm JOIN teams t ON tm.team_id = t.id WHERE t.name = 'removable-team'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count, 0);
}

#[test]
fn test_remove_member_not_found() {
    let conn = setup();
    tools::teams::create(&serde_json::json!({"name": "empty-team"}), &conn);

    let result = tools::teams::remove_member(
        &serde_json::json!({"team_name": "empty-team", "agent_id": "ghost-agent"}),
        &conn,
    );
    assert!(result.ok, "expected ok, got: {:?}", result.error);
    let data = result.data.unwrap();
    assert_eq!(data["removed"], false);
}
