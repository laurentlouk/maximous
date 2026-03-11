use axum::{
    extract::{State, Query, Path},
    response::sse::{Event, Sse},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::convert::Infallible;
use super::DbState;

#[derive(Deserialize, Default)]
pub struct PaginationParams {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Deserialize, Default)]
pub struct MemoryParams {
    pub namespace: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Deserialize, Default)]
pub struct TicketsParams {
    pub source: Option<String>,
    pub status: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Deserialize, Default)]
pub struct LaunchesParams {
    pub status: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Deserialize, Default)]
pub struct ChangesParams {
    pub since_id: Option<i64>,
    pub table_name: Option<String>,
    pub limit: Option<i64>,
}

pub async fn overview(State(db): State<DbState>) -> Json<Value> {
    let conn = db.lock().unwrap();
    let agent_count: i64 = conn.query_row("SELECT COUNT(*) FROM agents", [], |r| r.get(0)).unwrap_or(0);

    let task_counts = {
        let mut stmt = conn.prepare("SELECT status, COUNT(*) FROM tasks GROUP BY status").unwrap();
        let rows: Vec<(String, i64)> = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap().filter_map(|r| r.ok()).collect();
        rows
    };

    let team_count: i64 = conn.query_row("SELECT COUNT(*) FROM teams", [], |r| r.get(0)).unwrap_or(0);
    let ticket_count: i64 = conn.query_row("SELECT COUNT(*) FROM tickets", [], |r| r.get(0)).unwrap_or(0);
    let active_launches: i64 = conn.query_row(
        "SELECT COUNT(*) FROM launches WHERE status IN ('pending', 'running')", [], |r| r.get(0)
    ).unwrap_or(0);

    let memory_count: i64 = conn.query_row("SELECT COUNT(*) FROM memory", [], |r| r.get(0)).unwrap_or(0);

    let session_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sessions WHERE status = 'active'", [], |r| r.get(0)
    ).unwrap_or(0);

    let tasks_by_status: Value = task_counts.into_iter()
        .map(|(s, c)| (s, json!(c)))
        .collect::<serde_json::Map<String, Value>>()
        .into();

    Json(json!({
        "agents": agent_count,
        "tasks_by_status": tasks_by_status,
        "teams": team_count,
        "tickets": ticket_count,
        "active_launches": active_launches,
        "memory_entries": memory_count,
        "active_sessions": session_count,
    }))
}

pub async fn agents(State(db): State<DbState>, Query(params): Query<PaginationParams>) -> Json<Value> {
    let conn = db.lock().unwrap();
    let limit = params.limit.unwrap_or(100);
    let offset = params.offset.unwrap_or(0);
    let mut stmt = conn.prepare(
        "SELECT id, name, status, capabilities, metadata, last_heartbeat FROM agents ORDER BY name LIMIT ?1 OFFSET ?2"
    ).unwrap();
    let agents: Vec<Value> = stmt.query_map(rusqlite::params![limit, offset], |row| {
        Ok(json!({
            "id": row.get::<_, String>(0)?,
            "name": row.get::<_, String>(1)?,
            "status": row.get::<_, String>(2)?,
            "capabilities": row.get::<_, Option<String>>(3)?,
            "metadata": row.get::<_, Option<String>>(4)?,
            "last_heartbeat": row.get::<_, i64>(5)?,
        }))
    }).unwrap().filter_map(|r| r.ok()).collect();
    Json(json!({"agents": agents, "count": agents.len()}))
}

