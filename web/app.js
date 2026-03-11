const API = '';
let currentPage = 'overview';

// Navigation
document.querySelectorAll('[data-page]').forEach(link => {
    link.addEventListener('click', (e) => {
        e.preventDefault();
        navigateTo(link.dataset.page);
    });
});

function navigateTo(page) {
    document.querySelectorAll('.page').forEach(p => p.classList.remove('active'));
    document.querySelectorAll('[data-page]').forEach(l => l.classList.remove('active'));
    document.getElementById('page-' + page).classList.add('active');
    document.querySelector('[data-page="' + page + '"]').classList.add('active');
    currentPage = page;
    loaders[page]();
}

const loaders = {
    overview: loadOverview,
    agents: loadAgents,
    tasks: loadTasks,
    teams: loadTeams,
    tickets: loadTickets,
    launches: loadLaunches,
    memory: loadMemory,
    sessions: loadSessions,
    activity: loadActivity,
};

// Helpers
function formatTime(epoch) {
    if (!epoch) return '-';
    const d = new Date(epoch * 1000);
    return d.toLocaleString();
}

function timeAgo(epoch) {
    if (!epoch) return '-';
    const seconds = Math.floor(Date.now() / 1000) - epoch;
    if (seconds < 60) return seconds + 's ago';
    if (seconds < 3600) return Math.floor(seconds / 60) + 'm ago';
    if (seconds < 86400) return Math.floor(seconds / 3600) + 'h ago';
    return Math.floor(seconds / 86400) + 'd ago';
}

function badge(text, type) {
    return '<span class="badge badge-' + (type || text) + '">' + text + '</span>';
}

function priorityBadge(p) {
    const labels = ['critical', 'high', 'normal', 'low'];
    return '<span class="badge priority-' + p + '">' + (labels[p] || p) + '</span>';
}

function escapeHtml(str) {
    if (!str) return '';
    return String(str).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
}

async function fetchJSON(url) {
    const resp = await fetch(API + url);
    return resp.json();
}

// Safe DOM helper: sets sanitized HTML content on an element.
// All dynamic values are escaped via escapeHtml() before being
// interpolated into the markup strings, so the resulting HTML
// contains no unescaped user-controlled content.
function setContent(el, sanitizedMarkup) {
    el.innerHTML = sanitizedMarkup; // nosemgrep: innerHTML-with-escaped-content
}

// Page loaders
async function loadOverview() {
    var results = await Promise.all([
        fetchJSON('/api/overview'),
        fetchJSON('/api/prerequisites'),
    ]);
    const data = results[0];
    var prereqs = results[1];
    const el = document.getElementById('page-overview');
    var banner = '';
    if (!prereqs.all_ok) {
        banner = '<div class="prereq-banner">' +
            (prereqs.errors || []).map(function(e) {
                return '<div class="prereq-error">' + escapeHtml(e) + '</div>';
            }).join('') +
        '</div>';
    }
    const taskCards = Object.entries(data.tasks_by_status || {})
        .map(function(entry) {
            return '<div class="card"><h3>' + escapeHtml(entry[0]) + '</h3><div class="value">' + entry[1] + '</div></div>';
        })
        .join('');
    setContent(el,
        '<h2>Overview</h2>' +
        banner +
        '<div class="cards">' +
            '<div class="card"><h3>Agents</h3><div class="value">' + (data.agents || 0) + '</div></div>' +
            '<div class="card"><h3>Teams</h3><div class="value">' + (data.teams || 0) + '</div></div>' +
            '<div class="card"><h3>Tickets</h3><div class="value">' + (data.tickets || 0) + '</div></div>' +
            '<div class="card"><h3>Active Launches</h3><div class="value">' + (data.active_launches || 0) + '</div></div>' +
            '<div class="card"><h3>Memory Entries</h3><div class="value">' + (data.memory_entries || 0) + '</div></div>' +
            '<div class="card"><h3>Active Sessions</h3><div class="value">' + (data.active_sessions || 0) + '</div></div>' +
            taskCards +
        '</div>' +
        '<h3 style="margin-top:2rem">Recent Activity</h3>' +
        '<div id="overview-activity"></div>');
    var changes = await fetchJSON('/api/changes?limit=20');
    setContent(document.getElementById('overview-activity'), renderActivityFeed(changes.changes || []));
}

