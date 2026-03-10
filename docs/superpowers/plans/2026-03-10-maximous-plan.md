# Maximous Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a Rust MCP server that provides SQLite-backed shared memory, messaging, and task coordination for Claude Code multi-agent orchestration.

**Architecture:** Single Rust binary using `rusqlite` (bundled) for embedded SQLite with WAL mode. Communicates via JSON-RPC over stdio as an MCP server. 14 tools across 5 domains (memory, messages, tasks, agents, changes). SQLite triggers auto-populate a changes table for the observation pattern.

**Tech Stack:** Rust, rusqlite (bundled), serde/serde_json, tokio, clap, uuid

**Spec:** `docs/superpowers/specs/2026-03-10-maximous-design.md`

---

## File Structure

```
maximous/
├── Cargo.toml                    -- Project manifest, dependencies
├── src/
│   ├── main.rs                   -- CLI entry (clap), spawns MCP stdio loop
│   ├── lib.rs                    -- Library root, re-exports db, mcp, tools
│   ├── db.rs                     -- Database init, WAL setup, migrations, triggers
│   ├── schema.sql                -- Full SQL schema embedded via include_str!
│   ├── mcp.rs                    -- JSON-RPC types, stdio reader/writer, tool dispatcher
│   ├── tools/
│   │   ├── mod.rs                -- Tool registry, ToolResult type
│   │   ├── memory.rs             -- memory_set, memory_get, memory_search, memory_delete
│   │   ├── messages.rs           -- message_send, message_read, message_ack
│   │   ├── tasks.rs              -- task_create, task_update, task_list
│   │   ├── agents.rs             -- agent_register, agent_heartbeat, agent_list
│   │   └── changes.rs            -- poll_changes
├── tests/
│   ├── db_test.rs                -- Schema creation, WAL mode, trigger tests
│   ├── memory_test.rs            -- Memory tool integration tests
│   ├── messages_test.rs          -- Message tool integration tests
│   ├── tasks_test.rs             -- Task tool integration tests
│   ├── agents_test.rs            -- Agent tool integration tests
│   ├── changes_test.rs           -- Change log / observation tests
│   ├── integration_test.rs       -- Full multi-agent workflow test
│   └── mcp_test.rs               -- JSON-RPC protocol tests
├── plugin/
│   ├── plugin.json               -- Claude Code plugin manifest
│   └── .mcp.json                 -- MCP server declaration
└── README.md
```

---

## Chunk 1: Project Foundation & Database Layer

### Task 1: Initialize Rust Project

**Files:**
- Create: `Cargo.toml`
- Create: `src/main.rs`

- [ ] **Step 1: Create Cargo.toml**

```toml
[package]
name = "maximous"
version = "0.1.0"
edition = "2021"
description = "Lightweight SQLite brain for multi-agent orchestration"
license = "MIT"

[dependencies]
rusqlite = { version = "0.31", features = ["bundled"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }
clap = { version = "4", features = ["derive"] }
uuid = { version = "1", features = ["v4"] }

[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 2: Create minimal main.rs**

```rust
use clap::Parser;

#[derive(Parser)]
#[command(name = "maximous", about = "SQLite brain for multi-agent orchestration")]
struct Cli {
    /// Path to the SQLite database file
    #[arg(long, default_value = ".maximous/brain.db")]
    db: String,
}

fn main() {
    let cli = Cli::parse();
    println!("maximous starting with db: {}", cli.db);
}
```

- [ ] **Step 3: Create src/lib.rs (library crate for test imports)**

```rust
pub mod db;
pub mod mcp;
pub mod tools;
```

Note: This will fail to compile until `db.rs`, `mcp.rs`, and `tools/mod.rs` exist. That's expected — we create them in subsequent tasks.

- [ ] **Step 4: Verify it compiles**

Run: `cargo build`
Expected: Compiles successfully (or may have warnings about missing modules — that's fine, we add them next).

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml src/main.rs src/lib.rs
git commit -m "feat: initialize Rust project with dependencies"
```

---

### Task 2: SQL Schema

**Files:**
- Create: `src/schema.sql`

- [ ] **Step 1: Write the schema with tables and triggers**

```sql
-- Shared knowledge store
CREATE TABLE IF NOT EXISTS memory (
    namespace TEXT NOT NULL,
    key TEXT NOT NULL,
    value TEXT NOT NULL,
    ttl_seconds INTEGER,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (namespace, key)
);

-- Inter-agent messages
CREATE TABLE IF NOT EXISTS messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    channel TEXT NOT NULL,
    sender TEXT NOT NULL,
    priority INTEGER NOT NULL DEFAULT 2,
    content TEXT NOT NULL,
    acknowledged INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL
);

-- Task coordination
CREATE TABLE IF NOT EXISTS tasks (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    priority INTEGER NOT NULL DEFAULT 2,
    assigned_to TEXT,
    dependencies TEXT,
    result TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

-- Agent registry
CREATE TABLE IF NOT EXISTS agents (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'idle',
    capabilities TEXT,
    metadata TEXT,
    last_heartbeat INTEGER NOT NULL
);

-- Observation / event log
CREATE TABLE IF NOT EXISTS changes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    table_name TEXT NOT NULL,
    row_id TEXT NOT NULL,
    action TEXT NOT NULL,
    summary TEXT,
    created_at INTEGER NOT NULL
);

-- Key-value config
CREATE TABLE IF NOT EXISTS config (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_messages_channel ON messages(channel, created_at);
CREATE INDEX IF NOT EXISTS idx_messages_unacked ON messages(channel, acknowledged) WHERE acknowledged = 0;
CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status, priority DESC);
CREATE INDEX IF NOT EXISTS idx_tasks_assigned ON tasks(assigned_to) WHERE assigned_to IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_agents_heartbeat ON agents(last_heartbeat);
CREATE INDEX IF NOT EXISTS idx_changes_id ON changes(id);
CREATE INDEX IF NOT EXISTS idx_memory_namespace ON memory(namespace);

-- Triggers: auto-populate changes table

CREATE TRIGGER IF NOT EXISTS trg_memory_insert AFTER INSERT ON memory
BEGIN
    INSERT INTO changes (table_name, row_id, action, summary, created_at)
    VALUES ('memory', NEW.namespace || ':' || NEW.key, 'insert',
            json_object('namespace', NEW.namespace, 'key', NEW.key),
            strftime('%s', 'now'));
END;

CREATE TRIGGER IF NOT EXISTS trg_memory_update AFTER UPDATE ON memory
BEGIN
    INSERT INTO changes (table_name, row_id, action, summary, created_at)
    VALUES ('memory', NEW.namespace || ':' || NEW.key, 'update',
            json_object('namespace', NEW.namespace, 'key', NEW.key),
            strftime('%s', 'now'));
END;

CREATE TRIGGER IF NOT EXISTS trg_memory_delete AFTER DELETE ON memory
BEGIN
    INSERT INTO changes (table_name, row_id, action, summary, created_at)
    VALUES ('memory', OLD.namespace || ':' || OLD.key, 'delete',
            json_object('namespace', OLD.namespace, 'key', OLD.key),
            strftime('%s', 'now'));
END;

CREATE TRIGGER IF NOT EXISTS trg_messages_insert AFTER INSERT ON messages
BEGIN
    INSERT INTO changes (table_name, row_id, action, summary, created_at)
    VALUES ('messages', CAST(NEW.id AS TEXT), 'insert',
            json_object('channel', NEW.channel, 'sender', NEW.sender, 'priority', NEW.priority),
            strftime('%s', 'now'));
END;

CREATE TRIGGER IF NOT EXISTS trg_messages_update AFTER UPDATE ON messages
BEGIN
    INSERT INTO changes (table_name, row_id, action, summary, created_at)
    VALUES ('messages', CAST(NEW.id AS TEXT), 'update',
            json_object('channel', NEW.channel, 'acknowledged', NEW.acknowledged),
            strftime('%s', 'now'));
END;

CREATE TRIGGER IF NOT EXISTS trg_messages_delete AFTER DELETE ON messages
BEGIN
    INSERT INTO changes (table_name, row_id, action, summary, created_at)
    VALUES ('messages', CAST(OLD.id AS TEXT), 'delete',
            json_object('channel', OLD.channel, 'sender', OLD.sender),
            strftime('%s', 'now'));
END;

CREATE TRIGGER IF NOT EXISTS trg_tasks_insert AFTER INSERT ON tasks
BEGIN
    INSERT INTO changes (table_name, row_id, action, summary, created_at)
    VALUES ('tasks', NEW.id, 'insert',
            json_object('title', NEW.title, 'status', NEW.status, 'priority', NEW.priority),
            strftime('%s', 'now'));
END;

CREATE TRIGGER IF NOT EXISTS trg_tasks_update AFTER UPDATE ON tasks
BEGIN
    INSERT INTO changes (table_name, row_id, action, summary, created_at)
    VALUES ('tasks', NEW.id, 'update',
            json_object('title', NEW.title, 'status', NEW.status, 'assigned_to', NEW.assigned_to),
            strftime('%s', 'now'));
END;

CREATE TRIGGER IF NOT EXISTS trg_tasks_delete AFTER DELETE ON tasks
BEGIN
    INSERT INTO changes (table_name, row_id, action, summary, created_at)
    VALUES ('tasks', OLD.id, 'delete',
            json_object('title', OLD.title, 'status', OLD.status),
            strftime('%s', 'now'));
END;

CREATE TRIGGER IF NOT EXISTS trg_agents_insert AFTER INSERT ON agents
BEGIN
    INSERT INTO changes (table_name, row_id, action, summary, created_at)
    VALUES ('agents', NEW.id, 'insert',
            json_object('name', NEW.name, 'status', NEW.status),
            strftime('%s', 'now'));
END;

CREATE TRIGGER IF NOT EXISTS trg_agents_update AFTER UPDATE ON agents
BEGIN
    INSERT INTO changes (table_name, row_id, action, summary, created_at)
    VALUES ('agents', NEW.id, 'update',
            json_object('name', NEW.name, 'status', NEW.status),
            strftime('%s', 'now'));
END;

CREATE TRIGGER IF NOT EXISTS trg_agents_delete AFTER DELETE ON agents
BEGIN
    INSERT INTO changes (table_name, row_id, action, summary, created_at)
    VALUES ('agents', OLD.id, 'delete',
            json_object('name', OLD.name),
            strftime('%s', 'now'));
END;
```

