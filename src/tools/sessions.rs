use rusqlite::Connection;
use serde_json::Value;
use super::ToolResult;

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

pub fn start(args: &Value, conn: &Connection) -> ToolResult {
    let agent_id = args["agent_id"].as_str();
    let metadata = args["metadata"].as_str();
    let id = uuid::Uuid::new_v4().to_string();

    match conn.execute(
        "INSERT INTO sessions (id, agent_id, metadata, started_at) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![id, agent_id, metadata, now()],
    ) {
        Ok(_) => ToolResult::success(serde_json::json!({
            "id": id,
            "status": "active",
        })),
        Err(e) => ToolResult::fail(&format!("db error: {}", e)),
    }
}

pub fn end(args: &Value, conn: &Connection) -> ToolResult {
    let id = match args["id"].as_str() {
        Some(s) => s,
        None => return ToolResult::fail("missing required field: id"),
    };
    let summary = args["summary"].as_str();

    match conn.execute(
        "UPDATE sessions SET status = 'ended', summary = ?1, ended_at = ?2 WHERE id = ?3",
        rusqlite::params![summary, now(), id],
    ) {
        Ok(updated) if updated > 0 => ToolResult::success(serde_json::json!({"ended": true})),
        Ok(_) => ToolResult::fail(&format!("session not found: {}", id)),
        Err(e) => ToolResult::fail(&format!("db error: {}", e)),
    }
}

pub fn list(args: &Value, conn: &Connection) -> ToolResult {
    let agent_id = args["agent_id"].as_str();
    let status = args["status"].as_str();
    let limit = args["limit"].as_i64().unwrap_or(50);
    let offset = args["offset"].as_i64().unwrap_or(0);

    let mut conditions = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(a) = agent_id {
        conditions.push("agent_id = ?");
        params.push(Box::new(a.to_string()));
    }
    if let Some(s) = status {
        conditions.push("status = ?");
        params.push(Box::new(s.to_string()));
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    params.push(Box::new(limit));
    params.push(Box::new(offset));

    let sql = format!(
        "SELECT id, agent_id, status, metadata, summary, started_at, ended_at
         FROM sessions {} ORDER BY started_at DESC LIMIT ? OFFSET ?",
        where_clause
    );

    let params_ref: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let mut stmt = match conn.prepare(&sql) {
        Ok(s) => s,
        Err(e) => return ToolResult::fail(&format!("db error: {}", e)),
    };

    let sessions: Vec<Value> = stmt
        .query_map(params_ref.as_slice(), |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, String>(0)?,
                "agent_id": row.get::<_, Option<String>>(1)?,
                "status": row.get::<_, String>(2)?,
                "metadata": row.get::<_, Option<String>>(3)?,
                "summary": row.get::<_, Option<String>>(4)?,
                "started_at": row.get::<_, i64>(5)?,
                "ended_at": row.get::<_, Option<i64>>(6)?,
            }))
        })
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();

    let count = sessions.len();
    ToolResult::success(serde_json::json!({
        "sessions": sessions,
        "count": count,
        "limit": limit,
        "offset": offset,
    }))
}
