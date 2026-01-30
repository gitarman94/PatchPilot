// static/app.js
// Dashboard dynamic logic: fetch data, compute aggregates, render charts/tables.
// Requires Chart.js to be loaded before this file (template includes CDN).

document.addEventListener('DOMContentLoaded', () => {
  // Entry
  initDashboard().catch(err => {
    console.error('Dashboard initialization failed:', err);
  });
});

async function initDashboard() {
  // Start parallel fetches (be tolerant of partial failures)
  const fetches = {
    devices: fetchJson('/api/devices'),
    actions: fetchJson('/api/actions'),
    history: fetchJson('/api/'), // route logged as GET /api/
    audit: fetchJson('/api/audit'),
  };

  const results = await promiseAllSettledToValues(fetches);

  const devices = Array.isArray(results.devices) ? results.devices : [];
  const actions = Array.isArray(results.actions) ? results.actions : [];
  const history = Array.isArray(results.history) ? results.history : [];
  const audit = Array.isArray(results.audit) ? results.audit : [];

  // Aggregations
  const totalDevices = devices.length;
  const approvedDevices = safeCount(devices, d => truthy(d.approved));
  const pendingDevices = totalDevices - approvedDevices;
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

/* ---------- utilities ---------- */

async function fetchJson(url) {
  try {
    const res = await fetch(url, { credentials: 'same-origin' });
    if (!res.ok) {
      console.warn('fetch failed', url, res.status);
      return null;
    }
    return await res.json();
  } catch (e) {
    console.warn('fetch error', url, e);
    return null;
  }
}

async function promiseAllSettledToValues(obj) {
  const keys = Object.keys(obj);
  const promises = keys.map(k => obj[k].catch ? obj[k] : Promise.resolve(obj[k]));
  const settled = await Promise.allSettled(promises);
  const out = {};
  for (let i = 0; i < keys.length; i++) {
    out[keys[i]] = (settled[i].status === 'fulfilled') ? settled[i].value : null;
  }
  return out;
}

function setText(selector, value) {
  const el = document.querySelector(selector);
  if (!el) return;
  el.textContent = (value === undefined || value === null || value === '') ? '-' : String(value);
}

function truthy(v) {
  return v === true || v === 1 || v === '1' || v === 'true' || v === 'True';
}

function safeCount(arr, predicate) {
  if (!Array.isArray(arr)) return 0;
  let c = 0;
  for (const item of arr) {
    try {
      if (predicate(item)) c++;
    } catch (e) {}
  }
  return c;
}

function getField(item, possibleNames) {
  for (const name of possibleNames) {
    if (item && Object.prototype.hasOwnProperty.call(item, name)) {
      return item[name];
    }
  }
  return undefined;
}

/* ---------- rendering helpers ---------- */

function renderRecentDevices(devices) {
  const tbody = document.getElementById('recent-devices-body');
  if (!tbody) return;
  tbody.innerHTML = '';
  for (const d of devices) {
    const name = getField(d, ['device_name', 'name']) || getField(d, ['hostname']) || '-';
    const hostname = getField(d, ['hostname']) || '-';
    const ip = getField(d, ['ip_address', 'ip']) || '-';
    const lastSeen = formatTimestamp(getField(d, ['last_seen', 'last_seen_at', 'updated_at', 'last_seen_ts'])) || '-';
    const approved = truthy(getField(d, ['approved', 'is_approved'])) ? 'Yes' : 'No';

    const tr = document.createElement('tr');
    tr.innerHTML = `<td>${escapeHtml(name)}</td>
                    <td>${escapeHtml(hostname)}</td>
                    <td>${escapeHtml(ip)}</td>
                    <td>${escapeHtml(lastSeen)}</td>
                    <td>${escapeHtml(approved)}</td>`;
    tbody.appendChild(tr);
  }
}

function renderRecentActions(actions) {
  const tbody = document.getElementById('recent-actions-body');
  if (!tbody) return;
  tbody.innerHTML = '';
  for (const a of actions) {
    const actionName = getField(a, ['action_name', 'name', 'title']) || getField(a, ['description']) || '-';
    const device = getField(a, ['device_name', 'target_device', 'device']) || '-';
    const created = formatTimestamp(getField(a, ['created_at', 'submitted_at', 'timestamp'])) || '-';
    const status = getField(a, ['status', 'state']) || (getField(a, ['completed']) ? 'completed' : 'pending') || '-';

    const tr = document.createElement('tr');
    tr.innerHTML = `<td>${escapeHtml(actionName)}</td>
                    <td>${escapeHtml(device)}</td>
                    <td>${escapeHtml(created)}</td>
                    <td>${escapeHtml(status)}</td>`;
    tbody.appendChild(tr);
  }
}

function renderTtlExpiring(devices) {
  const ul = document.getElementById('ttl-expiring-list');
  if (!ul) return;
  ul.innerHTML = '';
  if (!Array.isArray(devices)) return;

  const now = Date.now();
  const soon = devices.filter(d => {
    const ttlExpires = getField(d, ['ttl_expires_at', 'ttl_expires', 'ttl_expire_ts']);
    if (!ttlExpires) return false;
    const t = parseTimestamp(ttlExpires);
    if (!t) return false;
    return (t - now) < (24 * 3600 * 1000) && (t - now) > 0;
  }).slice(0, 8);

  if (soon.length === 0) {
    ul.innerHTML = '<li class="muted">No devices expiring within 24 hours</li>';
    return;
  }

  for (const d of soon) {
    const name = getField(d, ['device_name', 'name']) || '-';
    const ttlExpires = formatTimestamp(getField(d, ['ttl_expires_at', 'ttl_expires', 'ttl_expire_ts'])) || '-';
    const li = document.createElement('li');
    li.innerHTML = `<strong>${escapeHtml(name)}</strong> — ${escapeHtml(ttlExpires)}`;
    ul.appendChild(li);
  }
}

/* ---------- charts ---------- */

function renderOsDistributionChart(devices) {
  const ctx = document.getElementById('os-distribution-chart');
  if (!ctx || !Array.isArray(devices)) return;

  const map = {};
  for (const d of devices) {
    const os = getField(d, ['os_name', 'os', 'platform']) || 'Unknown';
    map[os] = (map[os] || 0) + 1;
  }

  const labels = Object.keys(map);
  const values = labels.map(l => map[l]);

  // Destroy previous chart instance if any (Chart.js stores on element)
  if (ctx._chart) ctx._chart.destroy();

  ctx._chart = new Chart(ctx.getContext('2d'), {
    type: 'pie',
    data: {
      labels,
      datasets: [{
        data: values,
        // Do not set colors explicitly; Chart.js will pick defaults.
      }]
    },
    options: {
      responsive: true,
      plugins: {
        legend: { position: 'bottom' },
        tooltip: { mode: 'index' }
      }
    }
  });
}

function renderActionsOverTimeChart(actions) {
  const ctx = document.getElementById('actions-over-time-chart');
  if (!ctx || !Array.isArray(actions)) return;

  // Aggregate by day for past 30 days
  const now = new Date();
  const days = 30;
  const counts = {};
  for (let i = 0; i < days; i++) {
    const d = new Date(now);
    d.setDate(now.getDate() - (days - 1 - i));
    const key = d.toISOString().slice(0, 10);
    counts[key] = 0;
  }

  for (const a of actions) {
    const ts = getField(a, ['created_at', 'submitted_at', 'timestamp']);
    const d = parseTimestamp(ts);
    if (!d) continue;
    const key = (new Date(d)).toISOString().slice(0, 10);
    if (counts.hasOwnProperty(key)) counts[key] += 1;
  }

  const labels = Object.keys(counts);
  const data = labels.map(l => counts[l]);

  if (ctx._chart) ctx._chart.destroy();
  ctx._chart = new Chart(ctx.getContext('2d'), {
    type: 'bar',
    data: {
      labels,
      datasets: [{
        label: 'Actions',
        data,
        // Chart.js will pick defaults for colors
      }]
    },
    options: {
      responsive: true,
      scales: {
        x: { ticks: { maxRotation: 45, minRotation: 0 } },
        y: { beginAtZero: true, precision: 0 }
      },
      plugins: {
        legend: { display: false },
        tooltip: { mode: 'index' }
      }
    }
  });
}

/* ---------- small helpers ---------- */

function sortByLastSeen(a, b) {
  const ta = parseTimestamp( getField(a, ['last_seen', 'last_seen_at', 'updated_at']) ) || 0;
  const tb = parseTimestamp( getField(b, ['last_seen', 'last_seen_at', 'updated_at']) ) || 0;
  return tb - ta;
}

function sortByCreatedAt(a, b) {
  const ta = parseTimestamp( getField(a, ['created_at', 'submitted_at', 'timestamp']) ) || 0;
  const tb = parseTimestamp( getField(b, ['created_at', 'submitted_at', 'timestamp']) ) || 0;
  return tb - ta;
}

function parseTimestamp(v) {
  if (!v) return null;
  if (typeof v === 'number') return v;
  const n = Number(v);
  if (!Number.isNaN(n)) return n;
  const d = Date.parse(String(v));
  return Number.isNaN(d) ? null : d;
}

function formatTimestamp(v) {
  const t = parseTimestamp(v);
  if (!t) return null;
  const d = new Date(t);
  return d.toLocaleString();
}

function escapeHtml(s) {
  if (s === null || s === undefined) return '';
  return String(s)
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&#39;');
}
