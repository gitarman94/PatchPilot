// ---------- Helper Functions ----------

// Fetch JSON from API with error handling
async function fetchJSON(url) {
    const resp = await fetch(url);
    if (!resp.ok) throw new Error(`Failed to fetch ${url}: ${resp.status}`);
    return resp.json();
}

// Show alert messages (requires your existing showAlert function)
function showAlert(message, type = 'info', duration = 2000) {
    // Simple alert using native alert for fallback
    alert(`${type.toUpperCase()}: ${message}`);
}

// Apply dark mode
function applyDarkMode(enabled) {
    if (enabled) {
        document.body.classList.add('dark');
        localStorage.setItem('darkMode', 'true');
    } else {
        document.body.classList.remove('dark');
        localStorage.setItem('darkMode', 'false');
    }
}

// Initialize dark mode on page load
document.addEventListener('DOMContentLoaded', () => {
    const saved = localStorage.getItem('darkMode') === 'true';
    applyDarkMode(saved);
});

// ---------- Table Population Functions ----------

async function loadDevices() {
    try {
        const devices = await fetchJSON('/api/devices');
        const tbodyList = [document.querySelector('#devices-table tbody'), document.querySelector('#dashboard-table tbody')];
        tbodyList.forEach(tbody => {
            if (!tbody) return;
            tbody.innerHTML = '';
            devices.forEach(d => {
                tbody.innerHTML += `
                    <tr>
                        <td>${d.hostname}</td>
                        <td>${d.ip}</td>
                        <td>${d.os}</td>
                        <td>${d.status}</td>
                    </tr>`;
            });
        });
    } catch (err) {
        console.error(err);
        showAlert('Failed to load devices', 'danger');
    }
}

async function loadActions() {
    try {
        const actions = await fetchJSON('/api/actions');
        const tbody = document.querySelector('#actions-table tbody');
        if (!tbody) return;
        tbody.innerHTML = '';
        actions.forEach(a => {
            tbody.innerHTML += `
                <tr>
                    <td>${a.id}</td>
                    <td>${a.device_hostname}</td>
                    <td>${a.name}</td>
                    <td>${a.status}</td>
                    <td>${a.created_at}</td>
                </tr>`;
        });
    } catch (err) {
        console.error(err);
        showAlert('Failed to load actions', 'danger');
    }
}

async function loadHistory() {
    try {
        const history = await fetchJSON('/history');
        const tbody = document.querySelector('#history-table tbody');
        if (!tbody) return;
        tbody.innerHTML = '';
        history.forEach(h => {
            tbody.innerHTML += `
                <tr>
                    <td>${h.id}</td>
                    <td>${h.device_hostname}</td>
                    <td>${h.action_name}</td>
                    <td>${h.user}</td>
                    <td>${h.timestamp}</td>
                </tr>`;
        });
    } catch (err) {
        console.error(err);
        showAlert('Failed to load history', 'danger');
    }
}

async function loadAudit() {
    try {
        const audit = await fetchJSON('/audit');
        const tbody = document.querySelector('#audit-table tbody');
        if (!tbody) return;
        tbody.innerHTML = '';
        audit.forEach(a => {
            tbody.innerHTML += `
                <tr>
                    <td>${a.id}</td>
                    <td>${a.user}</td>
                    <td>${a.action}</td>
                    <td>${a.target}</td>
                    <td>${a.details}</td>
                    <td>${a.timestamp}</td>
                </tr>`;
        });
    } catch (err) {
        console.error(err);
        showAlert('Failed to load audit logs', 'danger');
    }
}

async function loadUsers() {
    try {
        const users = await fetchJSON('/users-groups');
        const tbody = document.querySelector('#users-table tbody');
        if (!tbody) return;
        tbody.innerHTML = '';
        users.forEach(u => {
            tbody.innerHTML += `
                <tr>
                    <td>${u.id}</td>
                    <td>${u.username}</td>
                    <td>${u.role}</td>
                </tr>`;
        });
    } catch (err) {
        console.error(err);
        showAlert('Failed to load users', 'danger');
    }
}

// ---------- Page Initialization ----------

document.addEventListener('DOMContentLoaded', () => {
    // Dark mode toggle button
    const darkBtn = document.getElementById('toggleDarkModeBtn');
    if (darkBtn) {
        darkBtn.addEventListener('click', () => {
            const enabled = !document.body.classList.contains('dark');
            applyDarkMode(enabled);
            showAlert(`Dark mode ${enabled ? 'enabled' : 'disabled'}`, 'info', 1400);
        });
    }

    // Load tables dynamically based on page
    if (document.querySelector('#devices-table') || document.querySelector('#dashboard-table')) loadDevices();
    if (document.querySelector('#actions-table')) loadActions();
    if (document.querySelector('#history-table')) loadHistory();
    if (document.querySelector('#audit-table')) loadAudit();
    if (document.querySelector('#users-table')) loadUsers();
});
