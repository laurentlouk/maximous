use rusqlite::Connection;
use serde_json::Value;
use super::ToolResult;

pub fn send(args: &Value, conn: &Connection) -> ToolResult {
    let channel = match args["channel"].as_str() {
        Some(s) => s,
        None => return ToolResult::fail("missing required field: channel"),
    };
    let sender = match args["sender"].as_str() {
        Some(s) => s,
        None => return ToolResult::fail("missing required field: sender"),
    };
    let content = match args["content"].as_str() {
        Some(s) => s,
        None => return ToolResult::fail("missing required field: content"),
    };
    let priority = args["priority"].as_i64().unwrap_or(2);

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    match conn.execute(
        "INSERT INTO messages (channel, sender, priority, content, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![channel, sender, priority, content, now],
    ) {
        Ok(_) => {
            let id = conn.last_insert_rowid();
            ToolResult::success(serde_json::json!({"id": id, "sent": true}))
        }
        Err(e) => ToolResult::fail(&format!("db error: {}", e)),
    }
}

pub fn read(args: &Value, conn: &Connection) -> ToolResult {
    let channel = match args["channel"].as_str() {
        Some(s) => s,
        None => return ToolResult::fail("missing required field: channel"),
    };
    let unacked_only = args["unacknowledged_only"].as_bool().unwrap_or(false);
    let limit = args["limit"].as_i64().unwrap_or(50);
    let offset = args["offset"].as_i64().unwrap_or(0);

    let sql = if unacked_only {
        "SELECT id, channel, sender, priority, content, acknowledged, created_at
         FROM messages WHERE channel = ?1 AND acknowledged = 0
         ORDER BY priority ASC, created_at ASC LIMIT ?2 OFFSET ?3"
    } else {
        "SELECT id, channel, sender, priority, content, acknowledged, created_at
         FROM messages WHERE channel = ?1
         ORDER BY priority ASC, created_at ASC LIMIT ?2 OFFSET ?3"
    };

    let mut stmt = match conn.prepare(sql) {
        Ok(s) => s,
        Err(e) => return ToolResult::fail(&format!("db error: {}", e)),
    };
    let messages: Vec<Value> = stmt
        .query_map(rusqlite::params![channel, limit, offset], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, i64>(0)?,
                "channel": row.get::<_, String>(1)?,
                "sender": row.get::<_, String>(2)?,
                "priority": row.get::<_, i64>(3)?,
                "content": row.get::<_, String>(4)?,
                "acknowledged": row.get::<_, bool>(5)?,
                "created_at": row.get::<_, i64>(6)?,
            }))
        })
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();

    let count = messages.len();
    ToolResult::success(serde_json::json!({"messages": messages, "count": count, "offset": offset}))
}

pub fn ack(args: &Value, conn: &Connection) -> ToolResult {
    let id = match args["id"].as_i64() {
        Some(i) => i,
        None => return ToolResult::fail("missing required field: id"),
    };

    match conn.execute(
        "UPDATE messages SET acknowledged = 1 WHERE id = ?1",
        rusqlite::params![id],
    ) {
        Ok(updated) => ToolResult::success(serde_json::json!({"acknowledged": updated > 0})),
        Err(e) => ToolResult::fail(&format!("db error: {}", e)),
    }
}