- [ ] **Step 2: Commit**

```bash
git add src/schema.sql
git commit -m "feat: add SQL schema with tables, indexes, and change triggers"
```

---

### Task 3: Database Module

**Files:**
- Create: `src/db.rs`
- Modify: `src/main.rs`
- Create: `tests/db_test.rs`

- [ ] **Step 1: Write the failing test**

Create `tests/db_test.rs`:

```rust
use rusqlite::Connection;
use std::path::Path;

// We test the db module by calling init_db and verifying tables exist
use maximous::db;

#[test]
fn test_init_db_creates_all_tables() {
    let conn = Connection::open_in_memory().unwrap();
    db::init_db(&conn).unwrap();

    let tables: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();

    assert!(tables.contains(&"memory".to_string()));
    assert!(tables.contains(&"messages".to_string()));
    assert!(tables.contains(&"tasks".to_string()));
    assert!(tables.contains(&"agents".to_string()));
    assert!(tables.contains(&"changes".to_string()));
    assert!(tables.contains(&"config".to_string()));
}

#[test]
fn test_wal_mode_enabled() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let conn = Connection::open(&db_path).unwrap();
    db::init_db(&conn).unwrap();

    let mode: String = conn
        .pragma_query_value(None, "journal_mode", |row| row.get(0))
        .unwrap();
    assert_eq!(mode, "wal");
}

#[test]
fn test_trigger_populates_changes_on_memory_insert() {
    let conn = Connection::open_in_memory().unwrap();
    db::init_db(&conn).unwrap();

    conn.execute(
        "INSERT INTO memory (namespace, key, value, created_at, updated_at) VALUES (?1, ?2, ?3, strftime('%s','now'), strftime('%s','now'))",
        rusqlite::params!["test-ns", "test-key", r#"{"hello":"world"}"#],
    ).unwrap();

    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM changes WHERE table_name = 'memory'", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 1);
}
```

- [ ] **Step 2: Write db.rs**

```rust
use rusqlite::{Connection, Result};

const SCHEMA: &str = include_str!("schema.sql");

pub fn init_db(conn: &Connection) -> Result<()> {
    // Enable WAL mode for concurrent access
    conn.pragma_update(None, "journal_mode", "wal")?;

    // Reasonable defaults for multi-process access
    conn.pragma_update(None, "busy_timeout", 5000)?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;

    // Run schema
    conn.execute_batch(SCHEMA)?;

    Ok(())
}

pub fn open_db(path: &str) -> Result<Connection> {
    // Ensure parent directory exists
    if let Some(parent) = std::path::Path::new(path).parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let conn = Connection::open(path)?;
    init_db(&conn)?;
    Ok(conn)
}
```

- [ ] **Step 3: Update main.rs to use db module**

```rust
mod db;

use clap::Parser;

#[derive(Parser)]
#[command(name = "maximous", about = "SQLite brain for multi-agent orchestration")]
struct Cli {
    /// Path to the SQLite database file
    #[arg(long, default_value = ".maximous/brain.db")]
    db: String,
}

fn main() {
    let cli = Cli::parse();
    let _conn = db::open_db(&cli.db).expect("Failed to open database");
    eprintln!("maximous: database ready at {}", cli.db);
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test --test db_test`
Expected: All 3 tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/db.rs src/main.rs tests/db_test.rs
git commit -m "feat: add database module with WAL mode, schema init, and triggers"
```

---

## Chunk 2: MCP Protocol Layer

### Task 4: JSON-RPC Types and MCP Protocol

**Files:**
- Create: `src/mcp.rs`
- Create: `tests/mcp_test.rs`

The MCP protocol uses JSON-RPC 2.0. We need to handle:
1. `initialize` — server capabilities handshake
2. `tools/list` — return available tools
3. `tools/call` — dispatch tool calls
4. `notifications/initialized` — client ready notification
5. `ping` — keepalive

- [ ] **Step 1: Write failing test for JSON-RPC parsing**

Create `tests/mcp_test.rs`:

```rust
use maximous::mcp;

#[test]
fn test_parse_initialize_request() {
    let input = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{}}}"#;
    let req: mcp::JsonRpcRequest = serde_json::from_str(input).unwrap();
    assert_eq!(req.method, "initialize");
    assert_eq!(req.id, Some(serde_json::json!(1)));
}

#[test]
fn test_serialize_success_response() {
    let resp = mcp::JsonRpcResponse::success(serde_json::json!(1), serde_json::json!({"ok": true}));
    let json = serde_json::to_string(&resp).unwrap();
    assert!(json.contains("\"result\""));
    assert!(!json.contains("\"error\""));
}

#[test]
fn test_serialize_error_response() {
    let resp = mcp::JsonRpcResponse::error(serde_json::json!(1), -32601, "Method not found");
    let json = serde_json::to_string(&resp).unwrap();
    assert!(json.contains("\"error\""));
    assert!(json.contains("Method not found"));
}

#[test]
fn test_tool_list_contains_all_tools() {
    let tools = mcp::tool_definitions();
    let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();

    assert!(names.contains(&"memory_set"));
    assert!(names.contains(&"memory_get"));
    assert!(names.contains(&"memory_search"));
    assert!(names.contains(&"memory_delete"));
    assert!(names.contains(&"message_send"));
    assert!(names.contains(&"message_read"));
    assert!(names.contains(&"message_ack"));
    assert!(names.contains(&"task_create"));
    assert!(names.contains(&"task_update"));
    assert!(names.contains(&"task_list"));
    assert!(names.contains(&"agent_register"));
    assert!(names.contains(&"agent_heartbeat"));
    assert!(names.contains(&"agent_list"));
    assert!(names.contains(&"poll_changes"));
    assert_eq!(tools.len(), 14);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test mcp_test`
Expected: FAIL — `mcp` module does not exist.

- [ ] **Step 3: Write mcp.rs**

```rust
use serde::{Deserialize, Serialize};
use serde_json::Value;

// --- JSON-RPC Types ---

#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    #[serde(default)]
    pub id: Option<Value>,
    pub method: String,
    #[serde(default)]
    pub params: Option<Value>,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
}

impl JsonRpcResponse {
    pub fn success(id: Value, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: Some(id),
            result: Some(result),
            error: None,
        }
    }

    pub fn error(id: Value, code: i64, message: &str) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: Some(id),
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.to_string(),
            }),
        }
    }
}

