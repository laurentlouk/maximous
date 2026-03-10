use rusqlite::Connection;
use serde_json::Value;
use super::ToolResult;

/// Strip <private>...</private> tags and replace with [REDACTED]
fn strip_private(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    let mut remaining = value;
    while let Some(start) = remaining.find("<private>") {
        result.push_str(&remaining[..start]);
        result.push_str("[REDACTED]");
        remaining = &remaining[start + 9..]; // skip "<private>"
        if let Some(end) = remaining.find("</private>") {
            remaining = &remaining[end + 10..]; // skip "</private>"
        } else {
            // No closing tag — redact rest
            return result;
        }
    }
    result.push_str(remaining);
    result
}

pub fn set(args: &Value, conn: &Connection) -> ToolResult {
    let namespace = match args["namespace"].as_str() {
        Some(s) => s,
        None => return ToolResult::fail("missing required field: namespace"),
    };
    let key = match args["key"].as_str() {
        Some(s) => s,
        None => return ToolResult::fail("missing required field: key"),
    };
    let value = match args["value"].as_str() {
        Some(s) => s,
        None => return ToolResult::fail("missing required field: value"),
    };
    let ttl = args["ttl_seconds"].as_i64();
    let observation_type = args["observation_type"].as_str();
    let category = args["category"].as_str();

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    match conn.execute(
        "INSERT INTO memory (namespace, key, value, ttl_seconds, observation_type, category, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)
         ON CONFLICT(namespace, key) DO UPDATE SET value=?3, ttl_seconds=?4, observation_type=?5, category=?6, updated_at=?7",
        rusqlite::params![namespace, key, value, ttl, observation_type, category, now],
    ) {
        Ok(_) => ToolResult::success(serde_json::json!({"stored": true})),
        Err(e) => ToolResult::fail(&format!("db error: {}", e)),
    }
}

pub fn get(args: &Value, conn: &Connection) -> ToolResult {
    let namespace = match args["namespace"].as_str() {
        Some(s) => s,
        None => return ToolResult::fail("missing required field: namespace"),
    };

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let _ = conn.execute(
        "DELETE FROM memory WHERE namespace = ?1 AND ttl_seconds IS NOT NULL AND (created_at + ttl_seconds) < ?2",
        rusqlite::params![namespace, now],
    );

    match args["key"].as_str() {
        Some(key) => {
            type MemRow = (String, Option<i64>, i64, Option<String>, Option<String>);
            let result: Result<MemRow, _> = conn.query_row(
                "SELECT value, ttl_seconds, updated_at, observation_type, category FROM memory WHERE namespace = ?1 AND key = ?2",
                rusqlite::params![namespace, key],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
            );
            match result {
                Ok((value, ttl, updated_at, observation_type, category)) => ToolResult::success(serde_json::json!({
                    "namespace": namespace,
                    "key": key,
                    "value": strip_private(&value),
                    "ttl_seconds": ttl,
                    "updated_at": updated_at,
                    "observation_type": observation_type,
                    "category": category,
                })),
                Err(_) => ToolResult::success(serde_json::json!({
                    "namespace": namespace,
                    "key": key,
                    "value": null,
                })),
            }
        }
        None => {
            let mut stmt = conn.prepare(
                "SELECT key, updated_at FROM memory WHERE namespace = ?1 ORDER BY key"
            ).unwrap();
            let keys: Vec<Value> = stmt
                .query_map(rusqlite::params![namespace], |row| {
                    Ok(serde_json::json!({
                        "key": row.get::<_, String>(0)?,
                        "updated_at": row.get::<_, i64>(1)?,
                    }))
                })
                .unwrap()
                .filter_map(|r| r.ok())
                .collect();
            ToolResult::success(serde_json::json!({"namespace": namespace, "keys": keys}))
        }
    }
}

pub fn search(args: &Value, conn: &Connection) -> ToolResult {
    let query = match args["query"].as_str() {
        Some(s) => s,
        None => return ToolResult::fail("missing required field: query"),
    };
    let namespace = args["namespace"].as_str();
    let observation_type = args["observation_type"].as_str();
    let limit = args["limit"].as_i64().unwrap_or(50);
    let offset = args["offset"].as_i64().unwrap_or(0);

    // Try FTS5 first, fall back to LIKE if the FTS table doesn't exist
    match search_fts(query, namespace, observation_type, limit, offset, conn) {
        Ok(result) => result,
        Err(_) => search_like(query, namespace, observation_type, limit, offset, conn),
    }
}