pub async fn tasks(State(db): State<DbState>, Query(params): Query<PaginationParams>) -> Json<Value> {
    let conn = db.lock().unwrap();
    let limit = params.limit.unwrap_or(100);
    let offset = params.offset.unwrap_or(0);
    let mut stmt = conn.prepare(
        "SELECT id, title, status, priority, assigned_to, dependencies, result, created_at, updated_at
         FROM tasks ORDER BY priority ASC, created_at ASC LIMIT ?1 OFFSET ?2"
    ).unwrap();
    let tasks: Vec<Value> = stmt.query_map(rusqlite::params![limit, offset], |row| {
        Ok(json!({
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
    }).unwrap().filter_map(|r| r.ok()).collect();
    Json(json!({"tasks": tasks, "count": tasks.len()}))
}

pub async fn agent_definitions(State(db): State<DbState>, Query(params): Query<PaginationParams>) -> Json<Value> {
    let conn = db.lock().unwrap();
    let limit = params.limit.unwrap_or(100);
    let offset = params.offset.unwrap_or(0);
    let mut stmt = conn.prepare(
        "SELECT id, name, capabilities, model, prompt_hint, created_at, updated_at
         FROM agent_definitions ORDER BY name LIMIT ?1 OFFSET ?2"
    ).unwrap();
    let agents: Vec<Value> = stmt.query_map(rusqlite::params![limit, offset], |row| {
        Ok(json!({
            "id": row.get::<_, String>(0)?,
            "name": row.get::<_, String>(1)?,
            "capabilities": row.get::<_, Option<String>>(2)?,
            "model": row.get::<_, Option<String>>(3)?,
            "prompt_hint": row.get::<_, Option<String>>(4)?,
            "created_at": row.get::<_, i64>(5)?,
            "updated_at": row.get::<_, i64>(6)?,
        }))
    }).unwrap().filter_map(|r| r.ok()).collect();
    Json(json!({"agents": agents, "count": agents.len()}))
}

pub async fn teams(State(db): State<DbState>, Query(params): Query<PaginationParams>) -> Json<Value> {
    let conn = db.lock().unwrap();
    let limit = params.limit.unwrap_or(100);
    let offset = params.offset.unwrap_or(0);
    let mut stmt = conn.prepare(
        "SELECT id, name, description, created_at, updated_at FROM teams ORDER BY name LIMIT ?1 OFFSET ?2"
    ).unwrap();
    let teams_raw: Vec<(String, String, Option<String>, i64, i64)> = stmt.query_map(rusqlite::params![limit, offset], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, i64>(3)?,
            row.get::<_, i64>(4)?,
        ))
    }).unwrap().filter_map(|r| r.ok()).collect();

    let teams: Vec<Value> = teams_raw.into_iter().map(|(id, name, description, created_at, updated_at)| {
        let mut member_stmt = conn.prepare(
            "SELECT tm.agent_id, tm.role, ad.name, ad.model
             FROM team_members tm
             LEFT JOIN agent_definitions ad ON tm.agent_id = ad.id
             WHERE tm.team_id = ?1"
        ).unwrap();
        let members: Vec<Value> = member_stmt.query_map(rusqlite::params![id], |row| {
            Ok(json!({
                "agent_id": row.get::<_, String>(0)?,
                "role": row.get::<_, Option<String>>(1)?,
                "name": row.get::<_, Option<String>>(2)?,
                "model": row.get::<_, Option<String>>(3)?,
            }))
        }).unwrap().filter_map(|r| r.ok()).collect();
        json!({
            "id": id,
            "name": name,
            "description": description,
            "created_at": created_at,
            "updated_at": updated_at,
            "members": members,
        })
    }).collect();

    Json(json!({"teams": teams, "count": teams.len()}))
}

