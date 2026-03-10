# Claude-Mem Feature Integration & Web Dashboard Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Integrate the best features from claude-mem into maximous (FTS5 search, typed observations, progressive disclosure, pagination, session tracking, token estimation, privacy tags) and add a web dashboard panel using axum.

**Architecture:** Extend the existing Rust MCP server with: (1) FTS5 virtual tables for full-text search, (2) new columns/tables for typed observations and sessions, (3) pagination across all list endpoints, (4) an embedded axum web server activated via `--web` flag serving a vanilla HTML/JS/CSS SPA compiled into the binary via `rust-embed`, with SSE real-time updates from the existing `changes` table.

**Tech Stack:** Rust, rusqlite (bundled with FTS5), axum 0.7, tower-http 0.5, rust-embed 8, vanilla HTML/JS/CSS

---

## Chunk 1: FTS5 Full-Text Search & Pagination

### Task 1: Add FTS5 Virtual Table to Schema

**Files:**
- Modify: `src/schema.sql`
- Modify: `src/db.rs`

- [ ] **Step 1: Write failing test for FTS5 search**

```rust
// tests/memory_test.rs
#[test]
fn test_memory_fts_search() {
    let conn = setup();
    tools::memory::set(
        &serde_json::json!({"namespace": "docs", "key": "rust-guide", "value": "Rust is a systems programming language focused on safety"}),
        &conn,
    );
    tools::memory::set(
        &serde_json::json!({"namespace": "docs", "key": "python-guide", "value": "Python is an interpreted high-level language"}),
        &conn,
    );
    tools::memory::set(
        &serde_json::json!({"namespace": "docs", "key": "rust-async", "value": "Async programming in Rust uses futures and tokio runtime"}),
        &conn,
    );
    // FTS should rank "Rust" matches and return them
    let result = tools::memory::search(
        &serde_json::json!({"query": "rust programming"}),
        &conn,
    );
    assert!(result.ok);
    let data = result.data.unwrap();
    let matches = data["matches"].as_array().unwrap();
    assert!(matches.len() >= 2); // both rust entries
    // FTS results should have a rank/score field
    assert!(matches[0].get("rank").is_some());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test test_memory_fts_search -- --nocapture`
Expected: FAIL — no `rank` field in current search results

- [ ] **Step 3: Add FTS5 virtual table and sync triggers to schema.sql**

Add to end of `src/schema.sql`:

```sql
-- FTS5 full-text search for memory values
CREATE VIRTUAL TABLE IF NOT EXISTS memory_fts USING fts5(
    namespace,
    key,
    value,
    content='memory',
    content_rowid='rowid'
);

-- Keep FTS index in sync with memory table
CREATE TRIGGER IF NOT EXISTS trg_memory_fts_insert AFTER INSERT ON memory
BEGIN
    INSERT INTO memory_fts(rowid, namespace, key, value)
    VALUES (NEW.rowid, NEW.namespace, NEW.key, NEW.value);
END;

CREATE TRIGGER IF NOT EXISTS trg_memory_fts_update AFTER UPDATE ON memory
BEGIN
    INSERT INTO memory_fts(memory_fts, rowid, namespace, key, value)
    VALUES ('delete', OLD.rowid, OLD.namespace, OLD.key, OLD.value);
    INSERT INTO memory_fts(rowid, namespace, key, value)
    VALUES (NEW.rowid, NEW.namespace, NEW.key, NEW.value);
END;

CREATE TRIGGER IF NOT EXISTS trg_memory_fts_delete AFTER DELETE ON memory
BEGIN
    INSERT INTO memory_fts(memory_fts, rowid, namespace, key, value)
    VALUES ('delete', OLD.rowid, OLD.namespace, OLD.key, OLD.value);
END;
```

- [ ] **Step 4: Enable FTS5 feature in rusqlite**

In `Cargo.toml`, change:
```toml
rusqlite = { version = "0.31", features = ["bundled", "bundled-sqlcipher-vendored-openssl"] }
```
Wait — FTS5 is included with `bundled` by default in rusqlite. Verify by checking if `bundled` includes it. If not, we need:
```toml
rusqlite = { version = "0.31", features = ["bundled"] }
```
The `bundled` feature compiles SQLite with FTS5 enabled. No change needed here.

- [ ] **Step 5: Update memory search to use FTS5 with fallback**

Replace the `search` function in `src/tools/memory.rs`:

```rust
pub fn search(args: &Value, conn: &Connection) -> ToolResult {
    let query = match args["query"].as_str() {
        Some(s) => s,
        None => return ToolResult::fail("missing required field: query"),
    };
    let namespace = args["namespace"].as_str();
    let limit = args["limit"].as_i64().unwrap_or(50);
    let offset = args["offset"].as_i64().unwrap_or(0);

    // Try FTS5 first, fall back to LIKE if FTS table doesn't exist
    let fts_result = search_fts(query, namespace, limit, offset, conn);
    match fts_result {
        Ok(result) => result,
        Err(_) => search_like(query, namespace, limit, offset, conn),
    }
}

fn search_fts(
    query: &str,
    namespace: Option<&str>,
    limit: i64,
    offset: i64,
    conn: &Connection,
) -> Result<ToolResult, rusqlite::Error> {
    let (sql, params): (String, Vec<Box<dyn rusqlite::types::ToSql>>) = match namespace {
        Some(ns) => (
            "SELECT m.namespace, m.key, m.value, rank
             FROM memory_fts f
             JOIN memory m ON m.rowid = f.rowid
             WHERE memory_fts MATCH ? AND m.namespace = ?
             ORDER BY rank
             LIMIT ? OFFSET ?".to_string(),
            vec![
                Box::new(query.to_string()),
                Box::new(ns.to_string()),
                Box::new(limit),
                Box::new(offset),
            ],
        ),
        None => (
            "SELECT m.namespace, m.key, m.value, rank
             FROM memory_fts f
             JOIN memory m ON m.rowid = f.rowid
             WHERE memory_fts MATCH ?
             ORDER BY rank
             LIMIT ? OFFSET ?".to_string(),
            vec![
                Box::new(query.to_string()),
                Box::new(limit),
                Box::new(offset),
            ],
        ),
    };

    let mut stmt = conn.prepare(&sql)?;
    let params_ref: Vec<&dyn rusqlite::types::ToSql> =
        params.iter().map(|p| p.as_ref()).collect();
    let matches: Vec<Value> = stmt
        .query_map(params_ref.as_slice(), |row| {
            Ok(serde_json::json!({
                "namespace": row.get::<_, String>(0)?,
                "key": row.get::<_, String>(1)?,
                "value": row.get::<_, String>(2)?,
                "rank": row.get::<_, f64>(3)?,
            }))
        })?
        .filter_map(|r| r.ok())
        .collect();

    Ok(ToolResult::success(serde_json::json!({
        "matches": matches,
        "count": matches.len(),
        "offset": offset,
        "limit": limit,
    })))
}

fn search_like(
    query: &str,
    namespace: Option<&str>,
    limit: i64,
    offset: i64,
    conn: &Connection,
) -> ToolResult {
    let (sql, params): (String, Vec<Box<dyn rusqlite::types::ToSql>>) = match namespace {
        Some(ns) => (
            "SELECT namespace, key, value FROM memory WHERE namespace = ? AND value LIKE ? LIMIT ? OFFSET ?".to_string(),
            vec![
                Box::new(ns.to_string()),
                Box::new(format!("%{}%", query)),
                Box::new(limit),
                Box::new(offset),
            ],
        ),
        None => (
            "SELECT namespace, key, value FROM memory WHERE value LIKE ? LIMIT ? OFFSET ?".to_string(),
            vec![
                Box::new(format!("%{}%", query)),
                Box::new(limit),
                Box::new(offset),
            ],
        ),
    };

    let mut stmt = conn.prepare(&sql).unwrap();
    let params_ref: Vec<&dyn rusqlite::types::ToSql> =
        params.iter().map(|p| p.as_ref()).collect();
    let matches: Vec<Value> = stmt
        .query_map(params_ref.as_slice(), |row| {
            Ok(serde_json::json!({
                "namespace": row.get::<_, String>(0)?,
                "key": row.get::<_, String>(1)?,
                "value": row.get::<_, String>(2)?,
            }))
        })
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();

    ToolResult::success(serde_json::json!({
        "matches": matches,
        "count": matches.len(),
        "offset": offset,
        "limit": limit,
    }))
}
```

