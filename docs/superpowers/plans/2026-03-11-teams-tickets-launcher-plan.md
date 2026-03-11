# Teams, Tickets & Launcher Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace unused message tools with agent registry + teams, add ticket caching from Linear/Jira, and build a launcher that deploys teams to parallel worktrees with PR creation.

**Architecture:** Rust MCP server with SQLite. New tables + tools replace messages. Dashboard (vanilla JS + Axum) gets new pages. All changes follow existing patterns (ToolResult, dispatch_tool, changes triggers, API endpoints).

**Tech Stack:** Rust, rusqlite, axum, serde_json, vanilla JS, SQLite

**Spec:** `docs/superpowers/specs/2026-03-11-teams-tickets-launcher-design.md`

---

## Chunk 1: Schema Migration — Remove Messages, Add New Tables

### Task 1: Update schema.sql — remove messages, add agent_definitions + teams + team_members

**Files:**
- Modify: `src/schema.sql`
- Modify: `src/db.rs:23-33` (migration function)

- [ ] **Step 1: Write failing test for new tables**

Create `tests/teams_test.rs`:

```rust
use rusqlite::Connection;
use maximous::db;

fn setup() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    db::init_db(&conn).unwrap();
    conn
}

#[test]
fn test_agent_definitions_table_exists() {
    let conn = setup();
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM agent_definitions", [], |r| r.get(0)
    ).unwrap();
    assert_eq!(count, 0);
}

#[test]
fn test_teams_table_exists() {
    let conn = setup();
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM teams", [], |r| r.get(0)
    ).unwrap();
    assert_eq!(count, 0);
}

#[test]
fn test_team_members_table_exists() {
    let conn = setup();
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM team_members", [], |r| r.get(0)
    ).unwrap();
    assert_eq!(count, 0);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test teams_test -- --nocapture`
Expected: FAIL — `no such table: agent_definitions`

- [ ] **Step 3: Update schema.sql**

In `src/schema.sql`, replace the messages table (lines 14-23) and its indexes (lines 65-66) and triggers (lines 99-121) with:

```sql
-- Agent definitions (reusable agent configs)
CREATE TABLE IF NOT EXISTS agent_definitions (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    capabilities TEXT NOT NULL DEFAULT '[]',
    model TEXT NOT NULL DEFAULT 'sonnet',
    prompt_hint TEXT NOT NULL DEFAULT '',
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    updated_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
);

-- Teams
CREATE TABLE IF NOT EXISTS teams (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    description TEXT NOT NULL DEFAULT '',
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    updated_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
);

-- Team members (many-to-many with role)
CREATE TABLE IF NOT EXISTS team_members (
    team_id TEXT NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
    agent_id TEXT NOT NULL REFERENCES agent_definitions(id) ON DELETE CASCADE,
    role TEXT NOT NULL DEFAULT '',
    PRIMARY KEY (team_id, agent_id)
);
```

Replace message indexes with:

```sql
CREATE INDEX IF NOT EXISTS idx_agent_definitions_name ON agent_definitions(name);
CREATE INDEX IF NOT EXISTS idx_teams_name ON teams(name);
CREATE INDEX IF NOT EXISTS idx_team_members_agent ON team_members(agent_id);
```

Replace message triggers with triggers for the 3 new tables (insert/update/delete for agent_definitions and teams, insert/delete for team_members):

```sql
CREATE TRIGGER IF NOT EXISTS trg_agent_definitions_insert AFTER INSERT ON agent_definitions
BEGIN
    INSERT INTO changes (table_name, row_id, action, summary, created_at)
    VALUES ('agent_definitions', NEW.id, 'insert',
            json_object('name', NEW.name, 'model', NEW.model),
            strftime('%s', 'now'));
END;

CREATE TRIGGER IF NOT EXISTS trg_agent_definitions_update AFTER UPDATE ON agent_definitions
BEGIN
    INSERT INTO changes (table_name, row_id, action, summary, created_at)
    VALUES ('agent_definitions', NEW.id, 'update',
            json_object('name', NEW.name, 'model', NEW.model),
            strftime('%s', 'now'));
END;

CREATE TRIGGER IF NOT EXISTS trg_agent_definitions_delete AFTER DELETE ON agent_definitions
BEGIN
    INSERT INTO changes (table_name, row_id, action, summary, created_at)
    VALUES ('agent_definitions', OLD.id, 'delete',
            json_object('name', OLD.name),
            strftime('%s', 'now'));
END;

CREATE TRIGGER IF NOT EXISTS trg_teams_insert AFTER INSERT ON teams
BEGIN
    INSERT INTO changes (table_name, row_id, action, summary, created_at)
    VALUES ('teams', NEW.id, 'insert',
            json_object('name', NEW.name),
            strftime('%s', 'now'));
END;

CREATE TRIGGER IF NOT EXISTS trg_teams_update AFTER UPDATE ON teams
BEGIN
    INSERT INTO changes (table_name, row_id, action, summary, created_at)
    VALUES ('teams', NEW.id, 'update',
            json_object('name', NEW.name),
            strftime('%s', 'now'));
END;

CREATE TRIGGER IF NOT EXISTS trg_teams_delete AFTER DELETE ON teams
BEGIN
    INSERT INTO changes (table_name, row_id, action, summary, created_at)
    VALUES ('teams', OLD.id, 'delete',
            json_object('name', OLD.name),
            strftime('%s', 'now'));
END;

CREATE TRIGGER IF NOT EXISTS trg_team_members_insert AFTER INSERT ON team_members
BEGIN
    INSERT INTO changes (table_name, row_id, action, summary, created_at)
    VALUES ('team_members', NEW.team_id || ':' || NEW.agent_id, 'insert',
            json_object('team_id', NEW.team_id, 'agent_id', NEW.agent_id, 'role', NEW.role),
            strftime('%s', 'now'));
END;

CREATE TRIGGER IF NOT EXISTS trg_team_members_delete AFTER DELETE ON team_members
BEGIN
    INSERT INTO changes (table_name, row_id, action, summary, created_at)
    VALUES ('team_members', OLD.team_id || ':' || OLD.agent_id, 'delete',
            json_object('team_id', OLD.team_id, 'agent_id', OLD.agent_id),
            strftime('%s', 'now'));
END;
```

- [ ] **Step 4: Add migration for existing databases**

In `src/db.rs`, add to the `migrate()` function after the existing migration:

```rust
// Migration: add agent_definitions, teams, team_members tables
let has_agent_defs: bool = conn
    .prepare("SELECT id FROM agent_definitions LIMIT 0")
    .is_ok();
if !has_agent_defs {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS agent_definitions (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            capabilities TEXT NOT NULL DEFAULT '[]',
            model TEXT NOT NULL DEFAULT 'sonnet',
            prompt_hint TEXT NOT NULL DEFAULT '',
            created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
            updated_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
        );
        CREATE TABLE IF NOT EXISTS teams (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL UNIQUE,
            description TEXT NOT NULL DEFAULT '',
            created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
            updated_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
        );
        CREATE TABLE IF NOT EXISTS team_members (
            team_id TEXT NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
            agent_id TEXT NOT NULL REFERENCES agent_definitions(id) ON DELETE CASCADE,
            role TEXT NOT NULL DEFAULT '',
            PRIMARY KEY (team_id, agent_id)
        );
        CREATE INDEX IF NOT EXISTS idx_agent_definitions_name ON agent_definitions(name);
        CREATE INDEX IF NOT EXISTS idx_teams_name ON teams(name);
        CREATE INDEX IF NOT EXISTS idx_team_members_agent ON team_members(agent_id);"
    )?;
}
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test --test teams_test -- --nocapture`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add src/schema.sql src/db.rs tests/teams_test.rs
git commit -m "feat: replace messages table with agent_definitions, teams, team_members"
```

### Task 2: Add tickets + launches tables to schema

**Files:**
- Modify: `src/schema.sql`
- Modify: `src/db.rs:23-33`

- [ ] **Step 1: Write failing test**

Add to `tests/teams_test.rs`:

```rust
#[test]
fn test_tickets_table_exists() {
    let conn = setup();
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM tickets", [], |r| r.get(0)
    ).unwrap();
    assert_eq!(count, 0);
}

#[test]
fn test_launches_table_exists() {
    let conn = setup();
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM launches", [], |r| r.get(0)
    ).unwrap();
    assert_eq!(count, 0);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test teams_test test_tickets_table_exists test_launches_table_exists -- --nocapture`
Expected: FAIL

- [ ] **Step 3: Add tickets and launches tables to schema.sql**

Append to `src/schema.sql`:

```sql
-- Cached tickets from Linear/Jira
CREATE TABLE IF NOT EXISTS tickets (
    id TEXT PRIMARY KEY,
    source TEXT NOT NULL,
    external_id TEXT NOT NULL,
    title TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL,
    priority INTEGER NOT NULL DEFAULT 2,
    url TEXT NOT NULL DEFAULT '',
    labels TEXT NOT NULL DEFAULT '[]',
    metadata TEXT NOT NULL DEFAULT '{}',
    fetched_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    updated_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    UNIQUE(source, external_id)
);

