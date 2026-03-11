# Maximous: Teams, Ticket Integration & Launcher

**Date:** 2026-03-11
**Status:** Approved

## Overview

Replace the unused message system with an agent registry + teams feature. Add ticket integration (Linear/Jira) cached for dashboard display. Add a launcher that deploys teams to worktrees per ticket with PR creation on completion.

## 1. Agent Registry + Teams

### Problem

Messages (`message_send`, `message_read`, `message_ack`) are unused. Setting up multi-agent orchestration requires manually describing agents every time.

### Solution

Replace 3 message tools with 6 agent/team tools. Persist reusable agent definitions and team compositions.

### Schema

```sql
CREATE TABLE agent_definitions (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  capabilities TEXT DEFAULT '[]',   -- JSON array of strings
  model TEXT DEFAULT 'sonnet',      -- sonnet, opus, haiku
  prompt_hint TEXT DEFAULT '',
  created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
  updated_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
);

CREATE TABLE teams (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL UNIQUE,
  description TEXT DEFAULT '',
  created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
  updated_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
);

CREATE TABLE team_members (
  team_id TEXT NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
  agent_id TEXT NOT NULL REFERENCES agent_definitions(id) ON DELETE CASCADE,
  role TEXT NOT NULL DEFAULT '',
  PRIMARY KEY (team_id, agent_id)
);
```

Triggers on all three tables to populate the `changes` table (same pattern as existing tables).

### Tools

| Tool | Params | Returns |
|------|--------|---------|
| `agent_define` | `id, name, capabilities?, model?, prompt_hint?` | `{ok, agent}` |
| `agent_catalog` | `limit?, offset?` | `{agents[], count}` |
| `agent_remove` | `id` | `{removed: bool}` |
| `team_create` | `name, description?, members?` | `{ok, team}` |
| `team_list` | `limit?, offset?` | `{teams[], count}` |
| `team_delete` | `name` | `{removed: bool}` |

`members` is `[{"agent_id": "...", "role": "..."}]`.

`team_list` returns teams with their members and agent definitions joined.

### Removed

- `message_send`, `message_read`, `message_ack` tools
- `messages` table (drop via migration)
- `communicate` skill

### Dashboard

- Replace "Messages" page with "Teams" page
- Teams page shows: team cards with member lists, create/delete teams
- Agent definitions listed in a section within Teams page (or as a sub-tab)
- Inline editing of agent definitions (name, capabilities, model, prompt_hint)
- Add/remove agents from teams via UI

## 2. Ticket Integration

### Problem

No way to view Linear/Jira tickets in the Maximous dashboard. Users must context-switch to external tools.

### Solution

Cache tickets fetched via existing Linear/Jira MCP plugins. Dashboard displays cached tickets with filters. Refetch via dashboard button or prompting.

### Schema

```sql
CREATE TABLE tickets (
  id TEXT PRIMARY KEY,
  source TEXT NOT NULL,          -- 'linear' or 'jira'
  external_id TEXT NOT NULL,
  title TEXT NOT NULL,
  description TEXT DEFAULT '',
  status TEXT NOT NULL,          -- 'todo', 'backlog'
  priority INTEGER DEFAULT 2,
  url TEXT DEFAULT '',
  labels TEXT DEFAULT '[]',     -- JSON array of strings
  metadata TEXT DEFAULT '{}',   -- JSON object
  fetched_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
  created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
  updated_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
  UNIQUE(source, external_id)
);
```

### Tools

| Tool | Params | Returns |
|------|--------|---------|
| `ticket_cache` | `id, source, external_id, title, status, url, description?, priority?, labels?, metadata?` | `{ok, ticket}` |
| `ticket_list` | `source?, status?, limit?, offset?` | `{tickets[], count}` |
| `ticket_clear` | `source?` | `{cleared: count}` |

### Workflow

1. User says "fetch Linear tickets" or clicks "Refetch" in dashboard
2. Orchestrator calls Linear MCP `list_issues` with status filter (Todo, Backlog)
3. For each issue, calls `ticket_cache` to store in Maximous
4. Dashboard auto-refreshes via SSE

### Status Mapping

| Source | External Status | Maximous Status |
|--------|----------------|-----------------|
| Linear | Todo | `todo` |
| Linear | Backlog | `backlog` |
| Jira | TO DO | `todo` |

### Dashboard

- New "Tickets" page
- Filter dropdown: source (Linear/Jira), status (todo/backlog)
- Each ticket shows: title, status badge, priority, source icon, external link
- "Refetch" button (per source)
- Checkbox selection for launching

### Prerequisite Checks

On dashboard load and before launch, verify:
- `gh` CLI: run `which gh` — show error banner if missing
- Linear MCP: check if Linear tools are available — show warning if not
- Jira MCP: check if Jira tools are available — show warning if not

Errors displayed as persistent banners at top of dashboard.

## 3. Ticket Launcher

### Problem