fn search_fts(
    query: &str,
    namespace: Option<&str>,
    observation_type: Option<&str>,
    limit: i64,
    offset: i64,
    conn: &Connection,
) -> Result<ToolResult, rusqlite::Error> {
    let mut sql = "SELECT m.namespace, m.key, m.value, f.rank \
             FROM memory_fts f \
             JOIN memory m ON m.rowid = f.rowid \
             WHERE memory_fts MATCH ?"
        .to_string();
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(query.to_string())];

    if let Some(ns) = namespace {
        sql.push_str(" AND m.namespace = ?");
        params.push(Box::new(ns.to_string()));
    }
    if let Some(ot) = observation_type {
        sql.push_str(" AND m.observation_type = ?");
        params.push(Box::new(ot.to_string()));
    }
    sql.push_str(" ORDER BY f.rank LIMIT ? OFFSET ?");
    params.push(Box::new(limit));
    params.push(Box::new(offset));

    let mut stmt = conn.prepare(&sql)?;
    let params_ref: Vec<&dyn rusqlite::types::ToSql> =
        params.iter().map(|p| p.as_ref()).collect();
    let matches: Vec<Value> = stmt
        .query_map(params_ref.as_slice(), |row| {
            let value: String = row.get(2)?;
            Ok(serde_json::json!({
                "namespace": row.get::<_, String>(0)?,
                "key": row.get::<_, String>(1)?,
                "value": strip_private(&value),
                "rank": row.get::<_, f64>(3)?,
            }))
        })?
        .filter_map(|r| r.ok())
        .collect();

    let count = matches.len() as i64;
    Ok(ToolResult::success(serde_json::json!({
        "matches": matches,
        "count": count,
        "offset": offset,
        "limit": limit,
    })))
}

fn search_like(
    query: &str,
    namespace: Option<&str>,
    observation_type: Option<&str>,
    limit: i64,
    offset: i64,
    conn: &Connection,
) -> ToolResult {
    let mut sql = "SELECT namespace, key, value FROM memory WHERE value LIKE ?".to_string();
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(format!("%{}%", query))];

    if let Some(ns) = namespace {
        sql.push_str(" AND namespace = ?");
        params.push(Box::new(ns.to_string()));
    }
    if let Some(ot) = observation_type {
        sql.push_str(" AND observation_type = ?");
        params.push(Box::new(ot.to_string()));
    }
    sql.push_str(" LIMIT ? OFFSET ?");
    params.push(Box::new(limit));
    params.push(Box::new(offset));

    let mut stmt = conn.prepare(&sql).unwrap();
    let params_ref: Vec<&dyn rusqlite::types::ToSql> =
        params.iter().map(|p| p.as_ref()).collect();
    let matches: Vec<Value> = stmt
        .query_map(params_ref.as_slice(), |row| {
            let value: String = row.get(2)?;
            Ok(serde_json::json!({
                "namespace": row.get::<_, String>(0)?,
                "key": row.get::<_, String>(1)?,
                "value": strip_private(&value),
            }))
        })
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();

    let count = matches.len() as i64;
    ToolResult::success(serde_json::json!({
        "matches": matches,
        "count": count,
        "offset": offset,
        "limit": limit,
    }))
}

pub fn search_index(args: &Value, conn: &Connection) -> ToolResult {
    let query = match args["query"].as_str() {
        Some(s) => s,
        None => return ToolResult::fail("missing required field: query"),
    };
    let namespace = args["namespace"].as_str();
    let observation_type = args["observation_type"].as_str();
    let limit = args["limit"].as_i64().unwrap_or(20);
    let offset = args["offset"].as_i64().unwrap_or(0);

    match search_index_fts(query, namespace, observation_type, limit, offset, conn) {
        Ok(result) => result,
        Err(_) => search_index_like(query, namespace, observation_type, limit, offset, conn),
    }
}

