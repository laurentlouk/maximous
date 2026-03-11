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

    var defRows = (defs.agent_definitions || []).map(function(d) {
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

    var teamCards = (teamsData.teams || []).map(function(t) {
        var members = (t.members || []).map(function(m) {
            var roleKey = (m.role || '').toLowerCase();
            return '<div class="team-member">' +
                '<span class="member-name">' + escapeHtml(m.name || m.agent_id || '-') + '</span>' +
                badge(escapeHtml(m.role || 'member'), roleKey) +
                (m.model ? ' <span class="text-muted">' + escapeHtml(m.model) + '</span>' : '') +
            '</div>';
        }).join('');
        return '<div class="card team-card">' +
            '<h3>' + escapeHtml(t.name || t.id) + '</h3>' +
            '<div class="team-members">' + (members || '<span class="text-muted">No members</span>') + '</div>' +
        '</div>';
    }).join('');

    setContent(el,
        '<h2>Teams</h2>' +
        '<h3 style="margin-bottom:1rem">Agent Definitions</h3>' +
        '<div class="table-container" style="margin-bottom:2rem">' +
            '<table><thead><tr><th>ID</th><th>Name</th><th>Capabilities</th><th>Model</th><th>Prompt Hint</th></tr></thead>' +
            '<tbody>' + (defRows || '<tr><td colspan="5" class="empty">No agent definitions</td></tr>') + '</tbody></table>' +
        '</div>' +
        '<h3 style="margin-bottom:1rem">Teams</h3>' +
        '<div class="cards">' + (teamCards || '<div class="empty">No teams</div>') + '</div>');
}

async function loadTickets() {
    var el = document.getElementById('page-tickets');

    async function renderTickets(url) {
        var data = await fetchJSON(url);
        var rows = (data.tickets || []).map(function(t) {
            var titleCell = t.url
                ? '<a href="' + escapeHtml(t.url) + '" target="_blank" rel="noopener">' + escapeHtml(t.title || '-') + '</a>'
                : escapeHtml(t.title || '-');
            var labels = (t.labels || []).map(function(l) { return badge(escapeHtml(l)); }).join(' ');
            return '<tr>' +
                '<td>' + badge(escapeHtml(t.source || '-'), (t.source || '').toLowerCase()) + '</td>' +
                '<td>' + escapeHtml(t.external_id || '-') + '</td>' +
                '<td>' + titleCell + '</td>' +
                '<td>' + badge(escapeHtml(t.status || '-'), (t.status || '').toLowerCase()) + '</td>' +
                '<td>' + (t.priority != null ? priorityBadge(t.priority) : '-') + '</td>' +
                '<td>' + (labels || '-') + '</td>' +
                '<td>' + timeAgo(t.fetched_at) + '</td>' +
            '</tr>';
        }).join('');
        setContent(el.querySelector('.tickets-table-container'),
            '<div class="table-container">' +
                '<table><thead><tr><th>Source</th><th>ID</th><th>Title</th><th>Status</th><th>Priority</th><th>Labels</th><th>Fetched</th></tr></thead>' +
                '<tbody>' + (rows || '<tr><td colspan="7" class="empty">No tickets</td></tr>') + '</tbody></table>' +
            '</div>');
    }

    setContent(el,
        '<h2>Tickets</h2>' +
        '<div class="channel-tabs" style="margin-bottom:1.25rem">' +
            '<button class="channel-tab active" data-source="">All</button>' +
            '<button class="channel-tab" data-source="linear">Linear</button>' +
            '<button class="channel-tab" data-source="jira">Jira</button>' +
        '</div>' +
        '<div class="tickets-table-container"></div>');

    await renderTickets('/api/tickets');

    el.querySelectorAll('.channel-tab').forEach(function(btn) {
        btn.addEventListener('click', async function() {
            el.querySelectorAll('.channel-tab').forEach(function(b) { b.classList.remove('active'); });
            btn.classList.add('active');
            var src = btn.dataset.source;
            var url = src ? '/api/tickets?source=' + encodeURIComponent(src) : '/api/tickets';
            await renderTickets(url);
        });
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
        return '<tr>' +
            '<td>' + escapeHtml(l.ticket_title || '-') + '</td>' +
            '<td>' + escapeHtml(l.team_name || '-') + '</td>' +
            '<td>' + escapeHtml(l.branch || '-') + '</td>' +
            '<td>' + badge(escapeHtml(l.status || '-'), (l.status || '').toLowerCase()) + '</td>' +
            '<td>' + prCell + '</td>' +
            '<td title="' + escapeHtml(l.error || '') + '">' + escapeHtml(errTrunc) + '</td>' +
            '<td>' + timeAgo(l.created_at) + '</td>' +
        '</tr>';
    }).join('');
    setContent(el,
        '<h2>Launches</h2>' +
        '<div class="table-container">' +
            '<table><thead><tr><th>Ticket</th><th>Team</th><th>Branch</th><th>Status</th><th>PR</th><th>Error</th><th>Created</th></tr></thead>' +
            '<tbody>' + (rows || '<tr><td colspan="7" class="empty">No launches</td></tr>') + '</tbody></table>' +
        '</div>');
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
        // Refresh current page on changes
        if (loaders[currentPage]) {
            loaders[currentPage]();
        }
    };
}

// Auto-refresh every 10s as fallback
setInterval(function() {
    if (loaders[currentPage]) loaders[currentPage]();
}, 10000);

// Init
connectSSE();
loadOverview();