-- Ticket launches (worktree deployments)
CREATE TABLE IF NOT EXISTS launches (
    id TEXT PRIMARY KEY,
    ticket_id TEXT NOT NULL REFERENCES tickets(id),
    team_id TEXT NOT NULL REFERENCES teams(id),
    branch TEXT NOT NULL,
    worktree_path TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'pending',
    pr_url TEXT NOT NULL DEFAULT '',
    error TEXT NOT NULL DEFAULT '',
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    updated_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_tickets_source ON tickets(source, status);
CREATE INDEX IF NOT EXISTS idx_launches_status ON launches(status);
CREATE INDEX IF NOT EXISTS idx_launches_ticket ON launches(ticket_id);

-- Triggers for tickets
CREATE TRIGGER IF NOT EXISTS trg_tickets_insert AFTER INSERT ON tickets
BEGIN
    INSERT INTO changes (table_name, row_id, action, summary, created_at)
    VALUES ('tickets', NEW.id, 'insert',
            json_object('title', NEW.title, 'source', NEW.source, 'status', NEW.status),
            strftime('%s', 'now'));
END;

CREATE TRIGGER IF NOT EXISTS trg_tickets_update AFTER UPDATE ON tickets
BEGIN
    INSERT INTO changes (table_name, row_id, action, summary, created_at)
    VALUES ('tickets', NEW.id, 'update',
            json_object('title', NEW.title, 'source', NEW.source, 'status', NEW.status),
            strftime('%s', 'now'));
END;

CREATE TRIGGER IF NOT EXISTS trg_tickets_delete AFTER DELETE ON tickets
BEGIN
    INSERT INTO changes (table_name, row_id, action, summary, created_at)
    VALUES ('tickets', OLD.id, 'delete',
            json_object('title', OLD.title, 'source', OLD.source),
            strftime('%s', 'now'));
END;

-- Triggers for launches
CREATE TRIGGER IF NOT EXISTS trg_launches_insert AFTER INSERT ON launches
BEGIN
    INSERT INTO changes (table_name, row_id, action, summary, created_at)
    VALUES ('launches', NEW.id, 'insert',
            json_object('ticket_id', NEW.ticket_id, 'team_id', NEW.team_id, 'status', NEW.status),
            strftime('%s', 'now'));
END;

CREATE TRIGGER IF NOT EXISTS trg_launches_update AFTER UPDATE ON launches
BEGIN
    INSERT INTO changes (table_name, row_id, action, summary, created_at)
    VALUES ('launches', NEW.id, 'update',
            json_object('ticket_id', NEW.ticket_id, 'status', NEW.status, 'pr_url', NEW.pr_url),
            strftime('%s', 'now'));
END;

CREATE TRIGGER IF NOT EXISTS trg_launches_delete AFTER DELETE ON launches
BEGIN
    INSERT INTO changes (table_name, row_id, action, summary, created_at)
    VALUES ('launches', OLD.id, 'delete',
            json_object('ticket_id', OLD.ticket_id),
            strftime('%s', 'now'));
END;
```

Also add migration in `src/db.rs` for existing databases (same pattern).

- [ ] **Step 4: Run tests**

Run: `cargo test --test teams_test -- --nocapture`
Expected: ALL PASS

- [ ] **Step 5: Commit**

```bash
git add src/schema.sql src/db.rs tests/teams_test.rs
git commit -m "feat: add tickets and launches tables to schema"
```

---

## Chunk 2: Rust Tool Implementations — Agent Definitions & Teams

### Task 3: Implement agent_define, agent_catalog, agent_remove tools

**Files:**
- Create: `src/tools/definitions.rs`
- Modify: `src/tools/mod.rs:1-52`

- [ ] **Step 1: Write failing tests**

Create `tests/definitions_test.rs`:

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
fn test_agent_define_create() {
    let conn = setup();
    let result = tools::definitions::define(
        &serde_json::json!({
            "id": "frontend-dev",
            "name": "Frontend Developer",
            "capabilities": ["typescript", "react"],
            "model": "sonnet",
            "prompt_hint": "Build React components"
        }),
        &conn,
    );
    assert!(result.ok);
    let data = result.data.unwrap();
    assert_eq!(data["agent"]["id"], "frontend-dev");
}

#[test]
fn test_agent_define_upsert() {
    let conn = setup();
    tools::definitions::define(
        &serde_json::json!({"id": "dev", "name": "Dev v1"}),
        &conn,
    );
    let result = tools::definitions::define(
        &serde_json::json!({"id": "dev", "name": "Dev v2", "model": "opus"}),
        &conn,
    );
    assert!(result.ok);
    let data = result.data.unwrap();
    assert_eq!(data["agent"]["name"], "Dev v2");
    assert_eq!(data["agent"]["model"], "opus");
}

#[test]
fn test_agent_catalog() {
    let conn = setup();
    tools::definitions::define(&serde_json::json!({"id": "a", "name": "Agent A"}), &conn);
    tools::definitions::define(&serde_json::json!({"id": "b", "name": "Agent B"}), &conn);
    let result = tools::definitions::catalog(&serde_json::json!({}), &conn);
    assert!(result.ok);
    let data = result.data.unwrap();
    assert_eq!(data["agents"].as_array().unwrap().len(), 2);
}

#[test]
fn test_agent_remove() {
    let conn = setup();
    tools::definitions::define(&serde_json::json!({"id": "x", "name": "X"}), &conn);
    let result = tools::definitions::remove(&serde_json::json!({"id": "x"}), &conn);
    assert!(result.ok);
    assert_eq!(result.data.unwrap()["removed"], true);
    let result = tools::definitions::catalog(&serde_json::json!({}), &conn);
    assert_eq!(result.data.unwrap()["agents"].as_array().unwrap().len(), 0);
}

#[test]
fn test_agent_define_missing_fields() {
    let conn = setup();
    let result = tools::definitions::define(&serde_json::json!({"id": "x"}), &conn);
    assert!(!result.ok);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test definitions_test -- --nocapture`
Expected: FAIL — module not found

- [ ] **Step 3: Create `src/tools/definitions.rs`**

```rust
use rusqlite::Connection;
use serde_json::Value;
use super::ToolResult;

pub fn define(args: &Value, conn: &Connection) -> ToolResult {
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
        .map(|c| serde_json::to_string(c).unwrap())
        .unwrap_or_else(|| "[]".to_string());
    let model = args["model"].as_str().unwrap_or("sonnet");
    let prompt_hint = args["prompt_hint"].as_str().unwrap_or("");

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;

    match conn.execute(
        "INSERT INTO agent_definitions (id, name, capabilities, model, prompt_hint, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)
         ON CONFLICT(id) DO UPDATE SET name=?2, capabilities=?3, model=?4, prompt_hint=?5, updated_at=?6",
        rusqlite::params![id, name, capabilities, model, prompt_hint, now],
    ) {
        Ok(_) => ToolResult::success(serde_json::json!({
            "agent": {
                "id": id, "name": name, "capabilities": capabilities,
                "model": model, "prompt_hint": prompt_hint
            }
        })),
        Err(e) => ToolResult::fail(&format!("db error: {}", e)),
    }
}

pub fn catalog(args: &Value, conn: &Connection) -> ToolResult {
    let limit = args["limit"].as_i64().unwrap_or(100);
    let offset = args["offset"].as_i64().unwrap_or(0);

    let mut stmt = match conn.prepare(
        "SELECT id, name, capabilities, model, prompt_hint, created_at, updated_at
         FROM agent_definitions ORDER BY name LIMIT ?1 OFFSET ?2"
    ) {
        Ok(s) => s,
        Err(e) => return ToolResult::fail(&format!("db error: {}", e)),
    };

    let agents: Vec<Value> = stmt.query_map(rusqlite::params![limit, offset], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, String>(0)?,
            "name": row.get::<_, String>(1)?,
            "capabilities": row.get::<_, String>(2)?,
            "model": row.get::<_, String>(3)?,
            "prompt_hint": row.get::<_, String>(4)?,
            "created_at": row.get::<_, i64>(5)?,
            "updated_at": row.get::<_, i64>(6)?,
        }))
    }).unwrap().filter_map(|r| r.ok()).collect();

    let count = agents.len();
    ToolResult::success(serde_json::json!({"agents": agents, "count": count}))
}

pub fn remove(args: &Value, conn: &Connection) -> ToolResult {
    let id = match args["id"].as_str() {
        Some(s) => s,
        None => return ToolResult::fail("missing required field: id"),
    };

    match conn.execute("DELETE FROM agent_definitions WHERE id = ?1", rusqlite::params![id]) {
        Ok(deleted) => ToolResult::success(serde_json::json!({"removed": deleted > 0})),
        Err(e) => ToolResult::fail(&format!("db error: {}", e)),
    }
}
```

- [ ] **Step 4: Update `src/tools/mod.rs`**

Replace `pub mod messages;` with `pub mod definitions;` and add dispatch entries:

```rust
pub mod definitions;
```

In `dispatch_tool`, remove the 3 message entries and add:

```rust
"agent_define" => definitions::define(args, conn),
"agent_catalog" => definitions::catalog(args, conn),
"agent_remove" => definitions::remove(args, conn),
```

- [ ] **Step 5: Run tests**

Run: `cargo test --test definitions_test -- --nocapture`
Expected: ALL PASS

- [ ] **Step 6: Commit**

```bash
git add src/tools/definitions.rs src/tools/mod.rs tests/definitions_test.rs
git commit -m "feat: add agent_define, agent_catalog, agent_remove tools"
```

### Task 4: Implement team_create, team_list, team_delete tools

**Files:**
- Create: `src/tools/teams.rs`
- Modify: `src/tools/mod.rs`

- [ ] **Step 1: Write failing tests**

Create `tests/team_tools_test.rs`:

```rust
use rusqlite::Connection;
use maximous::db;
use maximous::tools;

fn setup() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    db::init_db(&conn).unwrap();
    conn
}

fn create_agent(conn: &Connection, id: &str, name: &str) {
    tools::definitions::define(
        &serde_json::json!({"id": id, "name": name}),
        conn,
    );
}

#[test]
fn test_team_create_empty() {
    let conn = setup();
    let result = tools::teams::create(
        &serde_json::json!({"name": "frontend-squad", "description": "Frontend team"}),
        &conn,
    );
    assert!(result.ok);
    let data = result.data.unwrap();
    assert_eq!(data["team"]["name"], "frontend-squad");
}

#[test]
fn test_team_create_with_members() {
    let conn = setup();
    create_agent(&conn, "dev", "Developer");
    create_agent(&conn, "tester", "Tester");
    let result = tools::teams::create(
        &serde_json::json!({
            "name": "my-team",
            "members": [
                {"agent_id": "dev", "role": "implementer"},
                {"agent_id": "tester", "role": "tester"}
            ]
        }),
        &conn,
    );
    assert!(result.ok);
    let data = result.data.unwrap();
    assert_eq!(data["team"]["members"].as_array().unwrap().len(), 2);
}

#[test]
fn test_team_list() {
    let conn = setup();
    tools::teams::create(&serde_json::json!({"name": "team-a"}), &conn);
    tools::teams::create(&serde_json::json!({"name": "team-b"}), &conn);
    let result = tools::teams::list(&serde_json::json!({}), &conn);
    assert!(result.ok);
    assert_eq!(result.data.unwrap()["teams"].as_array().unwrap().len(), 2);
}

#[test]
fn test_team_list_includes_members() {
    let conn = setup();
    create_agent(&conn, "dev", "Developer");
    tools::teams::create(
        &serde_json::json!({"name": "squad", "members": [{"agent_id": "dev", "role": "impl"}]}),
        &conn,
    );
    let result = tools::teams::list(&serde_json::json!({}), &conn);
    let teams = result.data.unwrap();
    let team = &teams["teams"].as_array().unwrap()[0];
    assert_eq!(team["members"].as_array().unwrap().len(), 1);
    assert_eq!(team["members"][0]["role"], "impl");
}

#[test]
fn test_team_delete() {
    let conn = setup();
    tools::teams::create(&serde_json::json!({"name": "doomed"}), &conn);
    let result = tools::teams::delete(&serde_json::json!({"name": "doomed"}), &conn);
    assert!(result.ok);
    assert_eq!(result.data.unwrap()["removed"], true);
}

#[test]
fn test_team_create_duplicate_name_fails() {
    let conn = setup();
    tools::teams::create(&serde_json::json!({"name": "dup"}), &conn);
    let result = tools::teams::create(&serde_json::json!({"name": "dup"}), &conn);
    assert!(!result.ok);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test team_tools_test -- --nocapture`
Expected: FAIL

- [ ] **Step 3: Create `src/tools/teams.rs`**

```rust
use rusqlite::Connection;
use serde_json::Value;
use super::ToolResult;

pub fn create(args: &Value, conn: &Connection) -> ToolResult {
    let name = match args["name"].as_str() {
        Some(s) => s,
        None => return ToolResult::fail("missing required field: name"),
    };
    let description = args["description"].as_str().unwrap_or("");
    let id = uuid::Uuid::new_v4().to_string();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;

    if let Err(e) = conn.execute(
        "INSERT INTO teams (id, name, description, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?4)",
        rusqlite::params![id, name, description, now],
    ) {
        return ToolResult::fail(&format!("db error (name may already exist): {}", e));
    }

    // Add members if provided
    let mut members = Vec::new();
    if let Some(member_list) = args["members"].as_array() {
        for m in member_list {
            let agent_id = match m["agent_id"].as_str() {
                Some(s) => s,
                None => continue,
            };
            let role = m["role"].as_str().unwrap_or("");
            if let Err(e) = conn.execute(
                "INSERT INTO team_members (team_id, agent_id, role) VALUES (?1, ?2, ?3)",
                rusqlite::params![id, agent_id, role],
            ) {
                return ToolResult::fail(&format!("error adding member {}: {}", agent_id, e));
            }
            members.push(serde_json::json!({"agent_id": agent_id, "role": role}));
        }
    }

    ToolResult::success(serde_json::json!({
        "team": {"id": id, "name": name, "description": description, "members": members}
    }))
}

pub fn list(args: &Value, conn: &Connection) -> ToolResult {
    let limit = args["limit"].as_i64().unwrap_or(100);
    let offset = args["offset"].as_i64().unwrap_or(0);

    let mut stmt = match conn.prepare(
        "SELECT id, name, description, created_at, updated_at
         FROM teams ORDER BY name LIMIT ?1 OFFSET ?2"
    ) {
        Ok(s) => s,
        Err(e) => return ToolResult::fail(&format!("db error: {}", e)),
    };

    let teams: Vec<Value> = stmt.query_map(rusqlite::params![limit, offset], |row| {
        let team_id: String = row.get(0)?;
        Ok((team_id, row.get::<_, String>(1)?, row.get::<_, String>(2)?,
            row.get::<_, i64>(3)?, row.get::<_, i64>(4)?))
    }).unwrap().filter_map(|r| r.ok()).map(|(team_id, name, desc, created, updated)| {
        // Fetch members for this team
        let mut member_stmt = conn.prepare(
            "SELECT tm.agent_id, tm.role, ad.name, ad.capabilities, ad.model, ad.prompt_hint
             FROM team_members tm
             JOIN agent_definitions ad ON ad.id = tm.agent_id
             WHERE tm.team_id = ?1"
        ).unwrap();
        let members: Vec<Value> = member_stmt.query_map(rusqlite::params![team_id], |row| {
            Ok(serde_json::json!({
                "agent_id": row.get::<_, String>(0)?,
                "role": row.get::<_, String>(1)?,
                "name": row.get::<_, String>(2)?,
                "capabilities": row.get::<_, String>(3)?,
                "model": row.get::<_, String>(4)?,
                "prompt_hint": row.get::<_, String>(5)?,
            }))
        }).unwrap().filter_map(|r| r.ok()).collect();

        serde_json::json!({
            "id": team_id, "name": name, "description": desc,
            "members": members, "created_at": created, "updated_at": updated
        })
    }).collect();

    let count = teams.len();
    ToolResult::success(serde_json::json!({"teams": teams, "count": count}))
}

pub fn delete(args: &Value, conn: &Connection) -> ToolResult {
    let name = match args["name"].as_str() {
        Some(s) => s,
        None => return ToolResult::fail("missing required field: name"),
    };

    match conn.execute("DELETE FROM teams WHERE name = ?1", rusqlite::params![name]) {
        Ok(deleted) => ToolResult::success(serde_json::json!({"removed": deleted > 0})),
        Err(e) => ToolResult::fail(&format!("db error: {}", e)),
    }
}
```

- [ ] **Step 4: Update `src/tools/mod.rs`**

Add `pub mod teams;` and dispatch entries:

```rust
"team_create" => teams::create(args, conn),
"team_list" => teams::list(args, conn),
"team_delete" => teams::delete(args, conn),
```

- [ ] **Step 5: Run tests**

Run: `cargo test --test team_tools_test -- --nocapture`
Expected: ALL PASS

- [ ] **Step 6: Commit**

```bash
git add src/tools/teams.rs src/tools/mod.rs tests/team_tools_test.rs
git commit -m "feat: add team_create, team_list, team_delete tools"
```

---

## Chunk 3: Rust Tool Implementations — Tickets & Launches

### Task 5: Implement ticket_cache, ticket_list, ticket_clear tools

**Files:**
- Create: `src/tools/tickets.rs`
- Modify: `src/tools/mod.rs`

- [ ] **Step 1: Write failing tests**

Create `tests/tickets_test.rs`:

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
fn test_ticket_cache() {
    let conn = setup();
    let result = tools::tickets::cache(
        &serde_json::json!({
            "id": "t1", "source": "linear", "external_id": "LIN-123",
            "title": "Fix auth bug", "status": "todo", "url": "https://linear.app/..."
        }),
        &conn,
    );
    assert!(result.ok);
}

#[test]
fn test_ticket_cache_upsert() {
    let conn = setup();
    tools::tickets::cache(
        &serde_json::json!({"id": "t1", "source": "linear", "external_id": "LIN-1", "title": "v1", "status": "todo"}),
        &conn,
    );
    tools::tickets::cache(
        &serde_json::json!({"id": "t1", "source": "linear", "external_id": "LIN-1", "title": "v2", "status": "backlog"}),
        &conn,
    );
    let result = tools::tickets::list(&serde_json::json!({}), &conn);
    let tickets = result.data.unwrap();
    assert_eq!(tickets["tickets"].as_array().unwrap().len(), 1);
    assert_eq!(tickets["tickets"][0]["title"], "v2");
}

#[test]
fn test_ticket_list_filter_source() {
    let conn = setup();
    tools::tickets::cache(&serde_json::json!({"id": "t1", "source": "linear", "external_id": "L1", "title": "A", "status": "todo"}), &conn);
    tools::tickets::cache(&serde_json::json!({"id": "t2", "source": "jira", "external_id": "J1", "title": "B", "status": "todo"}), &conn);
    let result = tools::tickets::list(&serde_json::json!({"source": "linear"}), &conn);
    assert_eq!(result.data.unwrap()["tickets"].as_array().unwrap().len(), 1);
}

#[test]
fn test_ticket_list_filter_status() {
    let conn = setup();
    tools::tickets::cache(&serde_json::json!({"id": "t1", "source": "linear", "external_id": "L1", "title": "A", "status": "todo"}), &conn);
    tools::tickets::cache(&serde_json::json!({"id": "t2", "source": "linear", "external_id": "L2", "title": "B", "status": "backlog"}), &conn);
    let result = tools::tickets::list(&serde_json::json!({"status": "todo"}), &conn);
    assert_eq!(result.data.unwrap()["tickets"].as_array().unwrap().len(), 1);
}

#[test]
fn test_ticket_clear() {
    let conn = setup();
    tools::tickets::cache(&serde_json::json!({"id": "t1", "source": "linear", "external_id": "L1", "title": "A", "status": "todo"}), &conn);
    tools::tickets::cache(&serde_json::json!({"id": "t2", "source": "jira", "external_id": "J1", "title": "B", "status": "todo"}), &conn);
    let result = tools::tickets::clear(&serde_json::json!({"source": "linear"}), &conn);
    assert_eq!(result.data.unwrap()["cleared"], 1);
    let result = tools::tickets::list(&serde_json::json!({}), &conn);
    assert_eq!(result.data.unwrap()["tickets"].as_array().unwrap().len(), 1);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test tickets_test -- --nocapture`
Expected: FAIL

- [ ] **Step 3: Create `src/tools/tickets.rs`**

```rust
use rusqlite::Connection;
use serde_json::Value;
use super::ToolResult;

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
    let priority = args["priority"].as_i64().unwrap_or(2);
    let url = args["url"].as_str().unwrap_or("");
    let labels = args.get("labels")
        .filter(|l| l.is_array())
        .map(|l| serde_json::to_string(l).unwrap())
        .unwrap_or_else(|| "[]".to_string());
    let metadata = args.get("metadata")
        .filter(|m| m.is_object())
        .map(|m| serde_json::to_string(m).unwrap())
        .unwrap_or_else(|| "{}".to_string());

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;

    match conn.execute(
        "INSERT INTO tickets (id, source, external_id, title, description, status, priority, url, labels, metadata, fetched_at, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11, ?11)
         ON CONFLICT(id) DO UPDATE SET title=?4, description=?5, status=?6, priority=?7, url=?8, labels=?9, metadata=?10, fetched_at=?11, updated_at=?11",
        rusqlite::params![id, source, external_id, title, description, status, priority, url, labels, metadata, now],
    ) {
        Ok(_) => ToolResult::success(serde_json::json!({"cached": true, "id": id})),
        Err(e) => ToolResult::fail(&format!("db error: {}", e)),
    }
}

pub fn list(args: &Value, conn: &Connection) -> ToolResult {
    let source = args["source"].as_str();
    let status = args["status"].as_str();
    let limit = args["limit"].as_i64().unwrap_or(100);
    let offset = args["offset"].as_i64().unwrap_or(0);

    let mut conditions = vec!["1=1".to_string()];
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(s) = source {
        params.push(Box::new(s.to_string()));
        conditions.push(format!("source = ?{}", params.len()));
    }
    if let Some(s) = status {
        params.push(Box::new(s.to_string()));
        conditions.push(format!("status = ?{}", params.len()));
    }
    params.push(Box::new(limit));
    let limit_idx = params.len();
    params.push(Box::new(offset));
    let offset_idx = params.len();

    let sql = format!(
        "SELECT id, source, external_id, title, description, status, priority, url, labels, metadata, fetched_at
         FROM tickets WHERE {} ORDER BY priority ASC, fetched_at DESC LIMIT ?{} OFFSET ?{}",
        conditions.join(" AND "), limit_idx, offset_idx
    );

    let mut stmt = match conn.prepare(&sql) {
        Ok(s) => s,
        Err(e) => return ToolResult::fail(&format!("db error: {}", e)),
    };

    let params_ref: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let tickets: Vec<Value> = stmt.query_map(params_ref.as_slice(), |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, String>(0)?,
            "source": row.get::<_, String>(1)?,
            "external_id": row.get::<_, String>(2)?,
            "title": row.get::<_, String>(3)?,
            "description": row.get::<_, String>(4)?,
            "status": row.get::<_, String>(5)?,
            "priority": row.get::<_, i64>(6)?,
            "url": row.get::<_, String>(7)?,
            "labels": row.get::<_, String>(8)?,
            "metadata": row.get::<_, String>(9)?,
            "fetched_at": row.get::<_, i64>(10)?,
        }))
    }).unwrap().filter_map(|r| r.ok()).collect();

    let count = tickets.len();
    ToolResult::success(serde_json::json!({"tickets": tickets, "count": count}))
}

