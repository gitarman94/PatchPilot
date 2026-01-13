// DARK MODE 
function applyDarkMode(enabled){
  document.body.classList.toggle('dark', enabled);
  localStorage.setItem('darkMode', enabled?'enabled':'disabled');
}
document.body.classList.toggle('dark', localStorage.getItem('darkMode')==='enabled');

// ALERTS 
function showAlert(msg, type='success', timeout=3500){
  const id = 'a'+Date.now();
  const div = document.createElement('div');
  div.id = id;
  div.className = 'alert alert-'+type;
  div.textContent = msg;
  document.body.appendChild(div);
  setTimeout(()=>{ div.remove(); }, timeout);
}

// MODAL HELPERS 
function showModal(id){ document.getElementById(id).classList.add('show'); }
function hideModal(id){ document.getElementById(id).classList.remove('show'); }

// TABLE HELPERS 
function selectAllCheckboxes(name='selected'){ 
  document.querySelectorAll(`input[name="${name}"]`).forEach(c=>c.checked=true); 
}
function deselectAllCheckboxes(name='selected'){ 
  document.querySelectorAll(`input[name="${name}"]`).forEach(c=>c.checked=false); 
}

// DEVICE REFRESH 
async function fetchJSON(url){ const res=await fetch(url); return await res.json(); }
async function loadDevices(){ 
  try{
    const devices = await fetchJSON('/api/devices');
    console.log('Devices:', devices);
  }catch(e){ showAlert('Failed to load devices','danger'); console.error(e); }
}

// SETTINGS 
function saveSettings(){ 
  const dark = document.getElementById('settingsDarkMode').checked;
  const auto = document.getElementById('settingsAutoRefresh').checked;
  applyDarkMode(dark);
  localStorage.setItem('autoRefresh', auto?'enabled':'disabled');
}
