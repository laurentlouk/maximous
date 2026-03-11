---
name: session-continuity
description: This skill should be used when you need to "resume previous work", "pick up where I left off", "what did we do last time", "load session history", "find previous context", or want to understand how maximous provides automatic session continuity across Claude Code conversations.
---

# Session Continuity

Maximous automatically bridges Claude Code sessions so context carries across conversations. This works through hooks that fire at key moments — no manual setup required.

## How It Works

Four hooks run automatically:

| Hook | When | What It Does |
|------|------|-------------|
| **SessionStart** | Conversation begins | Loads previous session context from maximous memory |
| **SessionEnd** | Conversation ends | Saves structured summary of what was accomplished |
| **SubagentStop** | Subagent finishes | Preserves detailed findings before compression |
| **PreCompact** | Context window fills | Saves critical in-progress state before compression |

## Reserved Namespaces

These namespaces are used automatically by the hooks:

- `sessions` — Session summaries (key format: `YYYY-MM-DD-<topic>`)
- `agent-findings` — Detailed subagent research results
- `context-preservation` — State saved before context compression (24h TTL)

## Loading Previous Context

When a user's request relates to prior work, search for relevant context:

```
memory_search(query="refactoring auth module", namespace="sessions")
```

Or browse recent session summaries:

```
memory_get(namespace="sessions")
```

Then load a specific session:

```
memory_get(namespace="sessions", key="2025-03-10-auth-refactor")
```

## Saving Session Summaries

At session end, save a structured summary:

```
memory_set(
  namespace="sessions",
  key="2025-03-11-maximous-hooks",
  value="Implemented session continuity hooks for maximous. Added SessionEnd, SubagentStop, and PreCompact hooks. Enhanced SessionStart to load previous context. Files changed: hooks/hooks.json, plugin.json. Next: test the hooks end-to-end.",
  observation_type="insight",
  category="workflow"
)
```

## Preserving Subagent Findings

When a subagent completes research, its detailed context gets compressed. Save the rich details:

```
memory_set(
  namespace="agent-findings",
  key="api-endpoint-analysis",
  value="Analyzed 47 endpoints in src/api/. Found: 12 use auth middleware, 8 are public, 27 use rate limiting. Key files: src/api/routes.rs (main router), src/api/middleware/auth.rs (JWT validation). Pattern: all authenticated routes use extract::Auth<Claims> extractor.",
  observation_type="insight",
  category="architecture"
)
```

## Pre-Compression Context Preservation

Before context is compressed, save anything critical with a 24h TTL:

```
memory_set(
  namespace="context-preservation",
  key="current-debugging-state",
  value="Debugging flaky test in tests/integration/auth.rs:142. Root cause narrowed to race condition in session cleanup. Tried: adding mutex (didn't help), increasing timeout (masks issue). Next: check if the cleanup goroutine runs before assertion.",
  ttl_seconds=86400
)
```

## Multi-Instance Awareness

If multiple Claude Code instances are running on the same project, they share the same maximous database. Each instance can:

1. See other active sessions via `session_list(status="active")`
2. Check what other agents are working on via `agent_list()`
3. Avoid conflicts by checking task assignments via `task_list()`

## Pattern: Resume Previous Work

1. User says "continue the auth refactor from yesterday"
2. Search sessions: `memory_search(query="auth refactor")`
3. Load the session summary to understand what was done and what's next
4. Check for preserved subagent findings: `memory_search(query="auth", namespace="agent-findings")`
5. Pick up exactly where the previous session left off