pub fn clear(args: &Value, conn: &Connection) -> ToolResult {
    let source = args["source"].as_str();

    let result = if let Some(s) = source {
        conn.execute("DELETE FROM tickets WHERE source = ?1", rusqlite::params![s])
    } else {
        conn.execute("DELETE FROM tickets", [])
    };

    match result {
        Ok(cleared) => ToolResult::success(serde_json::json!({"cleared": cleared})),
        Err(e) => ToolResult::fail(&format!("db error: {}", e)),
    }
}
```

- [ ] **Step 4: Update `src/tools/mod.rs`**

Add `pub mod tickets;` and dispatch:

```rust
"ticket_cache" => tickets::cache(args, conn),
"ticket_list" => tickets::list(args, conn),
"ticket_clear" => tickets::clear(args, conn),
```

- [ ] **Step 5: Run tests**

Run: `cargo test --test tickets_test -- --nocapture`
Expected: ALL PASS

- [ ] **Step 6: Commit**

```bash
git add src/tools/tickets.rs src/tools/mod.rs tests/tickets_test.rs
git commit -m "feat: add ticket_cache, ticket_list, ticket_clear tools"
```

### Task 6: Implement launch_create, launch_update, launch_list tools

**Files:**
- Create: `src/tools/launches.rs`
- Modify: `src/tools/mod.rs`

- [ ] **Step 1: Write failing tests**

Create `tests/launches_test.rs`:

```rust
use rusqlite::Connection;
use maximous::db;
use maximous::tools;

