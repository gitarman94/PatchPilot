async function fetchJSON(url) {
    const resp = await fetch(url);
    if (!resp.ok) throw new Error(`Failed to fetch ${url}: ${resp.status}`);
    return resp.json();
}

async function loadDevices() {
    try {
        const devices = await fetchJSON('/api/devices');
        const tbody = document.querySelector('#devices-table tbody') || document.querySelector('#dashboard-table tbody');
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
    } catch (err) {
        console.error(err);
        alert("Failed to load devices");
    }
}

document.addEventListener('DOMContentLoaded', () => {
    if (document.querySelector('#devices-table') || document.querySelector('#dashboard-table')) {
        loadDevices();
    }
});
