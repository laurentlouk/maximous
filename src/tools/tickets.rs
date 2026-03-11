use rusqlite::Connection;
use serde_json::Value;
use super::ToolResult;

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

pub fn cache(args: &Value, conn: &Connection) -> ToolResult {
    let id = match args["id"].as_str() {
        Some(s) => s,
        None => return ToolResult::fail("missing required field: id"),
    };
    let source = match args["source"].as_str() {
        Some(s) => s,
        None => return ToolResult::fail("missing required field: source"),
    };
    let external_id = match args["external_id"].as_str() {
        Some(s) => s,
        None => return ToolResult::fail("missing required field: external_id"),
    };
    let title = match args["title"].as_str() {
        Some(s) => s,
        None => return ToolResult::fail("missing required field: title"),
    };
    let status = match args["status"].as_str() {
        Some(s) => s,
        None => return ToolResult::fail("missing required field: status"),
    };

    let description = args["description"].as_str().unwrap_or("");
    let assignee = args["assignee"].as_str().unwrap_or("");
    let priority = args["priority"].as_i64().unwrap_or(2);
    let url = args["url"].as_str().unwrap_or("");
    let labels = args.get("labels")
        .and_then(|v| if v.is_array() { Some(serde_json::to_string(v).unwrap()) } else { None })
        .unwrap_or_else(|| "[]".to_string());
    let metadata = args.get("metadata")
        .and_then(|v| if v.is_object() { Some(serde_json::to_string(v).unwrap()) } else { None })
        .unwrap_or_else(|| "{}".to_string());
    let ts = now();

    match conn.execute(
        "INSERT INTO tickets (id, source, external_id, title, description, status, assignee, priority, url, labels, metadata, fetched_at, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?12, ?12)
         ON CONFLICT(id) DO UPDATE SET
             source=?2, external_id=?3, title=?4, description=?5, status=?6, assignee=?7,
             priority=?8, url=?9, labels=?10, metadata=?11, fetched_at=?12, updated_at=?12",
        rusqlite::params![id, source, external_id, title, description, status, assignee, priority, url, labels, metadata, ts],
    ) {
        Ok(_) => ToolResult::success(serde_json::json!({"cached": true, "id": id})),
        Err(e) => ToolResult::fail(&format!("db error: {}", e)),
    }
}

pub fn get(args: &Value, conn: &Connection) -> ToolResult {
    let id = match args["id"].as_str() {
        Some(s) => s,
        None => return ToolResult::fail("missing required field: id"),
    };

    match conn.query_row(
        "SELECT id, source, external_id, title, description, status, assignee, priority, url, labels, metadata, fetched_at, created_at, updated_at
         FROM tickets WHERE id = ?1",
        rusqlite::params![id],
        |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, String>(0)?,
                "source": row.get::<_, String>(1)?,
                "external_id": row.get::<_, String>(2)?,
                "title": row.get::<_, String>(3)?,
                "description": row.get::<_, String>(4)?,
                "status": row.get::<_, String>(5)?,
                "assignee": row.get::<_, String>(6)?,
                "priority": row.get::<_, i64>(7)?,
                "url": row.get::<_, String>(8)?,
                "labels": row.get::<_, String>(9)?,
                "metadata": row.get::<_, String>(10)?,
                "fetched_at": row.get::<_, i64>(11)?,
                "created_at": row.get::<_, i64>(12)?,
                "updated_at": row.get::<_, i64>(13)?,
            }))
        },
    ) {
        Ok(ticket) => ToolResult::success(serde_json::json!({"ticket": ticket})),
        Err(rusqlite::Error::QueryReturnedNoRows) => ToolResult::fail(&format!("ticket not found: {}", id)),
        Err(e) => ToolResult::fail(&format!("db error: {}", e)),
    }
}

pub fn list(args: &Value, conn: &Connection) -> ToolResult {
    let source = args["source"].as_str();
    let status = args["status"].as_str();
    let limit = args["limit"].as_i64().unwrap_or(100);
    let offset = args["offset"].as_i64().unwrap_or(0);

    let mut conditions = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(s) = source {
        conditions.push("source = ?");
        params.push(Box::new(s.to_string()));
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
        "SELECT id, source, external_id, title, description, status, assignee, priority, url, labels, metadata, fetched_at, created_at, updated_at
         FROM tickets {} ORDER BY priority ASC, fetched_at DESC LIMIT ? OFFSET ?",
        where_clause
    );

    let params_ref: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let mut stmt = match conn.prepare(&sql) {
        Ok(s) => s,
        Err(e) => return ToolResult::fail(&format!("db error: {}", e)),
    };

    let tickets: Vec<Value> = stmt
        .query_map(params_ref.as_slice(), |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, String>(0)?,
                "source": row.get::<_, String>(1)?,
                "external_id": row.get::<_, String>(2)?,
                "title": row.get::<_, String>(3)?,
                "description": row.get::<_, String>(4)?,
                "status": row.get::<_, String>(5)?,
                "assignee": row.get::<_, String>(6)?,
                "priority": row.get::<_, i64>(7)?,
                "url": row.get::<_, String>(8)?,
                "labels": row.get::<_, String>(9)?,
                "metadata": row.get::<_, String>(10)?,
                "fetched_at": row.get::<_, i64>(11)?,
                "created_at": row.get::<_, i64>(12)?,
                "updated_at": row.get::<_, i64>(13)?,
            }))
        })
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();

    let count = tickets.len();
    ToolResult::success(serde_json::json!({"tickets": tickets, "count": count}))
}

pub fn clear(args: &Value, conn: &Connection) -> ToolResult {
    let source = args["source"].as_str();

    let result = if let Some(src) = source {
        conn.execute("DELETE FROM tickets WHERE source = ?1", rusqlite::params![src])
    } else {
        conn.execute("DELETE FROM tickets", [])
    };

    match result {
        Ok(count) => ToolResult::success(serde_json::json!({"cleared": count})),
        Err(e) => ToolResult::fail(&format!("db error: {}", e)),
    }
}