async function loadAgents() {
    var data = await fetchJSON('/api/agents?limit=100');
    var el = document.getElementById('page-agents');
    var now = Math.floor(Date.now() / 1000);
    var rows = (data.agents || []).map(function(a) {
        var stale = (now - a.last_heartbeat) > 60;
        return '<tr>' +
            '<td><span class="status-dot ' + (stale ? 'stale' : 'connected') + '"></span> ' + escapeHtml(a.name) + '</td>' +
            '<td>' + escapeHtml(a.id) + '</td>' +
            '<td>' + badge(a.status) + '</td>' +
            '<td>' + escapeHtml(a.capabilities || '-') + '</td>' +
            '<td>' + timeAgo(a.last_heartbeat) + '</td>' +
        '</tr>';
    }).join('');
    setContent(el,
        '<h2>Agents</h2>' +
        '<div class="table-container">' +
            '<table><thead><tr><th>Name</th><th>ID</th><th>Status</th><th>Capabilities</th><th>Last Heartbeat</th></tr></thead>' +
            '<tbody>' + (rows || '<tr><td colspan="5" class="empty">No agents registered</td></tr>') + '</tbody></table>' +
        '</div>');
}

async function loadTasks() {
    var data = await fetchJSON('/api/tasks?limit=100');
    var el = document.getElementById('page-tasks');
    var rows = (data.tasks || []).map(function(t) {
        return '<tr>' +
            '<td title="' + escapeHtml(t.id) + '">' + escapeHtml(t.id).substring(0, 8) + '...</td>' +
            '<td>' + escapeHtml(t.title) + '</td>' +
            '<td>' + badge(t.status) + '</td>' +
            '<td>' + priorityBadge(t.priority) + '</td>' +
            '<td>' + escapeHtml(t.assigned_to || '-') + '</td>' +
            '<td>' + timeAgo(t.created_at) + '</td>' +
        '</tr>';
    }).join('');
    setContent(el,
        '<h2>Tasks</h2>' +
        '<div class="table-container">' +
            '<table><thead><tr><th>ID</th><th>Title</th><th>Status</th><th>Priority</th><th>Assigned</th><th>Created</th></tr></thead>' +
            '<tbody>' + (rows || '<tr><td colspan="6" class="empty">No tasks</td></tr>') + '</tbody></table>' +
        '</div>');
}