// --- MCP Tool Definition ---

#[derive(Debug, Serialize)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
}

pub fn tool_definitions() -> Vec<ToolDef> {
    vec![
        ToolDef {
            name: "memory_set".into(),
            description: "Store a value in shared memory with optional TTL".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "namespace": {"type": "string", "description": "Memory namespace"},
                    "key": {"type": "string", "description": "Key within namespace"},
                    "value": {"type": "string", "description": "JSON string value to store"},
                    "ttl_seconds": {"type": "integer", "description": "Optional TTL in seconds"}
                },
                "required": ["namespace", "key", "value"]
            }),
        },
        ToolDef {
            name: "memory_get".into(),
            description: "Read a value from shared memory by namespace and key, or list all keys in a namespace".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "namespace": {"type": "string", "description": "Memory namespace"},
                    "key": {"type": "string", "description": "Key within namespace (omit to list all keys)"}
                },
                "required": ["namespace"]
            }),
        },
        ToolDef {
            name: "memory_search".into(),
            description: "Full-text search across memory values".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string", "description": "Search query (matches against values)"},
                    "namespace": {"type": "string", "description": "Optional namespace filter"}
                },
                "required": ["query"]
            }),
        },
        ToolDef {
            name: "memory_delete".into(),
            description: "Delete a key from memory or expire all stale entries".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "namespace": {"type": "string", "description": "Memory namespace"},
                    "key": {"type": "string", "description": "Key to delete (omit to expire all stale entries in namespace)"}
                },
                "required": ["namespace"]
            }),
        },
        ToolDef {
            name: "message_send".into(),
            description: "Send a message to a channel with priority".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "channel": {"type": "string", "description": "Target channel"},
                    "sender": {"type": "string", "description": "Sender identifier"},
                    "content": {"type": "string", "description": "Message content (JSON string)"},
                    "priority": {"type": "integer", "description": "0=critical, 1=high, 2=normal, 3=low", "default": 2}
                },
                "required": ["channel", "sender", "content"]
            }),
        },
        ToolDef {
            name: "message_read".into(),
            description: "Read messages from a channel, optionally only unacknowledged".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "channel": {"type": "string", "description": "Channel to read from"},
                    "unacknowledged_only": {"type": "boolean", "description": "Only return unacknowledged messages", "default": false},
                    "limit": {"type": "integer", "description": "Max messages to return", "default": 50}
                },
                "required": ["channel"]
            }),
        },
        ToolDef {
            name: "message_ack".into(),
            description: "Acknowledge a message by ID".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "id": {"type": "integer", "description": "Message ID to acknowledge"}
                },
                "required": ["id"]
            }),
        },
        ToolDef {
            name: "task_create".into(),
            description: "Create a new task with optional dependencies".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "title": {"type": "string", "description": "Task title"},
                    "priority": {"type": "integer", "description": "0=critical, 1=high, 2=normal, 3=low", "default": 2},
                    "dependencies": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Array of task IDs this task depends on"
                    }
                },
                "required": ["title"]
            }),
        },
        ToolDef {
            name: "task_update".into(),
            description: "Update a task's status, assignment, or result".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "id": {"type": "string", "description": "Task ID"},
                    "status": {"type": "string", "enum": ["pending", "ready", "running", "done", "failed"]},
                    "assigned_to": {"type": "string", "description": "Agent ID to assign to"},
                    "result": {"type": "string", "description": "Task result (JSON string)"}
                },
                "required": ["id"]
            }),
        },
        ToolDef {
            name: "task_list".into(),
            description: "List tasks filtered by status or assignee".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "status": {"type": "string", "description": "Filter by status"},
                    "assigned_to": {"type": "string", "description": "Filter by assigned agent"}
                }
            }),
        },
        ToolDef {
            name: "agent_register".into(),
            description: "Register an agent with capabilities".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "id": {"type": "string", "description": "Agent ID"},
                    "name": {"type": "string", "description": "Agent display name"},
                    "capabilities": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "List of capabilities"
                    },
                    "metadata": {"type": "string", "description": "Optional JSON metadata"}
                },
                "required": ["id", "name"]
            }),
        },
        ToolDef {
            name: "agent_heartbeat".into(),
            description: "Update agent heartbeat and optionally status".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "id": {"type": "string", "description": "Agent ID"},
                    "status": {"type": "string", "enum": ["idle", "active", "stopped"]}
                },
                "required": ["id"]
            }),
        },
        ToolDef {
            name: "agent_list".into(),
            description: "List active agents (heartbeat within last 60 seconds)".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "include_stale": {"type": "boolean", "description": "Include agents with old heartbeats", "default": false}
                }
            }),
        },
        ToolDef {
            name: "poll_changes".into(),
            description: "Get all changes since a given change ID (observation pattern)".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "since_id": {"type": "integer", "description": "Return changes with ID greater than this", "default": 0},
                    "table_name": {"type": "string", "description": "Optional filter by table name"},
                    "limit": {"type": "integer", "description": "Max changes to return", "default": 100}
                }
            }),
        },
    ]
}

// --- MCP Protocol Handlers ---

pub fn handle_initialize(id: Value) -> JsonRpcResponse {
    JsonRpcResponse::success(id, serde_json::json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {
            "tools": {}
        },
        "serverInfo": {
            "name": "maximous",
            "version": env!("CARGO_PKG_VERSION")
        }
    }))
}

pub fn handle_tools_list(id: Value) -> JsonRpcResponse {
    let tools = tool_definitions();
    JsonRpcResponse::success(id, serde_json::json!({ "tools": tools }))
}

// --- Stdio Loop ---

use rusqlite::Connection;
use std::io::{BufRead, Write};
use std::sync::{Arc, Mutex};

pub fn run_stdio(conn: Arc<Mutex<Connection>>) {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut stdout = stdout.lock();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };

        if line.trim().is_empty() {
            continue;
        }

        let req: JsonRpcRequest = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                let resp = JsonRpcResponse::error(
                    serde_json::json!(null),
                    -32700,
                    &format!("Parse error: {}", e),
                );
                let _ = writeln!(stdout, "{}", serde_json::to_string(&resp).unwrap());
                let _ = stdout.flush();
                continue;
            }
        };

        let response = dispatch(&req, &conn);

        if let Some(resp) = response {
            let _ = writeln!(stdout, "{}", serde_json::to_string(&resp).unwrap());
            let _ = stdout.flush();
        }
    }
}

fn dispatch(req: &JsonRpcRequest, conn: &Arc<Mutex<Connection>>) -> Option<JsonRpcResponse> {
    let id = req.id.clone();

    match req.method.as_str() {
        "initialize" => Some(handle_initialize(id?)),
        "notifications/initialized" => None, // notification, no response
        "ping" => Some(JsonRpcResponse::success(id?, serde_json::json!({}))),
        "tools/list" => Some(handle_tools_list(id?)),
        "tools/call" => {
            let params = req.params.as_ref();
            let tool_name = params
                .and_then(|p| p.get("name"))
                .and_then(|n| n.as_str())
                .unwrap_or("");
            let arguments = params
                .and_then(|p| p.get("arguments"))
                .cloned()
                .unwrap_or(serde_json::json!({}));

            let conn = conn.lock().unwrap();
            let result = crate::tools::dispatch_tool(tool_name, &arguments, &conn);

            Some(JsonRpcResponse::success(
                id?,
                serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": serde_json::to_string(&result).unwrap()
                    }]
                }),
            ))
        }
        _ => Some(JsonRpcResponse::error(
            id.unwrap_or(serde_json::json!(null)),
            -32601,
            &format!("Method not found: {}", req.method),
        )),
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test --test mcp_test`
Expected: All 4 tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/mcp.rs tests/mcp_test.rs
git commit -m "feat: add MCP protocol layer with JSON-RPC types and stdio loop"
```

---

### Task 5: Tool Registry (mod.rs)

**Files:**
- Create: `src/tools/mod.rs`

- [ ] **Step 1: Write tools/mod.rs with dispatch function**

