// static/app.js
document.addEventListener('DOMContentLoaded', () => {
  initDashboard().catch(err => console.error('Dashboard init error:', err));
});

async function initDashboard() {
  const [devicesRes, actionsRes, historyRes, auditRes] = await Promise.all([
    fetchJson('/api/devices'),
    fetchJson('/api/actions'),
    fetchJson('/api/'),
    fetchJson('/api/audit')
  ]);

  const devices = Array.isArray(devicesRes) ? devicesRes : [];
  const actions = Array.isArray(actionsRes) ? actionsRes : [];
  const history = Array.isArray(historyRes) ? historyRes : [];
  const audit = Array.isArray(auditRes) ? auditRes : [];

  const totalDevices = devices.length;
  const approvedDevices = safeCount(devices, d => truthy(getField(d, ['approved', 'is_approved'])));
  const pendingDevices = Math.max(0, totalDevices - approvedDevices);
  const totalActions = actions.length;

  setText('#total-devices', totalDevices);
  setText('#approved-devices', approvedDevices);
  setText('#pending-devices', pendingDevices);
  setText('#total-actions', totalActions);

  renderRecentDevices(devices.slice().sort(sortByLastSeen).slice(0, 10));
  renderRecentActions(actions.slice().sort(sortByCreatedAt).slice(0, 10));
  renderTtlExpiring(devices);

  renderOsDistributionChart(devices);
  renderActionsOverTimeChart(actions);
}

async function fetchJson(url) {
  try {
    const r = await fetch(url, { credentials: 'same-origin' });
    if (!r.ok) return null;
    return await r.json();
  } catch (e) {
    return null;
  }
}

function setText(selector, v) {
  const el = document.querySelector(selector);
  if (!el) return;
  el.textContent = (v === undefined || v === null) ? '-' : String(v);
}

function truthy(v) {
  return v === true || v === 1 || v === '1' || v === 'true' || v === 'True';
}

function safeCount(arr, pred) {
  if (!Array.isArray(arr)) return 0;
  let c = 0;
  for (const x of arr) { try { if (pred(x)) c++; } catch (e) {} }
  return c;
}

function getField(item, names) {
  if (!item) return undefined;
  for (const n of names) {
    if (Object.prototype.hasOwnProperty.call(item, n)) return item[n];
  }
  return undefined;
}

function renderRecentDevices(devices) {
  const tbody = document.getElementById('recent-devices-body');
  if (!tbody) return; tbody.innerHTML = '';
  for (const d of devices) {
    const name = getField(d, ['device_name', 'name']) || getField(d, ['hostname']) || '-';
    const hostname = getField(d, ['hostname']) || '-';
    const ip = getField(d, ['ip_address', 'ip']) || '-';
    const lastSeen = formatTimestamp(getField(d, ['last_seen', 'last_seen_at', 'updated_at'])) || '-';
    const approved = truthy(getField(d, ['approved', 'is_approved'])) ? 'Yes' : 'No';
    const tr = document.createElement('tr');
    tr.innerHTML = `<td>${escapeHtml(name)}</td><td>${escapeHtml(hostname)}</td><td>${escapeHtml(ip)}</td><td>${escapeHtml(lastSeen)}</td><td>${escapeHtml(approved)}</td>`;
    tbody.appendChild(tr);
  }
}

function renderRecentActions(actions) {
  const tbody = document.getElementById('recent-actions-body');
  if (!tbody) return; tbody.innerHTML = '';
  for (const a of actions) {
    const actionName = getField(a, ['action_name', 'name', 'title']) || getField(a, ['description']) || '-';
    const device = getField(a, ['device_name', 'target_device', 'device']) || '-';
    const created = formatTimestamp(getField(a, ['created_at', 'submitted_at', 'timestamp'])) || '-';
    const status = getField(a, ['status', 'state']) || (getField(a, ['completed']) ? 'completed' : 'pending') || '-';
    const tr = document.createElement('tr');
    tr.innerHTML = `<td>${escapeHtml(actionName)}</td><td>${escapeHtml(device)}</td><td>${escapeHtml(created)}</td><td>${escapeHtml(status)}</td>`;
    tbody.appendChild(tr);
  }
}