async function loadTeams() {
    var el = document.getElementById('page-teams');
    var results = await Promise.all([
        fetchJSON('/api/agent-definitions'),
        fetchJSON('/api/teams'),
    ]);
    var defs = results[0];
    var teamsData = results[1];

    var defRows = (defs.agents || defs.agent_definitions || []).map(function(d) {
        var caps = '';
        try { caps = JSON.parse(d.capabilities || '[]').join(', '); } catch(e) { caps = escapeHtml(d.capabilities || '-'); }
        var prompt = d.prompt_hint || '';
        var promptTrunc = prompt.substring(0, 60) + (prompt.length > 60 ? '...' : '');
        var modelKey = (d.model || '').toLowerCase().replace(/[^a-z]/g, '');
        var modelBadge = d.model ? badge(escapeHtml(d.model), modelKey) : '-';
        return '<tr>' +
            '<td title="' + escapeHtml(d.id) + '">' + escapeHtml(d.id).substring(0, 8) + '...</td>' +
            '<td>' + escapeHtml(d.name || '-') + '</td>' +
            '<td>' + escapeHtml(caps) + '</td>' +
            '<td>' + modelBadge + '</td>' +
            '<td title="' + escapeHtml(prompt) + '">' + escapeHtml(promptTrunc) + '</td>' +
        '</tr>';
    }).join('');

    var allAgents = defs.agents || defs.agent_definitions || [];
    var teamCards = (teamsData.teams || []).map(function(t) {
        var memberIds = (t.members || []).map(function(m) { return m.agent_id; });
        var members = (t.members || []).map(function(m) {
            var roleKey = (m.role || '').toLowerCase();
            return '<div class="team-member">' +
                '<span class="member-name">' + escapeHtml(m.name || m.agent_id || '-') + '</span>' +
                badge(escapeHtml(m.role || 'member'), roleKey) +
                (m.model ? ' <span class="text-muted">' + escapeHtml(m.model) + '</span>' : '') +
                '<button class="btn-remove-member" data-team="' + escapeHtml(t.name) + '" data-agent="' + escapeHtml(m.agent_id) + '" title="Remove">&times;</button>' +
            '</div>';
        }).join('');
        var availableAgents = allAgents.filter(function(a) { return memberIds.indexOf(a.id) === -1; });
        var addMemberHtml = '';
        if (availableAgents.length > 0) {
            var opts = availableAgents.map(function(a) {
                return '<option value="' + escapeHtml(a.id) + '">' + escapeHtml(a.name || a.id) + '</option>';
            }).join('');
            addMemberHtml = '<div class="team-card-actions">' +
                '<div class="add-member-row">' +
                    '<select class="agent-select" data-team="' + escapeHtml(t.name) + '">' +
                        '<option value="">Add agent...</option>' + opts +
                    '</select>' +
                    '<button class="btn-add-member" data-team="' + escapeHtml(t.name) + '">Add</button>' +
                '</div>' +
            '</div>';
        }
        return '<div class="card team-card">' +
            '<div class="team-card-header">' +
                '<h3>' + escapeHtml(t.name || t.id) + '</h3>' +
                '<button class="btn-delete-team" data-team="' + escapeHtml(t.name) + '" title="Delete team">&times;</button>' +
            '</div>' +
            (t.description ? '<p class="text-muted" style="font-size:0.8rem;margin:0 0 0.5rem">' + escapeHtml(t.description) + '</p>' : '') +
            '<div class="team-members">' + (members || '<span class="text-muted">No members</span>') + '</div>' +
            addMemberHtml +
        '</div>';
    }).join('');

    setContent(el,
        '<h2>Teams</h2>' +
        '<div class="section-header"><h3>Agent Definitions</h3><button class="btn-create" id="btn-new-agent">+ New Agent</button></div>' +
        '<div id="agent-form-container" class="form-container hidden"></div>' +
        '<div class="table-container" style="margin-bottom:2rem">' +
            '<table><thead><tr><th>ID</th><th>Name</th><th>Capabilities</th><th>Model</th><th>Prompt Hint</th></tr></thead>' +
            '<tbody>' + (defRows || '<tr><td colspan="5" class="empty">No agent definitions</td></tr>') + '</tbody></table>' +
        '</div>' +
        '<div class="section-header"><h3>Teams</h3><button class="btn-create" id="btn-new-team">+ New Team</button></div>' +
        '<div id="team-form-container" class="form-container hidden"></div>' +
        '<div class="teams-grid">' + (teamCards || '<div class="empty">No teams</div>') + '</div>');

    // New Agent form toggle
    document.getElementById('btn-new-agent').addEventListener('click', function() {
        var container = document.getElementById('agent-form-container');
        if (!container.classList.contains('hidden')) {
            container.classList.add('hidden');
            return;
        }
        setContent(container,
            '<form id="form-new-agent" class="create-form">' +
                '<div class="form-row">' +
                    '<label>ID <input type="text" name="id" placeholder="e.g. frontend-dev" required></label>' +
                    '<label>Name <input type="text" name="name" placeholder="e.g. Frontend Developer" required></label>' +
                '</div>' +
                '<div class="form-row">' +
                    '<label>Model <select name="model"><option value="sonnet">Sonnet</option><option value="opus">Opus</option><option value="haiku">Haiku</option></select></label>' +
                    '<label>Capabilities <input type="text" name="capabilities" placeholder="comma-separated, e.g. code,test,review"></label>' +
                '</div>' +
                '<div class="form-row">' +
                    '<label class="full-width">Prompt Hint <textarea name="prompt_hint" rows="3" placeholder="System prompt guidance for this agent"></textarea></label>' +
                '</div>' +
                '<div class="form-actions">' +
                    '<button type="submit" class="btn-submit">Create Agent</button>' +
                    '<button type="button" class="btn-cancel" id="cancel-agent">Cancel</button>' +
                '</div>' +
            '</form>');
        container.classList.remove('hidden');
        document.getElementById('cancel-agent').addEventListener('click', function() {
            container.classList.add('hidden');
        });
        document.getElementById('form-new-agent').addEventListener('submit', async function(e) {
            e.preventDefault();
            var f = e.target;
            var caps = f.capabilities.value.trim();
            var body = {
                id: f.id.value.trim(),
                name: f.name.value.trim(),
                model: f.model.value,
                capabilities: caps ? caps.split(',').map(function(s) { return s.trim(); }) : [],
                prompt_hint: f.prompt_hint.value.trim(),
            };
            var resp = await fetch(API + '/api/agent-definitions', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify(body),
            });
            var result = await resp.json();
            if (result.ok) {
                container.classList.add('hidden');
                loadTeams();
            } else {
                alert('Error: ' + (result.error || 'Unknown error'));
            }
        });
    });

    // New Team form toggle
    document.getElementById('btn-new-team').addEventListener('click', function() {
        var container = document.getElementById('team-form-container');
        if (!container.classList.contains('hidden')) {
            container.classList.add('hidden');
            return;
        }
        setContent(container,
            '<form id="form-new-team" class="create-form">' +
                '<div class="form-row">' +
                    '<label>Name <input type="text" name="name" placeholder="e.g. backend-team" required></label>' +
                    '<label>Description <input type="text" name="description" placeholder="Optional description"></label>' +
                '</div>' +
                '<div class="form-actions">' +
                    '<button type="submit" class="btn-submit">Create Team</button>' +
                    '<button type="button" class="btn-cancel" id="cancel-team">Cancel</button>' +
                '</div>' +
            '</form>');
        container.classList.remove('hidden');
        document.getElementById('cancel-team').addEventListener('click', function() {
            container.classList.add('hidden');
        });
        document.getElementById('form-new-team').addEventListener('submit', async function(e) {
            e.preventDefault();
            var f = e.target;
            var body = {
                name: f.name.value.trim(),
                description: f.description.value.trim(),
            };
            var resp = await fetch(API + '/api/teams', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify(body),
            });
            var result = await resp.json();
            if (result.ok) {
                container.classList.add('hidden');
                loadTeams();
            } else {
                alert('Error: ' + (result.error || 'Unknown error'));
            }
        });
    });

    // Add member to team
    el.querySelectorAll('.btn-add-member').forEach(function(btn) {
        btn.addEventListener('click', async function() {
            var teamName = this.dataset.team;
            var select = el.querySelector('.agent-select[data-team="' + teamName + '"]');
            var agentId = select.value;
            if (!agentId) return;
            var resp = await fetch(API + '/api/teams/' + encodeURIComponent(teamName) + '/members', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ agent_id: agentId }),
            });
            var result = await resp.json();
            if (result.ok) {
                loadTeams();
            } else {
                alert('Error: ' + (result.error || 'Unknown error'));
            }
        });
    });

    // Remove member from team
    el.querySelectorAll('.btn-remove-member').forEach(function(btn) {
        btn.addEventListener('click', async function() {
            var teamName = this.dataset.team;
            var agentId = this.dataset.agent;
            var resp = await fetch(API + '/api/teams/' + encodeURIComponent(teamName) + '/members/' + encodeURIComponent(agentId), {
                method: 'DELETE',
            });
            var result = await resp.json();
            if (result.ok) {
                loadTeams();
            } else {
                alert('Error: ' + (result.error || 'Unknown error'));
            }
        });
    });

    // Delete team
    el.querySelectorAll('.btn-delete-team').forEach(function(btn) {
        btn.addEventListener('click', async function() {
            var teamName = this.dataset.team;
            if (!confirm('Delete team "' + teamName + '"?')) return;
            var resp = await fetch(API + '/api/teams/' + encodeURIComponent(teamName), {
                method: 'DELETE',
            });
            var result = await resp.json();
            if (result.ok) {
                loadTeams();
            } else {
                alert('Error: ' + (result.error || 'Unknown error'));
            }
        });
    });
}

