-- Shared knowledge store
CREATE TABLE IF NOT EXISTS memory (
    namespace TEXT NOT NULL,
    key TEXT NOT NULL,
    value TEXT NOT NULL,
    ttl_seconds INTEGER,
    observation_type TEXT,
    category TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (namespace, key)
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

-- Cached tickets from Linear/Jira
CREATE TABLE IF NOT EXISTS tickets (
    id TEXT PRIMARY KEY,
    source TEXT NOT NULL,
    external_id TEXT NOT NULL,
    title TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL,
    assignee TEXT NOT NULL DEFAULT '',
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

-- Indexes
CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status, priority DESC);
CREATE INDEX IF NOT EXISTS idx_tasks_assigned ON tasks(assigned_to) WHERE assigned_to IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_agents_heartbeat ON agents(last_heartbeat);
CREATE INDEX IF NOT EXISTS idx_changes_id ON changes(id);
CREATE INDEX IF NOT EXISTS idx_memory_namespace ON memory(namespace);
CREATE INDEX IF NOT EXISTS idx_agent_definitions_name ON agent_definitions(name);
CREATE INDEX IF NOT EXISTS idx_teams_name ON teams(name);
CREATE INDEX IF NOT EXISTS idx_team_members_agent ON team_members(agent_id);
CREATE INDEX IF NOT EXISTS idx_tickets_source ON tickets(source, status);
CREATE INDEX IF NOT EXISTS idx_launches_status ON launches(status);
CREATE INDEX IF NOT EXISTS idx_launches_ticket ON launches(ticket_id);

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
            json_object('name', NEW.name, 'description', NEW.description),
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

CREATE TRIGGER IF NOT EXISTS trg_tickets_insert AFTER INSERT ON tickets
BEGIN
    INSERT INTO changes (table_name, row_id, action, summary, created_at)
    VALUES ('tickets', NEW.id, 'insert',
            json_object('title', NEW.title, 'status', NEW.status, 'source', NEW.source),
            strftime('%s', 'now'));
END;

CREATE TRIGGER IF NOT EXISTS trg_tickets_update AFTER UPDATE ON tickets
BEGIN
    INSERT INTO changes (table_name, row_id, action, summary, created_at)
    VALUES ('tickets', NEW.id, 'update',
            json_object('title', NEW.title, 'status', NEW.status),
            strftime('%s', 'now'));
END;

CREATE TRIGGER IF NOT EXISTS trg_tickets_delete AFTER DELETE ON tickets
BEGIN
    INSERT INTO changes (table_name, row_id, action, summary, created_at)
    VALUES ('tickets', OLD.id, 'delete',
            json_object('title', OLD.title, 'source', OLD.source),
            strftime('%s', 'now'));
END;

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
            json_object('ticket_id', OLD.ticket_id, 'status', OLD.status),
            strftime('%s', 'now'));
END;

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

-- FTS5 full-text search for memory values
CREATE VIRTUAL TABLE IF NOT EXISTS memory_fts USING fts5(
    namespace,
    key,
    value,
    content='memory',
    content_rowid='rowid'
);

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