fn setup() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    db::init_db(&conn).unwrap();
    conn
}

fn seed(conn: &Connection) -> (String, String) {
    // Create agent + team + ticket
    tools::definitions::define(&serde_json::json!({"id": "dev", "name": "Dev"}), conn);
    let team_result = tools::teams::create(&serde_json::json!({"name": "squad", "members": [{"agent_id": "dev", "role": "impl"}]}), conn);
    let team_id = team_result.data.unwrap()["team"]["id"].as_str().unwrap().to_string();
    tools::tickets::cache(&serde_json::json!({"id": "t1", "source": "linear", "external_id": "L1", "title": "Fix bug", "status": "todo"}), conn);
    (team_id, "t1".to_string())
}

#[test]
fn test_launch_create() {
    let conn = setup();
    let (team_id, ticket_id) = seed(&conn);
    let result = tools::launches::create(
        &serde_json::json!({"ticket_id": ticket_id, "team_id": team_id, "branch": "feat/fix-bug"}),
        &conn,
    );
    assert!(result.ok);
    let data = result.data.unwrap();
    assert_eq!(data["launch"]["status"], "pending");
}

#[test]
fn test_launch_update_status() {
    let conn = setup();
    let (team_id, ticket_id) = seed(&conn);
    let result = tools::launches::create(
        &serde_json::json!({"ticket_id": ticket_id, "team_id": team_id, "branch": "feat/x"}),
        &conn,
    );
    let launch_id = result.data.unwrap()["launch"]["id"].as_str().unwrap().to_string();
    let result = tools::launches::update(
        &serde_json::json!({"id": launch_id, "status": "running", "worktree_path": "/tmp/wt"}),
        &conn,
    );
    assert!(result.ok);
    assert_eq!(result.data.unwrap()["launch"]["status"], "running");
}

#[test]
fn test_launch_update_pr() {
    let conn = setup();
    let (team_id, ticket_id) = seed(&conn);
    let result = tools::launches::create(
        &serde_json::json!({"ticket_id": ticket_id, "team_id": team_id, "branch": "feat/x"}),
        &conn,
    );
    let launch_id = result.data.unwrap()["launch"]["id"].as_str().unwrap().to_string();
    tools::launches::update(
        &serde_json::json!({"id": launch_id, "status": "pr_created", "pr_url": "https://github.com/org/repo/pull/42"}),
        &conn,
    );
    let result = tools::launches::list(&serde_json::json!({}), &conn);
    let launches = result.data.unwrap();
    assert_eq!(launches["launches"][0]["pr_url"], "https://github.com/org/repo/pull/42");
}

#[test]
fn test_launch_list_filter_status() {
    let conn = setup();
    let (team_id, ticket_id) = seed(&conn);
    tools::launches::create(&serde_json::json!({"ticket_id": ticket_id, "team_id": team_id, "branch": "b1"}), &conn);
    let r2 = tools::launches::create(&serde_json::json!({"ticket_id": ticket_id, "team_id": team_id, "branch": "b2"}), &conn);
    let id2 = r2.data.unwrap()["launch"]["id"].as_str().unwrap().to_string();
    tools::launches::update(&serde_json::json!({"id": id2, "status": "running"}), &conn);
    let result = tools::launches::list(&serde_json::json!({"status": "running"}), &conn);
    assert_eq!(result.data.unwrap()["launches"].as_array().unwrap().len(), 1);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test launches_test -- --nocapture`
Expected: FAIL

- [ ] **Step 3: Create `src/tools/launches.rs`**

```rust
use rusqlite::Connection;
use serde_json::Value;
use super::ToolResult;

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
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;

    match conn.execute(
        "INSERT INTO launches (id, ticket_id, team_id, branch, worktree_path, status, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, 'pending', ?6, ?6)",
        rusqlite::params![id, ticket_id, team_id, branch, worktree_path, now],
    ) {
        Ok(_) => ToolResult::success(serde_json::json!({
            "launch": {"id": id, "ticket_id": ticket_id, "team_id": team_id, "branch": branch, "status": "pending"}
        })),
        Err(e) => ToolResult::fail(&format!("db error: {}", e)),
    }
}