- [ ] **Step 6: Update memory_search tool definition with new params**

In `src/mcp.rs`, update the `memory_search` tool definition to include `limit` and `offset`:

```rust
ToolDef {
    name: "memory_search".into(),
    description: "Full-text search across memory values using FTS5 ranking".into(),
    input_schema: serde_json::json!({
        "type": "object",
        "properties": {
            "query": {"type": "string", "description": "Search query (FTS5 syntax supported: AND, OR, NOT, phrases)"},
            "namespace": {"type": "string", "description": "Optional namespace filter"},
            "limit": {"type": "integer", "description": "Max results to return", "default": 50},
            "offset": {"type": "integer", "description": "Offset for pagination", "default": 0}
        },
        "required": ["query"]
    }),
},
```

- [ ] **Step 7: Run test to verify it passes**

Run: `cargo test test_memory_fts_search -- --nocapture`
Expected: PASS

- [ ] **Step 8: Commit**

```bash
git add src/schema.sql src/tools/memory.rs src/mcp.rs tests/memory_test.rs
git commit -m "feat: add FTS5 full-text search for memory with pagination"
```

---

### Task 2: Add Pagination to All List Endpoints

**Files:**
- Modify: `src/tools/tasks.rs`
- Modify: `src/tools/messages.rs`
- Modify: `src/tools/agents.rs`
- Modify: `src/tools/changes.rs`
- Modify: `src/mcp.rs`
- Create: `tests/pagination_test.rs`

- [ ] **Step 1: Write failing test for task list pagination**

```rust
// tests/pagination_test.rs
use rusqlite::Connection;
use maximous::db;
use maximous::tools;

fn setup() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    db::init_db(&conn).unwrap();
    conn
}

#[test]
fn test_task_list_pagination() {
    let conn = setup();
    for i in 0..10 {
        tools::tasks::create(
            &serde_json::json!({"title": format!("Task {}", i)}),
            &conn,
        );
    }
    // Get first page
    let result = tools::tasks::list(
        &serde_json::json!({"limit": 3, "offset": 0}),
        &conn,
    );
    assert!(result.ok);
    let data = result.data.unwrap();
    assert_eq!(data["tasks"].as_array().unwrap().len(), 3);
    assert_eq!(data["limit"], 3);
    assert_eq!(data["offset"], 0);

    // Get second page
    let result = tools::tasks::list(
        &serde_json::json!({"limit": 3, "offset": 3}),
        &conn,
    );
    assert!(result.ok);
    let data = result.data.unwrap();
    assert_eq!(data["tasks"].as_array().unwrap().len(), 3);
    assert_eq!(data["offset"], 3);
}

#[test]
fn test_agent_list_pagination() {
    let conn = setup();
    for i in 0..5 {
        tools::agents::register(
            &serde_json::json!({"id": format!("agent-{}", i), "name": format!("Agent {}", i)}),
            &conn,
        );
    }
    let result = tools::agents::list(
        &serde_json::json!({"include_stale": true, "limit": 2, "offset": 0}),
        &conn,
    );
    assert!(result.ok);
    let data = result.data.unwrap();
    assert_eq!(data["agents"].as_array().unwrap().len(), 2);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test test_task_list_pagination test_agent_list_pagination -- --nocapture`
Expected: FAIL — limit/offset not respected

- [ ] **Step 3: Add pagination to tasks::list**

In `src/tools/tasks.rs`, update the `list` function to accept `limit` and `offset`:

```rust
pub fn list(args: &Value, conn: &Connection) -> ToolResult {
    let status = args["status"].as_str();
    let assigned_to = args["assigned_to"].as_str();
    let limit = args["limit"].as_i64().unwrap_or(100);
    let offset = args["offset"].as_i64().unwrap_or(0);

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

    params.push(Box::new(limit));
    params.push(Box::new(offset));

    let sql = format!(
        "SELECT id, title, status, priority, assigned_to, dependencies, result, created_at, updated_at
         FROM tasks {} ORDER BY priority ASC, created_at ASC LIMIT ? OFFSET ?",
        where_clause
    );

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
    ToolResult::success(serde_json::json!({
        "tasks": tasks,
        "count": count,
        "limit": limit,
        "offset": offset,
    }))
}
```

- [ ] **Step 4: Add pagination to agents::list**

In `src/tools/agents.rs`, update the `list` function:

```rust
pub fn list(args: &Value, conn: &Connection) -> ToolResult {
    let include_stale = args["include_stale"].as_bool().unwrap_or(false);
    let limit = args["limit"].as_i64().unwrap_or(100);
    let offset = args["offset"].as_i64().unwrap_or(0);

    let cutoff = now() - 60;

    let (sql, params): (String, Vec<Box<dyn rusqlite::types::ToSql>>) = if include_stale {
        (
            "SELECT id, name, status, capabilities, metadata, last_heartbeat FROM agents ORDER BY name LIMIT ? OFFSET ?".to_string(),
            vec![Box::new(limit), Box::new(offset)],
        )
    } else {
        (
            "SELECT id, name, status, capabilities, metadata, last_heartbeat FROM agents WHERE last_heartbeat > ? ORDER BY name LIMIT ? OFFSET ?".to_string(),
            vec![Box::new(cutoff), Box::new(limit), Box::new(offset)],
        )
    };

    let mut stmt = conn.prepare(&sql).unwrap();
    let params_ref: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let agents: Vec<Value> = stmt
        .query_map(params_ref.as_slice(), |row| {
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
        .collect();

    ToolResult::success(serde_json::json!({
        "agents": agents,
        "count": agents.len(),
        "limit": limit,
        "offset": offset,
    }))
}
```

