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
pub struct MessagesParams {
    pub channel: Option<String>,
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

    let unacked_messages: i64 = conn.query_row(
        "SELECT COUNT(*) FROM messages WHERE acknowledged = 0", [], |r| r.get(0)
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
        "unacked_messages": unacked_messages,
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

pub async fn messages(State(db): State<DbState>, Query(params): Query<MessagesParams>) -> Json<Value> {
    let conn = db.lock().unwrap();
    let limit = params.limit.unwrap_or(50);
    let offset = params.offset.unwrap_or(0);

    // Get channels list
    let channels: Vec<String> = conn.prepare("SELECT DISTINCT channel FROM messages ORDER BY channel")
        .unwrap().query_map([], |row| row.get(0)).unwrap().filter_map(|r| r.ok()).collect();

    let (sql, sql_params): (String, Vec<Box<dyn rusqlite::types::ToSql>>) = match &params.channel {
        Some(ch) => (
            "SELECT id, channel, sender, priority, content, acknowledged, created_at
             FROM messages WHERE channel = ?1 ORDER BY created_at DESC LIMIT ?2 OFFSET ?3".to_string(),
            vec![Box::new(ch.clone()), Box::new(limit), Box::new(offset)],
        ),
        None => (
            "SELECT id, channel, sender, priority, content, acknowledged, created_at
             FROM messages ORDER BY created_at DESC LIMIT ?1 OFFSET ?2".to_string(),
            vec![Box::new(limit), Box::new(offset)],
        ),
    };

    let mut stmt = conn.prepare(&sql).unwrap();
    let params_ref: Vec<&dyn rusqlite::types::ToSql> = sql_params.iter().map(|p| p.as_ref()).collect();
    let messages: Vec<Value> = stmt.query_map(params_ref.as_slice(), |row| {
        Ok(json!({
            "id": row.get::<_, i64>(0)?,
            "channel": row.get::<_, String>(1)?,
            "sender": row.get::<_, String>(2)?,
            "priority": row.get::<_, i64>(3)?,
            "content": row.get::<_, String>(4)?,
            "acknowledged": row.get::<_, bool>(5)?,
            "created_at": row.get::<_, i64>(6)?,
        }))
    }).unwrap().filter_map(|r| r.ok()).collect();

    Json(json!({"messages": messages, "count": messages.len(), "channels": channels}))
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