```rust
pub mod memory;
pub mod messages;
pub mod tasks;
pub mod agents;
pub mod changes;

use rusqlite::Connection;
use serde_json::Value;

#[derive(Debug, serde::Serialize)]
pub struct ToolResult {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl ToolResult {
    pub fn success(data: Value) -> Self {
        Self { ok: true, data: Some(data), error: None }
    }

    pub fn fail(msg: &str) -> Self {
        Self { ok: false, data: None, error: Some(msg.to_string()) }
    }
}

pub fn dispatch_tool(name: &str, args: &Value, conn: &Connection) -> ToolResult {
    match name {
        "memory_set" => memory::set(args, conn),
        "memory_get" => memory::get(args, conn),
        "memory_search" => memory::search(args, conn),
        "memory_delete" => memory::delete(args, conn),
        "message_send" => messages::send(args, conn),
        "message_read" => messages::read(args, conn),
        "message_ack" => messages::ack(args, conn),
        "task_create" => tasks::create(args, conn),
        "task_update" => tasks::update(args, conn),
        "task_list" => tasks::list(args, conn),
        "agent_register" => agents::register(args, conn),
        "agent_heartbeat" => agents::heartbeat(args, conn),
        "agent_list" => agents::list(args, conn),
        "poll_changes" => changes::poll(args, conn),
        _ => ToolResult::fail(&format!("Unknown tool: {}", name)),
    }
}
```

- [ ] **Step 2: Commit**

```bash
git add src/tools/mod.rs
git commit -m "feat: add tool registry with dispatch routing"
```

---

## Chunk 3: Memory & Message Tools

### Task 6: Memory Tools

**Files:**
- Create: `src/tools/memory.rs`
- Create: `tests/memory_test.rs`

- [ ] **Step 1: Write failing tests**

Create `tests/memory_test.rs`:

```rust
use rusqlite::Connection;

use maximous::db;
use maximous::tools;

fn setup() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    db::init_db(&conn).unwrap();
    conn
}

#[test]
fn test_memory_set_and_get() {
    let conn = setup();
    let result = tools::memory::set(
        &serde_json::json!({"namespace": "test", "key": "foo", "value": "{\"bar\":1}"}),
        &conn,
    );
    assert!(result.ok);

    let result = tools::memory::get(
        &serde_json::json!({"namespace": "test", "key": "foo"}),
        &conn,
    );
    assert!(result.ok);
    let data = result.data.unwrap();
    assert_eq!(data["value"], "{\"bar\":1}");
}

#[test]
fn test_memory_list_keys() {
    let conn = setup();
    tools::memory::set(&serde_json::json!({"namespace": "ns", "key": "a", "value": "1"}), &conn);
    tools::memory::set(&serde_json::json!({"namespace": "ns", "key": "b", "value": "2"}), &conn);

    let result = tools::memory::get(&serde_json::json!({"namespace": "ns"}), &conn);
    assert!(result.ok);
    let keys = result.data.unwrap();
    let keys = keys["keys"].as_array().unwrap();
    assert_eq!(keys.len(), 2);
}

#[test]
fn test_memory_search() {
    let conn = setup();
    tools::memory::set(&serde_json::json!({"namespace": "ns", "key": "k1", "value": "hello world"}), &conn);
    tools::memory::set(&serde_json::json!({"namespace": "ns", "key": "k2", "value": "goodbye"}), &conn);

    let result = tools::memory::search(&serde_json::json!({"query": "hello"}), &conn);
    assert!(result.ok);
    let matches = result.data.unwrap();
    let matches = matches["matches"].as_array().unwrap();
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0]["key"], "k1");
}

#[test]
fn test_memory_delete() {
    let conn = setup();
    tools::memory::set(&serde_json::json!({"namespace": "ns", "key": "k", "value": "v"}), &conn);
    let result = tools::memory::delete(&serde_json::json!({"namespace": "ns", "key": "k"}), &conn);
    assert!(result.ok);

    let result = tools::memory::get(&serde_json::json!({"namespace": "ns", "key": "k"}), &conn);
    assert!(result.ok);
    assert!(result.data.unwrap().get("value").unwrap().is_null());
}

#[test]
fn test_memory_ttl_lazy_expiry() {
    let conn = setup();
    // Insert with ttl=0 (already expired)
    conn.execute(
        "INSERT INTO memory (namespace, key, value, ttl_seconds, created_at, updated_at) VALUES ('ns', 'expired', 'v', 0, 0, 0)",
        [],
    ).unwrap();

    // memory_get should clean it up
    let result = tools::memory::get(&serde_json::json!({"namespace": "ns", "key": "expired"}), &conn);
    assert!(result.ok);
    assert!(result.data.unwrap().get("value").unwrap().is_null());
}

#[test]
fn test_memory_set_upsert() {
    let conn = setup();
    tools::memory::set(&serde_json::json!({"namespace": "ns", "key": "k", "value": "v1"}), &conn);
    tools::memory::set(&serde_json::json!({"namespace": "ns", "key": "k", "value": "v2"}), &conn);

    let result = tools::memory::get(&serde_json::json!({"namespace": "ns", "key": "k"}), &conn);
    assert_eq!(result.data.unwrap()["value"], "v2");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test memory_test`
Expected: FAIL — module does not exist.

- [ ] **Step 3: Write memory.rs**

```rust
use rusqlite::Connection;
use serde_json::Value;
use super::ToolResult;

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

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    match conn.execute(
        "INSERT INTO memory (namespace, key, value, ttl_seconds, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?5)
         ON CONFLICT(namespace, key) DO UPDATE SET value=?3, ttl_seconds=?4, updated_at=?5",
        rusqlite::params![namespace, key, value, ttl, now],
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

    // Lazy TTL cleanup for this namespace
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
            // Get specific key
            let result: Result<(String, Option<i64>, i64), _> = conn.query_row(
                "SELECT value, ttl_seconds, updated_at FROM memory WHERE namespace = ?1 AND key = ?2",
                rusqlite::params![namespace, key],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            );
            match result {
                Ok((value, ttl, updated_at)) => ToolResult::success(serde_json::json!({
                    "namespace": namespace,
                    "key": key,
                    "value": value,
                    "ttl_seconds": ttl,
                    "updated_at": updated_at,
                })),
                Err(_) => ToolResult::success(serde_json::json!({
                    "namespace": namespace,
                    "key": key,
                    "value": null,
                })),
            }
        }
        None => {
            // List all keys in namespace
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

    let (sql, params): (&str, Vec<Box<dyn rusqlite::types::ToSql>>) = match namespace {
        Some(ns) => (
            "SELECT namespace, key, value FROM memory WHERE namespace = ? AND value LIKE ?",
            vec![Box::new(ns.to_string()), Box::new(format!("%{}%", query))],
        ),
        None => (
            "SELECT namespace, key, value FROM memory WHERE value LIKE ?",
            vec![Box::new(format!("%{}%", query))],
        ),
    };

    let mut stmt = conn.prepare(sql).unwrap();
    let params_ref: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
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

    ToolResult::success(serde_json::json!({"matches": matches, "count": matches.len()}))
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
            // Expire all stale entries in namespace
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
```

- [ ] **Step 4: Run tests**

Run: `cargo test --test memory_test`
Expected: All 6 tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/tools/memory.rs tests/memory_test.rs
git commit -m "feat: add memory tools (set, get, search, delete) with TTL"
```

---

### Task 7: Message Tools

**Files:**
- Create: `src/tools/messages.rs`
- Create: `tests/messages_test.rs`

- [ ] **Step 1: Write failing tests**

Create `tests/messages_test.rs`:

```rust
use rusqlite::Connection;

use maximous::db;
use maximous::tools;

fn setup() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    db::init_db(&conn).unwrap();
    conn
}

#[test]
fn test_send_and_read_message() {
    let conn = setup();
    let result = tools::messages::send(
        &serde_json::json!({"channel": "general", "sender": "agent-a", "content": "{\"text\":\"hello\"}"}),
        &conn,
    );
    assert!(result.ok);

    let result = tools::messages::read(
        &serde_json::json!({"channel": "general"}),
        &conn,
    );
    assert!(result.ok);
    let msgs = result.data.unwrap();
    let msgs = msgs["messages"].as_array().unwrap();
    assert_eq!(msgs.len(), 1);
    assert_eq!(msgs[0]["sender"], "agent-a");
}