async function loadTickets() {
    var el = document.getElementById('page-tickets');
    var teamsData = await fetchJSON('/api/teams');
    var teams = teamsData.teams || [];

    async function renderTickets(url) {
        var data = await fetchJSON(url);
        var teamOpts = teams.map(function(t) {
            return '<option value="' + escapeHtml(t.id) + '">' + escapeHtml(t.name) + '</option>';
        }).join('');
        var rows = (data.tickets || []).map(function(t) {
            var titleCell = t.url
                ? '<a href="' + escapeHtml(t.url) + '" target="_blank" rel="noopener">' + escapeHtml(t.title || '-') + '</a>'
                : escapeHtml(t.title || '-');
            var parsedLabels = typeof t.labels === 'string' ? JSON.parse(t.labels || '[]') : (t.labels || []);
            var labels = parsedLabels.map(function(l) { return badge(escapeHtml(l)); }).join(' ');
            var launchCell = teams.length > 0
                ? '<div class="launch-cell">' +
                    '<select class="launch-team-select" data-ticket="' + escapeHtml(t.id) + '">' +
                        '<option value="">Team...</option>' + teamOpts +
                    '</select>' +
                    '<button class="btn-launch" data-ticket="' + escapeHtml(t.id) + '">Launch</button>' +
                  '</div>'
                : '-';
            return '<tr>' +
                '<td>' + badge(escapeHtml(t.source || '-'), (t.source || '').toLowerCase()) + '</td>' +
                '<td>' + titleCell + '</td>' +
                '<td>' + badge(escapeHtml(t.status || '-'), (t.status || '').toLowerCase()) + '</td>' +
                '<td>' + escapeHtml(t.assignee || '-') + '</td>' +
                '<td>' + (t.priority != null ? priorityBadge(t.priority) : '-') + '</td>' +
                '<td>' + (labels || '-') + '</td>' +
                '<td>' + launchCell + '</td>' +
            '</tr>';
        }).join('');
        setContent(el.querySelector('.tickets-table-container'),
            '<div class="table-container">' +
                '<table><thead><tr><th>Source</th><th>Title</th><th>Status</th><th>Assignee</th><th>Priority</th><th>Labels</th><th>Launch</th></tr></thead>' +
                '<tbody>' + (rows || '<tr><td colspan="7" class="empty">No tickets</td></tr>') + '</tbody></table>' +
            '</div>');

        // Launch button handlers
        el.querySelectorAll('.btn-launch').forEach(function(btn) {
            btn.addEventListener('click', async function() {
                var ticketId = this.dataset.ticket;
                var select = el.querySelector('.launch-team-select[data-ticket="' + ticketId + '"]');
                var teamId = select.value;
                if (!teamId) { alert('Select a team first'); return; }
                this.disabled = true;
                this.textContent = 'Launching...';
                // Step 1: Create the launch
                var resp = await fetch(API + '/api/launches', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ ticket_id: ticketId, team_id: teamId }),
                });
                var result = await resp.json();
                if (!result.ok) {
                    alert('Error: ' + (result.error || 'Unknown error'));
                    this.disabled = false;
                    this.textContent = 'Launch';
                    return;
                }
                // Step 2: Execute — launches Claude Code in background
                var launchId = result.data && result.data.launch && result.data.launch.id;
                if (launchId) {
                    var execResp = await fetch(API + '/api/launches/' + encodeURIComponent(launchId) + '/execute', {
                        method: 'POST',
                    });
                    var execResult = await execResp.json();
                    if (execResult.ok) {
                        this.textContent = 'Running';
                        this.classList.add('btn-running');
                    } else {
                        this.textContent = 'Launched';
                        console.error('Execute error:', execResult.error);
                    }
                } else {
                    this.textContent = 'Launched';
                }
                setTimeout(function() { renderTickets(url); }, 2000);
            });
        });
    }

    setContent(el,
        '<div class="section-header"><h2>Tickets</h2><button class="btn-create" id="btn-refresh-tickets">Refresh</button></div>' +
        '<div class="tickets-table-container"></div>');

    await renderTickets('/api/tickets');

    document.getElementById('btn-refresh-tickets').addEventListener('click', function() {
        renderTickets('/api/tickets');
    });
}

