use rusqlite::Connection;
use serde_json::Value;
use super::ToolResult;

pub fn create(args: &Value, conn: &Connection) -> ToolResult {
    let title = match args["title"].as_str() {
        Some(s) => s,
        None => return ToolResult::fail("missing required field: title"),
    };
    let priority = args["priority"].as_i64().unwrap_or(2);
    let dependencies = args.get("dependencies")
        .filter(|d| d.is_array())
        .map(|d| serde_json::to_string(d).unwrap());

    let id = uuid::Uuid::new_v4().to_string();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    match conn.execute(
        "INSERT INTO tasks (id, title, priority, dependencies, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
        rusqlite::params![id, title, priority, dependencies, now],
    ) {
        Ok(_) => ToolResult::success(serde_json::json!({
            "id": id,
            "title": title,
            "status": "pending",
            "priority": priority,
            "dependencies": args.get("dependencies").unwrap_or(&serde_json::json!([])),
        })),
        Err(e) => ToolResult::fail(&format!("db error: {}", e)),
    }
}

pub fn update(args: &Value, conn: &Connection) -> ToolResult {
    let id = match args["id"].as_str() {
        Some(s) => s,
        None => return ToolResult::fail("missing required field: id"),
    };

    let exists: bool = conn.query_row(
        "SELECT COUNT(*) FROM tasks WHERE id = ?1",
        rusqlite::params![id],
        |row| row.get::<_, i64>(0).map(|c| c > 0),
    ).unwrap_or(false);

    if !exists {
        return ToolResult::fail(&format!("task not found: {}", id));
    }

    // If setting to "ready", check dependencies
    if let Some("ready") = args["status"].as_str() {
        let deps_json: Option<String> = conn.query_row(
            "SELECT dependencies FROM tasks WHERE id = ?1",
            rusqlite::params![id],
            |row| row.get(0),
        ).unwrap_or(None);

        if let Some(deps_str) = deps_json {
            if let Ok(deps) = serde_json::from_str::<Vec<String>>(&deps_str) {
                for dep_id in &deps {
                    let status: String = conn.query_row(
                        "SELECT status FROM tasks WHERE id = ?1",
                        rusqlite::params![dep_id],
                        |row| row.get(0),
                    ).unwrap_or_else(|_| "unknown".to_string());

                    if status != "done" {
                        return ToolResult::fail(&format!(
                            "dependency {} is '{}', must be 'done' before setting to ready",
                            dep_id, status
                        ));
                    }
                }
            }
        }
    }

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    let mut sets = vec!["updated_at = ?".to_string()];
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(now)];

    if let Some(status) = args["status"].as_str() {
        sets.push("status = ?".to_string());
        params.push(Box::new(status.to_string()));
    }
    if let Some(assigned) = args["assigned_to"].as_str() {
        sets.push("assigned_to = ?".to_string());
        params.push(Box::new(assigned.to_string()));
    }
    if let Some(result) = args["result"].as_str() {
        sets.push("result = ?".to_string());
        params.push(Box::new(result.to_string()));
    }

    params.push(Box::new(id.to_string()));
    let sql = format!("UPDATE tasks SET {} WHERE id = ?", sets.join(", "));
    let params_ref: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();

    match conn.execute(&sql, params_ref.as_slice()) {
        Ok(_) => ToolResult::success(serde_json::json!({"updated": true})),
        Err(e) => ToolResult::fail(&format!("db error: {}", e)),
    }
}

pub fn list(args: &Value, conn: &Connection) -> ToolResult {
    let status = args["status"].as_str();
    let assigned_to = args["assigned_to"].as_str();

    let mut conditions = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(s) = status {
        conditions.push("status = ?");
        params.push(Box::new(s.to_string()));
    }
    if let Some(a) = assigned_to {
        conditions.push("assigned_to = ?");
        params.push(Box::new(a.to_string()));
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    let limit = args["limit"].as_i64().unwrap_or(100);
    let offset = args["offset"].as_i64().unwrap_or(0);

    let sql = format!(
        "SELECT id, title, status, priority, assigned_to, dependencies, result, created_at, updated_at
         FROM tasks {} ORDER BY priority ASC, created_at ASC LIMIT ? OFFSET ?",
        where_clause
    );

    params.push(Box::new(limit));
    params.push(Box::new(offset));

    let params_ref: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let mut stmt = match conn.prepare(&sql) {
        Ok(s) => s,
        Err(e) => return ToolResult::fail(&format!("db error: {}", e)),
    };
    let tasks: Vec<Value> = stmt
        .query_map(params_ref.as_slice(), |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, String>(0)?,
                "title": row.get::<_, String>(1)?,
                "status": row.get::<_, String>(2)?,
                "priority": row.get::<_, i64>(3)?,
                "assigned_to": row.get::<_, Option<String>>(4)?,
                "dependencies": row.get::<_, Option<String>>(5)?,
                "result": row.get::<_, Option<String>>(6)?,
                "created_at": row.get::<_, i64>(7)?,
                "updated_at": row.get::<_, i64>(8)?,
            }))
        })
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();

    let count = tasks.len();
    ToolResult::success(serde_json::json!({"tasks": tasks, "count": count, "limit": limit, "offset": offset}))
}