- [ ] **Step 5: Update tool definitions in mcp.rs for pagination params**

Add `limit` and `offset` properties to `task_list`, `agent_list`, and `message_read` tool definitions in `src/mcp.rs`:

For `task_list`:
```json
"limit": {"type": "integer", "description": "Max results to return", "default": 100},
"offset": {"type": "integer", "description": "Offset for pagination", "default": 0}
```

For `agent_list`:
```json
"limit": {"type": "integer", "description": "Max results to return", "default": 100},
"offset": {"type": "integer", "description": "Offset for pagination", "default": 0}
```

For `message_read` — already has `limit`, add `offset`:
```json
"offset": {"type": "integer", "description": "Offset for pagination", "default": 0}
```

- [ ] **Step 6: Add offset to messages::read**

In `src/tools/messages.rs`, update the `read` function to accept `offset`:

Add after `let limit = ...`:
```rust
let offset = args["offset"].as_i64().unwrap_or(0);
```

Append `OFFSET ?3` to both SQL strings, and add `offset` to params:
```rust
let sql = if unacked_only {
    "SELECT id, channel, sender, priority, content, acknowledged, created_at
     FROM messages WHERE channel = ?1 AND acknowledged = 0
     ORDER BY priority ASC, created_at ASC LIMIT ?2 OFFSET ?3"
} else {
    "SELECT id, channel, sender, priority, content, acknowledged, created_at
     FROM messages WHERE channel = ?1
     ORDER BY priority ASC, created_at ASC LIMIT ?2 OFFSET ?3"
};
// ... query_map with params![channel, limit, offset]
```

- [ ] **Step 7: Run all tests**

Run: `cargo test -- --nocapture`
Expected: ALL PASS

- [ ] **Step 8: Commit**

```bash
git add src/tools/tasks.rs src/tools/agents.rs src/tools/messages.rs src/tools/changes.rs src/mcp.rs tests/pagination_test.rs
git commit -m "feat: add pagination (limit/offset) to all list endpoints"
```

---

### Task 3: Typed Observations for Memory

**Files:**
- Modify: `src/schema.sql`
- Modify: `src/tools/memory.rs`
- Modify: `src/mcp.rs`
- Create: `tests/observation_test.rs`

- [ ] **Step 1: Write failing test for typed observations**

```rust
// tests/observation_test.rs
use rusqlite::Connection;
use maximous::db;
use maximous::tools;

fn setup() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    db::init_db(&conn).unwrap();
    conn
}

#[test]
fn test_memory_set_with_observation_type() {
    let conn = setup();
    let result = tools::memory::set(
        &serde_json::json!({
            "namespace": "project",
            "key": "use-axum",
            "value": "Decided to use axum for the web server",
            "observation_type": "decision",
            "category": "architecture"
        }),
        &conn,
    );
    assert!(result.ok);

    let result = tools::memory::get(
        &serde_json::json!({"namespace": "project", "key": "use-axum"}),
        &conn,
    );
    assert!(result.ok);
    let data = result.data.unwrap();
    assert_eq!(data["observation_type"], "decision");
    assert_eq!(data["category"], "architecture");
}

#[test]
fn test_memory_search_filter_by_type() {
    let conn = setup();
    tools::memory::set(
        &serde_json::json!({
            "namespace": "ns", "key": "k1", "value": "some error occurred",
            "observation_type": "error"
        }),
        &conn,
    );
    tools::memory::set(
        &serde_json::json!({
            "namespace": "ns", "key": "k2", "value": "user prefers dark mode",
            "observation_type": "preference"
        }),
        &conn,
    );
    let result = tools::memory::search(
        &serde_json::json!({"query": "mode", "observation_type": "preference"}),
        &conn,
    );
    assert!(result.ok);
    let data = result.data.unwrap();
    assert_eq!(data["matches"].as_array().unwrap().len(), 1);
    assert_eq!(data["matches"][0]["key"], "k2");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test test_memory_set_with_observation_type test_memory_search_filter_by_type -- --nocapture`
Expected: FAIL

- [ ] **Step 3: Add columns to memory table in schema.sql**

Add new columns to the memory table (after `updated_at`):

```sql
CREATE TABLE IF NOT EXISTS memory (
    namespace TEXT NOT NULL,
    key TEXT NOT NULL,
    value TEXT NOT NULL,
    ttl_seconds INTEGER,
    observation_type TEXT,  -- decision, error, preference, insight, pattern, learning
    category TEXT,          -- architecture, debugging, workflow, api, ui, data, config
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (namespace, key)
);
```

Note: Since SQLite uses `CREATE TABLE IF NOT EXISTS`, adding columns to an existing DB requires `ALTER TABLE`. Add migration logic in `db.rs`:

```rust
pub fn init_db(conn: &Connection) -> Result<()> {
    conn.pragma_update(None, "journal_mode", "wal")?;
    conn.pragma_update(None, "busy_timeout", 5000)?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.execute_batch(SCHEMA)?;
    migrate(conn)?;
    Ok(())
}

fn migrate(conn: &Connection) -> Result<()> {
    // Add observation_type column if missing
    let has_obs_type: bool = conn
        .prepare("SELECT observation_type FROM memory LIMIT 0")
        .is_ok();
    if !has_obs_type {
        conn.execute_batch(
            "ALTER TABLE memory ADD COLUMN observation_type TEXT;
             ALTER TABLE memory ADD COLUMN category TEXT;"
        )?;
    }
    Ok(())
}
```

- [ ] **Step 4: Update memory::set to store observation_type and category**

In `src/tools/memory.rs`, update the `set` function to accept and store the new fields:

```rust
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
```

- [ ] **Step 5: Update memory::get to return observation_type and category**

In the `get` function, update the single-key query:

```rust
Some(key) => {
    let result: Result<(String, Option<i64>, i64, Option<String>, Option<String>), _> = conn.query_row(
        "SELECT value, ttl_seconds, updated_at, observation_type, category FROM memory WHERE namespace = ?1 AND key = ?2",
        rusqlite::params![namespace, key],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
    );
    match result {
        Ok((value, ttl, updated_at, obs_type, cat)) => ToolResult::success(serde_json::json!({
            "namespace": namespace,
            "key": key,
            "value": value,
            "ttl_seconds": ttl,
            "updated_at": updated_at,
            "observation_type": obs_type,
            "category": cat,
        })),
        Err(_) => ToolResult::success(serde_json::json!({
            "namespace": namespace,
            "key": key,
            "value": null,
        })),
    }
}
```

