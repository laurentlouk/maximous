# Maximous

A SQLite brain for multi-agent orchestration with FTS5 search, typed observations, session tracking, and a built-in web dashboard. Single Rust binary, zero runtime dependencies.

Maximous gives Claude Code agents (sub-agents, team agents, parallel agents) a shared database for coordination, communication, and knowledge sharing via the MCP protocol.

## How It Works

```
Agent A (subagent)  --stdio-->  maximous process A  --WAL--+
Agent B (subagent)  --stdio-->  maximous process B  --WAL--+-->  brain.db
Agent C (team)      --stdio-->  maximous process C  --WAL--+
                                                            |
                              Web Dashboard  <--HTTP/SSE----+
                              http://127.0.0.1:8375
```

Each agent spawns its own MCP server process. All processes share a single SQLite file using WAL mode (concurrent reads, serialized writes, crash-safe). The optional web dashboard provides a real-time view into all agent activity.

### What Agents Can Do

| Domain | Tools | Purpose |
|---|---|---|
| **Memory** | `memory_set`, `memory_get`, `memory_search`, `memory_search_index`, `memory_delete` | FTS5-powered shared knowledge store with typed observations, privacy tags, and progressive disclosure |
| **Messages** | `message_send`, `message_read`, `message_ack` | Priority message queue with channels |
| **Tasks** | `task_create`, `task_update`, `task_list` | Task board with dependencies and pagination |
| **Agents** | `agent_register`, `agent_heartbeat`, `agent_list` | Agent registry with heartbeat |
| **Sessions** | `session_start`, `session_end`, `session_list` | Track agent work sessions with summaries |
| **Observe** | `poll_changes` | Watch for state changes across all tables |

**18 tools** across 6 domains.

## Key Features

### FTS5 Full-Text Search
Memory search uses SQLite FTS5 for ranked results instead of basic LIKE queries. Supports FTS5 syntax (AND, OR, NOT, phrases).

### Progressive Disclosure
`memory_search_index` returns compact results (~50-100 tokens each) with snippets and token estimates. Use `memory_get` to fetch full values only when needed. 10x token savings vs fetching everything.

### Typed Observations
Tag memory entries with `observation_type` (decision, error, preference, insight, pattern, learning) and `category` (architecture, debugging, workflow, api, ui, data, config) for structured knowledge capture.

### Privacy Tags
Wrap sensitive data in `<private>...</private>` tags. It's stored in the database but redacted to `[REDACTED]` on all read operations.

### Session Tracking
Track agent work sessions with start/end timestamps, summaries, and per-agent filtering.

### Web Dashboard
Built-in web dashboard at `http://127.0.0.1:8375` with 7 views:
- **Overview** — stat cards + activity feed
- **Agents** — registry with heartbeat status
- **Tasks** — table with status/priority badges and dependencies
- **Messages** — channel-based message browser
- **Memory** — 3-pane namespace/key/value explorer
- **Sessions** — session history with summaries
- **Activity** — real-time change feed via SSE

Start the dashboard:
```bash
# Start with default port 8375
maximous --db .maximous/brain.db --web

# Or specify a custom port
maximous --db .maximous/brain.db --web --port 9000
```

Then open `http://127.0.0.1:8375` in your browser. The dashboard reads from the same `brain.db` that agents write to, so you see live data. SSE (Server-Sent Events) pushes changes to the browser automatically -- no manual refresh needed.

**Note:** The web dashboard runs instead of the MCP server (not alongside it). Your agents use the MCP stdio server as usual; the dashboard is a separate process for human observation.

### Pagination
All list endpoints support `limit` and `offset` parameters for efficient pagination.

## Installation

### As a Claude Code plugin (recommended)

First, add the marketplace:

```
/plugin marketplace add https://github.com/laurentlouk/claude-plugins
```

Then install the plugin from the marketplace:

```
/plugin install maximous
```

Or browse available plugins interactively with `/plugin` -> **Discover** tab.

This installs maximous as a plugin with all skills, hooks, and the MCP server. The binary needs to be available -- either build from source or download a release.

### Download pre-built binary

```bash
curl -fsSL https://raw.githubusercontent.com/laurentlouk/maximous/main/scripts/install.sh | bash
```

This detects your OS and architecture, downloads the correct binary from GitHub Releases, and installs it to `~/.cargo/bin/`.

Supported platforms: macOS (arm64, x86_64), Linux (arm64, x86_64).

### Build from source

