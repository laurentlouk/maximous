---
name: dashboard
description: Open the maximous web dashboard in your browser
allowed_tools: ["Bash"]
---

Launch the maximous web dashboard. First kill any existing dashboard process, then start a new one in the background using `run_in_background`:

```bash
lsof -ti:8375 | xargs kill 2>/dev/null; sleep 0.5; maximous dashboard --db .maximous/brain.db
```

Use the Bash tool with `run_in_background: true` so the server runs without blocking the conversation.

After launching, tell the user the dashboard is opening at http://127.0.0.1:8375 and should appear in their browser automatically.
