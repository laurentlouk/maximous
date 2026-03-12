use rusqlite::Connection;
use serde_json::Value;
use std::thread;
use std::time::{Duration, Instant};
use super::ToolResult;

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

pub fn create(args: &Value, conn: &Connection) -> ToolResult {
    let ticket_id = match args["ticket_id"].as_str() {
        Some(s) => s,
        None => return ToolResult::fail("missing required field: ticket_id"),
    };
    let team_id = match args["team_id"].as_str() {
        Some(s) => s,
        None => return ToolResult::fail("missing required field: team_id"),
    };
    let branch = match args["branch"].as_str() {
        Some(s) => s,
        None => return ToolResult::fail("missing required field: branch"),
    };
    let worktree_path = args["worktree_path"].as_str().unwrap_or("");

    let id = uuid::Uuid::new_v4().to_string();
    let ts = now();

    match conn.execute(
        "INSERT INTO launches (id, ticket_id, team_id, branch, worktree_path, status, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, 'pending', ?6, ?6)",
        rusqlite::params![id, ticket_id, team_id, branch, worktree_path, ts],
    ) {
        Ok(_) => ToolResult::success(serde_json::json!({
            "launch": {
                "id": id,
                "ticket_id": ticket_id,
                "team_id": team_id,
                "branch": branch,
                "status": "pending",
            }
        })),
        Err(e) => ToolResult::fail(&format!("db error: {}", e)),
    }
}

pub fn update(args: &Value, conn: &Connection) -> ToolResult {
    let id = match args["id"].as_str() {
        Some(s) => s,
        None => return ToolResult::fail("missing required field: id"),
    };

    let ts = now();
    let mut sets = vec!["updated_at = ?".to_string()];
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(ts)];

    if let Some(status) = args["status"].as_str() {
        sets.push("status = ?".to_string());
        params.push(Box::new(status.to_string()));
    }
    if let Some(pr_url) = args["pr_url"].as_str() {
        sets.push("pr_url = ?".to_string());
        params.push(Box::new(pr_url.to_string()));
    }
    if let Some(error) = args["error"].as_str() {
        sets.push("error = ?".to_string());
        params.push(Box::new(error.to_string()));
    }
    if let Some(worktree_path) = args["worktree_path"].as_str() {
        sets.push("worktree_path = ?".to_string());
        params.push(Box::new(worktree_path.to_string()));
    }

    params.push(Box::new(id.to_string()));
    let sql = format!("UPDATE launches SET {} WHERE id = ?", sets.join(", "));
    let params_ref: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();

    match conn.execute(&sql, params_ref.as_slice()) {
        Ok(0) => return ToolResult::fail(&format!("launch not found: {}", id)),
        Ok(_) => {}
        Err(e) => return ToolResult::fail(&format!("db error: {}", e)),
    }

    // SELECT and return the full launch record
    match conn.query_row(
        "SELECT id, ticket_id, team_id, branch, worktree_path, status, pr_url, error, created_at, updated_at
         FROM launches WHERE id = ?1",
        rusqlite::params![id],
        |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, String>(0)?,
                "ticket_id": row.get::<_, String>(1)?,
                "team_id": row.get::<_, String>(2)?,
                "branch": row.get::<_, String>(3)?,
                "worktree_path": row.get::<_, String>(4)?,
                "status": row.get::<_, String>(5)?,
                "pr_url": row.get::<_, String>(6)?,
                "error": row.get::<_, String>(7)?,
                "created_at": row.get::<_, i64>(8)?,
                "updated_at": row.get::<_, i64>(9)?,
            }))
        },
    ) {
        Ok(launch) => ToolResult::success(serde_json::json!({"launch": launch})),
        Err(e) => ToolResult::fail(&format!("db error fetching launch: {}", e)),
    }
}

pub fn delete(args: &Value, conn: &Connection) -> ToolResult {
    let id = match args["id"].as_str() {
        Some(s) => s,
        None => return ToolResult::fail("missing required field: id"),
    };

    match conn.execute("DELETE FROM launches WHERE id = ?1", rusqlite::params![id]) {
        Ok(deleted) => ToolResult::success(serde_json::json!({"removed": deleted > 0})),
        Err(e) => ToolResult::fail(&format!("db error: {}", e)),
    }
}