```bash
git clone https://github.com/laurentlouk/maximous.git
cd maximous
cargo build --release
```

To install globally:

```bash
cargo install --path .
```

### Manual MCP setup (without plugin)

If you just want the MCP server without the full plugin, add to your project's `.mcp.json`:

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

## Usage

Claude Code auto-spawns the MCP server and makes all 18 tools available to agents. The SessionStart hook runs automatically every session to ensure the binary and `.maximous/` directory exist.

### Skills

The plugin includes 7 skills that teach agents how to use maximous. Skills trigger automatically based on what you say:

| Skill | Trigger examples | Purpose |
|---|---|---|
| **orchestrate** | "orchestrate agents", "set up multi-agent workflow" | Set up full multi-agent workflows |
| **coordinate** | "manage tasks", "create task graph" | Task lifecycle and dependency management |
| **communicate** | "send a message to agents", "use message channels" | Message channels and priority queues |
| **memory** | "store this in memory", "share data between agents" | Shared key-value storage with TTL |
| **observe** | "watch for changes", "poll for updates" | Watch for state changes via polling |
| **status** | "maximous status", "show agent status" | Quick overview of current state |
| **cleanup** | "clean up maximous", "expire old data" | Expire stale data and maintain the database |

### When Does Maximous Activate?

Maximous tools are available in every session but Claude only uses them when there's a reason to. In practice, maximous becomes useful when you:

- **Run parallel subagents** that need to share data
- **Set up task graphs** with dependencies between agents
- **Need agents to communicate** with each other via message channels
- **Want to observe** when another agent finishes a task
- **Need persistent memory** with full-text search across sessions

### Standalone

```bash
# Start the MCP server (reads JSON-RPC from stdin, writes to stdout)
maximous --db .maximous/brain.db

# Start with web dashboard
maximous --db .maximous/brain.db --web --port 8375

# Custom database path
maximous --db /tmp/my-project.db
```

### Quick smoke test

```bash
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{}}}' | maximous --db /tmp/test.db
```

Should return:
```json
{"jsonrpc":"2.0","id":1,"result":{"capabilities":{"tools":{}},"protocolVersion":"2024-11-05","serverInfo":{"name":"maximous","version":"0.2.0"}}}
```

## Multi-Agent Example

Here's how agents coordinate through maximous:

```
1. Orchestrator creates tasks with dependencies:
   task_create("parse-api", deps=[])
   task_create("build-ui", deps=["parse-api"])

2. Agent A picks up "parse-api", runs it, stores result:
   memory_set("task-results", "parse-api", {"endpoints": ["/users"]},
              observation_type="decision", category="api")
   task_update("parse-api", status="done")

3. Agent B polls for changes:
   poll_changes(since_id=5)  -->  sees "parse-api" is done

4. Agent B searches memory efficiently:
   memory_search_index("api endpoints")  -->  compact index with token estimates
   memory_get("task-results", "parse-api")  -->  full value when needed

5. Agents communicate via messages:
   message_send(channel="team", sender="agent-b", content="which framework?")

6. Track work sessions:
   session_start(agent_id="agent-b")
   // ... do work ...
   session_end(id="...", summary="Built UI components for /users endpoint")
```

## Architecture

```
maximous/
├── Cargo.toml
├── .claude-plugin/      # Plugin manifest
│   └── plugin.json
├── .mcp.json            # MCP server config
├── skills/              # 7 agent skills
├── hooks/               # SessionStart hook
├── scripts/             # Launcher, installer, db init
├── web/                 # Dashboard frontend (compiled into binary)
│   ├── index.html
│   ├── app.js
│   └── style.css
├── src/
│   ├── main.rs          # CLI entry: MCP stdio or web dashboard
│   ├── lib.rs           # Library root
│   ├── db.rs            # SQLite init, WAL mode, migrations
│   ├── schema.sql       # 7 tables, 9 indexes, 19 triggers, FTS5
│   ├── mcp.rs           # JSON-RPC types, stdio loop, 18 tool defs
│   ├── tools/
│   │   ├── mod.rs       # ToolResult type, dispatch router
│   │   ├── memory.rs    # FTS5 search, observations, privacy, progressive disclosure
│   │   ├── messages.rs  # Priority queue with channels
│   │   ├── tasks.rs     # Dependency graph, status lifecycle
│   │   ├── agents.rs    # Registry with heartbeat
│   │   ├── sessions.rs  # Session tracking
│   │   └── changes.rs   # Observation/change log polling
│   └── web/
│       ├── mod.rs       # Axum router, static assets via rust-embed
│       └── api.rs       # REST endpoints + SSE stream
├── tests/               # 61 tests
├── benches/             # Criterion benchmarks
└── .github/workflows/   # CI + release builds
```