- [ ] **Step 6: Update memory search to filter by observation_type**

In the FTS search function, add optional observation_type filter:

```rust
// In search(), before calling search_fts:
let observation_type = args["observation_type"].as_str();
// Pass to search_fts and search_like
```

Add `AND m.observation_type = ?` clause when observation_type is provided.

- [ ] **Step 7: Update tool definitions in mcp.rs**

Add to `memory_set`:
```json
"observation_type": {"type": "string", "description": "Type: decision, error, preference, insight, pattern, learning"},
"category": {"type": "string", "description": "Category: architecture, debugging, workflow, api, ui, data, config"}
```

Add to `memory_search`:
```json
"observation_type": {"type": "string", "description": "Filter by observation type"}
```

- [ ] **Step 8: Run all tests**

Run: `cargo test -- --nocapture`
Expected: ALL PASS

- [ ] **Step 9: Commit**

```bash
git add src/schema.sql src/db.rs src/tools/memory.rs src/mcp.rs tests/observation_test.rs
git commit -m "feat: add typed observations (observation_type, category) to memory"
```

---

## Chunk 2: Progressive Disclosure, Token Estimation & Privacy

### Task 4: Progressive Disclosure — memory_search_index Tool

**Files:**
- Modify: `src/tools/memory.rs`
- Modify: `src/tools/mod.rs`
- Modify: `src/mcp.rs`
- Create: `tests/progressive_test.rs`

- [ ] **Step 1: Write failing test for search_index**

```rust
// tests/progressive_test.rs
use rusqlite::Connection;
use maximous::db;
use maximous::tools;

fn setup() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    db::init_db(&conn).unwrap();
    conn
}

#[test]
fn test_memory_search_index_returns_compact() {
    let conn = setup();
    let long_value = "x".repeat(5000);
    tools::memory::set(
        &serde_json::json!({"namespace": "docs", "key": "big-doc", "value": long_value}),
        &conn,
    );
    // search_index should return compact entries without full value
    let result = tools::memory::search_index(
        &serde_json::json!({"query": "xxx"}),
        &conn,
    );
    assert!(result.ok);
    let data = result.data.unwrap();
    let matches = data["matches"].as_array().unwrap();
    assert_eq!(matches.len(), 1);
    // Should have snippet (truncated), not full value
    let snippet = matches[0]["snippet"].as_str().unwrap();
    assert!(snippet.len() <= 200);
    // Should have estimated token count
    assert!(matches[0]["estimated_tokens"].as_i64().unwrap() > 0);
    // Should NOT have full "value" field
    assert!(matches[0].get("value").is_none());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test test_memory_search_index_returns_compact -- --nocapture`
Expected: FAIL — `search_index` function doesn't exist

- [ ] **Step 3: Implement search_index function**

In `src/tools/memory.rs`, add:

```rust
pub fn search_index(args: &Value, conn: &Connection) -> ToolResult {
    let query = match args["query"].as_str() {
        Some(s) => s,
        None => return ToolResult::fail("missing required field: query"),
    };
    let namespace = args["namespace"].as_str();
    let observation_type = args["observation_type"].as_str();
    let limit = args["limit"].as_i64().unwrap_or(20);
    let offset = args["offset"].as_i64().unwrap_or(0);

    // Use FTS if available, fall back to LIKE
    let (sql, params): (String, Vec<Box<dyn rusqlite::types::ToSql>>) = {
        let mut conditions = vec!["memory_fts MATCH ?".to_string()];
        let mut p: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(query.to_string())];

        if let Some(ns) = namespace {
            conditions.push("m.namespace = ?".to_string());
            p.push(Box::new(ns.to_string()));
        }
        if let Some(ot) = observation_type {
            conditions.push("m.observation_type = ?".to_string());
            p.push(Box::new(ot.to_string()));
        }
        p.push(Box::new(limit));
        p.push(Box::new(offset));

        (
            format!(
                "SELECT m.namespace, m.key, SUBSTR(m.value, 1, 150) as snippet,
                        LENGTH(m.value) as value_len, m.observation_type, m.category, rank
                 FROM memory_fts f
                 JOIN memory m ON m.rowid = f.rowid
                 WHERE {}
                 ORDER BY rank
                 LIMIT ? OFFSET ?",
                conditions.join(" AND ")
            ),
            p,
        )
    };

    let mut stmt = match conn.prepare(&sql) {
        Ok(s) => s,
        Err(_) => {
            // Fallback to LIKE-based compact search
            return search_index_like(query, namespace, limit, offset, conn);
        }
    };

    let params_ref: Vec<&dyn rusqlite::types::ToSql> =
        params.iter().map(|p| p.as_ref()).collect();
    let matches: Vec<Value> = stmt
        .query_map(params_ref.as_slice(), |row| {
            let snippet: String = row.get(2)?;
            let value_len: i64 = row.get(3)?;
            let estimated_tokens = value_len / 4; // ~4 chars per token
            Ok(serde_json::json!({
                "namespace": row.get::<_, String>(0)?,
                "key": row.get::<_, String>(1)?,
                "snippet": if value_len > 150 { format!("{}...", snippet) } else { snippet },
                "estimated_tokens": estimated_tokens,
                "observation_type": row.get::<_, Option<String>>(4)?,
                "category": row.get::<_, Option<String>>(5)?,
                "rank": row.get::<_, f64>(6)?,
            }))
        })
        .unwrap_or_else(|_| Vec::new().into_iter())
        .filter_map(|r| r.ok())
        .collect();

    ToolResult::success(serde_json::json!({
        "matches": matches,
        "count": matches.len(),
        "offset": offset,
        "limit": limit,
        "hint": "Use memory_get with namespace+key to retrieve full values"
    }))
}

fn search_index_like(
    query: &str,
    namespace: Option<&str>,
    limit: i64,
    offset: i64,
    conn: &Connection,
) -> ToolResult {
    let (sql, params): (String, Vec<Box<dyn rusqlite::types::ToSql>>) = match namespace {
        Some(ns) => (
            "SELECT namespace, key, SUBSTR(value, 1, 150), LENGTH(value), observation_type, category
             FROM memory WHERE namespace = ? AND value LIKE ? LIMIT ? OFFSET ?".to_string(),
            vec![
                Box::new(ns.to_string()),
                Box::new(format!("%{}%", query)),
                Box::new(limit),
                Box::new(offset),
            ],
        ),
        None => (
            "SELECT namespace, key, SUBSTR(value, 1, 150), LENGTH(value), observation_type, category
             FROM memory WHERE value LIKE ? LIMIT ? OFFSET ?".to_string(),
            vec![
                Box::new(format!("%{}%", query)),
                Box::new(limit),
                Box::new(offset),
            ],
        ),
    };

    let mut stmt = conn.prepare(&sql).unwrap();
    let params_ref: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let matches: Vec<Value> = stmt
        .query_map(params_ref.as_slice(), |row| {
            let snippet: String = row.get(2)?;
            let value_len: i64 = row.get(3)?;
            Ok(serde_json::json!({
                "namespace": row.get::<_, String>(0)?,
                "key": row.get::<_, String>(1)?,
                "snippet": if value_len > 150 { format!("{}...", snippet) } else { snippet },
                "estimated_tokens": value_len / 4,
                "observation_type": row.get::<_, Option<String>>(4)?,
                "category": row.get::<_, Option<String>>(5)?,
            }))
        })
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();

    ToolResult::success(serde_json::json!({
        "matches": matches,
        "count": matches.len(),
        "offset": offset,
        "limit": limit,
        "hint": "Use memory_get with namespace+key to retrieve full values"
    }))
}
```