pub fn list(args: &Value, conn: &Connection) -> ToolResult {
    let status = args["status"].as_str();
    let limit = args["limit"].as_i64().unwrap_or(100);
    let offset = args["offset"].as_i64().unwrap_or(0);

    let mut conditions = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(s) = status {
        conditions.push("l.status = ?");
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
        "SELECT l.id, l.ticket_id, l.team_id, l.branch, l.worktree_path, l.status, l.pr_url, l.error,
                l.created_at, l.updated_at, t.title AS ticket_title, tm.name AS team_name
         FROM launches l
         LEFT JOIN tickets t ON l.ticket_id = t.id
         LEFT JOIN teams tm ON l.team_id = tm.id
         {} ORDER BY l.created_at DESC LIMIT ? OFFSET ?",
        where_clause
    );

    let params_ref: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let mut stmt = match conn.prepare(&sql) {
        Ok(s) => s,
        Err(e) => return ToolResult::fail(&format!("db error: {}", e)),
    };

    let launches: Vec<Value> = stmt
        .query_map(params_ref.as_slice(), |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, String>(0)?,
                "ticket_id": row.get::<_, String>(1)?,
                "team_id": row.get::<_, String>(2)?,
                "branch": row.get::<_, String>(3)?,
                "worktree_path": row.get::<_, String>(4)?,
                "status": row.get::<_, String>(5)?,
                "pr_url": row.get::<_, String>(6)?,
                "error": row.get::<_, String>(7)?,
                "created_at": row.get::<_, i64>(8)?,
                "updated_at": row.get::<_, i64>(9)?,
                "ticket_title": row.get::<_, Option<String>>(10)?,
                "team_name": row.get::<_, Option<String>>(11)?,
            }))
        })
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();

    let count = launches.len();
    ToolResult::success(serde_json::json!({"launches": launches, "count": count}))
}

pub fn wait(args: &Value, conn: &Connection) -> ToolResult {
    let timeout_secs = args["timeout"].as_i64().unwrap_or(120).clamp(1, 300) as u64;
    let mut since_id = args["since_id"].as_i64().unwrap_or(0);

    // If since_id is 0, default to current max change ID so we don't pick up stale launches
    if since_id == 0 {
        since_id = conn
            .query_row("SELECT COALESCE(MAX(id), 0) FROM changes", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap_or(0);
    }

    let deadline = Instant::now() + Duration::from_secs(timeout_secs);

    loop {
        // Look for a new pending launch insert in the changes table
        let change: Option<(i64, String)> = conn
            .query_row(
                "SELECT id, row_id FROM changes WHERE id > ?1 AND table_name = 'launches' AND action = 'insert' AND summary LIKE '%\"status\":\"pending\"%' ORDER BY id ASC LIMIT 1",
                rusqlite::params![since_id],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
            )
            .ok();

        if let Some((change_id, launch_id)) = change {
            // Fetch the full launch record with JOINs (same as list())
            let sql = "SELECT l.id, l.ticket_id, l.team_id, l.branch, l.worktree_path, l.status, l.pr_url, l.error,
                        l.created_at, l.updated_at, t.title AS ticket_title, tm.name AS team_name
                 FROM launches l
                 LEFT JOIN tickets t ON l.ticket_id = t.id
                 LEFT JOIN teams tm ON l.team_id = tm.id
                 WHERE l.id = ?1";

            match conn.query_row(sql, rusqlite::params![launch_id], |row| {
                Ok(serde_json::json!({
                    "id": row.get::<_, String>(0)?,
                    "ticket_id": row.get::<_, String>(1)?,
                    "team_id": row.get::<_, String>(2)?,
                    "branch": row.get::<_, String>(3)?,
                    "worktree_path": row.get::<_, String>(4)?,
                    "status": row.get::<_, String>(5)?,
                    "pr_url": row.get::<_, String>(6)?,
                    "error": row.get::<_, String>(7)?,
                    "created_at": row.get::<_, i64>(8)?,
                    "updated_at": row.get::<_, i64>(9)?,
                    "ticket_title": row.get::<_, Option<String>>(10)?,
                    "team_name": row.get::<_, Option<String>>(11)?,
                }))
            }) {
                Ok(launch) => {
                    return ToolResult::success(serde_json::json!({
                        "launches": [launch],
                        "cursor": change_id,
                        "timed_out": false
                    }));
                }
                Err(e) => {
                    return ToolResult::fail(&format!("db error fetching launch: {}", e));
                }
            }
        }

        if Instant::now() >= deadline {
            // Timeout: return the highest change ID as cursor
            let max_id: i64 = conn
                .query_row("SELECT COALESCE(MAX(id), 0) FROM changes", [], |row| {
                    row.get::<_, i64>(0)
                })
                .unwrap_or(since_id);

            return ToolResult::success(serde_json::json!({
                "launches": [],
                "cursor": max_id,
                "timed_out": true
            }));
        }

        thread::sleep(Duration::from_millis(500));
    }
}
