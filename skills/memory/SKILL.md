---
name: memory
description: This skill should be used when agents need to "store shared data", "read shared memory", "cache results", "share knowledge between agents", "use namespaced storage", "set TTL on data", or need a persistent key-value store through maximous memory tools.
---

# Shared Memory

Store and retrieve data between agents using maximous namespaced key-value memory. All agents sharing the same database file can read and write to the same memory.

## Namespaces

Organize data with namespaces to avoid key collisions:

- `task-results` — output from completed tasks
- `config` — shared configuration
- `cache` — temporary computed data (use TTL)
- `agent-state` — per-agent state snapshots
- `shared` — general-purpose shared data

**Reserved namespaces** (used automatically by session continuity hooks):

- `sessions` — session summaries saved at session end
- `agent-findings` — detailed subagent research preserved after completion
- `context-preservation` — state saved before context compression (24h TTL)

## Writing

```
memory_set(
  namespace="task-results",
  key="parse-api",
  value="{\"endpoints\":[\"/users\",\"/items\"]}",
  ttl_seconds=3600
)
```

- Values are JSON strings
- `ttl_seconds` is optional — omit for permanent storage
- Writing to an existing key overwrites it (upsert)

## Reading

Get a specific key:
```
memory_get(namespace="task-results", key="parse-api")
```

List all keys in a namespace:
```
memory_get(namespace="task-results")
```
Returns `{"keys": [{"key": "parse-api", "updated_at": 1710000000}, ...]}`.

## Searching

Full-text search across all values (or within a namespace):
```
memory_search(query="endpoints", namespace="task-results")
```
Uses FTS5 full-text search — supports word matching, prefix queries (`end*`), and phrase matching (`"api endpoints"`).

### Progressive Disclosure with `memory_search_index`

For large result sets, use `memory_search_index` to get a summary first:
```
memory_search_index(namespace="task-results")
```
Returns keys with metadata (timestamps, sizes) without full values — useful for deciding what to fetch.

## Typed Observations

Enrich memory entries with structured metadata:

- `observation_type` — categorize entries (e.g. `"result"`, `"error"`, `"insight"`)
- `category` — group related entries (e.g. `"api"`, `"auth"`, `"perf"`)
- Wrap sensitive data in `<private>` tags — these entries are excluded from broad searches and only returned on exact key lookups

## TTL and Expiry

- Expired entries are cleaned up lazily on the next `memory_get` for that namespace
- No background threads — cleanup happens on read
- Set `ttl_seconds=0` for immediate expiry on next read
- Omit `ttl_seconds` for data that should persist indefinitely

## Deleting

Delete a specific key:
```
memory_delete(namespace="cache", key="old-result")
```

Expire all stale entries in a namespace:
```
memory_delete(namespace="cache")
```

## Pattern: Upstream Data Sharing

1. Agent A completes a task, stores result: `memory_set(namespace="task-results", key=task_id, value=result_json)`
2. Agent B polls changes, sees task is done
3. Agent B reads upstream result: `memory_get(namespace="task-results", key=task_id)`
4. Agent B uses the data without re-computing it