- [ ] **Step 4: Register the new tool**

In `src/tools/mod.rs`, add to dispatch:
```rust
"memory_search_index" => memory::search_index(args, conn),
```

In `src/mcp.rs`, add tool definition:
```rust
ToolDef {
    name: "memory_search_index".into(),
    description: "Search memory returning compact index (snippet + token estimate). Use memory_get to fetch full values. 10x more token-efficient than memory_search.".into(),
    input_schema: serde_json::json!({
        "type": "object",
        "properties": {
            "query": {"type": "string", "description": "Search query"},
            "namespace": {"type": "string", "description": "Optional namespace filter"},
            "observation_type": {"type": "string", "description": "Filter by observation type"},
            "limit": {"type": "integer", "description": "Max results", "default": 20},
            "offset": {"type": "integer", "description": "Offset for pagination", "default": 0}
        },
        "required": ["query"]
    }),
},
```

- [ ] **Step 5: Run tests**

Run: `cargo test -- --nocapture`
Expected: ALL PASS

- [ ] **Step 6: Commit**

```bash
git add src/tools/memory.rs src/tools/mod.rs src/mcp.rs tests/progressive_test.rs
git commit -m "feat: add memory_search_index for progressive disclosure (10x token savings)"
```

---

### Task 5: Privacy Tags Support

**Files:**
- Modify: `src/tools/memory.rs`
- Create: `tests/privacy_test.rs`

- [ ] **Step 1: Write failing test for privacy filtering**

```rust
// tests/privacy_test.rs
use rusqlite::Connection;
use maximous::db;
use maximous::tools;

fn setup() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    db::init_db(&conn).unwrap();
    conn
}

#[test]
fn test_private_tags_stripped_on_read() {
    let conn = setup();
    tools::memory::set(
        &serde_json::json!({
            "namespace": "notes",
            "key": "api-keys",
            "value": "Use the API at api.example.com. <private>API_KEY=sk-secret123</private> The rate limit is 100/min."
        }),
        &conn,
    );
    let result = tools::memory::get(
        &serde_json::json!({"namespace": "notes", "key": "api-keys"}),
        &conn,
    );
    assert!(result.ok);
    let data = result.data.unwrap();
    let value = data["value"].as_str().unwrap();
    assert!(!value.contains("sk-secret123"));
    assert!(!value.contains("<private>"));
    assert!(value.contains("api.example.com"));
    assert!(value.contains("[REDACTED]"));
}

#[test]
fn test_private_tags_stripped_in_search() {
    let conn = setup();
    tools::memory::set(
        &serde_json::json!({
            "namespace": "ns",
            "key": "k",
            "value": "public info <private>secret</private> more public"
        }),
        &conn,
    );
    let result = tools::memory::search(
        &serde_json::json!({"query": "public"}),
        &conn,
    );
    assert!(result.ok);
    let matches = result.data.unwrap();
    let value = matches["matches"][0]["value"].as_str().unwrap();
    assert!(!value.contains("secret"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test test_private_tags -- --nocapture`
Expected: FAIL

- [ ] **Step 3: Add privacy stripping helper**

In `src/tools/memory.rs`, add a helper function:

```rust
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
```

- [ ] **Step 4: Apply strip_private to get and search outputs**

In the `get` function, wrap the value:
```rust
"value": strip_private(&value),
```

In the `search` / `search_fts` / `search_like` functions, apply to value output:
```rust
"value": strip_private(&row.get::<_, String>(2)?),
```

In `search_index` / `search_index_like`, apply to snippet.

- [ ] **Step 5: Run tests**

Run: `cargo test -- --nocapture`
Expected: ALL PASS

- [ ] **Step 6: Commit**

```bash
git add src/tools/memory.rs tests/privacy_test.rs
git commit -m "feat: add <private> tag support for redacting sensitive data in memory"
```

---

### Task 6: Session Tracking

**Files:**
- Modify: `src/schema.sql`
- Modify: `src/db.rs`
- Create: `src/tools/sessions.rs`
- Modify: `src/tools/mod.rs`
- Modify: `src/mcp.rs`
- Create: `tests/sessions_test.rs`

- [ ] **Step 1: Write failing test for session tracking**

```rust
// tests/sessions_test.rs
use rusqlite::Connection;
use maximous::db;
use maximous::tools;

fn setup() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    db::init_db(&conn).unwrap();
    conn
}

#[test]
fn test_session_start_and_end() {
    let conn = setup();
    let result = tools::sessions::start(
        &serde_json::json!({"agent_id": "agent-1", "metadata": "{\"project\": \"maximous\"}"}),
        &conn,
    );
    assert!(result.ok);
    let data = result.data.unwrap();
    let session_id = data["id"].as_str().unwrap().to_string();

    let result = tools::sessions::end(
        &serde_json::json!({"id": session_id, "summary": "Implemented FTS5 search"}),
        &conn,
    );
    assert!(result.ok);
}

#[test]
fn test_session_list() {
    let conn = setup();
    tools::sessions::start(
        &serde_json::json!({"agent_id": "agent-1"}),
        &conn,
    );
    tools::sessions::start(
        &serde_json::json!({"agent_id": "agent-2"}),
        &conn,
    );
    let result = tools::sessions::list(
        &serde_json::json!({}),
        &conn,
    );
    assert!(result.ok);
    let data = result.data.unwrap();
    assert_eq!(data["sessions"].as_array().unwrap().len(), 2);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test test_session -- --nocapture`
Expected: FAIL — module doesn't exist

- [ ] **Step 3: Add sessions table to schema.sql**

