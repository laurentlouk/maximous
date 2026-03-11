use axum::{
    extract::{State, Query},
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
        "SELECT id, source, external_id, title, status, priority, url, labels, fetched_at
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
            "priority": row.get::<_, Option<String>>(5)?,
            "url": row.get::<_, Option<String>>(6)?,
            "labels": row.get::<_, Option<String>>(7)?,
            "fetched_at": row.get::<_, i64>(8)?,
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
        "SELECT l.id, l.ticket_id, l.team_id, l.status, l.created_at, l.updated_at,
                t.title AS ticket_title, tm.name AS team_name
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
            "status": row.get::<_, String>(3)?,
            "created_at": row.get::<_, i64>(4)?,
            "updated_at": row.get::<_, i64>(5)?,
            "ticket_title": row.get::<_, Option<String>>(6)?,
            "team_name": row.get::<_, Option<String>>(7)?,
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