Launching parallel work on multiple tickets requires manual worktree setup, agent configuration, and PR creation.

### Solution

Select tickets + team in dashboard (or via prompt), launch parallel worktrees with team agents. Track progress. Create PRs on completion.

### Schema

```sql
CREATE TABLE launches (
  id TEXT PRIMARY KEY,
  ticket_id TEXT NOT NULL REFERENCES tickets(id),
  team_id TEXT NOT NULL REFERENCES teams(id),
  branch TEXT NOT NULL,
  worktree_path TEXT DEFAULT '',
  status TEXT NOT NULL DEFAULT 'pending',  -- pending, running, completed, pr_created, failed
  pr_url TEXT DEFAULT '',
  error TEXT DEFAULT '',
  created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
  updated_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
);
```

### Tools

| Tool | Params | Returns |
|------|--------|---------|
| `launch_create` | `ticket_id, team_id, branch, worktree_path?` | `{ok, launch}` |
| `launch_update` | `id, status?, pr_url?, error?, worktree_path?` | `{ok, launch}` |
| `launch_list` | `status?, limit?, offset?` | `{launches[], count}` |

### Launch Flow

```
User selects tickets [T1, T2, T3] + team "frontend-squad"
  │
  ├─► For each ticket (parallel):
  │     1. launch_create(ticket_id=T1, team_id=frontend-squad, branch=T1-slug)
  │     2. git worktree add .worktrees/T1-slug -b feat/T1-slug
  │     3. launch_update(id, worktree_path=..., status=running)
  │     4. Spawn subagent in worktree with team config:
  │        - Agent reads team definition (roles, capabilities, model, prompt_hint)
  │        - Agent reads ticket (title, description, labels)
  │        - Agent works in isolated worktree
  │     5. Agent shares findings via memory (namespace: shared-exploration)
  │     6. On completion:
  │        a. git add + commit in worktree
  │        b. git push -u origin feat/T1-slug
  │        c. gh pr create --title "T1: <title>" --body "..."
  │        d. launch_update(id, status=pr_created, pr_url=<url>)
  │     7. On failure:
  │        launch_update(id, status=failed, error=<message>)
```

### Inter-Agent Communication

Agents in parallel worktrees share knowledge via maximous memory:
- **Namespace:** `shared-exploration` — code patterns, API findings, architecture notes
- **Before exploring:** check `memory_search(query, namespace="shared-exploration")`
- **After discovering:** `memory_set(namespace="shared-exploration", key="<topic>", value="<finding>")`
- **React to new knowledge:** `poll_changes(table_name="memory")` to see new shared entries

### Dashboard

- New "Launches" page
- Table: ticket title, team name, branch, status badge, PR link (clickable), error message
- Status colors: pending=gray, running=blue, completed=green, pr_created=purple, failed=red
- Auto-refresh via SSE
- "Launch" button on Tickets page (select tickets + choose team from dropdown)

## 4. Dashboard Updates Summary

### Pages

| Page | Status | Description |
|------|--------|-------------|
| Overview | Updated | Add launch stats, team count |
| Agents | Kept | Runtime agent registry (unchanged) |
| Tasks | Kept | Task coordination (unchanged) |
| Teams | New | Agent definitions + team management |
| Tickets | New | Cached Linear/Jira tickets with filters |
| Launches | New | Active launches with PR links |
| Memory | Kept | Unchanged |
| Sessions | Kept | Unchanged |
| Activity | Kept | Now includes team/ticket/launch changes |
| Messages | Removed | Replaced by Teams |

### API Endpoints (new)

```
GET    /api/agent-definitions         → list agent definitions
POST   /api/agent-definitions         → create/update agent definition
DELETE /api/agent-definitions/:id     → remove agent definition

GET    /api/teams                     → list teams with members
POST   /api/teams                     → create team
DELETE /api/teams/:name               → delete team
POST   /api/teams/:name/members       → add member
DELETE /api/teams/:name/members/:id   → remove member

GET    /api/tickets                   → list cached tickets (filter: source, status)
DELETE /api/tickets                   → clear ticket cache

GET    /api/launches                  → list launches (filter: status)
```

## 5. Tool Count

| Category | Before | After |
|----------|--------|-------|
| Memory | 5 | 5 |
| Messages | 3 | 0 (removed) |
| Tasks | 3 | 3 |
| Agents (runtime) | 3 | 3 |
| Sessions | 3 | 3 |
| Observation | 1 | 1 |
| Agent definitions | 0 | 3 (new) |
| Teams | 0 | 3 (new) |
| Tickets | 0 | 3 (new) |
| Launches | 0 | 3 (new) |
| **Total** | **18** | **24** |

## 6. Migration Strategy

1. Add new tables (agent_definitions, teams, team_members, tickets, launches)
2. Add triggers for new tables → changes table
3. Drop messages table
4. Remove message tool registrations
5. Add new tool registrations
6. Update dashboard frontend

All in a single schema migration. Existing data in other tables is untouched.