async function loadLaunches() {
    var data = await fetchJSON('/api/launches');
    var el = document.getElementById('page-launches');
    var rows = (data.launches || []).map(function(l) {
        var prCell = l.pr_url
            ? '<a href="' + escapeHtml(l.pr_url) + '" target="_blank" rel="noopener">PR</a>'
            : '-';
        var errTrunc = l.error ? l.error.substring(0, 60) + (l.error.length > 60 ? '...' : '') : '';
        var statusBadge = badge(escapeHtml(l.status || 'pending'));
        return '<tr>' +
            '<td>' + escapeHtml(l.ticket_title || l.ticket_id || '-') + '</td>' +
            '<td>' + escapeHtml(l.team_name || '-') + '</td>' +
            '<td><code style="font-size:0.75rem">' + escapeHtml(l.branch || '-') + '</code></td>' +
            '<td>' + statusBadge + '</td>' +
            '<td>' + prCell + '</td>' +
            '<td title="' + escapeHtml(l.error || '') + '">' + escapeHtml(errTrunc) + '</td>' +
            '<td>' + timeAgo(l.created_at) + '</td>' +
            '<td><button class="btn-remove-member btn-delete-launch" data-id="' + escapeHtml(l.id) + '" title="Delete">&times;</button></td>' +
        '</tr>';
    }).join('');
    setContent(el,
        '<h2>Launches</h2>' +
        '<div class="table-container">' +
            '<table><thead><tr><th>Ticket</th><th>Team</th><th>Branch</th><th>Status</th><th>PR</th><th>Error</th><th>Created</th><th></th></tr></thead>' +
            '<tbody>' + (rows || '<tr><td colspan="8" class="empty">No launches</td></tr>') + '</tbody></table>' +
        '</div>');

    // Delete launch handlers
    el.querySelectorAll('.btn-delete-launch').forEach(function(btn) {
        btn.addEventListener('click', async function() {
            if (!confirm('Delete this launch?')) return;
            var id = this.dataset.id;
            var resp = await fetch(API + '/api/launches/' + encodeURIComponent(id), {
                method: 'DELETE',
            });
            var result = await resp.json();
            if (result.ok) {
                loadLaunches();
            } else {
                alert('Error: ' + (result.error || 'Unknown error'));
            }
        });
    });
}