pub fn update(args: &Value, conn: &Connection) -> ToolResult {
    let id = match args["id"].as_str() {
        Some(s) => s,
        None => return ToolResult::fail("missing required field: id"),
    };

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;

    // Build SET clause dynamically
    let mut sets = vec!["updated_at = ?1".to_string()];
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(now)];

    if let Some(s) = args["status"].as_str() {
        params.push(Box::new(s.to_string()));
        sets.push(format!("status = ?{}", params.len()));
    }
    if let Some(s) = args["pr_url"].as_str() {
        params.push(Box::new(s.to_string()));
        sets.push(format!("pr_url = ?{}", params.len()));
    }
    if let Some(s) = args["error"].as_str() {
        params.push(Box::new(s.to_string()));
        sets.push(format!("error = ?{}", params.len()));
    }
    if let Some(s) = args["worktree_path"].as_str() {
        params.push(Box::new(s.to_string()));
        sets.push(format!("worktree_path = ?{}", params.len()));
    }

    params.push(Box::new(id.to_string()));
    let id_idx = params.len();

    let sql = format!("UPDATE launches SET {} WHERE id = ?{}", sets.join(", "), id_idx);
    let params_ref: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();

    match conn.execute(&sql, params_ref.as_slice()) {
        Ok(0) => ToolResult::fail(&format!("launch not found: {}", id)),
        Ok(_) => {
            // Return updated launch
            let mut stmt = conn.prepare(
                "SELECT id, ticket_id, team_id, branch, worktree_path, status, pr_url, error, created_at, updated_at
                 FROM launches WHERE id = ?1"
            ).unwrap();
            match stmt.query_row(rusqlite::params![id], |row| {
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
            }) {
                Ok(launch) => ToolResult::success(serde_json::json!({"launch": launch})),
                Err(e) => ToolResult::fail(&format!("db error: {}", e)),
            }
        }
        Err(e) => ToolResult::fail(&format!("db error: {}", e)),
    }
}