#[test]
fn test_message_priority_ordering() {
    let conn = setup();
    tools::messages::send(&serde_json::json!({"channel": "ch", "sender": "a", "content": "low", "priority": 3}), &conn);
    tools::messages::send(&serde_json::json!({"channel": "ch", "sender": "a", "content": "critical", "priority": 0}), &conn);
    tools::messages::send(&serde_json::json!({"channel": "ch", "sender": "a", "content": "normal", "priority": 2}), &conn);

    let result = tools::messages::read(&serde_json::json!({"channel": "ch"}), &conn);
    let msgs = result.data.unwrap();
    let msgs = msgs["messages"].as_array().unwrap();
    // Should be ordered by priority ASC (critical first)
    assert_eq!(msgs[0]["content"], "critical");
    assert_eq!(msgs[1]["content"], "normal");
    assert_eq!(msgs[2]["content"], "low");
}

#[test]
fn test_message_acknowledge() {
    let conn = setup();
    tools::messages::send(&serde_json::json!({"channel": "ch", "sender": "a", "content": "msg"}), &conn);

    let result = tools::messages::read(&serde_json::json!({"channel": "ch"}), &conn);
    let msg_id = result.data.unwrap()["messages"][0]["id"].as_i64().unwrap();

    let result = tools::messages::ack(&serde_json::json!({"id": msg_id}), &conn);
    assert!(result.ok);

    // Unacknowledged only should return empty
    let result = tools::messages::read(&serde_json::json!({"channel": "ch", "unacknowledged_only": true}), &conn);
    let msgs = result.data.unwrap();
    assert_eq!(msgs["messages"].as_array().unwrap().len(), 0);
}

#[test]
fn test_message_read_limit() {
    let conn = setup();
    for i in 0..10 {
        tools::messages::send(&serde_json::json!({"channel": "ch", "sender": "a", "content": format!("msg-{}", i)}), &conn);
    }

    let result = tools::messages::read(&serde_json::json!({"channel": "ch", "limit": 3}), &conn);
    let msgs = result.data.unwrap();
    assert_eq!(msgs["messages"].as_array().unwrap().len(), 3);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test messages_test`
Expected: FAIL.

- [ ] **Step 3: Write messages.rs**

```rust
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

    let sql = if unacked_only {
        "SELECT id, channel, sender, priority, content, acknowledged, created_at
         FROM messages WHERE channel = ?1 AND acknowledged = 0
         ORDER BY priority ASC, created_at ASC LIMIT ?2"
    } else {
        "SELECT id, channel, sender, priority, content, acknowledged, created_at
         FROM messages WHERE channel = ?1
         ORDER BY priority ASC, created_at ASC LIMIT ?2"
    };

    let mut stmt = conn.prepare(sql).unwrap();
    let messages: Vec<Value> = stmt
        .query_map(rusqlite::params![channel, limit], |row| {
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

    ToolResult::success(serde_json::json!({"messages": messages, "count": messages.len()}))
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
```

- [ ] **Step 4: Run tests**

Run: `cargo test --test messages_test`
Expected: All 4 tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/tools/messages.rs tests/messages_test.rs
git commit -m "feat: add message tools (send, read, ack) with priority queue"
```

---

## Chunk 4: Task & Agent Tools

### Task 8: Task Tools

**Files:**
- Create: `src/tools/tasks.rs`
- Create: `tests/tasks_test.rs`

- [ ] **Step 1: Write failing tests**

Create `tests/tasks_test.rs`:

```rust
use rusqlite::Connection;

use maximous::db;
use maximous::tools;

fn setup() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    db::init_db(&conn).unwrap();
    conn
}

#[test]
fn test_task_create_and_list() {
    let conn = setup();
    let result = tools::tasks::create(
        &serde_json::json!({"title": "Build API"}),
        &conn,
    );
    assert!(result.ok);
    let id = result.data.unwrap()["id"].as_str().unwrap().to_string();
    assert!(!id.is_empty());

    let result = tools::tasks::list(&serde_json::json!({}), &conn);
    assert!(result.ok);
    let tasks = result.data.unwrap();
    assert_eq!(tasks["tasks"].as_array().unwrap().len(), 1);
}

#[test]
fn test_task_with_dependencies() {
    let conn = setup();
    let r1 = tools::tasks::create(&serde_json::json!({"title": "Step 1"}), &conn);
    let id1 = r1.data.unwrap()["id"].as_str().unwrap().to_string();

    let r2 = tools::tasks::create(
        &serde_json::json!({"title": "Step 2", "dependencies": [id1]}),
        &conn,
    );
    assert!(r2.ok);
    let data = r2.data.unwrap();
    assert_eq!(data["dependencies"].as_array().unwrap().len(), 1);
}

#[test]
fn test_task_update_status() {
    let conn = setup();
    let r = tools::tasks::create(&serde_json::json!({"title": "Task"}), &conn);
    let id = r.data.unwrap()["id"].as_str().unwrap().to_string();

    let result = tools::tasks::update(
        &serde_json::json!({"id": id, "status": "running", "assigned_to": "agent-1"}),
        &conn,
    );
    assert!(result.ok);

    let result = tools::tasks::list(&serde_json::json!({"status": "running"}), &conn);
    let tasks = result.data.unwrap();
    assert_eq!(tasks["tasks"].as_array().unwrap().len(), 1);
    assert_eq!(tasks["tasks"][0]["assigned_to"], "agent-1");
}

#[test]
fn test_task_ready_checks_dependencies() {
    let conn = setup();
    let r1 = tools::tasks::create(&serde_json::json!({"title": "Dep"}), &conn);
    let dep_id = r1.data.unwrap()["id"].as_str().unwrap().to_string();

    let r2 = tools::tasks::create(&serde_json::json!({"title": "Main", "dependencies": [dep_id]}), &conn);
    let main_id = r2.data.unwrap()["id"].as_str().unwrap().to_string();

    // Try to set to ready — should fail because dep is not done
    let result = tools::tasks::update(&serde_json::json!({"id": main_id, "status": "ready"}), &conn);
    assert!(!result.ok);

    // Complete the dependency
    tools::tasks::update(&serde_json::json!({"id": dep_id, "status": "done"}), &conn);

    // Now ready should work
    let result = tools::tasks::update(&serde_json::json!({"id": main_id, "status": "ready"}), &conn);
    assert!(result.ok);
}

#[test]
fn test_task_list_filter_by_assignee() {
    let conn = setup();
    let r1 = tools::tasks::create(&serde_json::json!({"title": "T1"}), &conn);
    let id1 = r1.data.unwrap()["id"].as_str().unwrap().to_string();
    tools::tasks::update(&serde_json::json!({"id": id1, "assigned_to": "agent-a"}), &conn);

    let r2 = tools::tasks::create(&serde_json::json!({"title": "T2"}), &conn);
    let id2 = r2.data.unwrap()["id"].as_str().unwrap().to_string();
    tools::tasks::update(&serde_json::json!({"id": id2, "assigned_to": "agent-b"}), &conn);

    let result = tools::tasks::list(&serde_json::json!({"assigned_to": "agent-a"}), &conn);
    let tasks = result.data.unwrap();
    assert_eq!(tasks["tasks"].as_array().unwrap().len(), 1);
    assert_eq!(tasks["tasks"][0]["title"], "T1");
}

#[test]
fn test_task_priority_ordering() {
    let conn = setup();
    tools::tasks::create(&serde_json::json!({"title": "Low", "priority": 3}), &conn);
    tools::tasks::create(&serde_json::json!({"title": "Critical", "priority": 0}), &conn);
    tools::tasks::create(&serde_json::json!({"title": "Normal", "priority": 2}), &conn);

    let result = tools::tasks::list(&serde_json::json!({}), &conn);
    let tasks = result.data.unwrap();
    let tasks = tasks["tasks"].as_array().unwrap();
    assert_eq!(tasks[0]["title"], "Critical");
    assert_eq!(tasks[1]["title"], "Normal");
    assert_eq!(tasks[2]["title"], "Low");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test tasks_test`
Expected: FAIL.

- [ ] **Step 3: Write tasks.rs**

```rust
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

    // Check if task exists
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

    // Build dynamic UPDATE
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

    let sql = format!(
        "SELECT id, title, status, priority, assigned_to, dependencies, result, created_at, updated_at
         FROM tasks {} ORDER BY priority ASC, created_at ASC",
        where_clause
    );

    let params_ref: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let mut stmt = conn.prepare(&sql).unwrap();
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

    ToolResult::success(serde_json::json!({"tasks": tasks, "count": tasks.len()}))
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test --test tasks_test`
Expected: All 6 tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/tools/tasks.rs tests/tasks_test.rs
git commit -m "feat: add task tools (create, update, list) with dependency checking"
```

---

### Task 9: Agent Tools

**Files:**
- Create: `src/tools/agents.rs`
- Create: `tests/agents_test.rs`

- [ ] **Step 1: Write failing tests**

Create `tests/agents_test.rs`:

```rust
use rusqlite::Connection;

use maximous::db;
use maximous::tools;

fn setup() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    db::init_db(&conn).unwrap();
    conn
}

#[test]
fn test_agent_register_and_list() {
    let conn = setup();
    let result = tools::agents::register(
        &serde_json::json!({"id": "agent-1", "name": "Parser", "capabilities": ["parsing", "analysis"]}),
        &conn,
    );
    assert!(result.ok);

    let result = tools::agents::list(&serde_json::json!({}), &conn);
    assert!(result.ok);
    let agents = result.data.unwrap();
    assert_eq!(agents["agents"].as_array().unwrap().len(), 1);
    assert_eq!(agents["agents"][0]["name"], "Parser");
}

#[test]
fn test_agent_heartbeat() {
    let conn = setup();
    tools::agents::register(&serde_json::json!({"id": "a1", "name": "Agent"}), &conn);

    let result = tools::agents::heartbeat(
        &serde_json::json!({"id": "a1", "status": "active"}),
        &conn,
    );
    assert!(result.ok);
}

#[test]
fn test_agent_register_upsert() {
    let conn = setup();
    tools::agents::register(&serde_json::json!({"id": "a1", "name": "V1"}), &conn);
    tools::agents::register(&serde_json::json!({"id": "a1", "name": "V2", "capabilities": ["new"]}), &conn);

    let result = tools::agents::list(&serde_json::json!({"include_stale": true}), &conn);
    let agents = result.data.unwrap();
    let agents = agents["agents"].as_array().unwrap();
    assert_eq!(agents.len(), 1);
    assert_eq!(agents[0]["name"], "V2");
}

#[test]
fn test_agent_stale_filtering() {
    let conn = setup();
    // Insert agent with old heartbeat (more than 60s ago)
    conn.execute(
        "INSERT INTO agents (id, name, status, last_heartbeat) VALUES ('stale', 'Old', 'idle', 0)",
        [],
    ).unwrap();
    // Insert fresh agent
    tools::agents::register(&serde_json::json!({"id": "fresh", "name": "New"}), &conn);

    // Default: only active agents
    let result = tools::agents::list(&serde_json::json!({}), &conn);
    let agents = result.data.unwrap();
    let agents = agents["agents"].as_array().unwrap();
    assert_eq!(agents.len(), 1);
    assert_eq!(agents[0]["id"], "fresh");

    // Include stale
    let result = tools::agents::list(&serde_json::json!({"include_stale": true}), &conn);
    let agents = result.data.unwrap();
    assert_eq!(agents["agents"].as_array().unwrap().len(), 2);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test agents_test`
Expected: FAIL.

- [ ] **Step 3: Write agents.rs**

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

    let sql = if include_stale {
        "SELECT id, name, status, capabilities, metadata, last_heartbeat FROM agents ORDER BY name"
    } else {
        "SELECT id, name, status, capabilities, metadata, last_heartbeat FROM agents WHERE last_heartbeat > ?1 ORDER BY name"
    };

    let cutoff = now() - 60;

    let agents: Vec<Value> = if include_stale {
        let mut stmt = conn.prepare(sql).unwrap();
        stmt.query_map([], |row| {
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
        let mut stmt = conn.prepare(sql).unwrap();
        stmt.query_map(rusqlite::params![cutoff], |row| {
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

    ToolResult::success(serde_json::json!({"agents": agents, "count": agents.len()}))
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test --test agents_test`
Expected: All 4 tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/tools/agents.rs tests/agents_test.rs
git commit -m "feat: add agent tools (register, heartbeat, list) with stale filtering"
```

---

## Chunk 5: Changes Tool, Main Integration & Plugin

### Task 10: Changes / Observation Tool

**Files:**
- Create: `src/tools/changes.rs`
- Create: `tests/changes_test.rs`

- [ ] **Step 1: Write failing tests**

Create `tests/changes_test.rs`:

```rust
use rusqlite::Connection;

use maximous::db;
use maximous::tools;

fn setup() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    db::init_db(&conn).unwrap();
    conn
}

#[test]
fn test_poll_changes_empty() {
    let conn = setup();
    let result = tools::changes::poll(&serde_json::json!({}), &conn);
    assert!(result.ok);
    let data = result.data.unwrap();
    assert_eq!(data["changes"].as_array().unwrap().len(), 0);
}

#[test]
fn test_poll_changes_after_operations() {
    let conn = setup();

    // Do some operations that trigger changes
    tools::memory::set(&serde_json::json!({"namespace": "ns", "key": "k", "value": "v"}), &conn);
    tools::messages::send(&serde_json::json!({"channel": "ch", "sender": "a", "content": "hi"}), &conn);

    let result = tools::changes::poll(&serde_json::json!({"since_id": 0}), &conn);
    let data = result.data.unwrap();
    let changes = data["changes"].as_array().unwrap();
    assert!(changes.len() >= 2); // At least memory insert + message insert
}

#[test]
fn test_poll_changes_since_id() {
    let conn = setup();

    tools::memory::set(&serde_json::json!({"namespace": "ns", "key": "k1", "value": "v1"}), &conn);

    // Get current max change ID
    let result = tools::changes::poll(&serde_json::json!({"since_id": 0}), &conn);
    let data = result.data.unwrap();
    let changes = data["changes"].as_array().unwrap();
    let last_id = changes.last().unwrap()["id"].as_i64().unwrap();

    // Do another operation
    tools::memory::set(&serde_json::json!({"namespace": "ns", "key": "k2", "value": "v2"}), &conn);

    // Poll since last ID — should only get the new change
    let result = tools::changes::poll(&serde_json::json!({"since_id": last_id}), &conn);
    let data = result.data.unwrap();
    let changes = data["changes"].as_array().unwrap();
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0]["table_name"], "memory");
}