```sql
-- Session tracking
CREATE TABLE IF NOT EXISTS sessions (
    id TEXT PRIMARY KEY,
    agent_id TEXT,
    status TEXT NOT NULL DEFAULT 'active',
    metadata TEXT,
    summary TEXT,
    started_at INTEGER NOT NULL,
    ended_at INTEGER
);

CREATE INDEX IF NOT EXISTS idx_sessions_agent ON sessions(agent_id);
CREATE INDEX IF NOT EXISTS idx_sessions_status ON sessions(status);

-- Session change triggers
CREATE TRIGGER IF NOT EXISTS trg_sessions_insert AFTER INSERT ON sessions
BEGIN
    INSERT INTO changes (table_name, row_id, action, summary, created_at)
    VALUES ('sessions', NEW.id, 'insert',
            json_object('agent_id', NEW.agent_id, 'status', NEW.status),
            strftime('%s', 'now'));
END;

CREATE TRIGGER IF NOT EXISTS trg_sessions_update AFTER UPDATE ON sessions
BEGIN
    INSERT INTO changes (table_name, row_id, action, summary, created_at)
    VALUES ('sessions', NEW.id, 'update',
            json_object('agent_id', NEW.agent_id, 'status', NEW.status),
            strftime('%s', 'now'));
END;
```

- [ ] **Step 4: Create src/tools/sessions.rs**

```rust
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
            "started_at": now(),
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
```

- [ ] **Step 5: Register session tools**

In `src/tools/mod.rs`:
```rust
pub mod sessions;
```

Add to dispatch:
```rust
"session_start" => sessions::start(args, conn),
"session_end" => sessions::end(args, conn),
"session_list" => sessions::list(args, conn),
```

Add tool definitions in `src/mcp.rs` (3 new ToolDef entries for session_start, session_end, session_list).

- [ ] **Step 6: Run tests**

Run: `cargo test -- --nocapture`
Expected: ALL PASS

- [ ] **Step 7: Commit**

```bash
git add src/schema.sql src/tools/sessions.rs src/tools/mod.rs src/mcp.rs tests/sessions_test.rs
git commit -m "feat: add session tracking (start, end, list) with change triggers"
```

---

## Chunk 3: Web Dashboard — Backend (axum)

### Task 7: Add Web Server Dependencies & Skeleton