async function loadMemory() {
    var data = await fetchJSON('/api/memory?limit=50');
    var el = document.getElementById('page-memory');
    var nsList = (data.namespaces || []).map(function(ns) {
        return '<li class="ns-item" data-ns="' + escapeHtml(ns) + '">' + escapeHtml(ns) + '</li>';
    }).join('');
    var entries = renderMemoryEntries(data.entries || []);
    setContent(el,
        '<h2>Memory</h2>' +
        '<div class="memory-layout">' +
            '<div class="namespace-list">' +
                '<h4>Namespaces</h4>' +
                '<ul><li class="ns-item active" data-ns="">All</li>' + nsList + '</ul>' +
            '</div>' +
            '<div class="memory-content">' + entries + '</div>' +
        '</div>');
    el.querySelectorAll('.ns-item').forEach(function(item) {
        item.addEventListener('click', async function() {
            el.querySelectorAll('.ns-item').forEach(function(i) { i.classList.remove('active'); });
            item.classList.add('active');
            var ns = item.dataset.ns;
            var url = ns ? '/api/memory?namespace=' + encodeURIComponent(ns) : '/api/memory?limit=50';
            var filtered = await fetchJSON(url);
            setContent(el.querySelector('.memory-content'), renderMemoryEntries(filtered.entries || []));
        });
    });
}

