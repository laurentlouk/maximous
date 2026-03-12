---
name: launch-listener
description: This skill should be used when the user wants to "listen for launches", "watch for pending launches", "start the launch listener", "pick up launches", or wants their Claude Code session to automatically dispatch agents when launches are queued from the dashboard.
---

# Launch Listener

Listen for pending launches from the maximous dashboard and dispatch them as background agents with worktree isolation.

> Uses server-push via `launch_wait` — no polling, no sleeping. The tool blocks until a launch appears, keeping the session responsive.

## How It Works

1. The dashboard creates launches with status `"pending"` when the user clicks "Launch"
2. This Claude Code session calls `launch_wait` which blocks until a launch is queued
3. For each pending launch, call the execute API to get full context (ticket, team, members)
4. Dispatch an Agent with `isolation: "worktree"` and `run_in_background: true`
5. The agent works independently in its own worktree — no interference with main session

## Steps

### 1. Wait for pending launches

Use the maximous MCP tool:

```
launch_wait(timeout=120)
```

This blocks server-side until a pending launch appears (or the timeout expires). No sleep or polling needed.

The response includes a `cursor` value. Save it — you will pass it as `since_id` on your next call so you only receive launches queued after this point.

If the response contains `timed_out: true`, no new launch was found within the timeout window. Simply call `launch_wait` again with the same `since_id` cursor.

### 2. For each pending launch, get full context

```
Call POST /api/launches/{id}/execute via the API (this marks it as "running" and returns launch details + team members)
```

Or use WebFetch to call: `http://localhost:8375/api/launches/{id}/execute` with POST method.

### 3. Dispatch Agent with worktree isolation

For each launch, dispatch a background Agent:

```
Agent(
  description: "Launch: {ticket_title}",
  prompt: "<the orchestration prompt built from launch context>",
  isolation: "worktree",
  run_in_background: true,
  mode: "auto"
)
```

The prompt should include:
- Team name and members (with roles/models)
- Ticket title and URL
- Branch name
- Launch ID
- Instructions to use `/maximous:orchestrate`, create tasks, dispatch sub-agents, and update launch status when done

### 4. Wait for next launch

Call `launch_wait(since_id=<cursor>)` again — it blocks until the next launch is queued.

- No sleep needed, no `/loop` needed
- The tool returns immediately if a launch is already pending
- On timeout (`timed_out: true`), just call again with the same cursor
- Each response returns a new `cursor` — always use the latest one for `since_id`

## Example orchestration prompt template

```
You are an orchestrator for team "{team_name}" working on a launch.

Team members:
- {member_name} ({member_id}, model: {member_model})
...

Ticket: {ticket_title}
URL: {ticket_url}
Branch: {branch}
Launch ID: {launch_id}

INSTRUCTIONS:
1. Use /maximous:orchestrate to coordinate this work
2. Start a maximous session (session_start) for this launch
3. Break down the ticket into tasks (task_create) and assign them to team members
4. Use the Agent tool to dispatch sub-agents for each team member's tasks
5. Each sub-agent should use maximous tools (agent_heartbeat, task_update) to report progress
6. When all tasks are done, update the launch status to 'completed' (launch_update)
7. Create a PR if code changes were made

The maximous dashboard is watching — all activities will be visible in real-time.
```