**Files:**
- Modify: `Cargo.toml`
- Create: `src/web/mod.rs`
- Create: `src/web/api.rs`
- Modify: `src/main.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: Add dependencies to Cargo.toml**

```toml
[dependencies]
rusqlite = { version = "0.31", features = ["bundled"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }
clap = { version = "4", features = ["derive"] }
uuid = { version = "1", features = ["v4"] }
axum = "0.7"
tower-http = { version = "0.5", features = ["cors"] }
rust-embed = "8"
```

- [ ] **Step 2: Create web module skeleton**

Create `src/web/mod.rs`:

```rust
pub mod api;

use axum::{Router, routing::get, response::Html};
use rust_embed::Embed;
use std::sync::{Arc, Mutex};
use rusqlite::Connection;
use tower_http::cors::{CorsLayer, Any};

#[derive(Embed)]
#[folder = "web/"]
struct Assets;

pub type DbState = Arc<Mutex<Connection>>;

pub fn create_router(db: DbState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/", get(index_handler))
        .route("/app.js", get(js_handler))
        .route("/style.css", get(css_handler))
        .route("/api/overview", get(api::overview))
        .route("/api/agents", get(api::agents))
        .route("/api/tasks", get(api::tasks))
        .route("/api/messages", get(api::messages))
        .route("/api/memory", get(api::memory))
        .route("/api/sessions", get(api::sessions))
        .route("/api/changes", get(api::changes))
        .route("/api/events", get(api::events_sse))
        .layer(cors)
        .with_state(db)
}

async fn index_handler() -> Html<String> {
    match Assets::get("index.html") {
        Some(content) => Html(String::from_utf8_lossy(content.data.as_ref()).to_string()),
        None => Html("<h1>maximous dashboard</h1><p>Assets not found</p>".to_string()),
    }
}

async fn js_handler() -> ([(axum::http::header::HeaderName, &'static str); 1], String) {
    let content = Assets::get("app.js")
        .map(|f| String::from_utf8_lossy(f.data.as_ref()).to_string())
        .unwrap_or_default();
    ([(axum::http::header::CONTENT_TYPE, "application/javascript")], content)
}

async fn css_handler() -> ([(axum::http::header::HeaderName, &'static str); 1], String) {
    let content = Assets::get("style.css")
        .map(|f| String::from_utf8_lossy(f.data.as_ref()).to_string())
        .unwrap_or_default();
    ([(axum::http::header::CONTENT_TYPE, "text/css")], content)
}

pub async fn serve(db: DbState, port: u16) {
    let app = create_router(db);
    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port))
        .await
        .expect("Failed to bind web server");
    eprintln!("maximous dashboard: http://127.0.0.1:{}", port);
    axum::serve(listener, app).await.expect("Web server error");
}
```

- [ ] **Step 3: Create API handlers skeleton**

Create `src/web/api.rs`:

```rust
use axum::{
    extract::State,
    extract::Query,
    response::sse::{Event, Sse},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::convert::Infallible;
use tokio_stream::StreamExt;
use super::DbState;

#[derive(Deserialize, Default)]
pub struct PaginationParams {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Deserialize, Default)]
pub struct MemoryParams {
    pub namespace: Option<String>,
    pub query: Option<String>,
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
    let agent_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM agents", [], |r| r.get(0))
        .unwrap_or(0);
    let task_counts: Vec<(String, i64)> = {
        let mut stmt = conn
            .prepare("SELECT status, COUNT(*) FROM tasks GROUP BY status")
            .unwrap();
        stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect()
    };
    let unacked_messages: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM messages WHERE acknowledged = 0",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let memory_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM memory", [], |r| r.get(0))
        .unwrap_or(0);
    let session_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sessions WHERE status = 'active'",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);

    let tasks_by_status: Value = task_counts
        .into_iter()
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

pub async fn agents(
    State(db): State<DbState>,
    Query(params): Query<PaginationParams>,
) -> Json<Value> {
    let conn = db.lock().unwrap();
    let limit = params.limit.unwrap_or(100);
    let offset = params.offset.unwrap_or(0);
    let mut stmt = conn
        .prepare(
            "SELECT id, name, status, capabilities, metadata, last_heartbeat
             FROM agents ORDER BY name LIMIT ?1 OFFSET ?2",
        )
        .unwrap();
    let agents: Vec<Value> = stmt
        .query_map(rusqlite::params![limit, offset], |row| {
            Ok(json!({
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
        .collect();
    Json(json!({"agents": agents, "count": agents.len()}))
}

pub async fn tasks(
    State(db): State<DbState>,
    Query(params): Query<PaginationParams>,
) -> Json<Value> {
    let conn = db.lock().unwrap();
    let limit = params.limit.unwrap_or(100);
    let offset = params.offset.unwrap_or(0);
    let mut stmt = conn
        .prepare(
            "SELECT id, title, status, priority, assigned_to, dependencies, result, created_at, updated_at
             FROM tasks ORDER BY priority ASC, created_at ASC LIMIT ?1 OFFSET ?2",
        )
        .unwrap();
    let tasks: Vec<Value> = stmt
        .query_map(rusqlite::params![limit, offset], |row| {
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
        })
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();
    Json(json!({"tasks": tasks, "count": tasks.len()}))
}

pub async fn messages(
    State(db): State<DbState>,
    Query(params): Query<MessagesParams>,
) -> Json<Value> {
    let conn = db.lock().unwrap();
    let limit = params.limit.unwrap_or(50);
    let offset = params.offset.unwrap_or(0);

    let (sql, sql_params): (String, Vec<Box<dyn rusqlite::types::ToSql>>) = match &params.channel {
        Some(ch) => (
            "SELECT id, channel, sender, priority, content, acknowledged, created_at
             FROM messages WHERE channel = ?1
             ORDER BY created_at DESC LIMIT ?2 OFFSET ?3".to_string(),
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
    let messages: Vec<Value> = stmt
        .query_map(params_ref.as_slice(), |row| {
            Ok(json!({
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

    // Also get list of channels
    let channels: Vec<String> = conn
        .prepare("SELECT DISTINCT channel FROM messages ORDER BY channel")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();

    Json(json!({"messages": messages, "count": messages.len(), "channels": channels}))
}

pub async fn memory(
    State(db): State<DbState>,
    Query(params): Query<MemoryParams>,
) -> Json<Value> {
    let conn = db.lock().unwrap();
    let limit = params.limit.unwrap_or(50);
    let offset = params.offset.unwrap_or(0);

    // Get namespaces
    let namespaces: Vec<String> = conn
        .prepare("SELECT DISTINCT namespace FROM memory ORDER BY namespace")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();

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
    let entries: Vec<Value> = stmt
        .query_map(params_ref.as_slice(), |row| {
            Ok(json!({
                "namespace": row.get::<_, String>(0)?,
                "key": row.get::<_, String>(1)?,
                "value": row.get::<_, String>(2)?,
                "observation_type": row.get::<_, Option<String>>(3)?,
                "category": row.get::<_, Option<String>>(4)?,
                "updated_at": row.get::<_, i64>(5)?,
            }))
        })
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();

    Json(json!({
        "entries": entries,
        "count": entries.len(),
        "namespaces": namespaces,
    }))
}

pub async fn sessions(
    State(db): State<DbState>,
    Query(params): Query<PaginationParams>,
) -> Json<Value> {
    let conn = db.lock().unwrap();
    let limit = params.limit.unwrap_or(50);
    let offset = params.offset.unwrap_or(0);
    let mut stmt = conn
        .prepare(
            "SELECT id, agent_id, status, metadata, summary, started_at, ended_at
             FROM sessions ORDER BY started_at DESC LIMIT ?1 OFFSET ?2",
        )
        .unwrap();
    let sessions: Vec<Value> = stmt
        .query_map(rusqlite::params![limit, offset], |row| {
            Ok(json!({
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
    Json(json!({"sessions": sessions, "count": sessions.len()}))
}

pub async fn changes(
    State(db): State<DbState>,
    Query(params): Query<ChangesParams>,
) -> Json<Value> {
    let conn = db.lock().unwrap();
    let since_id = params.since_id.unwrap_or(0);
    let limit = params.limit.unwrap_or(100);

    let (sql, sql_params): (String, Vec<Box<dyn rusqlite::types::ToSql>>) = match &params.table_name {
        Some(tn) => (
            "SELECT id, table_name, row_id, action, summary, created_at
             FROM changes WHERE id > ?1 AND table_name = ?2
             ORDER BY id DESC LIMIT ?3".to_string(),
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
    let changes: Vec<Value> = stmt
        .query_map(params_ref.as_slice(), |row| {
            Ok(json!({
                "id": row.get::<_, i64>(0)?,
                "table_name": row.get::<_, String>(1)?,
                "row_id": row.get::<_, String>(2)?,
                "action": row.get::<_, String>(3)?,
                "summary": row.get::<_, Option<String>>(4)?,
                "created_at": row.get::<_, i64>(5)?,
            }))
        })
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();
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
                let mut stmt = conn
                    .prepare(
                        "SELECT id, table_name, row_id, action, summary, created_at
                         FROM changes WHERE id > ?1 ORDER BY id ASC LIMIT 50",
                    )
                    .unwrap();
                stmt.query_map(rusqlite::params![last_id], |row| {
                    Ok(json!({
                        "id": row.get::<_, i64>(0)?,
                        "table_name": row.get::<_, String>(1)?,
                        "row_id": row.get::<_, String>(2)?,
                        "action": row.get::<_, String>(3)?,
                        "summary": row.get::<_, Option<String>>(4)?,
                        "created_at": row.get::<_, i64>(5)?,
                    }))
                })
                .unwrap()
                .filter_map(|r| r.ok())
                .collect()
            };

            for change in &new_changes {
                if let Some(id) = change["id"].as_i64() {
                    if id > last_id {
                        last_id = id;
                    }
                }
                yield Ok(Event::default().data(change.to_string()));
            }
        }
    };

    Sse::new(stream)
}
```

- [ ] **Step 4: Add async-stream and tokio-stream to Cargo.toml**

```toml
async-stream = "0.3"
tokio-stream = "0.1"
```

- [ ] **Step 5: Update main.rs to support --web flag**

```rust
mod db;
mod mcp;
mod tools;
mod web;

use clap::Parser;
use std::sync::{Arc, Mutex};

#[derive(Parser)]
#[command(name = "maximous", about = "SQLite brain for multi-agent orchestration")]
struct Cli {
    /// Path to the SQLite database file
    #[arg(long, default_value = ".maximous/brain.db")]
    db: String,

    /// Enable web dashboard
    #[arg(long)]
    web: bool,

    /// Web dashboard port
    #[arg(long, default_value = "8375")]
    port: u16,
}

fn main() {
    let cli = Cli::parse();
    let conn = db::open_db(&cli.db).expect("Failed to open database");
    eprintln!("maximous: database ready at {}", cli.db);

    let conn = Arc::new(Mutex::new(conn));

    if cli.web {
        // Run web server only
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(web::serve(conn, cli.port));
    } else {
        // Run MCP stdio server (default)
        mcp::run_stdio(conn);
    }
}
```

- [ ] **Step 6: Update lib.rs**

```rust
pub mod db;
pub mod mcp;
pub mod tools;
pub mod web;
```

- [ ] **Step 7: Create placeholder web assets**

Create `web/` directory with empty placeholder files so rust-embed compiles:
- `web/index.html` — placeholder `<h1>maximous</h1>`
- `web/app.js` — empty
- `web/style.css` — empty

- [ ] **Step 8: Verify it compiles**

Run: `cargo build`
Expected: Compiles without errors

- [ ] **Step 9: Commit**

```bash
git add Cargo.toml src/main.rs src/lib.rs src/web/ web/
git commit -m "feat: add axum web server skeleton with API endpoints and SSE"
```

---

## Chunk 4: Web Dashboard — Frontend

### Task 8: Dashboard Frontend — HTML/JS/CSS

**Files:**
- Create: `web/index.html`
- Create: `web/app.js`
- Create: `web/style.css`

- [ ] **Step 1: Create index.html**

```html
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>maximous dashboard</title>
    <link rel="stylesheet" href="/style.css">
</head>
<body>
    <nav id="sidebar">
        <h1>maximous</h1>
        <ul>
            <li><a href="#" data-page="overview" class="active">Overview</a></li>
            <li><a href="#" data-page="agents">Agents</a></li>
            <li><a href="#" data-page="tasks">Tasks</a></li>
            <li><a href="#" data-page="messages">Messages</a></li>
            <li><a href="#" data-page="memory">Memory</a></li>
            <li><a href="#" data-page="sessions">Sessions</a></li>
            <li><a href="#" data-page="activity">Activity</a></li>
        </ul>
    </nav>
    <main id="content">
        <div id="page-overview" class="page active"></div>
        <div id="page-agents" class="page"></div>
        <div id="page-tasks" class="page"></div>
        <div id="page-messages" class="page"></div>
        <div id="page-memory" class="page"></div>
        <div id="page-sessions" class="page"></div>
        <div id="page-activity" class="page"></div>
    </main>
    <script src="/app.js"></script>
</body>
</html>
```

- [ ] **Step 2: Create style.css**

Dark-themed dashboard CSS — ~200 lines covering: sidebar nav, cards grid, tables, status badges, priority badges, responsive layout. Use CSS custom properties for theming.

Key classes:
- `.card` — stat cards on overview
- `.table` — data tables for agents/tasks/messages
- `.badge-*` — status/priority indicators
- `.page.active` — visible page
- `.sidebar` — left navigation
- `.memory-browser` — 3-pane layout for memory

- [ ] **Step 3: Create app.js**

Vanilla JS application (~400 lines) with:

```javascript
// Core: page navigation, API fetching, SSE connection
const API = '';

// Navigation
document.querySelectorAll('[data-page]').forEach(link => {
    link.addEventListener('click', (e) => {
        e.preventDefault();
        navigateTo(link.dataset.page);
    });
});

function navigateTo(page) {
    document.querySelectorAll('.page').forEach(p => p.classList.remove('active'));
    document.querySelectorAll('[data-page]').forEach(l => l.classList.remove('active'));
    document.getElementById('page-' + page).classList.add('active');
    document.querySelector('[data-page="' + page + '"]').classList.add('active');
    loaders[page]();
}

// Page loaders
const loaders = {
    overview: loadOverview,
    agents: loadAgents,
    tasks: loadTasks,
    messages: loadMessages,
    memory: loadMemory,
    sessions: loadSessions,
    activity: loadActivity,
};

async function loadOverview() { /* fetch /api/overview, render cards */ }
async function loadAgents() { /* fetch /api/agents, render table with heartbeat indicators */ }
async function loadTasks() { /* fetch /api/tasks, render table with status badges + dependency info */ }
async function loadMessages() { /* fetch /api/messages, render channel selector + message list */ }
async function loadMemory() { /* fetch /api/memory, render 3-pane namespace/key/value explorer */ }
async function loadSessions() { /* fetch /api/sessions, render table */ }
async function loadActivity() { /* fetch /api/changes, render activity feed */ }

// SSE for real-time updates
const eventSource = new EventSource('/api/events');
eventSource.onmessage = (event) => {
    const change = JSON.parse(event.data);
    // Update active page if relevant
    updateActivityFeed(change);
};

// Helper: format timestamps, render tables, etc.
function formatTime(epoch) {
    return new Date(epoch * 1000).toLocaleString();
}

function renderTable(headers, rows) { /* ... */ }

// Initial load
loadOverview();
```

Each page loader function:
- Fetches from the corresponding API endpoint
- Renders HTML into the page div
- Handles empty states

- [ ] **Step 4: Verify dashboard works end-to-end**

Run: `cargo build && ./target/debug/maximous --db .maximous/brain.db --web --port 8375`
Open: `http://127.0.0.1:8375`
Expected: Dashboard loads, shows overview with stats

- [ ] **Step 5: Commit**

```bash
git add web/index.html web/app.js web/style.css
git commit -m "feat: add web dashboard frontend with 7 views and real-time SSE updates"
```

---

## Chunk 5: Integration, Version Bump & Polish

### Task 9: Update Plugin Configuration

**Files:**
- Modify: `.claude-plugin/plugin.json`
- Modify: `Cargo.toml`

- [ ] **Step 1: Bump version to 0.2.0**

In `Cargo.toml`: `version = "0.2.0"`

- [ ] **Step 2: Update plugin.json description**

Update to mention new features:
```json
{
  "description": "SQLite brain for multi-agent orchestration — shared memory with FTS5 search, typed observations, progressive disclosure, messaging, task coordination, session tracking, and web dashboard"
}
```

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml .claude-plugin/plugin.json
git commit -m "chore: bump version to 0.2.0 with claude-mem-inspired features"
```

---

### Task 10: Run Full Test Suite & Fix Issues

- [ ] **Step 1: Run all tests**

Run: `cargo test -- --nocapture`
Expected: ALL PASS

- [ ] **Step 2: Run clippy**

Run: `cargo clippy -- -D warnings`
Fix any warnings.

- [ ] **Step 3: Build release**

Run: `cargo build --release`
Verify binary size is reasonable.

- [ ] **Step 4: Smoke test the web dashboard**

Run: `./target/release/maximous --db /tmp/test-brain.db --web --port 8375`
Verify: All 7 pages load, SSE events stream when changes occur.

- [ ] **Step 5: Final commit**

```bash
git add -A
git commit -m "fix: address clippy warnings and polish for 0.2.0 release"
```

---

## Summary

| Task | Feature | New Tools | Effort |
|------|---------|-----------|--------|
| 1 | FTS5 full-text search | - | ~30min |
| 2 | Pagination on all endpoints | - | ~30min |
| 3 | Typed observations | - | ~30min |
| 4 | Progressive disclosure | `memory_search_index` | ~30min |
| 5 | Privacy tags | - | ~20min |
| 6 | Session tracking | `session_start`, `session_end`, `session_list` | ~30min |
| 7 | Web server skeleton | - | ~45min |
| 8 | Dashboard frontend | - | ~2hr |
| 9 | Plugin config update | - | ~5min |
| 10 | Test & polish | - | ~30min |

**Total new MCP tools:** 4 (memory_search_index, session_start, session_end, session_list) → 18 total
**Total effort estimate:** ~6-8 hours