fn search_index_fts(
    query: &str,
    namespace: Option<&str>,
    observation_type: Option<&str>,
    limit: i64,
    offset: i64,
    conn: &Connection,
) -> Result<ToolResult, rusqlite::Error> {
    let mut sql = "SELECT m.namespace, m.key, SUBSTR(m.value, 1, 150) as snippet, \
             LENGTH(m.value) as value_len, m.observation_type, m.category, f.rank \
             FROM memory_fts f \
             JOIN memory m ON m.rowid = f.rowid \
             WHERE memory_fts MATCH ?"
        .to_string();
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(query.to_string())];

    if let Some(ns) = namespace {
        sql.push_str(" AND m.namespace = ?");
        params.push(Box::new(ns.to_string()));
    }
    if let Some(ot) = observation_type {
        sql.push_str(" AND m.observation_type = ?");
        params.push(Box::new(ot.to_string()));
    }
    sql.push_str(" ORDER BY f.rank LIMIT ? OFFSET ?");
    params.push(Box::new(limit));
    params.push(Box::new(offset));

    let mut stmt = conn.prepare(&sql)?;
    let params_ref: Vec<&dyn rusqlite::types::ToSql> =
        params.iter().map(|p| p.as_ref()).collect();
    let matches: Vec<Value> = stmt
        .query_map(params_ref.as_slice(), |row| {
            let ns: String = row.get(0)?;
            let key: String = row.get(1)?;
            let snip: String = row.get(2)?;
            let vlen: i64 = row.get(3)?;
            let obs_type: Option<String> = row.get(4)?;
            let cat: Option<String> = row.get(5)?;
            let rank: f64 = row.get(6)?;
            let display_snippet = if vlen > 150 {
                format!("{}...", snip)
            } else {
                snip
            };
            Ok(serde_json::json!({
                "namespace": ns,
                "key": key,
                "snippet": strip_private(&display_snippet),
                "estimated_tokens": vlen / 4,
                "observation_type": obs_type,
                "category": cat,
                "rank": rank,
            }))
        })?
        .filter_map(|r| r.ok())
        .collect();

    let count = matches.len() as i64;
    Ok(ToolResult::success(serde_json::json!({
        "matches": matches,
        "count": count,
        "offset": offset,
        "limit": limit,
        "hint": "Use memory_get with namespace+key to retrieve full values",
    })))
}

fn search_index_like(
    query: &str,
    namespace: Option<&str>,
    observation_type: Option<&str>,
    limit: i64,
    offset: i64,
    conn: &Connection,
) -> ToolResult {
    let mut sql = "SELECT namespace, key, SUBSTR(value, 1, 150), LENGTH(value), observation_type, category FROM memory WHERE value LIKE ?".to_string();
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(format!("%{}%", query))];

    if let Some(ns) = namespace {
        sql.push_str(" AND namespace = ?");
        params.push(Box::new(ns.to_string()));
    }
    if let Some(ot) = observation_type {
        sql.push_str(" AND observation_type = ?");
        params.push(Box::new(ot.to_string()));
    }
    sql.push_str(" LIMIT ? OFFSET ?");
    params.push(Box::new(limit));
    params.push(Box::new(offset));

    let mut stmt = conn.prepare(&sql).unwrap();
    let params_ref: Vec<&dyn rusqlite::types::ToSql> =
        params.iter().map(|p| p.as_ref()).collect();
    let matches: Vec<Value> = stmt
        .query_map(params_ref.as_slice(), |row| {
            let ns: String = row.get(0)?;
            let key: String = row.get(1)?;
            let snip: String = row.get(2)?;
            let vlen: i64 = row.get(3)?;
            let obs_type: Option<String> = row.get(4)?;
            let cat: Option<String> = row.get(5)?;
            let display_snippet = if vlen > 150 {
                format!("{}...", snip)
            } else {
                snip
            };
            Ok(serde_json::json!({
                "namespace": ns,
                "key": key,
                "snippet": strip_private(&display_snippet),
                "estimated_tokens": vlen / 4,
                "observation_type": obs_type,
                "category": cat,
            }))
        })
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();

    let count = matches.len() as i64;
    ToolResult::success(serde_json::json!({
        "matches": matches,
        "count": count,
        "offset": offset,
        "limit": limit,
        "hint": "Use memory_get with namespace+key to retrieve full values",
    }))
}

pub fn delete(args: &Value, conn: &Connection) -> ToolResult {
    let namespace = match args["namespace"].as_str() {
        Some(s) => s,
        None => return ToolResult::fail("missing required field: namespace"),
    };

    match args["key"].as_str() {
        Some(key) => {
            let deleted = conn.execute(
                "DELETE FROM memory WHERE namespace = ?1 AND key = ?2",
                rusqlite::params![namespace, key],
            ).unwrap_or(0);
            ToolResult::success(serde_json::json!({"deleted": deleted}))
        }
        None => {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64;
            let deleted = conn.execute(
                "DELETE FROM memory WHERE namespace = ?1 AND ttl_seconds IS NOT NULL AND (created_at + ttl_seconds) < ?2",
                rusqlite::params![namespace, now],
            ).unwrap_or(0);
            ToolResult::success(serde_json::json!({"expired": deleted}))
        }
    }
}