#[test]
fn test_poll_changes_filter_by_table() {
    let conn = setup();

    tools::memory::set(&serde_json::json!({"namespace": "ns", "key": "k", "value": "v"}), &conn);
    tools::messages::send(&serde_json::json!({"channel": "ch", "sender": "a", "content": "hi"}), &conn);

    let result = tools::changes::poll(&serde_json::json!({"table_name": "messages"}), &conn);
    let data = result.data.unwrap();
    let changes = data["changes"].as_array().unwrap();
    assert!(changes.iter().all(|c| c["table_name"] == "messages"));
}

#[test]
fn test_poll_changes_limit() {
    let conn = setup();

    for i in 0..10 {
        tools::memory::set(&serde_json::json!({"namespace": "ns", "key": format!("k{}", i), "value": "v"}), &conn);
    }

    let result = tools::changes::poll(&serde_json::json!({"limit": 3}), &conn);
    let data = result.data.unwrap();
    assert_eq!(data["changes"].as_array().unwrap().len(), 3);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test changes_test`
Expected: FAIL.

- [ ] **Step 3: Write changes.rs**

```rust
use rusqlite::Connection;
use serde_json::Value;
use super::ToolResult;

pub fn poll(args: &Value, conn: &Connection) -> ToolResult {
    let since_id = args["since_id"].as_i64().unwrap_or(0);
    let table_name = args["table_name"].as_str();
    let limit = args["limit"].as_i64().unwrap_or(100);

    let (sql, params): (String, Vec<Box<dyn rusqlite::types::ToSql>>) = match table_name {
        Some(tn) => (
            "SELECT id, table_name, row_id, action, summary, created_at FROM changes WHERE id > ? AND table_name = ? ORDER BY id ASC LIMIT ?".to_string(),
            vec![Box::new(since_id), Box::new(tn.to_string()), Box::new(limit)],
        ),
        None => (
            "SELECT id, table_name, row_id, action, summary, created_at FROM changes WHERE id > ? ORDER BY id ASC LIMIT ?".to_string(),
            vec![Box::new(since_id), Box::new(limit)],
        ),
    };

    let params_ref: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let mut stmt = conn.prepare(&sql).unwrap();
    let changes: Vec<Value> = stmt
        .query_map(params_ref.as_slice(), |row| {
            Ok(serde_json::json!({
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

    ToolResult::success(serde_json::json!({"changes": changes, "count": changes.len()}))
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test --test changes_test`
Expected: All 5 tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/tools/changes.rs tests/changes_test.rs
git commit -m "feat: add poll_changes observation tool"
```

---

### Task 11: Wire Up main.rs

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Update main.rs to run MCP server**

```rust
mod db;
mod mcp;
mod tools;

use clap::Parser;
use std::sync::{Arc, Mutex};

#[derive(Parser)]
#[command(name = "maximous", about = "SQLite brain for multi-agent orchestration")]
struct Cli {
    /// Path to the SQLite database file
    #[arg(long, default_value = ".maximous/brain.db")]
    db: String,
}

fn main() {
    let cli = Cli::parse();
    let conn = db::open_db(&cli.db).expect("Failed to open database");
    eprintln!("maximous: database ready at {}", cli.db);

    let conn = Arc::new(Mutex::new(conn));
    mcp::run_stdio(conn);
}
```

- [ ] **Step 2: Verify full build**

Run: `cargo build`
Expected: Compiles with no errors.

- [ ] **Step 3: Commit**

```bash
git add src/main.rs
git commit -m "feat: wire up main.rs with CLI, database, and MCP stdio loop"
```

---

### Task 12: Plugin Manifest

**Files:**
- Create: `plugin/plugin.json`
- Create: `plugin/.mcp.json`

- [ ] **Step 1: Create plugin.json**

```json
{
  "name": "maximous",
  "version": "0.1.0",
  "description": "Lightweight SQLite brain for multi-agent orchestration",
  "mcp_servers": {
    "maximous": {
      "command": "maximous",
      "args": ["--db", ".maximous/brain.db"]
    }
  }
}
```

- [ ] **Step 2: Create .mcp.json**

```json
{
  "mcpServers": {
    "maximous": {
      "command": "maximous",
      "args": ["--db", ".maximous/brain.db"]
    }
  }
}
```

- [ ] **Step 3: Commit**

```bash
git add plugin/plugin.json plugin/.mcp.json
git commit -m "feat: add Claude Code plugin manifest and MCP server declaration"
```

---

### Task 13: Full Integration Test

**Files:**
- Create: `tests/mcp_test.rs` (extend existing)

- [ ] **Step 1: Write end-to-end test simulating multi-agent workflow**

Add to `tests/mcp_test.rs` (or create `tests/integration_test.rs`):

```rust
use rusqlite::Connection;

use maximous::db;
use maximous::tools;

fn setup() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    db::init_db(&conn).unwrap();
    conn
}

#[test]
fn test_full_multi_agent_workflow() {
    let conn = setup();

    // 1. Register two agents
    tools::agents::register(
        &serde_json::json!({"id": "parser", "name": "Parser Agent", "capabilities": ["parsing"]}),
        &conn,
    );
    tools::agents::register(
        &serde_json::json!({"id": "builder", "name": "Builder Agent", "capabilities": ["building"]}),
        &conn,
    );

    // 2. Create tasks with dependency
    let r1 = tools::tasks::create(&serde_json::json!({"title": "Parse API spec", "priority": 1}), &conn);
    let parse_id = r1.data.unwrap()["id"].as_str().unwrap().to_string();

    let r2 = tools::tasks::create(&serde_json::json!({
        "title": "Build endpoints",
        "dependencies": [parse_id]
    }), &conn);
    let build_id = r2.data.unwrap()["id"].as_str().unwrap().to_string();

    // 3. Parser picks up task
    tools::tasks::update(&serde_json::json!({"id": parse_id, "status": "running", "assigned_to": "parser"}), &conn);

    // 4. Parser stores result in memory
    tools::memory::set(&serde_json::json!({
        "namespace": "task-results",
        "key": parse_id,
        "value": "{\"endpoints\":[\"/users\",\"/items\"]}"
    }), &conn);

    // 5. Parser completes task
    tools::tasks::update(&serde_json::json!({"id": parse_id, "status": "done", "result": "{\"success\":true}"}), &conn);

    // 6. Builder polls changes and sees completion
    let changes = tools::changes::poll(&serde_json::json!({"since_id": 0, "table_name": "tasks"}), &conn);
    let changes_data = changes.data.unwrap();
    let task_changes = changes_data["changes"].as_array().unwrap();
    assert!(task_changes.iter().any(|c| {
        c["row_id"].as_str() == Some(parse_id.as_str()) && c["action"] == "update"
    }));

    // 7. Builder can now set dependent task to ready
    let result = tools::tasks::update(&serde_json::json!({"id": build_id, "status": "ready"}), &conn);
    assert!(result.ok);

    // 8. Builder picks it up and reads upstream result
    tools::tasks::update(&serde_json::json!({"id": build_id, "status": "running", "assigned_to": "builder"}), &conn);
    let mem = tools::memory::get(&serde_json::json!({"namespace": "task-results", "key": parse_id}), &conn);
    assert!(mem.data.unwrap()["value"].as_str().unwrap().contains("/users"));

    // 9. Agents communicate
    tools::messages::send(&serde_json::json!({
        "channel": "team",
        "sender": "builder",
        "content": "{\"question\":\"REST or GraphQL?\"}",
        "priority": 1
    }), &conn);

    let msgs = tools::messages::read(&serde_json::json!({"channel": "team", "unacknowledged_only": true}), &conn);
    assert_eq!(msgs.data.unwrap()["count"], 1);

    // 10. Verify agent list
    let agents = tools::agents::list(&serde_json::json!({}), &conn);
    assert_eq!(agents.data.unwrap()["count"], 2);
}
```

- [ ] **Step 2: Run all tests**

Run: `cargo test`
Expected: All tests pass.

- [ ] **Step 3: Commit**

```bash
git add tests/integration_test.rs
git commit -m "feat: add full multi-agent workflow integration test"
```

---

### Task 14: Release Build & Verify Binary Size

- [ ] **Step 1: Build release binary**

Run: `cargo build --release`
Expected: Compiles successfully.

- [ ] **Step 2: Check binary size**

Run: `ls -lh target/release/maximous`
Expected: Should be under 10MB.

- [ ] **Step 3: Smoke test the binary**

Run: `echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{}}}' | target/release/maximous --db /tmp/test-maximous.db`
Expected: Returns JSON with `protocolVersion` and `serverInfo`.

- [ ] **Step 4: Verify all tests pass**

Run: `cargo test`
Expected: All tests pass. No further commit needed — all code was committed in prior tasks.

---

## Chunk 6: Performance Benchmarks

### Task 15: Efficiency Benchmarks

**Files:**
- Create: `benches/performance.rs`
- Modify: `Cargo.toml` (add bench harness config)

- [ ] **Step 1: Add bench dependency to Cargo.toml**

Add to `Cargo.toml`:

```toml
[[bench]]
name = "performance"
harness = false

[dev-dependencies]
tempfile = "3"
criterion = { version = "0.5", features = ["html_reports"] }
```

- [ ] **Step 2: Write the benchmark suite**

Create `benches/performance.rs`:

```rust
use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};
use rusqlite::Connection;
use std::sync::{Arc, Mutex};
use std::thread;

// Import from library crate
use maximous::db;
use maximous::tools;

fn setup() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    db::init_db(&conn).unwrap();
    conn
}

/// Benchmark: memory_set + memory_get round-trip
/// Target: <1ms average per operation pair
fn bench_memory_roundtrip(c: &mut Criterion) {
    let conn = setup();

    c.bench_function("memory_set+get", |b| {
        let mut i = 0u64;
        b.iter(|| {
            let key = format!("key-{}", i);
            tools::memory::set(
                &serde_json::json!({"namespace": "bench", "key": key, "value": "{\"data\":true}"}),
                &conn,
            );
            tools::memory::get(
                &serde_json::json!({"namespace": "bench", "key": key}),
                &conn,
            );
            i += 1;
        });
    });
}

/// Benchmark: memory_set throughput (pure writes)
/// Target: >10,000 ops/sec
fn bench_memory_write_throughput(c: &mut Criterion) {
    let conn = setup();

    c.bench_function("memory_set_throughput", |b| {
        let mut i = 0u64;
        b.iter(|| {
            let key = format!("tp-{}", i);
            tools::memory::set(
                &serde_json::json!({"namespace": "throughput", "key": key, "value": "{\"n\":1}"}),
                &conn,
            );
            i += 1;
        });
    });
}

/// Benchmark: message_send + message_read
/// Target: <1ms average
fn bench_message_roundtrip(c: &mut Criterion) {
    let conn = setup();

    c.bench_function("message_send+read", |b| {
        b.iter(|| {
            tools::messages::send(
                &serde_json::json!({"channel": "bench", "sender": "agent", "content": "{\"ping\":true}"}),
                &conn,
            );
            tools::messages::read(
                &serde_json::json!({"channel": "bench", "limit": 1}),
                &conn,
            );
        });
    });
}

/// Benchmark: poll_changes scaling
/// Insert N changes, then measure poll time
/// Target: <5ms for poll regardless of table size
fn bench_poll_changes_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("poll_changes_scaling");

    for size in [100, 1_000, 10_000, 50_000].iter() {
        let conn = setup();

        // Pre-populate with N changes via memory writes
        for i in 0..*size {
            conn.execute(
                "INSERT INTO changes (table_name, row_id, action, summary, created_at) VALUES ('bench', ?1, 'insert', '{}', 0)",
                rusqlite::params![format!("row-{}", i)],
            ).unwrap();
        }

        // Benchmark polling from near the end (last 100 changes)
        let since_id = (*size as i64) - 100;
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            size,
            |b, _| {
                b.iter(|| {
                    tools::changes::poll(
                        &serde_json::json!({"since_id": since_id, "limit": 100}),
                        &conn,
                    );
                });
            },
        );
    }
    group.finish();
}

/// Benchmark: task_create + dependency check
/// Target: <2ms per create with dependency validation
fn bench_task_with_deps(c: &mut Criterion) {
    let conn = setup();

    // Create a "done" dependency task
    let dep = tools::tasks::create(&serde_json::json!({"title": "dep"}), &conn);
    let dep_id = dep.data.unwrap()["id"].as_str().unwrap().to_string();
    tools::tasks::update(&serde_json::json!({"id": dep_id, "status": "done"}), &conn);

    c.bench_function("task_create_with_dep_check", |b| {
        let mut i = 0u64;
        b.iter(|| {
            let r = tools::tasks::create(
                &serde_json::json!({"title": format!("task-{}", i), "dependencies": [dep_id]}),
                &conn,
            );
            let id = r.data.unwrap()["id"].as_str().unwrap().to_string();
            // Validate dependency check when setting to ready
            tools::tasks::update(&serde_json::json!({"id": id, "status": "ready"}), &conn);
            i += 1;
        });
    });
}

/// Benchmark: memory_search over growing dataset
/// Target: <10ms for LIKE search over 10,000 entries
fn bench_memory_search_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_search_scaling");

    for size in [100, 1_000, 10_000].iter() {
        let conn = setup();

        for i in 0..*size {
            let value = if i % 100 == 0 {
                format!("{{\"data\":\"needle-{}\"}}", i)
            } else {
                format!("{{\"data\":\"haystack-{}\"}}", i)
            };
            conn.execute(
                "INSERT INTO memory (namespace, key, value, created_at, updated_at) VALUES ('search', ?1, ?2, 0, 0)",
                rusqlite::params![format!("k-{}", i), value],
            ).unwrap();
        }

        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            size,
            |b, _| {
                b.iter(|| {
                    tools::memory::search(
                        &serde_json::json!({"query": "needle", "namespace": "search"}),
                        &conn,
                    );
                });
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_memory_roundtrip,
    bench_memory_write_throughput,
    bench_message_roundtrip,
    bench_poll_changes_scaling,
    bench_task_with_deps,
    bench_memory_search_scaling,
);
criterion_main!(benches);
```

- [ ] **Step 3: Run benchmarks**

Run: `cargo bench`
Expected output includes timing for each benchmark. Verify against targets:

| Benchmark | Target |
|---|---|
| `memory_set+get` | <1ms |
| `memory_set_throughput` | >10,000 ops/sec |
| `message_send+read` | <1ms |
| `poll_changes_scaling/50000` | <5ms |
| `task_create_with_dep_check` | <2ms |
| `memory_search_scaling/10000` | <10ms |

- [ ] **Step 4: Write concurrent WAL stress test**

Add `tests/concurrent_test.rs`:

```rust
use rusqlite::Connection;
use std::sync::{Arc, Barrier};
use std::thread;

use maximous::db;
use maximous::tools;

/// Stress test: 4 threads doing concurrent writes via WAL
/// Verifies no SQLITE_BUSY errors and all writes succeed
#[test]
fn test_concurrent_wal_writes() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("concurrent.db");
    let db_path_str = db_path.to_str().unwrap().to_string();

    // Initialize DB
    let conn = db::open_db(&db_path_str).unwrap();
    drop(conn);

    let num_threads = 4;
    let ops_per_thread = 500;
    let barrier = Arc::new(Barrier::new(num_threads));

    let handles: Vec<_> = (0..num_threads)
        .map(|t| {
            let barrier = Arc::clone(&barrier);
            let path = db_path_str.clone();
            thread::spawn(move || {
                let conn = db::open_db(&path).unwrap();
                barrier.wait(); // All threads start at the same time

                let mut successes = 0;
                for i in 0..ops_per_thread {
                    let result = tools::memory::set(
                        &serde_json::json!({
                            "namespace": format!("thread-{}", t),
                            "key": format!("key-{}", i),
                            "value": format!("{{\"thread\":{},\"op\":{}}}", t, i)
                        }),
                        &conn,
                    );
                    if result.ok {
                        successes += 1;
                    }
                }
                successes
            })
        })
        .collect();

    let total_successes: usize = handles.into_iter().map(|h| h.join().unwrap()).sum();
    let expected = num_threads * ops_per_thread;
    assert_eq!(
        total_successes, expected,
        "Expected {} successful writes, got {} ({} failed)",
        expected, total_successes, expected - total_successes
    );

    // Verify all data is readable
    let conn = db::open_db(&db_path_str).unwrap();
    for t in 0..num_threads {
        let result = tools::memory::get(
            &serde_json::json!({"namespace": format!("thread-{}", t)}),
            &conn,
        );
        let keys = result.data.unwrap()["keys"].as_array().unwrap().len();
        assert_eq!(keys, ops_per_thread, "Thread {} should have {} keys", t, ops_per_thread);
    }
}

/// Stress test: concurrent reads and writes (mixed workload)
#[test]
fn test_concurrent_read_write_mix() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("mixed.db");
    let db_path_str = db_path.to_str().unwrap().to_string();

    let conn = db::open_db(&db_path_str).unwrap();
    // Pre-populate some data
    for i in 0..100 {
        tools::memory::set(
            &serde_json::json!({"namespace": "shared", "key": format!("pre-{}", i), "value": "initial"}),
            &conn,
        );
    }
    drop(conn);

    let barrier = Arc::new(Barrier::new(4));

    // 2 writer threads + 2 reader threads
    let mut handles = vec![];

    for t in 0..2 {
        let barrier = Arc::clone(&barrier);
        let path = db_path_str.clone();
        handles.push(thread::spawn(move || {
            let conn = db::open_db(&path).unwrap();
            barrier.wait();
            for i in 0..200 {
                tools::memory::set(
                    &serde_json::json!({"namespace": "shared", "key": format!("w{}-{}", t, i), "value": "written"}),
                    &conn,
                );
            }
            true
        }));
    }

    for _ in 0..2 {
        let barrier = Arc::clone(&barrier);
        let path = db_path_str.clone();
        handles.push(thread::spawn(move || {
            let conn = db::open_db(&path).unwrap();
            barrier.wait();
            for _ in 0..200 {
                tools::memory::get(
                    &serde_json::json!({"namespace": "shared"}),
                    &conn,
                );
                tools::changes::poll(
                    &serde_json::json!({"since_id": 0, "limit": 10}),
                    &conn,
                );
            }
            true
        }));
    }

    for h in handles {
        assert!(h.join().unwrap(), "Thread should complete without panic");
    }
}
```

- [ ] **Step 5: Run concurrent tests**

Run: `cargo test --test concurrent_test -- --test-threads=1`
Expected: Both tests pass with zero failures.

- [ ] **Step 6: Commit**

```bash
git add benches/performance.rs tests/concurrent_test.rs Cargo.toml
git commit -m "feat: add performance benchmarks and concurrent WAL stress tests"
```
