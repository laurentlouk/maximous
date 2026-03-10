# Maximous

A lightweight SQLite brain for multi-agent orchestration. Single Rust binary, 2.9MB, zero dependencies at runtime.

Maximous gives Claude Code agents (sub-agents, team agents, parallel agents) a shared database for coordination, communication, and knowledge sharing via the MCP protocol.

## How It Works

```
Agent A (subagent)  --stdio-->  maximous process A  --WAL--+
Agent B (subagent)  --stdio-->  maximous process B  --WAL--+-->  brain.db
Agent C (team)      --stdio-->  maximous process C  --WAL--+
```

Each agent spawns its own MCP server process. All processes share a single SQLite file using WAL mode (concurrent reads, serialized writes, crash-safe).

### What Agents Can Do

| Domain | Tools | Purpose |
|---|---|---|
| **Memory** | `memory_set`, `memory_get`, `memory_search`, `memory_delete` | Shared key-value store with namespaces and TTL |
| **Messages** | `message_send`, `message_read`, `message_ack` | Priority message queue with channels |
| **Tasks** | `task_create`, `task_update`, `task_list` | Task board with dependencies |
| **Agents** | `agent_register`, `agent_heartbeat`, `agent_list` | Agent registry with heartbeat |
| **Observe** | `poll_changes` | Watch for state changes across all tables |

## Installation

### Prerequisites

- [Rust](https://rustup.rs/) (1.70+)

### Build from source

```bash
git clone https://github.com/your-username/maximous.git
cd maximous
cargo build --release
```

The binary is at `target/release/maximous` (about 3MB).

### Install globally

```bash
cargo install --path .
```

This puts `maximous` in your `$CARGO_HOME/bin` (usually `~/.cargo/bin`).

## Usage

### As a Claude Code plugin

Copy the `plugin/` directory into your Claude Code plugins folder, or add the MCP server directly to your project's `.mcp.json`:

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

Claude Code will auto-spawn the server and make all 14 tools available to agents.

### Standalone

```bash
# Start the MCP server (reads JSON-RPC from stdin, writes to stdout)
maximous --db .maximous/brain.db

# Custom database path
maximous --db /tmp/my-project.db
```

### Quick smoke test

```bash
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{}}}' | maximous --db /tmp/test.db
```

Should return:
```json
{"jsonrpc":"2.0","id":1,"result":{"capabilities":{"tools":{}},"protocolVersion":"2024-11-05","serverInfo":{"name":"maximous","version":"0.1.0"}}}
```

## Multi-Agent Example

Here's how agents coordinate through maximous:

```
1. Orchestrator creates tasks with dependencies:
   task_create("parse-api", deps=[])
   task_create("build-ui", deps=["parse-api"])

2. Agent A picks up "parse-api", runs it, stores result:
   memory_set("task-results", "parse-api", {"endpoints": ["/users"]})
   task_update("parse-api", status="done")

3. Agent B polls for changes:
   poll_changes(since_id=5)  -->  sees "parse-api" is done

4. Agent B sets dependent task to ready and picks it up:
   task_update("build-ui", status="ready")  // auto-checks deps
   task_update("build-ui", status="running")
   memory_get("task-results", "parse-api")  // reads upstream data

5. Agents communicate via messages:
   message_send(channel="team", sender="agent-b", content="which framework?")
```

## Architecture

```
maximous/
├── Cargo.toml
├── src/
│   ├── main.rs          # CLI entry, spawns MCP stdio loop
│   ├── lib.rs           # Library root
│   ├── db.rs            # SQLite init, WAL mode, migrations
│   ├── schema.sql       # 6 tables, 7 indexes, 13 triggers
│   ├── mcp.rs           # JSON-RPC types, stdio loop, tool dispatch
│   └── tools/
│       ├── mod.rs       # ToolResult type, dispatch router
│       ├── memory.rs    # Namespaced KV with TTL
│       ├── messages.rs  # Priority queue with channels
│       ├── tasks.rs     # Dependency graph, status lifecycle
│       ├── agents.rs    # Registry with heartbeat
│       └── changes.rs   # Observation/change log polling
├── tests/               # 35 tests
├── benches/             # Criterion benchmarks
└── plugin/              # Claude Code plugin manifest
```

### Database Schema

6 tables + 1 change log, connected by SQLite triggers:

- **memory** — `(namespace, key)` primary key, JSON values, optional TTL
- **messages** — auto-increment ID, channels, priority (0-3), acknowledgment
- **tasks** — UUID ID, status lifecycle (pending/ready/running/done/failed), JSON dependencies
- **agents** — heartbeat-based liveness, JSON capabilities
- **changes** — auto-populated by triggers on INSERT/UPDATE/DELETE across all tables
- **config** — simple key-value settings

### Design Decisions

| Decision | Why |
|---|---|
| Rust | Single binary, no runtime, sub-ms startup, 3MB |
| stdio MCP | Native Claude Code integration, no networking, no auth |
| SQLite WAL | Crash recovery, multi-process safe, concurrent reads |
| Triggers | Changes table auto-populated, zero application code needed |
| Lazy TTL | No background threads, expiry on read |

## Development

### Setup

```bash
git clone https://github.com/your-username/maximous.git
cd maximous
cargo build
```

### Running tests

```bash
# All tests (35 total)
cargo test

# Specific test suite
cargo test --test memory_test
cargo test --test messages_test
cargo test --test tasks_test
cargo test --test agents_test
cargo test --test changes_test
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
| `src/db.rs` | Database initialization only. Change schema in `schema.sql`. |
| `src/schema.sql` | All tables, indexes, triggers. Single source of truth. |
| `src/mcp.rs` | JSON-RPC protocol and tool definitions. Add new tools here first. |
| `src/tools/mod.rs` | Dispatch router. Wire new tools here. |
| `src/tools/*.rs` | One file per domain. Each tool is a pure function `(args, conn) -> ToolResult`. |

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

## Contributing

1. Fork the repo
2. Create a feature branch (`git checkout -b feat/my-feature`)
3. Write tests first, then implement
4. Run `cargo test` — all tests must pass
5. Run `cargo clippy` — no warnings
6. Submit a pull request

## License

MIT
