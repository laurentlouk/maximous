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