pub fn list(args: &Value, conn: &Connection) -> ToolResult {
    let status = args["status"].as_str();
    let limit = args["limit"].as_i64().unwrap_or(100);
    let offset = args["offset"].as_i64().unwrap_or(0);

    let (sql, sql_params): (String, Vec<Box<dyn rusqlite::types::ToSql>>) = match status {
        Some(s) => (
            "SELECT l.id, l.ticket_id, l.team_id, l.branch, l.worktree_path, l.status, l.pr_url, l.error, l.created_at, l.updated_at,
                    t.title as ticket_title, tm.name as team_name
             FROM launches l
             LEFT JOIN tickets t ON t.id = l.ticket_id
             LEFT JOIN teams tm ON tm.id = l.team_id
             WHERE l.status = ?1
             ORDER BY l.created_at DESC LIMIT ?2 OFFSET ?3".to_string(),
            vec![Box::new(s.to_string()), Box::new(limit), Box::new(offset)],
        ),
        None => (
            "SELECT l.id, l.ticket_id, l.team_id, l.branch, l.worktree_path, l.status, l.pr_url, l.error, l.created_at, l.updated_at,
                    t.title as ticket_title, tm.name as team_name
             FROM launches l
             LEFT JOIN tickets t ON t.id = l.ticket_id
             LEFT JOIN teams tm ON tm.id = l.team_id
             ORDER BY l.created_at DESC LIMIT ?1 OFFSET ?2".to_string(),
            vec![Box::new(limit), Box::new(offset)],
        ),
    };

    let mut stmt = match conn.prepare(&sql) {
        Ok(s) => s,
        Err(e) => return ToolResult::fail(&format!("db error: {}", e)),
    };

    let params_ref: Vec<&dyn rusqlite::types::ToSql> = sql_params.iter().map(|p| p.as_ref()).collect();
    let launches: Vec<Value> = stmt.query_map(params_ref.as_slice(), |row| {
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
    }).unwrap().filter_map(|r| r.ok()).collect();

    let count = launches.len();
    ToolResult::success(serde_json::json!({"launches": launches, "count": count}))
}
```

- [ ] **Step 4: Update `src/tools/mod.rs`**

Add `pub mod launches;` and dispatch:

```rust
"launch_create" => launches::create(args, conn),
"launch_update" => launches::update(args, conn),
"launch_list" => launches::list(args, conn),
```

- [ ] **Step 5: Run tests**

Run: `cargo test --test launches_test -- --nocapture`
Expected: ALL PASS

- [ ] **Step 6: Commit**

```bash
git add src/tools/launches.rs src/tools/mod.rs tests/launches_test.rs
git commit -m "feat: add launch_create, launch_update, launch_list tools"
```

---

## Chunk 4: MCP Tool Definitions & Cleanup

### Task 7: Update MCP tool_definitions — remove messages, add 12 new tools

**Files:**
- Modify: `src/mcp.rs:67-313`

- [ ] **Step 1: Remove message tool definitions**

In `src/mcp.rs` `tool_definitions()`, remove the 3 `ToolDef` entries for `message_send`, `message_read`, `message_ack` (lines 139-177).

- [ ] **Step 2: Add agent_define, agent_catalog, agent_remove definitions**

```rust
ToolDef {
    name: "agent_define".into(),
    description: "Create or update a reusable agent definition with capabilities and model".into(),
    input_schema: serde_json::json!({
        "type": "object",
        "properties": {
            "id": {"type": "string", "description": "Unique agent definition ID"},
            "name": {"type": "string", "description": "Display name"},
            "capabilities": {"type": "array", "items": {"type": "string"}, "description": "List of capabilities (e.g. typescript, react)"},
            "model": {"type": "string", "enum": ["sonnet", "opus", "haiku"], "description": "Model to use", "default": "sonnet"},
            "prompt_hint": {"type": "string", "description": "System prompt guidance for this agent"}
        },
        "required": ["id", "name"]
    }),
},
ToolDef {
    name: "agent_catalog".into(),
    description: "List all agent definitions in the registry".into(),
    input_schema: serde_json::json!({
        "type": "object",
        "properties": {
            "limit": {"type": "integer", "description": "Max results", "default": 100},
            "offset": {"type": "integer", "description": "Offset for pagination", "default": 0}
        }
    }),
},
ToolDef {
    name: "agent_remove".into(),
    description: "Remove an agent definition from the registry".into(),
    input_schema: serde_json::json!({
        "type": "object",
        "properties": {
            "id": {"type": "string", "description": "Agent definition ID to remove"}
        },
        "required": ["id"]
    }),
},
```

- [ ] **Step 3: Add team_create, team_list, team_delete definitions**

```rust
ToolDef {
    name: "team_create".into(),
    description: "Create a team and optionally assign agent members with roles".into(),
    input_schema: serde_json::json!({
        "type": "object",
        "properties": {
            "name": {"type": "string", "description": "Unique team name"},
            "description": {"type": "string", "description": "Team description"},
            "members": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "agent_id": {"type": "string"},
                        "role": {"type": "string"}
                    },
                    "required": ["agent_id"]
                },
                "description": "Agent members with roles"
            }
        },
        "required": ["name"]
    }),
},
ToolDef {
    name: "team_list".into(),
    description: "List all teams with their members and agent definitions".into(),
    input_schema: serde_json::json!({
        "type": "object",
        "properties": {
            "limit": {"type": "integer", "description": "Max results", "default": 100},
            "offset": {"type": "integer", "description": "Offset for pagination", "default": 0}
        }
    }),
},
ToolDef {
    name: "team_delete".into(),
    description: "Delete a team by name".into(),
    input_schema: serde_json::json!({
        "type": "object",
        "properties": {
            "name": {"type": "string", "description": "Team name to delete"}
        },
        "required": ["name"]
    }),
},
```

- [ ] **Step 4: Add ticket_cache, ticket_list, ticket_clear definitions**

```rust
ToolDef {
    name: "ticket_cache".into(),
    description: "Cache a ticket fetched from Linear or Jira for dashboard display".into(),
    input_schema: serde_json::json!({
        "type": "object",
        "properties": {
            "id": {"type": "string", "description": "Ticket ID"},
            "source": {"type": "string", "enum": ["linear", "jira"], "description": "Ticket source"},
            "external_id": {"type": "string", "description": "External ticket ID (e.g. LIN-123)"},
            "title": {"type": "string", "description": "Ticket title"},
            "status": {"type": "string", "description": "Status: todo, backlog"},
            "url": {"type": "string", "description": "URL to original ticket"},
            "description": {"type": "string", "description": "Ticket description"},
            "priority": {"type": "integer", "description": "Priority 0-3"},
            "labels": {"type": "array", "items": {"type": "string"}, "description": "Labels"},
            "metadata": {"type": "object", "description": "Additional metadata"}
        },
        "required": ["id", "source", "external_id", "title", "status"]
    }),
},
ToolDef {
    name: "ticket_list".into(),
    description: "List cached tickets with optional source and status filters".into(),
    input_schema: serde_json::json!({
        "type": "object",
        "properties": {
            "source": {"type": "string", "description": "Filter by source (linear, jira)"},
            "status": {"type": "string", "description": "Filter by status (todo, backlog)"},
            "limit": {"type": "integer", "description": "Max results", "default": 100},
            "offset": {"type": "integer", "description": "Offset for pagination", "default": 0}
        }
    }),
},
ToolDef {
    name: "ticket_clear".into(),
    description: "Clear cached tickets, optionally filtered by source".into(),
    input_schema: serde_json::json!({
        "type": "object",
        "properties": {
            "source": {"type": "string", "description": "Clear only tickets from this source"}
        }
    }),
},
```

- [ ] **Step 5: Add launch_create, launch_update, launch_list definitions**

```rust
ToolDef {
    name: "launch_create".into(),
    description: "Create a launch record for deploying a team to work on a ticket in a worktree".into(),
    input_schema: serde_json::json!({
        "type": "object",
        "properties": {
            "ticket_id": {"type": "string", "description": "Ticket ID to work on"},
            "team_id": {"type": "string", "description": "Team ID to deploy"},
            "branch": {"type": "string", "description": "Git branch name"},
            "worktree_path": {"type": "string", "description": "Path to git worktree"}
        },
        "required": ["ticket_id", "team_id", "branch"]
    }),
},
ToolDef {
    name: "launch_update".into(),
    description: "Update a launch status, PR URL, error, or worktree path".into(),
    input_schema: serde_json::json!({
        "type": "object",
        "properties": {
            "id": {"type": "string", "description": "Launch ID"},
            "status": {"type": "string", "enum": ["pending", "running", "completed", "pr_created", "failed"]},
            "pr_url": {"type": "string", "description": "Pull request URL"},
            "error": {"type": "string", "description": "Error message if failed"},
            "worktree_path": {"type": "string", "description": "Worktree path"}
        },
        "required": ["id"]
    }),
},
ToolDef {
    name: "launch_list".into(),
    description: "List launches with optional status filter, includes ticket and team info".into(),
    input_schema: serde_json::json!({
        "type": "object",
        "properties": {
            "status": {"type": "string", "description": "Filter by status"},
            "limit": {"type": "integer", "description": "Max results", "default": 100},
            "offset": {"type": "integer", "description": "Offset for pagination", "default": 0}
        }
    }),
},
```

- [ ] **Step 6: Run all tests to verify nothing is broken**

Run: `cargo test`
Expected: ALL PASS (message tests will fail — delete them next step)

- [ ] **Step 7: Delete messages test file and messages.rs**

```bash
rm tests/messages_test.rs src/tools/messages.rs
```

- [ ] **Step 8: Run full test suite**

Run: `cargo test`
Expected: ALL PASS

- [ ] **Step 9: Commit**

```bash
git add -A
git commit -m "feat: update MCP tool definitions — remove messages, add 12 new tools"
```

---

## Chunk 5: Dashboard — Backend API Endpoints

### Task 8: Replace messages API with teams, tickets, launches endpoints

**Files:**
- Modify: `src/web/api.rs`
- Modify: `src/web/mod.rs:15-35`

- [ ] **Step 1: Remove MessagesParams and messages function from api.rs**

Remove lines 25-29 (`MessagesParams`) and lines 117-154 (`messages` function).

- [ ] **Step 2: Add new query param structs**

In `src/web/api.rs`:

```rust
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
```

- [ ] **Step 3: Add agent_definitions endpoint**

```rust
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
            "capabilities": row.get::<_, String>(2)?,
            "model": row.get::<_, String>(3)?,
            "prompt_hint": row.get::<_, String>(4)?,
            "created_at": row.get::<_, i64>(5)?,
            "updated_at": row.get::<_, i64>(6)?,
        }))
    }).unwrap().filter_map(|r| r.ok()).collect();
    Json(json!({"agents": agents, "count": agents.len()}))
}
```

- [ ] **Step 4: Add teams endpoint**

```rust
pub async fn teams(State(db): State<DbState>, Query(params): Query<PaginationParams>) -> Json<Value> {
    let conn = db.lock().unwrap();
    let limit = params.limit.unwrap_or(100);
    let offset = params.offset.unwrap_or(0);
    let mut stmt = conn.prepare(
        "SELECT id, name, description, created_at, updated_at FROM teams ORDER BY name LIMIT ?1 OFFSET ?2"
    ).unwrap();
    let teams: Vec<Value> = stmt.query_map(rusqlite::params![limit, offset], |row| {
        let team_id: String = row.get(0)?;
        Ok((team_id, row.get::<_, String>(1)?, row.get::<_, String>(2)?,
            row.get::<_, i64>(3)?, row.get::<_, i64>(4)?))
    }).unwrap().filter_map(|r| r.ok()).map(|(team_id, name, desc, created, updated)| {
        let mut member_stmt = conn.prepare(
            "SELECT tm.agent_id, tm.role, ad.name, ad.model
             FROM team_members tm JOIN agent_definitions ad ON ad.id = tm.agent_id
             WHERE tm.team_id = ?1"
        ).unwrap();
        let members: Vec<Value> = member_stmt.query_map(rusqlite::params![team_id], |row| {
            Ok(json!({"agent_id": row.get::<_, String>(0)?, "role": row.get::<_, String>(1)?,
                       "name": row.get::<_, String>(2)?, "model": row.get::<_, String>(3)?}))
        }).unwrap().filter_map(|r| r.ok()).collect();
        json!({"id": team_id, "name": name, "description": desc,
               "members": members, "created_at": created, "updated_at": updated})
    }).collect();
    Json(json!({"teams": teams, "count": teams.len()}))
}
```

- [ ] **Step 5: Add tickets endpoint**

```rust
pub async fn tickets(State(db): State<DbState>, Query(params): Query<TicketsParams>) -> Json<Value> {
    let conn = db.lock().unwrap();
    let limit = params.limit.unwrap_or(100);
    let offset = params.offset.unwrap_or(0);
    let mut conditions = vec!["1=1".to_string()];
    let mut bind_params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    if let Some(ref s) = params.source {
        bind_params.push(Box::new(s.clone()));
        conditions.push(format!("source = ?{}", bind_params.len()));
    }
    if let Some(ref s) = params.status {
        bind_params.push(Box::new(s.clone()));
        conditions.push(format!("status = ?{}", bind_params.len()));
    }
    bind_params.push(Box::new(limit));
    let li = bind_params.len();
    bind_params.push(Box::new(offset));
    let oi = bind_params.len();
    let sql = format!(
        "SELECT id, source, external_id, title, status, priority, url, labels, fetched_at
         FROM tickets WHERE {} ORDER BY priority ASC, fetched_at DESC LIMIT ?{} OFFSET ?{}",
        conditions.join(" AND "), li, oi
    );
    let mut stmt = conn.prepare(&sql).unwrap();
    let refs: Vec<&dyn rusqlite::types::ToSql> = bind_params.iter().map(|p| p.as_ref()).collect();
    let tickets: Vec<Value> = stmt.query_map(refs.as_slice(), |row| {
        Ok(json!({
            "id": row.get::<_, String>(0)?, "source": row.get::<_, String>(1)?,
            "external_id": row.get::<_, String>(2)?, "title": row.get::<_, String>(3)?,
            "status": row.get::<_, String>(4)?, "priority": row.get::<_, i64>(5)?,
            "url": row.get::<_, String>(6)?, "labels": row.get::<_, String>(7)?,
            "fetched_at": row.get::<_, i64>(8)?,
        }))
    }).unwrap().filter_map(|r| r.ok()).collect();
    Json(json!({"tickets": tickets, "count": tickets.len()}))
}
```

- [ ] **Step 6: Add launches endpoint**

```rust
pub async fn launches(State(db): State<DbState>, Query(params): Query<LaunchesParams>) -> Json<Value> {
    let conn = db.lock().unwrap();
    let limit = params.limit.unwrap_or(100);
    let offset = params.offset.unwrap_or(0);
    let (sql, sql_params): (String, Vec<Box<dyn rusqlite::types::ToSql>>) = match &params.status {
        Some(s) => (
            "SELECT l.id, l.ticket_id, l.team_id, l.branch, l.worktree_path, l.status, l.pr_url, l.error, l.created_at, l.updated_at,
                    t.title, tm.name
             FROM launches l LEFT JOIN tickets t ON t.id = l.ticket_id LEFT JOIN teams tm ON tm.id = l.team_id
             WHERE l.status = ?1 ORDER BY l.created_at DESC LIMIT ?2 OFFSET ?3".to_string(),
            vec![Box::new(s.clone()), Box::new(limit), Box::new(offset)],
        ),
        None => (
            "SELECT l.id, l.ticket_id, l.team_id, l.branch, l.worktree_path, l.status, l.pr_url, l.error, l.created_at, l.updated_at,
                    t.title, tm.name
             FROM launches l LEFT JOIN tickets t ON t.id = l.ticket_id LEFT JOIN teams tm ON tm.id = l.team_id
             ORDER BY l.created_at DESC LIMIT ?1 OFFSET ?2".to_string(),
            vec![Box::new(limit), Box::new(offset)],
        ),
    };
    let mut stmt = conn.prepare(&sql).unwrap();
    let refs: Vec<&dyn rusqlite::types::ToSql> = sql_params.iter().map(|p| p.as_ref()).collect();
    let launches: Vec<Value> = stmt.query_map(refs.as_slice(), |row| {
        Ok(json!({
            "id": row.get::<_, String>(0)?, "ticket_id": row.get::<_, String>(1)?,
            "team_id": row.get::<_, String>(2)?, "branch": row.get::<_, String>(3)?,
            "worktree_path": row.get::<_, String>(4)?, "status": row.get::<_, String>(5)?,
            "pr_url": row.get::<_, String>(6)?, "error": row.get::<_, String>(7)?,
            "created_at": row.get::<_, i64>(8)?, "updated_at": row.get::<_, i64>(9)?,
            "ticket_title": row.get::<_, Option<String>>(10)?,
            "team_name": row.get::<_, Option<String>>(11)?,
        }))
    }).unwrap().filter_map(|r| r.ok()).collect();
    Json(json!({"launches": launches, "count": launches.len()}))
}
```

- [ ] **Step 7: Update overview to include new counts**

Replace `unacked_messages` query with team and launch counts:

```rust
let team_count: i64 = conn.query_row("SELECT COUNT(*) FROM teams", [], |r| r.get(0)).unwrap_or(0);
let ticket_count: i64 = conn.query_row("SELECT COUNT(*) FROM tickets", [], |r| r.get(0)).unwrap_or(0);
let active_launches: i64 = conn.query_row(
    "SELECT COUNT(*) FROM launches WHERE status IN ('pending', 'running')", [], |r| r.get(0)
).unwrap_or(0);
```

Add to response JSON: `"teams": team_count, "tickets": ticket_count, "active_launches": active_launches`

- [ ] **Step 8: Update router in `src/web/mod.rs`**

Replace `.route("/api/messages", get(api::messages))` with:

```rust
.route("/api/agent-definitions", get(api::agent_definitions))
.route("/api/teams", get(api::teams))
.route("/api/tickets", get(api::tickets))
.route("/api/launches", get(api::launches))
```

- [ ] **Step 9: Verify compilation**

Run: `cargo build`
Expected: SUCCESS

- [ ] **Step 10: Commit**

```bash
git add src/web/api.rs src/web/mod.rs
git commit -m "feat: replace messages API with teams, tickets, launches endpoints"
```

---

## Chunk 6: Dashboard — Frontend Update

### Task 9: Update index.html — replace Messages nav with new pages

**Files:**
- Modify: `web/index.html`

- [ ] **Step 1: Update sidebar navigation**

Replace the Messages link with Teams, Tickets, Launches:

```html
<li><a href="#" data-page="overview" class="active">Overview</a></li>
<li><a href="#" data-page="agents">Agents</a></li>
<li><a href="#" data-page="tasks">Tasks</a></li>
<li><a href="#" data-page="teams">Teams</a></li>
<li><a href="#" data-page="tickets">Tickets</a></li>
<li><a href="#" data-page="launches">Launches</a></li>
<li><a href="#" data-page="memory">Memory</a></li>
<li><a href="#" data-page="sessions">Sessions</a></li>
<li><a href="#" data-page="activity">Activity</a></li>
```

- [ ] **Step 2: Update page divs**

Replace `<div id="page-messages" class="page"></div>` with:

```html
<div id="page-teams" class="page"></div>
<div id="page-tickets" class="page"></div>
<div id="page-launches" class="page"></div>
```

- [ ] **Step 3: Commit**

```bash
git add web/index.html
git commit -m "feat: update dashboard navigation — teams, tickets, launches"
```

### Task 10: Update app.js — replace messages loader with new page loaders

**Files:**
- Modify: `web/app.js`

- [ ] **Step 1: Update loaders map**

Replace `messages: loadMessages,` with:

```javascript
teams: loadTeams,
tickets: loadTickets,
launches: loadLaunches,
```

- [ ] **Step 2: Remove `loadMessages` function**

Delete the entire `loadMessages` function (lines 141-180).

- [ ] **Step 3: Add `loadTeams` function**

```javascript
async function loadTeams() {
    var defs = await fetchJSON('/api/agent-definitions?limit=100');
    var data = await fetchJSON('/api/teams?limit=100');
    var el = document.getElementById('page-teams');

    // Agent definitions section
    var agentRows = (defs.agents || []).map(function(a) {
        var caps = a.capabilities || '[]';
        try { caps = JSON.parse(caps).join(', '); } catch(e) {}
        return '<tr>' +
            '<td>' + escapeHtml(a.id) + '</td>' +
            '<td>' + escapeHtml(a.name) + '</td>' +
            '<td>' + escapeHtml(caps) + '</td>' +
            '<td>' + badge(a.model) + '</td>' +
            '<td title="' + escapeHtml(a.prompt_hint) + '">' + escapeHtml((a.prompt_hint || '').substring(0, 60)) + '</td>' +
        '</tr>';
    }).join('');

    // Teams section
    var teamCards = (data.teams || []).map(function(t) {
        var memberList = (t.members || []).map(function(m) {
            return '<div class="team-member">' +
                '<span class="member-name">' + escapeHtml(m.name || m.agent_id) + '</span>' +
                (m.role ? ' <span class="badge">' + escapeHtml(m.role) + '</span>' : '') +
                ' <span class="text-muted">' + escapeHtml(m.model || '') + '</span>' +
            '</div>';
        }).join('');
        return '<div class="card team-card">' +
            '<h3>' + escapeHtml(t.name) + '</h3>' +
            '<p class="text-muted" style="margin:0.5rem 0;font-size:0.8rem">' + escapeHtml(t.description || '') + '</p>' +
            '<div class="team-members">' + (memberList || '<span class="text-muted">No members</span>') + '</div>' +
        '</div>';
    }).join('');

    setContent(el,
        '<h2>Teams</h2>' +
        '<h3 style="margin-bottom:1rem">Agent Definitions</h3>' +
        '<div class="table-container" style="margin-bottom:2rem">' +
            '<table><thead><tr><th>ID</th><th>Name</th><th>Capabilities</th><th>Model</th><th>Prompt Hint</th></tr></thead>' +
            '<tbody>' + (agentRows || '<tr><td colspan="5" class="empty">No agent definitions</td></tr>') + '</tbody></table>' +
        '</div>' +
        '<h3 style="margin-bottom:1rem">Teams</h3>' +
        '<div class="cards">' + (teamCards || '<div class="empty">No teams defined</div>') + '</div>');
}
```

- [ ] **Step 4: Add `loadTickets` function**

```javascript
async function loadTickets() {
    var data = await fetchJSON('/api/tickets?limit=100');
    var el = document.getElementById('page-tickets');
    var rows = (data.tickets || []).map(function(t) {
        var labels = '[]';
        try { labels = JSON.parse(t.labels || '[]').join(', '); } catch(e) {}
        return '<tr>' +
            '<td>' + badge(t.source, t.source) + '</td>' +
            '<td>' + escapeHtml(t.external_id) + '</td>' +
            '<td>' + (t.url ? '<a href="' + escapeHtml(t.url) + '" target="_blank" style="color:var(--accent)">' + escapeHtml(t.title) + '</a>' : escapeHtml(t.title)) + '</td>' +
            '<td>' + badge(t.status) + '</td>' +
            '<td>' + priorityBadge(t.priority) + '</td>' +
            '<td>' + escapeHtml(labels) + '</td>' +
            '<td>' + timeAgo(t.fetched_at) + '</td>' +
        '</tr>';
    }).join('');

    setContent(el,
        '<h2>Tickets</h2>' +
        '<div style="margin-bottom:1rem;display:flex;gap:0.5rem">' +
            '<button class="channel-tab" data-filter-source="">All</button>' +
            '<button class="channel-tab" data-filter-source="linear">Linear</button>' +
            '<button class="channel-tab" data-filter-source="jira">Jira</button>' +
        '</div>' +
        '<div class="table-container">' +
            '<table><thead><tr><th>Source</th><th>ID</th><th>Title</th><th>Status</th><th>Priority</th><th>Labels</th><th>Fetched</th></tr></thead>' +
            '<tbody id="tickets-body">' + (rows || '<tr><td colspan="7" class="empty">No tickets cached. Use prompting to fetch from Linear or Jira.</td></tr>') + '</tbody></table>' +
        '</div>');

    el.querySelectorAll('[data-filter-source]').forEach(function(btn) {
        btn.addEventListener('click', async function() {
            var src = btn.dataset.filterSource;
            var url = src ? '/api/tickets?source=' + encodeURIComponent(src) : '/api/tickets?limit=100';
            var filtered = await fetchJSON(url);
            var fRows = (filtered.tickets || []).map(function(t) {
                var labels = '[]';
                try { labels = JSON.parse(t.labels || '[]').join(', '); } catch(e) {}
                return '<tr>' +
                    '<td>' + badge(t.source, t.source) + '</td>' +
                    '<td>' + escapeHtml(t.external_id) + '</td>' +
                    '<td>' + (t.url ? '<a href="' + escapeHtml(t.url) + '" target="_blank" style="color:var(--accent)">' + escapeHtml(t.title) + '</a>' : escapeHtml(t.title)) + '</td>' +
                    '<td>' + badge(t.status) + '</td>' +
                    '<td>' + priorityBadge(t.priority) + '</td>' +
                    '<td>' + escapeHtml(labels) + '</td>' +
                    '<td>' + timeAgo(t.fetched_at) + '</td>' +
                '</tr>';
            }).join('');
            setContent(document.getElementById('tickets-body'), fRows || '<tr><td colspan="7" class="empty">No tickets</td></tr>');
        });
    });
}
```

- [ ] **Step 5: Add `loadLaunches` function**

```javascript
async function loadLaunches() {
    var data = await fetchJSON('/api/launches?limit=100');
    var el = document.getElementById('page-launches');
    var rows = (data.launches || []).map(function(l) {
        var prLink = l.pr_url ? '<a href="' + escapeHtml(l.pr_url) + '" target="_blank" style="color:var(--accent)">PR</a>' : '-';
        return '<tr>' +
            '<td>' + escapeHtml(l.ticket_title || l.ticket_id) + '</td>' +
            '<td>' + escapeHtml(l.team_name || l.team_id) + '</td>' +
            '<td>' + escapeHtml(l.branch) + '</td>' +
            '<td>' + badge(l.status) + '</td>' +
            '<td>' + prLink + '</td>' +
            '<td title="' + escapeHtml(l.error) + '">' + escapeHtml((l.error || '').substring(0, 50)) + '</td>' +
            '<td>' + timeAgo(l.created_at) + '</td>' +
        '</tr>';
    }).join('');

    setContent(el,
        '<h2>Launches</h2>' +
        '<div class="table-container">' +
            '<table><thead><tr><th>Ticket</th><th>Team</th><th>Branch</th><th>Status</th><th>PR</th><th>Error</th><th>Started</th></tr></thead>' +
            '<tbody>' + (rows || '<tr><td colspan="7" class="empty">No launches yet</td></tr>') + '</tbody></table>' +
        '</div>');
}
```

- [ ] **Step 6: Update overview loader**

Update `loadOverview` to show team/ticket/launch counts instead of messages:

Replace `'Unacked Messages'` card with `'Teams'`, `'Tickets'`, and `'Active Launches'` cards using the new API fields.

- [ ] **Step 7: Verify build compiles (frontend is embedded)**

Run: `cargo build`
Expected: SUCCESS

- [ ] **Step 8: Commit**

```bash
git add web/app.js
git commit -m "feat: update dashboard frontend — teams, tickets, launches pages"
```

---

## Chunk 7: CSS + Badge Updates & Skill Cleanup

### Task 11: Add CSS for new dashboard elements

**Files:**
- Modify: `web/style.css`

- [ ] **Step 1: Add new badge styles**

```css
/* Source badges */
.badge-linear { background: rgba(99, 91, 255, 0.15); color: #635bff; }
.badge-jira { background: rgba(0, 82, 204, 0.15); color: #0052cc; }
.badge-todo { background: rgba(255, 167, 38, 0.15); color: var(--warning); }
.badge-backlog { background: rgba(90, 100, 120, 0.15); color: var(--text-muted); }
.badge-pr_created { background: rgba(171, 71, 188, 0.15); color: #ab47bc; }
.badge-sonnet { background: rgba(79, 195, 247, 0.15); color: var(--accent); }
.badge-opus { background: rgba(171, 71, 188, 0.15); color: #ab47bc; }
.badge-haiku { background: rgba(102, 187, 106, 0.15); color: var(--success); }

/* Team cards */
.team-card { min-height: 120px; }
.team-card h3 { color: var(--accent); font-size: 0.9rem; text-transform: none; letter-spacing: normal; }
.team-members { display: flex; flex-direction: column; gap: 0.4rem; margin-top: 0.75rem; }
.team-member { display: flex; align-items: center; gap: 0.5rem; font-size: 0.8rem; }
.member-name { color: var(--text-primary); font-weight: 500; }
```

- [ ] **Step 2: Commit**

```bash
git add web/style.css
git commit -m "feat: add CSS for teams, tickets, launches badges"
```

### Task 12: Remove communicate skill, add teams skill

**Files:**
- Delete: `skills/communicate/SKILL.md`
- Create: `skills/teams/SKILL.md`

- [ ] **Step 1: Delete communicate skill**

```bash
rm -rf skills/communicate
```

- [ ] **Step 2: Create teams skill**

Create `skills/teams/SKILL.md`:

```markdown
---
name: teams
description: This skill should be used when agents need to "define agents", "create teams", "list teams", "manage agent registry", "configure team members", or want to set up reusable agent definitions and team compositions for orchestration.
---

# Agent Registry & Teams

Define reusable agent configurations and group them into teams for fast orchestration.

## Agent Definitions

Create agents once, reuse everywhere:

```
agent_define(
  id="frontend-dev",
  name="Frontend Developer",
  capabilities=["typescript", "react", "tailwind"],
  model="sonnet",
  prompt_hint="Implement components following project conventions"
)
```

List all definitions:
```
agent_catalog()
```

## Teams

Group agents into teams:
```
team_create(
  name="frontend-squad",
  description="Build frontend features",
  members=[
    {"agent_id": "frontend-dev", "role": "implementer"},
    {"agent_id": "test-writer", "role": "tester"},
    {"agent_id": "code-reviewer", "role": "reviewer"}
  ]
)
```

Same agent can belong to multiple teams.

List teams with members:
```
team_list()
```

## Using Teams for Orchestration

When launching work: specify the team name, and the orchestrator reads the full team config (agent definitions, roles, capabilities, models) to spawn the right subagents.
```

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "feat: replace communicate skill with teams skill"
```

### Task 13: Delete messages_test.rs, update any remaining references

**Files:**
- Check: All test files for message references
- Check: `skills/status/SKILL.md` and `skills/cleanup/SKILL.md` for message references

- [ ] **Step 1: Check and update status skill**

Search for "message" references in skills and update accordingly.

- [ ] **Step 2: Check and update cleanup skill**

Update any references to message acknowledgment.

- [ ] **Step 3: Run full test suite**

Run: `cargo test`
Expected: ALL PASS

- [ ] **Step 4: Build release binary**

Run: `cargo build --release`
Expected: SUCCESS

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "chore: clean up message references from skills and tests"
```

---

## Chunk 8: Integration Test & Final Verification

### Task 14: Write integration test for full workflow

**Files:**
- Create: `tests/teams_integration_test.rs`

- [ ] **Step 1: Write integration test**

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
fn test_full_workflow_define_agents_create_team_cache_ticket_launch() {
    let conn = setup();

    // 1. Define agents
    tools::definitions::define(&serde_json::json!({
        "id": "dev", "name": "Developer", "capabilities": ["rust"], "model": "sonnet"
    }), &conn);
    tools::definitions::define(&serde_json::json!({
        "id": "reviewer", "name": "Reviewer", "model": "opus"
    }), &conn);

    // 2. Create team
    let team = tools::teams::create(&serde_json::json!({
        "name": "rust-team",
        "members": [
            {"agent_id": "dev", "role": "implementer"},
            {"agent_id": "reviewer", "role": "reviewer"}
        ]
    }), &conn);
    assert!(team.ok);
    let team_id = team.data.unwrap()["team"]["id"].as_str().unwrap().to_string();

    // 3. Cache ticket
    tools::tickets::cache(&serde_json::json!({
        "id": "ticket-1", "source": "linear", "external_id": "LIN-42",
        "title": "Add auth middleware", "status": "todo",
        "url": "https://linear.app/team/LIN-42"
    }), &conn);

    // 4. Create launch
    let launch = tools::launches::create(&serde_json::json!({
        "ticket_id": "ticket-1", "team_id": team_id, "branch": "feat/auth-middleware"
    }), &conn);
    assert!(launch.ok);
    let launch_id = launch.data.unwrap()["launch"]["id"].as_str().unwrap().to_string();

    // 5. Update launch to running
    tools::launches::update(&serde_json::json!({
        "id": launch_id, "status": "running", "worktree_path": "/tmp/wt/auth"
    }), &conn);

    // 6. Complete with PR
    tools::launches::update(&serde_json::json!({
        "id": launch_id, "status": "pr_created", "pr_url": "https://github.com/org/repo/pull/42"
    }), &conn);

    // 7. Verify launch list shows everything
    let result = tools::launches::list(&serde_json::json!({}), &conn);
    let data = result.data.unwrap();
    let launches = data["launches"].as_array().unwrap();
    assert_eq!(launches.len(), 1);
    assert_eq!(launches[0]["status"], "pr_created");
    assert_eq!(launches[0]["pr_url"], "https://github.com/org/repo/pull/42");
    assert_eq!(launches[0]["ticket_title"], "Add auth middleware");
    assert_eq!(launches[0]["team_name"], "rust-team");

    // 8. Verify changes table captured everything
    let result = tools::changes::poll(&serde_json::json!({"since_id": 0}), &conn);
    let changes = result.data.unwrap()["changes"].as_array().unwrap().len();
    assert!(changes >= 6); // at least: 2 agent_defs + 1 team + 2 team_members + 1 ticket + 2 launches
}
```

- [ ] **Step 2: Run integration test**

Run: `cargo test --test teams_integration_test -- --nocapture`
Expected: PASS

- [ ] **Step 3: Run full test suite**

Run: `cargo test`
Expected: ALL PASS

- [ ] **Step 4: Build release binary**

Run: `cargo build --release`
Expected: SUCCESS

- [ ] **Step 5: Commit**

```bash
git add tests/teams_integration_test.rs
git commit -m "test: add full workflow integration test for teams + tickets + launches"
```

---

## Summary

| Chunk | Tasks | What it does |
|-------|-------|-------------|
| 1 | 1-2 | Schema migration: new tables, remove messages |
| 2 | 3-4 | Agent definition + team tools (Rust) |
| 3 | 5-6 | Ticket + launch tools (Rust) |
| 4 | 7 | MCP tool definitions update |
| 5 | 8 | Dashboard API endpoints |
| 6 | 9-10 | Dashboard frontend (HTML + JS) |
| 7 | 11-13 | CSS, skills, cleanup |
| 8 | 14 | Integration test + final build |

**Total: 14 tasks, ~8 commits, estimated 24 new tool slots replacing 3 unused ones.**