function renderMemoryEntries(entries) {
    if (!entries.length) return '<div class="empty">No memory entries</div>';
    var rows = entries.map(function(e) {
        var val = e.value || '';
        var truncated = val.substring(0, 100) + (val.length > 100 ? '...' : '');
        return '<tr>' +
            '<td>' + escapeHtml(e.namespace) + '</td>' +
            '<td>' + escapeHtml(e.key) + '</td>' +
            '<td class="value-cell" title="' + escapeHtml(val) + '">' + escapeHtml(truncated) + '</td>' +
            '<td>' + (e.observation_type ? badge(e.observation_type) : '-') + '</td>' +
            '<td>' + escapeHtml(e.category || '-') + '</td>' +
            '<td>' + timeAgo(e.updated_at) + '</td>' +
        '</tr>';
    }).join('');
    return '<div class="table-container"><table>' +
        '<thead><tr><th>Namespace</th><th>Key</th><th>Value</th><th>Type</th><th>Category</th><th>Updated</th></tr></thead>' +
        '<tbody>' + rows + '</tbody></table></div>';
}

async function loadSessions() {
    var data = await fetchJSON('/api/sessions?limit=50');
    var el = document.getElementById('page-sessions');
    var rows = (data.sessions || []).map(function(s) {
        return '<tr>' +
            '<td title="' + escapeHtml(s.id) + '">' + escapeHtml(s.id).substring(0, 8) + '...</td>' +
            '<td>' + escapeHtml(s.agent_id || '-') + '</td>' +
            '<td>' + badge(s.status) + '</td>' +
            '<td>' + escapeHtml(s.summary || '-') + '</td>' +
            '<td>' + formatTime(s.started_at) + '</td>' +
            '<td>' + (s.ended_at ? formatTime(s.ended_at) : '-') + '</td>' +
        '</tr>';
    }).join('');
    setContent(el,
        '<h2>Sessions</h2>' +
        '<div class="table-container">' +
            '<table><thead><tr><th>ID</th><th>Agent</th><th>Status</th><th>Summary</th><th>Started</th><th>Ended</th></tr></thead>' +
            '<tbody>' + (rows || '<tr><td colspan="6" class="empty">No sessions</td></tr>') + '</tbody></table>' +
        '</div>');
}

async function loadActivity() {
    var data = await fetchJSON('/api/changes?limit=100');
    setContent(document.getElementById('page-activity'),
        '<h2>Activity Feed</h2>' +
        renderActivityFeed(data.changes || []));
}

function renderActivityFeed(changes) {
    if (!changes.length) return '<div class="empty">No activity</div>';
    var items = changes.map(function(c) {
        var desc = '';
        if (c.summary) {
            try {
                var summary = JSON.parse(c.summary);
                desc = Object.entries(summary).map(function(entry) {
                    return entry[0] + ': ' + entry[1];
                }).join(', ');
            } catch (e) {
                desc = c.summary;
            }
        }
        return '<div class="activity-item">' +
            '<span class="time">' + timeAgo(c.created_at) + '</span>' +
            '<span class="badge badge-' + escapeHtml(c.action) + '">' + escapeHtml(c.action) + '</span>' +
            '<span class="table-name">' + escapeHtml(c.table_name) + '</span>' +
            '<span class="desc">' + escapeHtml(desc) + '</span>' +
        '</div>';
    }).join('');
    return '<div class="activity-feed">' + items + '</div>';
}

// SSE real-time updates
var eventSource;
function connectSSE() {
    if (eventSource) {
        eventSource.close();
    }
    eventSource = new EventSource('/api/events');
    eventSource.onopen = function() {
        document.getElementById('connection-status').classList.add('connected');
        document.getElementById('connection-status').classList.remove('disconnected');
    };
    eventSource.onerror = function() {
        document.getElementById('connection-status').classList.remove('connected');
        document.getElementById('connection-status').classList.add('disconnected');
        eventSource.close();
        setTimeout(connectSSE, 3000);
    };
    eventSource.onmessage = function() {
        // Skip refresh when a form is open to avoid destroying user input
        if (document.querySelector('.create-form')) return;
        if (loaders[currentPage]) {
            loaders[currentPage]();
        }
    };
}

// Auto-refresh every 10s as fallback
setInterval(function() {
    // Skip refresh when a form is open to avoid destroying user input
    if (document.querySelector('.create-form')) return;
    if (loaders[currentPage]) loaders[currentPage]();
}, 10000);

// Init
connectSSE();
loadOverview();
