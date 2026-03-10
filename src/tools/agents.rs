use rusqlite::Connection;
use serde_json::Value;
use super::ToolResult;

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

pub fn register(args: &Value, conn: &Connection) -> ToolResult {
    let id = match args["id"].as_str() {
        Some(s) => s,
        None => return ToolResult::fail("missing required field: id"),
    };
    let name = match args["name"].as_str() {
        Some(s) => s,
        None => return ToolResult::fail("missing required field: name"),
    };
    let capabilities = args.get("capabilities")
        .filter(|c| c.is_array())
        .map(|c| serde_json::to_string(c).unwrap());
    let metadata = args.get("metadata")
        .filter(|m| m.is_string())
        .and_then(|m| m.as_str())
        .map(|s| s.to_string());

    match conn.execute(
        "INSERT INTO agents (id, name, capabilities, metadata, last_heartbeat)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(id) DO UPDATE SET name=?2, capabilities=?3, metadata=?4, last_heartbeat=?5",
        rusqlite::params![id, name, capabilities, metadata, now()],
    ) {
        Ok(_) => ToolResult::success(serde_json::json!({"registered": true, "id": id})),
        Err(e) => ToolResult::fail(&format!("db error: {}", e)),
    }
}

pub fn heartbeat(args: &Value, conn: &Connection) -> ToolResult {
    let id = match args["id"].as_str() {
        Some(s) => s,
        None => return ToolResult::fail("missing required field: id"),
    };
    let status = args["status"].as_str();

    let result = if let Some(status) = status {
        conn.execute(
            "UPDATE agents SET last_heartbeat = ?1, status = ?2 WHERE id = ?3",
            rusqlite::params![now(), status, id],
        )
    } else {
        conn.execute(
            "UPDATE agents SET last_heartbeat = ?1 WHERE id = ?2",
            rusqlite::params![now(), id],
        )
    };

    match result {
        Ok(updated) if updated > 0 => ToolResult::success(serde_json::json!({"ok": true})),
        Ok(_) => ToolResult::fail(&format!("agent not found: {}", id)),
        Err(e) => ToolResult::fail(&format!("db error: {}", e)),
    }
}

pub fn list(args: &Value, conn: &Connection) -> ToolResult {
    let include_stale = args["include_stale"].as_bool().unwrap_or(false);
    let limit = args["limit"].as_i64().unwrap_or(100);
    let offset = args["offset"].as_i64().unwrap_or(0);

    let sql = if include_stale {
        "SELECT id, name, status, capabilities, metadata, last_heartbeat FROM agents ORDER BY name LIMIT ?1 OFFSET ?2".to_string()
    } else {
        "SELECT id, name, status, capabilities, metadata, last_heartbeat FROM agents WHERE last_heartbeat > ?1 ORDER BY name LIMIT ?2 OFFSET ?3".to_string()
    };

    let cutoff = now() - 60;

    let agents: Vec<Value> = if include_stale {
        let mut stmt = conn.prepare(&sql).unwrap();
        stmt.query_map(rusqlite::params![limit, offset], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, String>(0)?,
                "name": row.get::<_, String>(1)?,
                "status": row.get::<_, String>(2)?,
                "capabilities": row.get::<_, Option<String>>(3)?,
                "metadata": row.get::<_, Option<String>>(4)?,
                "last_heartbeat": row.get::<_, i64>(5)?,
            }))
        })
        .unwrap()
        .filter_map(|r| r.ok())
        .collect()
    } else {
        let mut stmt = conn.prepare(&sql).unwrap();
        stmt.query_map(rusqlite::params![cutoff, limit, offset], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, String>(0)?,
                "name": row.get::<_, String>(1)?,
                "status": row.get::<_, String>(2)?,
                "capabilities": row.get::<_, Option<String>>(3)?,
                "metadata": row.get::<_, Option<String>>(4)?,
                "last_heartbeat": row.get::<_, i64>(5)?,
            }))
        })
        .unwrap()
        .filter_map(|r| r.ok())
        .collect()
    };

    ToolResult::success(serde_json::json!({"agents": agents, "count": agents.len(), "limit": limit, "offset": offset}))
}