pub async fn tickets(State(db): State<DbState>, Query(params): Query<TicketsParams>) -> Json<Value> {
    let conn = db.lock().unwrap();
    let limit = params.limit.unwrap_or(50);
    let offset = params.offset.unwrap_or(0);

    let mut conditions: Vec<String> = Vec::new();
    let mut sql_params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut idx = 1usize;

    if let Some(source) = &params.source {
        conditions.push(format!("source = ?{}", idx));
        sql_params.push(Box::new(source.clone()));
        idx += 1;
    }
    if let Some(status) = &params.status {
        conditions.push(format!("status = ?{}", idx));
        sql_params.push(Box::new(status.clone()));
        idx += 1;
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    let sql = format!(
        "SELECT id, source, external_id, title, status, assignee, priority, url, labels, fetched_at
         FROM tickets {} ORDER BY fetched_at DESC LIMIT ?{} OFFSET ?{}",
        where_clause, idx, idx + 1
    );
    sql_params.push(Box::new(limit));
    sql_params.push(Box::new(offset));

    let mut stmt = conn.prepare(&sql).unwrap();
    let params_ref: Vec<&dyn rusqlite::types::ToSql> = sql_params.iter().map(|p| p.as_ref()).collect();
    let tickets: Vec<Value> = stmt.query_map(params_ref.as_slice(), |row| {
        Ok(json!({
            "id": row.get::<_, String>(0)?,
            "source": row.get::<_, String>(1)?,
            "external_id": row.get::<_, Option<String>>(2)?,
            "title": row.get::<_, String>(3)?,
            "status": row.get::<_, Option<String>>(4)?,
            "assignee": row.get::<_, Option<String>>(5)?,
            "priority": row.get::<_, Option<i64>>(6)?,
            "url": row.get::<_, Option<String>>(7)?,
            "labels": row.get::<_, Option<String>>(8)?,
            "fetched_at": row.get::<_, i64>(9)?,
        }))
    }).unwrap().filter_map(|r| r.ok()).collect();

    Json(json!({"tickets": tickets, "count": tickets.len()}))
}

pub async fn launches(State(db): State<DbState>, Query(params): Query<LaunchesParams>) -> Json<Value> {
    let conn = db.lock().unwrap();
    let limit = params.limit.unwrap_or(50);
    let offset = params.offset.unwrap_or(0);

    let mut sql_params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut idx = 1usize;

    let status_clause = if let Some(status) = &params.status {
        let clause = format!("WHERE l.status = ?{}", idx);
        sql_params.push(Box::new(status.clone()));
        idx += 1;
        clause
    } else {
        String::new()
    };

    let sql = format!(
        "SELECT l.id, l.ticket_id, l.team_id, l.branch, l.worktree_path, l.status, l.pr_url, l.error,
                l.created_at, l.updated_at, t.title AS ticket_title, tm.name AS team_name
         FROM launches l
         LEFT JOIN tickets t ON l.ticket_id = t.id
         LEFT JOIN teams tm ON l.team_id = tm.id
         {} ORDER BY l.created_at DESC LIMIT ?{} OFFSET ?{}",
        status_clause, idx, idx + 1
    );
    sql_params.push(Box::new(limit));
    sql_params.push(Box::new(offset));

    let mut stmt = conn.prepare(&sql).unwrap();
    let params_ref: Vec<&dyn rusqlite::types::ToSql> = sql_params.iter().map(|p| p.as_ref()).collect();
    let launches: Vec<Value> = stmt.query_map(params_ref.as_slice(), |row| {
        Ok(json!({
            "id": row.get::<_, String>(0)?,
            "ticket_id": row.get::<_, Option<String>>(1)?,
            "team_id": row.get::<_, Option<String>>(2)?,
            "branch": row.get::<_, Option<String>>(3)?,
            "worktree_path": row.get::<_, Option<String>>(4)?,
            "status": row.get::<_, String>(5)?,
            "pr_url": row.get::<_, Option<String>>(6)?,
            "error": row.get::<_, Option<String>>(7)?,
            "created_at": row.get::<_, i64>(8)?,
            "updated_at": row.get::<_, i64>(9)?,
            "ticket_title": row.get::<_, Option<String>>(10)?,
            "team_name": row.get::<_, Option<String>>(11)?,
        }))
    }).unwrap().filter_map(|r| r.ok()).collect();

    Json(json!({"launches": launches, "count": launches.len()}))
}

pub async fn memory(State(db): State<DbState>, Query(params): Query<MemoryParams>) -> Json<Value> {
    let conn = db.lock().unwrap();
    let limit = params.limit.unwrap_or(50);
    let offset = params.offset.unwrap_or(0);

    let namespaces: Vec<String> = conn.prepare("SELECT DISTINCT namespace FROM memory ORDER BY namespace")
        .unwrap().query_map([], |row| row.get(0)).unwrap().filter_map(|r| r.ok()).collect();

    let (sql, sql_params): (String, Vec<Box<dyn rusqlite::types::ToSql>>) = match &params.namespace {
        Some(ns) => (
            "SELECT namespace, key, value, observation_type, category, updated_at
             FROM memory WHERE namespace = ?1 ORDER BY key LIMIT ?2 OFFSET ?3".to_string(),
            vec![Box::new(ns.clone()), Box::new(limit), Box::new(offset)],
        ),
        None => (
            "SELECT namespace, key, value, observation_type, category, updated_at
             FROM memory ORDER BY namespace, key LIMIT ?1 OFFSET ?2".to_string(),
            vec![Box::new(limit), Box::new(offset)],
        ),
    };

    let mut stmt = conn.prepare(&sql).unwrap();
    let params_ref: Vec<&dyn rusqlite::types::ToSql> = sql_params.iter().map(|p| p.as_ref()).collect();
    let entries: Vec<Value> = stmt.query_map(params_ref.as_slice(), |row| {
        Ok(json!({
            "namespace": row.get::<_, String>(0)?,
            "key": row.get::<_, String>(1)?,
            "value": row.get::<_, String>(2)?,
            "observation_type": row.get::<_, Option<String>>(3)?,
            "category": row.get::<_, Option<String>>(4)?,
            "updated_at": row.get::<_, i64>(5)?,
        }))
    }).unwrap().filter_map(|r| r.ok()).collect();

    Json(json!({"entries": entries, "count": entries.len(), "namespaces": namespaces}))
}

pub async fn sessions(State(db): State<DbState>, Query(params): Query<PaginationParams>) -> Json<Value> {
    let conn = db.lock().unwrap();
    let limit = params.limit.unwrap_or(50);
    let offset = params.offset.unwrap_or(0);
    let mut stmt = conn.prepare(
        "SELECT id, agent_id, status, metadata, summary, started_at, ended_at
         FROM sessions ORDER BY started_at DESC LIMIT ?1 OFFSET ?2"
    ).unwrap();
    let sessions: Vec<Value> = stmt.query_map(rusqlite::params![limit, offset], |row| {
        Ok(json!({
            "id": row.get::<_, String>(0)?,
            "agent_id": row.get::<_, Option<String>>(1)?,
            "status": row.get::<_, String>(2)?,
            "metadata": row.get::<_, Option<String>>(3)?,
            "summary": row.get::<_, Option<String>>(4)?,
            "started_at": row.get::<_, i64>(5)?,
            "ended_at": row.get::<_, Option<i64>>(6)?,
        }))
    }).unwrap().filter_map(|r| r.ok()).collect();
    Json(json!({"sessions": sessions, "count": sessions.len()}))
}

pub async fn changes(State(db): State<DbState>, Query(params): Query<ChangesParams>) -> Json<Value> {
    let conn = db.lock().unwrap();
    let since_id = params.since_id.unwrap_or(0);
    let limit = params.limit.unwrap_or(100);

    let (sql, sql_params): (String, Vec<Box<dyn rusqlite::types::ToSql>>) = match &params.table_name {
        Some(tn) => (
            "SELECT id, table_name, row_id, action, summary, created_at
             FROM changes WHERE id > ?1 AND table_name = ?2 ORDER BY id DESC LIMIT ?3".to_string(),
            vec![Box::new(since_id), Box::new(tn.clone()), Box::new(limit)],
        ),
        None => (
            "SELECT id, table_name, row_id, action, summary, created_at
             FROM changes WHERE id > ?1 ORDER BY id DESC LIMIT ?2".to_string(),
            vec![Box::new(since_id), Box::new(limit)],
        ),
    };

    let mut stmt = conn.prepare(&sql).unwrap();
    let params_ref: Vec<&dyn rusqlite::types::ToSql> = sql_params.iter().map(|p| p.as_ref()).collect();
    let changes: Vec<Value> = stmt.query_map(params_ref.as_slice(), |row| {
        Ok(json!({
            "id": row.get::<_, i64>(0)?,
            "table_name": row.get::<_, String>(1)?,
            "row_id": row.get::<_, String>(2)?,
            "action": row.get::<_, String>(3)?,
            "summary": row.get::<_, Option<String>>(4)?,
            "created_at": row.get::<_, i64>(5)?,
        }))
    }).unwrap().filter_map(|r| r.ok()).collect();
    Json(json!({"changes": changes, "count": changes.len()}))
}

pub async fn prerequisites() -> Json<Value> {
    let gh_available = std::process::Command::new("which")
        .arg("gh")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    let git_available = std::process::Command::new("which")
        .arg("git")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    let mut errors: Vec<String> = Vec::new();
    if !gh_available {
        errors.push("GitHub CLI (gh) not found. Install: https://cli.github.com/".to_string());
    }
    if !git_available {
        errors.push("git not found".to_string());
    }

    Json(json!({
        "gh": gh_available,
        "git": git_available,
        "errors": errors,
        "all_ok": errors.is_empty()
    }))
}

/// SSE endpoint that streams changes in real-time
pub async fn events_sse(
    State(db): State<DbState>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let stream = async_stream::stream! {
        let mut last_id: i64 = {
            let conn = db.lock().unwrap();
            conn.query_row("SELECT COALESCE(MAX(id), 0) FROM changes", [], |r| r.get(0))
                .unwrap_or(0)
        };

        loop {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            let new_changes: Vec<Value> = {
                let conn = db.lock().unwrap();
                let mut stmt = conn.prepare(
                    "SELECT id, table_name, row_id, action, summary, created_at
                     FROM changes WHERE id > ?1 ORDER BY id ASC LIMIT 50"
                ).unwrap();
                stmt.query_map(rusqlite::params![last_id], |row| {
                    Ok(json!({
                        "id": row.get::<_, i64>(0)?,
                        "table_name": row.get::<_, String>(1)?,
                        "row_id": row.get::<_, String>(2)?,
                        "action": row.get::<_, String>(3)?,
                        "summary": row.get::<_, Option<String>>(4)?,
                        "created_at": row.get::<_, i64>(5)?,
                    }))
                }).unwrap().filter_map(|r| r.ok()).collect()
            };

            for change in &new_changes {
                if let Some(id) = change["id"].as_i64() {
                    if id > last_id { last_id = id; }
                }
                yield Ok(Event::default().data(change.to_string()));
            }
        }
    };

    Sse::new(stream)
}

#[derive(Deserialize)]
pub struct CreateAgentDefinitionBody {
    pub id: String,
    pub name: String,
    pub capabilities: Option<Vec<String>>,
    pub model: Option<String>,
    pub prompt_hint: Option<String>,
}

pub async fn create_agent_definition(
    State(db): State<DbState>,
    Json(body): Json<CreateAgentDefinitionBody>,
) -> Json<Value> {
    let args = json!({
        "id": body.id,
        "name": body.name,
        "capabilities": body.capabilities.unwrap_or_default(),
        "model": body.model.unwrap_or_else(|| "sonnet".to_string()),
        "prompt_hint": body.prompt_hint.unwrap_or_default(),
    });
    let conn = db.lock().unwrap();
    let result = crate::tools::definitions::define(&args, &conn);
    Json(json!({"ok": result.ok, "data": result.data, "error": result.error}))
}

#[derive(Deserialize)]
pub struct CreateTeamBody {
    pub name: String,
    pub description: Option<String>,
}

pub async fn create_team(
    State(db): State<DbState>,
    Json(body): Json<CreateTeamBody>,
) -> Json<Value> {
    let args = json!({
        "name": body.name,
        "description": body.description.unwrap_or_default(),
    });
    let conn = db.lock().unwrap();
    let result = crate::tools::teams::create(&args, &conn);
    Json(json!({"ok": result.ok, "data": result.data, "error": result.error}))
}

#[derive(Deserialize)]
pub struct AddMemberBody {
    pub agent_id: String,
    pub role: Option<String>,
}

pub async fn add_team_member(
    State(db): State<DbState>,
    Path(name): Path<String>,
    Json(body): Json<AddMemberBody>,
) -> Json<Value> {
    let args = json!({
        "team_name": name,
        "agent_id": body.agent_id,
        "role": body.role.unwrap_or_default(),
    });
    let conn = db.lock().unwrap();
    let result = crate::tools::teams::add_member(&args, &conn);
    Json(json!({"ok": result.ok, "data": result.data, "error": result.error}))
}

pub async fn remove_team_member(
    State(db): State<DbState>,
    Path((name, agent_id)): Path<(String, String)>,
) -> Json<Value> {
    let args = json!({
        "team_name": name,
        "agent_id": agent_id,
    });
    let conn = db.lock().unwrap();
    let result = crate::tools::teams::remove_member(&args, &conn);
    Json(json!({"ok": result.ok, "data": result.data, "error": result.error}))
}

pub async fn delete_team(
    State(db): State<DbState>,
    Path(name): Path<String>,
) -> Json<Value> {
    let args = json!({ "name": name });
    let conn = db.lock().unwrap();
    let result = crate::tools::teams::delete(&args, &conn);
    Json(json!({"ok": result.ok, "data": result.data, "error": result.error}))
}

#[derive(Deserialize)]
pub struct CreateLaunchBody {
    pub ticket_id: String,
    pub team_id: String,
    pub branch: Option<String>,
    pub worktree_path: Option<String>,
}

pub async fn create_launch(
    State(db): State<DbState>,
    Json(body): Json<CreateLaunchBody>,
) -> Json<Value> {
    let branch = body.branch.unwrap_or_else(|| {
        format!("launch/{}-{}", body.ticket_id.to_lowercase(), &uuid::Uuid::new_v4().to_string()[..8])
    });
    let args = json!({
        "ticket_id": body.ticket_id,
        "team_id": body.team_id,
        "branch": branch,
        "worktree_path": body.worktree_path.unwrap_or_default(),
    });
    let conn = db.lock().unwrap();
    let result = crate::tools::launches::create(&args, &conn);
    Json(json!({"ok": result.ok, "data": result.data, "error": result.error}))
}

pub async fn update_launch(
    State(db): State<DbState>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let mut args = body;
    args["id"] = json!(id);
    let conn = db.lock().unwrap();
    let result = crate::tools::launches::update(&args, &conn);
    Json(json!({"ok": result.ok, "data": result.data, "error": result.error}))
}

pub async fn delete_launch(
    State(db): State<DbState>,
    Path(id): Path<String>,
) -> Json<Value> {
    let args = json!({ "id": id });
    let conn = db.lock().unwrap();
    let result = crate::tools::launches::delete(&args, &conn);
    Json(json!({"ok": result.ok, "data": result.data, "error": result.error}))
}

pub async fn execute_launch(
    State(db): State<DbState>,
    Path(id): Path<String>,
) -> Json<Value> {
    let conn = db.lock().unwrap();

    // Fetch launch + ticket + team details
    let launch_info = conn.query_row(
        "SELECT l.id, l.ticket_id, l.team_id, l.branch, t.title, t.url, t.source, t.status,
                tm.name AS team_name
         FROM launches l
         LEFT JOIN tickets t ON l.ticket_id = t.id
         LEFT JOIN teams tm ON l.team_id = tm.id
         WHERE l.id = ?1",
        rusqlite::params![id],
        |row| {
            Ok(json!({
                "id": row.get::<_, String>(0)?,
                "ticket_id": row.get::<_, Option<String>>(1)?,
                "team_id": row.get::<_, Option<String>>(2)?,
                "branch": row.get::<_, Option<String>>(3)?,
                "ticket_title": row.get::<_, Option<String>>(4)?,
                "ticket_url": row.get::<_, Option<String>>(5)?,
                "ticket_source": row.get::<_, Option<String>>(6)?,
                "ticket_status": row.get::<_, Option<String>>(7)?,
                "team_name": row.get::<_, Option<String>>(8)?,
            }))
        },
    );

    let info = match launch_info {
        Ok(v) => v,
        Err(_) => return Json(json!({"ok": false, "error": "launch not found"})),
    };

    // Get team members
    let team_id = info["team_id"].as_str().unwrap_or("");
    let members: Vec<(String, String, String)> = if !team_id.is_empty() {
        let mut stmt = conn.prepare(
            "SELECT tm.agent_id, ad.name, ad.model
             FROM team_members tm
             LEFT JOIN agent_definitions ad ON tm.agent_id = ad.id
             WHERE tm.team_id = ?1"
        ).unwrap();
        stmt.query_map(rusqlite::params![team_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                row.get::<_, Option<String>>(2)?.unwrap_or_default(),
            ))
        }).unwrap().filter_map(|r| r.ok()).collect()
    } else {
        Vec::new()
    };

    // Update launch status to running
    let _ = conn.execute(
        "UPDATE launches SET status = 'running', updated_at = strftime('%s', 'now') WHERE id = ?1",
        rusqlite::params![id],
    );

    drop(conn); // Release the lock before spawning

    // Build the prompt
    let ticket_title = info["ticket_title"].as_str().unwrap_or("Unknown ticket");
    let ticket_url = info["ticket_url"].as_str().unwrap_or("");
    let team_name = info["team_name"].as_str().unwrap_or("Unknown team");
    let branch = info["branch"].as_str().unwrap_or("");
    let launch_id = info["id"].as_str().unwrap_or(&id);

    let members_desc = if members.is_empty() {
        String::new()
    } else {
        let list: Vec<String> = members.iter().map(|(id, name, model)| {
            format!("- {} ({}, model: {})", name, id, model)
        }).collect();
        format!("\nTeam members:\n{}", list.join("\n"))
    };

    let ticket_ref = if ticket_url.is_empty() {
        ticket_title.to_string()
    } else {
        format!("{}\nURL: {}", ticket_title, ticket_url)
    };

    let prompt = format!(
        "You are an orchestrator for team \"{}\" working on a launch.\n\
         {}\n\
         Ticket: {}\n\
         Branch: {}\n\
         Launch ID: {}\n\n\
         INSTRUCTIONS:\n\
         1. Use /maximous:orchestrate to coordinate this work\n\
         2. Start a maximous session (session_start) for this launch\n\
         3. Break down the ticket into tasks (task_create) and assign them to team members\n\
         4. Use the Agent tool to dispatch sub-agents for each team member's tasks\n\
         5. Each sub-agent should use maximous tools (agent_heartbeat, task_update) to report progress\n\
         6. When all tasks are done, update the launch status to 'done' (launch_update)\n\
         7. Create a PR if code changes were made\n\n\
         The maximous dashboard is watching — all activities, tasks, and agent statuses will be visible in real-time.\n\
         Start by understanding the ticket, then plan and execute.",
        team_name, members_desc, ticket_ref, branch, launch_id
    );

    // Write prompt to a temp file to avoid shell escaping issues
    let prompt_file = format!("/tmp/maximous-launch-{}.txt", id);
    if let Err(e) = std::fs::write(&prompt_file, &prompt) {
        return Json(json!({"ok": false, "error": format!("failed to write prompt file: {}", e)}));
    }

    // Get the current working directory (project root)
    let cwd = std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| ".".to_string());

    let escaped_branch = branch.replace('\'', "'\\''");

    // Build the shell command: checkout branch, then run claude with the prompt
    let shell_cmd = format!(
        "cd '{cwd}' && git checkout -b '{branch}' 2>/dev/null; git checkout '{branch}' 2>/dev/null; claude < '{prompt_file}'",
        cwd = cwd,
        branch = escaped_branch,
        prompt_file = prompt_file,
    );

    // Use osascript on macOS to open a new Terminal tab
    let apple_script = format!(
        "tell application \"Terminal\"\n\
           activate\n\
           do script \"{}\"\n\
         end tell",
        shell_cmd.replace('\\', "\\\\").replace('"', "\\\"")
    );

    let result = std::process::Command::new("osascript")
        .arg("-e")
        .arg(&apple_script)
        .spawn();

    match result {
        Ok(_) => Json(json!({"ok": true, "message": "Claude Code launched in new terminal"})),
        Err(e) => Json(json!({"ok": false, "error": format!("failed to open terminal: {}", e)})),
    }
}
