use rusqlite::Connection;
use serde_json::Value;
use super::ToolResult;

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

pub fn create(args: &Value, conn: &Connection) -> ToolResult {
    let name = match args["name"].as_str() {
        Some(s) => s,
        None => return ToolResult::fail("missing required field: name"),
    };
    let description = args["description"].as_str().unwrap_or("");
    let id = uuid::Uuid::new_v4().to_string();
    let ts = now();

    match conn.execute(
        "INSERT INTO teams (id, name, description, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?4)",
        rusqlite::params![id, name, description, ts],
    ) {
        Ok(_) => {}
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("UNIQUE") {
                return ToolResult::fail(&format!("team name already exists: {}", name));
            }
            return ToolResult::fail(&format!("db error: {}", e));
        }
    }

    let mut members_out: Vec<Value> = Vec::new();

    if let Some(members) = args["members"].as_array() {
        for member in members {
            let agent_id = match member["agent_id"].as_str() {
                Some(s) => s,
                None => return ToolResult::fail("member missing required field: agent_id"),
            };
            let role = member["role"].as_str().unwrap_or("");

            match conn.execute(
                "INSERT INTO team_members (team_id, agent_id, role) VALUES (?1, ?2, ?3)",
                rusqlite::params![id, agent_id, role],
            ) {
                Ok(_) => {}
                Err(e) => return ToolResult::fail(&format!("db error inserting member: {}", e)),
            }

            members_out.push(serde_json::json!({
                "agent_id": agent_id,
                "role": role,
            }));
        }
    }

    ToolResult::success(serde_json::json!({
        "team": {
            "id": id,
            "name": name,
            "description": description,
            "members": members_out,
        }
    }))
}

pub fn list(args: &Value, conn: &Connection) -> ToolResult {
    let limit = args["limit"].as_i64().unwrap_or(100);
    let offset = args["offset"].as_i64().unwrap_or(0);

    let mut stmt = match conn.prepare(
        "SELECT id, name, description, created_at, updated_at FROM teams ORDER BY name LIMIT ?1 OFFSET ?2"
    ) {
        Ok(s) => s,
        Err(e) => return ToolResult::fail(&format!("db error: {}", e)),
    };

    let team_rows: Vec<(String, String, String, i64, i64)> = stmt
        .query_map(rusqlite::params![limit, offset], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
            ))
        })
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();

    let mut teams: Vec<Value> = Vec::new();

    for (team_id, team_name, team_desc, created_at, updated_at) in team_rows {
        let mut member_stmt = match conn.prepare(
            "SELECT tm.agent_id, tm.role, ad.name, ad.capabilities, ad.model, ad.prompt_hint
             FROM team_members tm
             JOIN agent_definitions ad ON tm.agent_id = ad.id
             WHERE tm.team_id = ?1
             ORDER BY ad.name"
        ) {
            Ok(s) => s,
            Err(e) => return ToolResult::fail(&format!("db error: {}", e)),
        };

        let members: Vec<Value> = member_stmt
            .query_map(rusqlite::params![team_id], |row| {
                Ok(serde_json::json!({
                    "agent_id": row.get::<_, String>(0)?,
                    "role": row.get::<_, String>(1)?,
                    "name": row.get::<_, String>(2)?,
                    "capabilities": row.get::<_, String>(3)?,
                    "model": row.get::<_, String>(4)?,
                    "prompt_hint": row.get::<_, String>(5)?,
                }))
            })
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        teams.push(serde_json::json!({
            "id": team_id,
            "name": team_name,
            "description": team_desc,
            "members": members,
            "created_at": created_at,
            "updated_at": updated_at,
        }));
    }

    let count = teams.len();
    ToolResult::success(serde_json::json!({"teams": teams, "count": count}))
}

pub fn delete(args: &Value, conn: &Connection) -> ToolResult {
    let name = match args["name"].as_str() {
        Some(s) => s,
        None => return ToolResult::fail("missing required field: name"),
    };

    match conn.execute(
        "DELETE FROM teams WHERE name = ?1",
        rusqlite::params![name],
    ) {
        Ok(deleted) => ToolResult::success(serde_json::json!({"removed": deleted > 0})),
        Err(e) => ToolResult::fail(&format!("db error: {}", e)),
    }
}

pub fn add_member(args: &Value, conn: &Connection) -> ToolResult {
    let team_name = match args["team_name"].as_str() {
        Some(s) => s,
        None => return ToolResult::fail("missing required field: team_name"),
    };
    let agent_id = match args["agent_id"].as_str() {
        Some(s) => s,
        None => return ToolResult::fail("missing required field: agent_id"),
    };
    let role = args["role"].as_str().unwrap_or("");

    // Look up team by name
    let team_id: String = match conn.query_row(
        "SELECT id FROM teams WHERE name = ?1",
        rusqlite::params![team_name],
        |row| row.get(0),
    ) {
        Ok(id) => id,
        Err(_) => return ToolResult::fail(&format!("team not found: {}", team_name)),
    };

    // Verify agent exists
    let agent_exists: bool = conn.query_row(
        "SELECT COUNT(*) FROM agent_definitions WHERE id = ?1",
        rusqlite::params![agent_id],
        |row| row.get::<_, i64>(0),
    ).unwrap_or(0) > 0;

    if !agent_exists {
        return ToolResult::fail(&format!("agent not found: {}", agent_id));
    }

    match conn.execute(
        "INSERT INTO team_members (team_id, agent_id, role) VALUES (?1, ?2, ?3)",
        rusqlite::params![team_id, agent_id, role],
    ) {
        Ok(_) => ToolResult::success(serde_json::json!({
            "added": true,
            "team_name": team_name,
            "agent_id": agent_id,
            "role": role,
        })),
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("UNIQUE") {
                return ToolResult::fail(&format!("agent {} is already a member of team {}", agent_id, team_name));
            }
            ToolResult::fail(&format!("db error: {}", e))
        }
    }
}

pub fn remove_member(args: &Value, conn: &Connection) -> ToolResult {
    let team_name = match args["team_name"].as_str() {
        Some(s) => s,
        None => return ToolResult::fail("missing required field: team_name"),
    };
    let agent_id = match args["agent_id"].as_str() {
        Some(s) => s,
        None => return ToolResult::fail("missing required field: agent_id"),
    };

    // Look up team by name
    let team_id: String = match conn.query_row(
        "SELECT id FROM teams WHERE name = ?1",
        rusqlite::params![team_name],
        |row| row.get(0),
    ) {
        Ok(id) => id,
        Err(_) => return ToolResult::fail(&format!("team not found: {}", team_name)),
    };

    match conn.execute(
        "DELETE FROM team_members WHERE team_id = ?1 AND agent_id = ?2",
        rusqlite::params![team_id, agent_id],
    ) {
        Ok(deleted) => ToolResult::success(serde_json::json!({"removed": deleted > 0})),
        Err(e) => ToolResult::fail(&format!("db error: {}", e)),
    }
}
