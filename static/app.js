document.addEventListener('DOMContentLoaded', () => {
  initDashboard().catch(err => console.error('Dashboard init error:', err));
});

async function initDashboard() {
  const [devicesRes, actionsRes] = await Promise.all([
    fetchJson('/api/devices'),
    fetchJson('/api/actions')
  ]);

  const devices = Array.isArray(devicesRes) ? devicesRes : [];
  const actions = Array.isArray(actionsRes) ? actionsRes : [];

  const totalDevices = devices.length;
  const approvedDevices = safeCount(devices, d => truthy(getField(d, ['approved', 'is_approved'])));
  const pendingDevices = Math.max(0, totalDevices - approvedDevices);
  const totalActions = actions.length;

  const onlineDevices = safeCount(devices, d => {
    const ts = parseTimestamp(getField(d, ['last_seen', 'last_seen_at', 'updated_at']));
    if (!ts) return false;
    return (Date.now() - ts) < 5 * 60 * 1000;
  });

  const highCpuDevices = safeCount(devices, d => Number(getField(d, ['cpu_usage'])) >= 85);

  const lowDiskDevices = safeCount(devices, d => {
    const total = Number(getField(d, ['disk_total'])) || 0;
    const free = Number(getField(d, ['disk_free'])) || 0;
    if (!total || !free) return false;
    return ((free / total) * 100) <= 10;
  });

  setText('#total-devices', totalDevices);
  setText('#online-devices', onlineDevices);
  setText('#pending-devices', pendingDevices);
  setText('#total-actions', totalActions);
  setText('#high-cpu-devices', highCpuDevices);
  setText('#low-disk-devices', lowDiskDevices);

  renderRecentDevices(devices.slice().sort(sortByLastSeen).slice(0, 10));
  renderRecentActions(actions.slice().sort(sortByCreatedAt).slice(0, 10));
  renderFleetHealth(devices);
  renderOsDistributionChart(devices);
  renderActionsOverTimeChart(actions);
}