### Database Schema

7 tables + FTS5 + change log, connected by SQLite triggers:

- **memory** — `(namespace, key)` primary key, JSON values, optional TTL, observation_type, category
- **memory_fts** — FTS5 virtual table for ranked full-text search
- **messages** — auto-increment ID, channels, priority (0-3), acknowledgment
- **tasks** — UUID ID, status lifecycle (pending/ready/running/done/failed), JSON dependencies
- **agents** — heartbeat-based liveness, JSON capabilities
- **sessions** — agent work sessions with start/end times and summaries
- **changes** — auto-populated by triggers on INSERT/UPDATE/DELETE across all tables
- **config** — simple key-value settings

### Design Decisions

| Decision | Why |
|---|---|
| Rust | Single binary, no runtime, sub-ms startup |
| stdio MCP | Native Claude Code integration, no networking, no auth |
| SQLite WAL | Crash recovery, multi-process safe, concurrent reads |
| FTS5 | Ranked full-text search with minimal overhead |
| Triggers | Changes table auto-populated, zero application code needed |
| Lazy TTL | No background threads, expiry on read |
| axum + rust-embed | Dashboard compiled into binary, no separate process |
| SSE | Real-time updates from changes table, simpler than WebSocket |

## Development

### Setup

```bash
git clone https://github.com/laurentlouk/maximous.git
cd maximous
cargo build
```

### Running tests

```bash
# All tests (61 total)
cargo test

# Specific test suite
cargo test --test memory_test
cargo test --test messages_test
cargo test --test tasks_test
cargo test --test agents_test
cargo test --test changes_test
cargo test --test sessions_test
cargo test --test pagination_test
cargo test --test observation_test
cargo test --test progressive_test
cargo test --test privacy_test
cargo test --test integration_test
cargo test --test concurrent_test
cargo test --test mcp_test
cargo test --test db_test
```

### Running benchmarks

```bash
cargo bench
```

Benchmarks cover:
- Memory set+get round-trip latency
- Write throughput
- Message send+read latency
- `poll_changes` scaling (100 to 50,000 rows)
- Task creation with dependency validation
- Memory search scaling (100 to 10,000 entries)

### Project structure for contributors

| File | Responsibility |
|---|---|
| `src/db.rs` | Database initialization and migrations. Change schema in `schema.sql`. |
| `src/schema.sql` | All tables, indexes, triggers, FTS5. Single source of truth. |
| `src/mcp.rs` | JSON-RPC protocol and tool definitions. Add new tools here first. |
| `src/tools/mod.rs` | Dispatch router. Wire new tools here. |
| `src/tools/*.rs` | One file per domain. Each tool is a pure function `(args, conn) -> ToolResult`. |
| `src/web/mod.rs` | Axum router and static asset serving. |
| `src/web/api.rs` | REST API endpoints and SSE stream. |
| `web/*.html/js/css` | Dashboard frontend. Compiled into binary via rust-embed. |

### Adding a new tool

1. Define the tool schema in `src/mcp.rs` in `tool_definitions()`
2. Create the function in the appropriate `src/tools/*.rs` file
3. Wire it in `src/tools/mod.rs` `dispatch_tool()`
4. Add tests in `tests/`
5. Update the tool count assertion in `tests/mcp_test.rs`

### Code conventions

- Every tool function has the signature `fn(args: &Value, conn: &Connection) -> ToolResult`
- Use `ToolResult::success(json)` or `ToolResult::fail("message")`
- Validate required fields at the top of each function
- Use `rusqlite::params![]` for parameterized queries (never string interpolation)
- Tests use `Connection::open_in_memory()` with `db::init_db()` for isolation

### Releasing

Tag a version to trigger cross-platform builds and a GitHub Release:

```bash
git tag v0.2.0
git push origin v0.2.0
```

GitHub Actions builds binaries for macOS (arm64, x86_64) and Linux (arm64, x86_64), then creates a release with the tarballs attached.

## Contributing

1. Fork the repo
2. Create a feature branch (`git checkout -b feat/my-feature`)
3. Write tests first, then implement
4. Run `cargo test` -- all tests must pass
5. Run `cargo clippy` -- no warnings
6. Submit a pull request

## License

MIT