function renderTtlExpiring(devices) {
  const ul = document.getElementById('ttl-expiring-list'); if (!ul) return; ul.innerHTML = '';
  const now = Date.now();
  const soon = (Array.isArray(devices) ? devices : []).filter(d => {
    const t = parseTimestamp(getField(d, ['ttl_expires_at', 'ttl_expires', 'ttl_expire_ts']));
    return t && (t - now) < 24*3600*1000 && (t - now) > 0;
  }).slice(0,8);
  if (!soon.length) { ul.innerHTML = '<li class="muted">No devices expiring within 24 hours</li>'; return; }
  for (const d of soon) {
    const name = getField(d, ['device_name','name']) || '-';
    const ttl = formatTimestamp(getField(d, ['ttl_expires_at','ttl_expires','ttl_expire_ts'])) || '-';
    const li = document.createElement('li'); li.innerHTML = `<strong>${escapeHtml(name)}</strong> — ${escapeHtml(ttl)}`; ul.appendChild(li);
  }
}

function renderOsDistributionChart(devices) {
  const el = document.getElementById('os-distribution-chart'); if (!el || !Array.isArray(devices)) return;
  const map = {};
  for (const d of devices) {
    const os = getField(d, ['os_name','os','platform']) || 'Unknown';
    map[os] = (map[os] || 0) + 1;
  }
  const labels = Object.keys(map), values = labels.map(l=>map[l]);
  if (el._chart) el._chart.destroy();
  el._chart = new Chart(el.getContext('2d'), { type: 'pie', data: { labels, datasets: [{ data: values }] }, options: { responsive: true, plugins: { legend: { position: 'bottom' } } } });
}

function renderActionsOverTimeChart(actions) {
  const el = document.getElementById('actions-over-time-chart'); if (!el || !Array.isArray(actions)) return;
  const now = new Date(); const days = 30; const counts = {};
  for (let i=0;i<days;i++){ const d=new Date(now); d.setDate(now.getDate() - (days-1-i)); counts[d.toISOString().slice(0,10)] = 0; }
  for (const a of actions) {
    const t = parseTimestamp(getField(a, ['created_at','submitted_at','timestamp'])); if (!t) continue;
    const k = new Date(t).toISOString().slice(0,10); if (counts.hasOwnProperty(k)) counts[k] += 1;
  }
  const labels = Object.keys(counts), data = labels.map(l=>counts[l]);
  if (el._chart) el._chart.destroy();
  el._chart = new Chart(el.getContext('2d'), { type: 'bar', data: { labels, datasets: [{ label: 'Actions', data }] }, options: { responsive: true, scales: { y: { beginAtZero: true } }, plugins: { legend: { display: false } } } });
}

function sortByLastSeen(a,b) { return (parseTimestamp(getField(b,['last_seen','last_seen_at','updated_at']))||0) - (parseTimestamp(getField(a,['last_seen','last_seen_at','updated_at']))||0); }
function sortByCreatedAt(a,b) { return (parseTimestamp(getField(b,['created_at','submitted_at','timestamp']))||0) - (parseTimestamp(getField(a,['created_at','submitted_at','timestamp']))||0); }

function parseTimestamp(v) {
  if (!v) return null;
  if (typeof v === 'number') return v;
  const n = Number(v);
  if (!Number.isNaN(n)) return n;
  const d = Date.parse(String(v)); return Number.isNaN(d) ? null : d;
}

function formatTimestamp(v) { const t = parseTimestamp(v); if (!t) return null; return new Date(t).toLocaleString(); }

function escapeHtml(s) { if (s === null || s === undefined) return ''; return String(s).replaceAll('&','&amp;').replaceAll('<','&lt;').replaceAll('>','&gt;').replaceAll('"','&quot;').replaceAll("'",'&#39;'); }