async function fetchJson(url) {
  try {
    const r = await fetch(url, { credentials: 'same-origin' });
    if (!r.ok) return null;
    return await r.json();
  } catch {
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

  for (const x of arr) {
    try {
      if (pred(x)) c++;
    } catch {}
  }

  return c;
}

function getField(item, names) {
  if (!item) return undefined;

  for (const n of names) {
    if (Object.prototype.hasOwnProperty.call(item, n)) {
      return item[n];
    }
  }

  return undefined;
}

function renderRecentDevices(devices) {
  const tbody = document.getElementById('recent-devices-body');
  if (!tbody) return;

  tbody.innerHTML = '';

  if (!devices.length) {
    tbody.innerHTML = `
      <tr>
        <td colspan="8" class="empty-state">
          No managed endpoints have checked in yet.
        </td>
      </tr>
    `;
    return;
  }

  for (const d of devices) {
    const hostname = getField(d, ['hostname']) || '-';
    const ip = getField(d, ['ip_address', 'ip']) || '-';
    const os = getField(d, ['os_name', 'os']) || '-';
    const cpu = Number(getField(d, ['cpu_usage'])) || 0;
    const ramUsed = Number(getField(d, ['ram_used'])) || 0;
    const ramTotal = Number(getField(d, ['ram_total'])) || 0;
    const diskFree = Number(getField(d, ['disk_free'])) || 0;
    const lastSeen = formatTimestamp(getField(d, ['last_seen', 'last_seen_at', 'updated_at'])) || '-';

    const memoryPercent = ramTotal ? ((ramUsed / ramTotal) * 100).toFixed(0) : '-';

    const online = isOnline(d);

    const tr = document.createElement('tr');

    tr.innerHTML = `
      <td>${escapeHtml(hostname)}</td>
      <td>${escapeHtml(ip)}</td>
      <td>${escapeHtml(os)}</td>
      <td>${cpu.toFixed(1)}%</td>
      <td>${memoryPercent}%</td>
      <td>${formatBytes(diskFree)}</td>
      <td>
        <span class="${online ? 'status-online' : 'status-offline'}">
          ${online ? 'Online' : 'Offline'}
        </span>
      </td>
      <td>${escapeHtml(lastSeen)}</td>
    `;

    tbody.appendChild(tr);
  }
}

function renderRecentActions(actions) {
  const tbody = document.getElementById('recent-actions-body');
  if (!tbody) return;

  tbody.innerHTML = '';

  if (!actions.length) {
    tbody.innerHTML = `
      <tr>
        <td colspan="4" class="empty-state">
          No remote actions have been executed recently.
        </td>
      </tr>
    `;
    return;
  }

  for (const a of actions) {
    const actionName = getField(a, ['action_name', 'name', 'title']) || '-';
    const device = getField(a, ['device_name', 'target_device', 'device']) || '-';
    const created = formatTimestamp(getField(a, ['created_at', 'submitted_at', 'timestamp'])) || '-';
    const status = getField(a, ['status', 'state']) || 'pending';

    const tr = document.createElement('tr');

    tr.innerHTML = `
      <td>${escapeHtml(actionName)}</td>
      <td>${escapeHtml(device)}</td>
      <td>
        <span class="${status === 'completed' ? 'status-online' : 'status-pending'}">
          ${escapeHtml(status)}
        </span>
      </td>
      <td>${escapeHtml(created)}</td>
    `;

    tbody.appendChild(tr);
  }
}

function renderFleetHealth(devices) {
  const tbody = document.getElementById('fleet-health-body');
  if (!tbody) return;

  tbody.innerHTML = '';

  const flagged = devices.filter(d => {
    const cpu = Number(getField(d, ['cpu_usage'])) || 0;
    const diskTotal = Number(getField(d, ['disk_total'])) || 0;
    const diskFree = Number(getField(d, ['disk_free'])) || 0;

    const diskPercent = diskTotal ? ((diskFree / diskTotal) * 100) : 100;

    return cpu >= 85 || diskPercent <= 10;
  });

  if (!flagged.length) {
    tbody.innerHTML = `
      <tr>
        <td colspan="5" class="empty-state">
          No infrastructure alerts detected.
        </td>
      </tr>
    `;
    return;
  }

  for (const d of flagged.slice(0, 10)) {
    const hostname = getField(d, ['hostname']) || '-';
    const cpu = Number(getField(d, ['cpu_usage'])) || 0;

    const ramUsed = Number(getField(d, ['ram_used'])) || 0;
    const ramTotal = Number(getField(d, ['ram_total'])) || 0;
    const ramPercent = ramTotal ? ((ramUsed / ramTotal) * 100).toFixed(0) : '-';

    const diskFree = Number(getField(d, ['disk_free'])) || 0;

    let status = 'Warning';

    if (cpu >= 95) status = 'Critical';

    const tr = document.createElement('tr');

    tr.innerHTML = `
      <td>${escapeHtml(hostname)}</td>
      <td>${cpu.toFixed(1)}%</td>
      <td>${ramPercent}%</td>
      <td>${formatBytes(diskFree)}</td>
      <td>
        <span class="${status === 'Critical' ? 'status-offline' : 'status-warning'}">
          ${status}
        </span>
      </td>
    `;

    tbody.appendChild(tr);
  }
}

function renderOsDistributionChart(devices) {
  const el = document.getElementById('os-distribution-chart');
  if (!el || !Array.isArray(devices)) return;

  const map = {};

  for (const d of devices) {
    const os = getField(d, ['os_name', 'os', 'platform']) || 'Unknown';
    map[os] = (map[os] || 0) + 1;
  }

  const labels = Object.keys(map);
  const values = labels.map(l => map[l]);

  if (el._chart) el._chart.destroy();

  el._chart = new Chart(el.getContext('2d'), {
    type: 'doughnut',
    data: {
      labels,
      datasets: [{
        data: values
      }]
    },
    options: {
      responsive: true,
      plugins: {
        legend: {
          position: 'bottom'
        }
      }
    }
  });
}

function renderActionsOverTimeChart(actions) {
  const el = document.getElementById('actions-over-time-chart');
  if (!el || !Array.isArray(actions)) return;

  const now = new Date();
  const days = 30;
  const counts = {};

  for (let i = 0; i < days; i++) {
    const d = new Date(now);
    d.setDate(now.getDate() - (days - 1 - i));
    counts[d.toISOString().slice(0, 10)] = 0;
  }

  for (const a of actions) {
    const t = parseTimestamp(getField(a, ['created_at', 'submitted_at', 'timestamp']));
    if (!t) continue;

    const k = new Date(t).toISOString().slice(0, 10);

    if (counts.hasOwnProperty(k)) {
      counts[k] += 1;
    }
  }

  const labels = Object.keys(counts);
  const data = labels.map(l => counts[l]);

  if (el._chart) el._chart.destroy();

  el._chart = new Chart(el.getContext('2d'), {
    type: 'bar',
    data: {
      labels,
      datasets: [{
        label: 'Actions',
        data
      }]
    },
    options: {
      responsive: true,
      scales: {
        y: {
          beginAtZero: true
        }
      },
      plugins: {
        legend: {
          display: false
        }
      }
    }
  });
}

function isOnline(device) {
  const ts = parseTimestamp(getField(device, ['last_seen', 'last_seen_at', 'updated_at']));
  if (!ts) return false;
  return (Date.now() - ts) < 5 * 60 * 1000;
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
  return new Date(t).toLocaleString();
}

function formatBytes(bytes) {
  if (!bytes || bytes <= 0) return '-';

  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  let i = 0;
  let n = bytes;

  while (n >= 1024 && i < units.length - 1) {
    n /= 1024;
    i++;
  }

  return `${n.toFixed(1)} ${units[i]}`;
}

function sortByLastSeen(a, b) {
  return (parseTimestamp(getField(b, ['last_seen', 'last_seen_at', 'updated_at'])) || 0)
    - (parseTimestamp(getField(a, ['last_seen', 'last_seen_at', 'updated_at'])) || 0);
}

function sortByCreatedAt(a, b) {
  return (parseTimestamp(getField(b, ['created_at', 'submitted_at', 'timestamp'])) || 0)
    - (parseTimestamp(getField(a, ['created_at', 'submitted_at', 'timestamp'])) || 0);
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