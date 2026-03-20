const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

let trafficEntries = [];
let trafficPaused = false;
let captureEnabled = false;
let selectedTrafficId = null;
let editingRuleId = null;
let editingProfile = null;

let daemonRunning = false;
let proxyActive = false;
let activeProfile = null;

async function init() {
  setupTabs();
  setupEventListeners();
  startEventStream();
  await refreshStatus();
  await loadProfilesTab();
  await refreshCaptureStatus();
  setInterval(refreshStatus, 3000);

  // load app version into About section
  try {
    const ver = await window.__TAURI__.app.getVersion();
    document.getElementById("about-version").textContent = "v" + ver;
  } catch (_) {}

  // refresh when window becomes visible again (reopened from tray)
  document.addEventListener("visibilitychange", async () => {
    if (!document.hidden) {
      await refreshStatus();
      await loadProfilesTab();
    }
  });
}

// -- status bar --

async function refreshStatus() {
  const dot = document.getElementById("status-dot");
  const text = document.getElementById("status-text");
  const profileEl = document.getElementById("status-profile");
  const btn = document.getElementById("btn-start-stop");

  try {
    const status = await invoke("get_status");
    daemonRunning = true;
    proxyActive = !!status.running;
    activeProfile = status.profile || null;

    if (proxyActive) {
      dot.className = "dot active";
      text.textContent = "Giant Proxy running";
      profileEl.textContent = activeProfile || "";
      btn.textContent = "Stop";
      btn.style.display = "";
    } else {
      dot.className = "dot idle";
      text.textContent = "Giant Proxy running";
      profileEl.textContent = "No profile loaded";
      btn.textContent = "Stop";
      btn.style.display = "";
    }
  } catch (_) {
    daemonRunning = false;
    proxyActive = false;
    activeProfile = null;
    dot.className = "dot offline";
    text.textContent = "Giant Proxy not running";
    profileEl.textContent = "";
    btn.style.display = "none";
  }
}

function setupTabs() {
  document.querySelectorAll("#tab-bar .tab").forEach(btn => {
    btn.addEventListener("click", () => {
      document.querySelectorAll("#tab-bar .tab").forEach(b => b.classList.remove("active"));
      document.querySelectorAll(".tab-panel").forEach(p => p.classList.remove("active"));
      btn.classList.add("active");
      const panel = document.getElementById("tab-" + btn.dataset.tab);
      if (panel) panel.classList.add("active");

      switch (btn.dataset.tab) {
        case "profiles": loadProfilesTab(); break;
        case "traffic": refreshCaptureStatus(); break;
        case "settings": loadSettingsTab(); break;
      }
    });
  });
}

// -- profiles + rules --

async function loadProfilesTab() {
  let profiles = [];
  try {
    const resp = await invoke("list_profiles_local");
    profiles = resp.profiles || [];
  } catch (e) {
    console.error("list profiles failed:", e);
  }

  const container = document.getElementById("profiles-list");

  if (!profiles.length) {
    container.innerHTML = '<div class="empty-state">No profiles found -- import from Proxyman or create one</div>';
    return;
  }

  container.innerHTML = profiles.map(p => {
    const isActive = p.name === activeProfile;
    const rules = p.rules || [];

    let actionBtn = "";
    if (isActive && proxyActive) {
      actionBtn = `<button class="action-btn small danger" data-action="stop-profile" data-profile="${p.name}">Stop</button>`;
    } else if (daemonRunning) {
      actionBtn = `<button class="action-btn small" data-action="activate-profile" data-profile="${p.name}">Activate</button>`;
    } else {
      actionBtn = `<button class="action-btn small primary" data-action="start-profile" data-profile="${p.name}">Start</button>`;
    }

    let badge = "";
    if (isActive && proxyActive) {
      badge = '<span class="profile-badge active">Running</span>';
    }

    const dragHandle = `<span class="drag-handle" data-action="move-profile-up" data-profile="${p.name}" title="Drag to reorder">&#9776;</span>`;

    const rulesHtml = rules.length
      ? `<div class="rule-list">${rules.map(r => `
          <div class="rule-row ${r.enabled ? '' : 'rule-disabled'}">
            <input type="checkbox" class="rule-toggle" ${r.enabled ? "checked" : ""} data-action="toggle-rule" data-profile="${p.name}" data-rule="${r.id}">
            <div class="rule-info" data-action="edit-rule" data-profile="${p.name}" data-rule="${r.id}">
              <span class="rule-id">${r.id}</span>
              <span class="rule-match" title="${escapeHtml(r.match_display)}">${truncate(r.match_display, 50)}</span>
              <span class="rule-target">${r.target}</span>
            </div>
            <div class="rule-actions">
              <button class="icon-btn" title="Edit" data-action="edit-rule" data-profile="${p.name}" data-rule="${r.id}">&#9998;</button>
              <button class="icon-btn danger" title="Delete" data-action="delete-rule" data-profile="${p.name}" data-rule="${r.id}">&#128465;</button>
            </div>
          </div>`).join("")}
        </div>`
      : '<div class="profile-no-rules">No rules</div>';

    return `
      <div class="profile-card ${isActive ? 'active' : ''}">
        <div class="profile-header">
          ${dragHandle}
          <div class="profile-name" data-action="rename-profile" data-profile="${p.name}" title="Click to rename">${p.name}</div>
          ${badge}
          <span class="profile-rule-count">${rules.length} rule${rules.length !== 1 ? 's' : ''}</span>
          ${actionBtn}
          <button class="icon-btn add-rule-btn" data-action="add-rule" data-profile="${p.name}" title="Add rule">+</button>
        </div>
        ${rulesHtml}
      </div>
    `;
  }).join("");
}

// -- event delegation for all profile/rule actions --

function setupProfileActions() {
  document.getElementById("profiles-list").addEventListener("click", async (e) => {
    const btn = e.target.closest("[data-action]");
    if (!btn) return;

    const action = btn.dataset.action;
    const profile = btn.dataset.profile;
    const rule = btn.dataset.rule;

    switch (action) {
      case "start-profile":
        await startAndActivate(profile);
        break;
      case "activate-profile":
        await activateProfile(profile);
        break;
      case "stop-profile":
        await stopDaemon();
        break;
      case "add-rule":
        openNewRuleModal(profile);
        break;
      case "edit-rule":
        await openEditRuleModal(profile, rule);
        break;
      case "delete-rule":
        await deleteRule(profile, rule);
        break;
      case "toggle-rule":
        await toggleRule(profile, rule);
        break;
      case "rename-profile":
        await renameProfilePrompt(profile);
        break;
      case "move-profile-up":
        await moveProfile(profile, -1);
        break;
      case "move-profile-down":
        await moveProfile(profile, 1);
        break;
    }
  });
}

async function startAndActivate(profileName) {
  try {
    await invoke("start_daemon");
    await invoke("switch_profile", { profile: profileName });
    await refreshStatus();
    await loadProfilesTab();
  } catch (e) {
    showConfirm("Error", "Failed to start: " + e);
  }
}

async function activateProfile(name) {
  try {
    await invoke("switch_profile", { profile: name });
    await refreshStatus();
    await loadProfilesTab();
  } catch (e) {
    showConfirm("Error", "Failed to activate: " + e);
  }
}

async function stopDaemon() {
  try {
    await invoke("stop_daemon");
    await refreshStatus();
    await loadProfilesTab();
  } catch (e) {
    showConfirm("Error", "Failed to stop: " + e);
  }
}

async function toggleRule(profileName, ruleId) {
  try {
    await invoke("toggle_profile_rule", { profileName, ruleId });
    await loadProfilesTab();
  } catch (e) {
    showConfirm("Error", "Toggle failed: " + e);
  }
}

async function deleteRule(profileName, ruleId) {
  const ok = await showConfirm("Delete Rule", "Delete rule '" + ruleId + "' from " + profileName + "?");
  if (!ok) return;
  try {
    await invoke("delete_profile_rule", { profileName, ruleId });
    await loadProfilesTab();
  } catch (e) {
    await showConfirm("Error", "Delete failed: " + e);
  }
}

// -- create / rename / reorder --

async function createNewProfile() {
  const raw = await showInput("New Profile", "Profile name (lowercase, hyphens/underscores ok):", "");
  if (!raw) return;
  const name = raw.toLowerCase().replace(/[^a-z0-9_-]/g, '_').replace(/_+/g, '_').replace(/^_|_$/g, '');
  if (!name) return;
  try {
    await invoke("create_profile", { name });
    await loadProfilesTab();
  } catch (e) {
    await showConfirm("Error", "Create profile failed: " + e);
  }
}

async function renameProfilePrompt(oldName) {
  const newName = await showInput("Rename Profile", "New name for '" + oldName + "':", oldName);
  if (!newName || newName === oldName) return;
  const slug = newName.toLowerCase().replace(/[^a-z0-9_-]/g, '_').replace(/_+/g, '_').replace(/^_|_$/g, '');
  if (!slug) return;
  try {
    await invoke("rename_profile", { oldName, newName: slug });
    await loadProfilesTab();
  } catch (e) {
    await showConfirm("Error", "Rename failed: " + e);
  }
}

async function moveProfile(name, direction) {
  // get current order from the rendered cards
  const cards = document.querySelectorAll(".profile-card .profile-name");
  const names = Array.from(cards).map(el => el.dataset.profile);
  const idx = names.indexOf(name);
  if (idx < 0) return;
  const newIdx = idx + direction;
  if (newIdx < 0 || newIdx >= names.length) return;
  // swap
  [names[idx], names[newIdx]] = [names[newIdx], names[idx]];
  try {
    await invoke("reorder_profiles", { names });
    await loadProfilesTab();
  } catch (e) {
    showConfirm("Error", "Reorder failed: " + e);
  }
}

// -- rule modal --

function openNewRuleModal(profileName) {
  editingRuleId = null;
  editingProfile = profileName;
  document.getElementById("modal-title").textContent = "New Rule in " + profileName;
  document.getElementById("rule-profile").value = profileName;
  document.getElementById("rule-id").value = "";
  document.getElementById("rule-host-pattern").value = "";
  document.getElementById("rule-path-pattern").value = "";
  document.getElementById("rule-exclude-path").value = "";
  document.getElementById("rule-method").value = "ANY";
  document.getElementById("rule-target-host").value = "localhost";
  document.getElementById("rule-target-port").value = "3000";
  document.getElementById("rule-target-scheme").value = "http";
  document.getElementById("rule-preserve-host").checked = true;
  document.getElementById("rule-regex").value = "";
  document.getElementById("rule-modal").style.display = "flex";
}

async function openEditRuleModal(profileName, ruleId) {
  editingRuleId = ruleId;
  editingProfile = profileName;
  document.getElementById("modal-title").textContent = "Edit Rule in " + profileName;
  document.getElementById("rule-profile").value = profileName;
  try {
    const rule = await invoke("get_profile_rule", { profileName, ruleId });
    document.getElementById("rule-id").value = rule.id || "";
    document.getElementById("rule-host-pattern").value = (rule.match_rule && rule.match_rule.host) || "";
    document.getElementById("rule-path-pattern").value = (rule.match_rule && rule.match_rule.path) || "";
    document.getElementById("rule-exclude-path").value = (rule.match_rule && rule.match_rule.not_path) || "";
    document.getElementById("rule-method").value = (rule.match_rule && rule.match_rule.method) || "ANY";
    document.getElementById("rule-target-host").value = (rule.target && rule.target.host) || "";
    document.getElementById("rule-target-port").value = (rule.target && rule.target.port) || "";
    document.getElementById("rule-target-scheme").value = (rule.target && rule.target.scheme) || "http";
    document.getElementById("rule-preserve-host").checked = rule.preserve_host !== false;
    document.getElementById("rule-regex").value = (rule.match_rule && rule.match_rule.regex) || "";
    document.getElementById("rule-modal").style.display = "flex";
  } catch (e) {
    showConfirm("Error", "Failed to load rule: " + e);
  }
}

function closeModal() {
  document.getElementById("rule-modal").style.display = "none";
  editingRuleId = null;
  editingProfile = null;
}

async function saveRule() {
  const profileName = document.getElementById("rule-profile").value;
  const method = document.getElementById("rule-method").value;

  const rulePayload = {
    id: document.getElementById("rule-id").value,
    enabled: true,
    match: {
      host: document.getElementById("rule-host-pattern").value || null,
      path: document.getElementById("rule-path-pattern").value || null,
      not_path: document.getElementById("rule-exclude-path").value || null,
      method: method === "ANY" ? null : method,
      regex: document.getElementById("rule-regex").value || null,
    },
    target: {
      host: document.getElementById("rule-target-host").value,
      port: parseInt(document.getElementById("rule-target-port").value) || 3000,
      scheme: document.getElementById("rule-target-scheme").value,
    },
    preserve_host: document.getElementById("rule-preserve-host").checked,
    priority: 0,
  };

  try {
    await invoke("save_profile_rule", {
      profileName,
      rule: rulePayload,
      oldRuleId: editingRuleId || null,
    });
    closeModal();
    await loadProfilesTab();
  } catch (e) {
    showConfirm("Error", "Failed to save rule: " + e);
  }
}

// -- traffic --

async function refreshCaptureStatus() {
  try {
    const resp = await invoke("get_traffic_status");
    captureEnabled = resp.enabled;
    updateCaptureButton();
  } catch (_) {}
}

function updateCaptureButton() {
  const btn = document.getElementById("btn-toggle-capture");
  if (captureEnabled) {
    btn.textContent = "Stop Capture";
    btn.classList.add("capture-active");
  } else {
    btn.textContent = "Start Capture";
    btn.classList.remove("capture-active");
  }
}

async function toggleCapture() {
  try {
    const resp = await invoke("toggle_traffic_capture", { enabled: !captureEnabled });
    captureEnabled = resp.enabled;
    updateCaptureButton();
    if (captureEnabled) {
      document.getElementById("traffic-empty").textContent = "Waiting for traffic...";
    }
  } catch (e) {
    showConfirm("Error", "Toggle capture failed: " + e);
  }
}

function renderTraffic() {
  const tbody = document.getElementById("traffic-tbody");
  const empty = document.getElementById("traffic-empty");
  const countEl = document.getElementById("traffic-count");
  const filter = (document.getElementById("traffic-filter").value || "").toLowerCase();

  const filtered = trafficEntries.filter(e =>
    !filter || e.url.toLowerCase().includes(filter) || (e.rule_id || "").toLowerCase().includes(filter)
  );

  countEl.textContent = filtered.length + " request" + (filtered.length !== 1 ? "s" : "");

  if (!filtered.length) {
    tbody.innerHTML = "";
    empty.style.display = "flex";
    return;
  }

  empty.style.display = "none";
  tbody.innerHTML = filtered.slice(0, 500).map(e => {
    const isRedirect = !!e.rule_id;
    const selected = e.id === selectedTrafficId ? "selected" : "";
    const statusClass = e.status >= 400 ? "status-error" : e.status >= 300 ? "status-redirect" : "status-ok";
    return `
    <tr class="${isRedirect ? 'log-redirect' : ''} ${selected}" data-traffic-id="${e.id}">
      <td class="col-time mono">${e.timestamp || ""}</td>
      <td class="col-method">${e.method || "GET"}</td>
      <td class="mono traffic-url" title="${escapeHtml(e.url)}">${truncate(e.url, 80)}</td>
      <td class="col-status ${statusClass}">${e.status || ""}</td>
      <td class="col-duration mono">${e.duration_ms != null ? e.duration_ms + "ms" : ""}</td>
      <td class="col-rule">${e.rule_id || "-"}</td>
    </tr>`;
  }).join("");
}

async function selectTrafficEntry(id) {
  selectedTrafficId = id;
  renderTraffic();

  const detail = document.getElementById("traffic-detail");
  detail.style.display = "flex";

  // find in local cache first
  let entry = trafficEntries.find(e => e.id === id);

  // if we only have summary data, fetch full entry from daemon
  if (entry && !entry.request_headers) {
    try {
      entry = await invoke("get_traffic_entry", { id });
    } catch (_) {}
  }

  if (!entry) {
    detail.style.display = "none";
    return;
  }

  document.getElementById("detail-method").textContent = entry.method;
  const statusEl = document.getElementById("detail-status");
  statusEl.textContent = entry.status;
  statusEl.className = "detail-status " + (entry.status >= 400 ? "status-error" : entry.status >= 300 ? "status-redirect" : "status-ok");
  document.getElementById("detail-url").textContent = entry.url;

  renderHeaders("detail-req-headers", entry.request_headers || []);
  renderHeaders("detail-res-headers", entry.response_headers || []);
}

function renderHeaders(tableId, headers) {
  const tbody = document.querySelector("#" + tableId + " tbody");
  if (!headers.length) {
    tbody.innerHTML = '<tr><td colspan="2" class="empty-headers">No headers</td></tr>';
    return;
  }
  tbody.innerHTML = headers.map(h => {
    const [key, value] = Array.isArray(h) ? h : [h.key || h[0], h.value || h[1]];
    return `<tr><td class="header-key">${escapeHtml(key)}</td><td class="header-value">${escapeHtml(value)}</td></tr>`;
  }).join("");
}

function closeDetail() {
  selectedTrafficId = null;
  document.getElementById("traffic-detail").style.display = "none";
  renderTraffic();
}

function toggleTrafficPause() {
  trafficPaused = !trafficPaused;
  document.getElementById("btn-pause-traffic").textContent = trafficPaused ? "Resume" : "Pause";
}

async function clearTraffic() {
  trafficEntries = [];
  selectedTrafficId = null;
  document.getElementById("traffic-detail").style.display = "none";
  try {
    await invoke("clear_traffic");
  } catch (_) {}
  renderTraffic();
}

// -- settings --

async function loadSettingsTab() {
  try {
    const cfg = await invoke("get_settings");
    document.getElementById("setting-listen-port").value = cfg.listen_port || 9456;
    document.getElementById("setting-pac-port").value = cfg.pac_port || 9876;
    document.getElementById("setting-log-level").value = cfg.log_level || "info";
    document.getElementById("setting-routing-mode").value = cfg.routing_mode || "manual";
    document.getElementById("setting-auto-start").checked = !!cfg.auto_start;
    document.getElementById("setting-default-profile").value = cfg.default_profile || "";
    document.getElementById("setting-bypass-hosts").value = (cfg.bypass_hosts || []).join("\n");
  } catch (e) {
    console.error("load settings failed:", e);
  }
  try {
    const enabled = await invoke("get_launch_at_login");
    document.getElementById("setting-launch-at-login").checked = enabled;
  } catch (e) {
    console.error("load launch-at-login failed:", e);
  }
}

async function saveSettings() {
  const settings = {
    listen_port: parseInt(document.getElementById("setting-listen-port").value) || 9456,
    pac_port: parseInt(document.getElementById("setting-pac-port").value) || 9876,
    log_level: document.getElementById("setting-log-level").value,
    routing_mode: document.getElementById("setting-routing-mode").value,
    auto_start: document.getElementById("setting-auto-start").checked,
    default_profile: document.getElementById("setting-default-profile").value,
    bypass_hosts: document.getElementById("setting-bypass-hosts").value,
  };
  try {
    await invoke("save_settings", { settings });
    const btn = document.getElementById("btn-save-settings");
    btn.textContent = "Saved";
    setTimeout(() => { btn.textContent = "Save Settings"; }, 1500);
  } catch (e) {
    showConfirm("Error", "Save failed: " + e);
  }
}

// -- events --

async function startEventStream() {
  await listen("proxy-event", (event) => {
    const data = event.payload;
    switch (data.type) {
      case "TrafficEntry":
        if (!trafficPaused && data.data) {
          trafficEntries.unshift(data.data);
          if (trafficEntries.length > 1000) trafficEntries.length = 1000;
          renderTraffic();
        }
        break;
      case "TrafficCaptureChanged":
        if (data.data) {
          captureEnabled = data.data.enabled;
          updateCaptureButton();
        }
        break;
      case "RuleToggled":
      case "ProfileSwitched":
      case "ProxyStarted":
      case "ProxyStopped":
        refreshStatus();
        loadProfilesTab();
        break;
    }
  });
}

// -- start/stop from status bar --

async function handleStartStop() {
  try {
    if (proxyActive) {
      await invoke("stop_daemon");
    } else if (!daemonRunning) {
      await invoke("start_daemon");
    } else {
      // daemon running but proxy not active -- start proxy
      await invoke("start_proxy");
    }
    await refreshStatus();
    await loadProfilesTab();
  } catch (e) {
    showConfirm("Error", "Failed: " + e);
  }
}

// -- import --

async function importProxyman() {
  try {
    const result = await window.__TAURI__.dialog.open({
      title: "Import Proxyman Map Remote Config",
      filters: [
        { name: "Proxyman Config", extensions: ["config", "json"] },
        { name: "All Files", extensions: ["*"] },
      ],
      multiple: false,
    });
    if (!result) return;

    const resp = await invoke("import_proxyman_file", { filePath: result });
    const profiles = resp.profiles || [];
    const names = profiles.map(p => `${p.name} (${p.rules} rules)`).join(", ");
    showConfirm("Imported", names);
    await loadProfilesTab();
  } catch (e) {
    showConfirm("Error", "Import failed: " + e);
  }
}

// -- auto-import from proxyman --

async function importProxymanAuto() {
  try {
    const resp = await invoke("import_proxyman_auto");
    const profiles = resp.profiles || [];
    const names = profiles.map(p => `${p.name} (${p.rules} rules)`).join(", ");
    showConfirm("Imported", names);
    // switch to profiles tab to show results
    document.querySelectorAll("#tab-bar .tab").forEach(b => b.classList.remove("active"));
    document.querySelectorAll(".tab-panel").forEach(p => p.classList.remove("active"));
    document.querySelector('[data-tab="profiles"]').classList.add("active");
    document.getElementById("tab-profiles").classList.add("active");
    await loadProfilesTab();
  } catch (e) {
    showConfirm("Error", "Import failed: " + e);
  }
}

// -- update check --

async function checkForUpdate() {
  const btn = document.getElementById("btn-check-update");
  const result = document.getElementById("update-result");
  btn.disabled = true;
  result.textContent = "checking...";
  result.className = "update-result";
  try {
    const resp = await invoke("check_for_update");
    if (resp.update_available) {
      result.className = "update-result update-available";
      result.innerHTML = `v${resp.latest} available -- <a href="#" id="update-link">view release</a><br><span style="font-size:11px;color:#6c7086">brew upgrade giant-proxy</span>`;
      document.getElementById("update-link").addEventListener("click", (e) => {
        e.preventDefault();
        window.__TAURI__.shell.open(resp.url);
      });
    } else {
      result.textContent = "up to date (v" + resp.current + ")";
    }
  } catch (e) {
    result.textContent = "check failed: " + e;
  }
  btn.disabled = false;
}

// -- helpers --

function escapeHtml(s) {
  const d = document.createElement("div");
  d.textContent = s;
  return d.innerHTML;
}

function truncate(s, n) {
  return s.length > n ? s.substring(0, n) + "..." : s;
}

// modal prompt/confirm replacements for WKWebView
function showPromptModal(title, message, defaultValue) {
  return new Promise((resolve) => {
    const overlay = document.getElementById("prompt-modal");
    const inputWrap = document.getElementById("prompt-input-wrap");
    const input = document.getElementById("prompt-input");
    document.getElementById("prompt-title").textContent = title;
    document.getElementById("prompt-message").textContent = message || "";
    document.getElementById("prompt-message").style.display = message ? "" : "none";
    if (defaultValue !== undefined) {
      inputWrap.style.display = "";
      input.value = defaultValue || "";
    } else {
      inputWrap.style.display = "none";
      input.value = "";
    }
    overlay.style.display = "flex";
    if (defaultValue !== undefined) {
      input.focus();
      input.select();
    }

    function cleanup(result) {
      overlay.style.display = "none";
      document.getElementById("btn-prompt-ok").removeEventListener("click", onOk);
      document.getElementById("btn-prompt-cancel").removeEventListener("click", onCancel);
      document.getElementById("btn-prompt-close").removeEventListener("click", onCancel);
      input.removeEventListener("keydown", onKey);
      resolve(result);
    }
    function onOk() { cleanup(defaultValue !== undefined ? input.value : true); }
    function onCancel() { cleanup(null); }
    function onKey(e) {
      if (e.key === "Enter") onOk();
      if (e.key === "Escape") onCancel();
    }
    document.getElementById("btn-prompt-ok").addEventListener("click", onOk);
    document.getElementById("btn-prompt-cancel").addEventListener("click", onCancel);
    document.getElementById("btn-prompt-close").addEventListener("click", onCancel);
    input.addEventListener("keydown", onKey);
  });
}

function showConfirm(title, message) {
  return showPromptModal(title, message, undefined).then(v => v === true);
}

function showInput(title, message, defaultValue) {
  return showPromptModal(title, message, defaultValue || "");
}

// -- event listeners --

function setupEventListeners() {
  setupProfileActions();
  document.getElementById("btn-modal-save").addEventListener("click", saveRule);
  document.getElementById("btn-modal-cancel").addEventListener("click", closeModal);
  document.getElementById("btn-modal-close").addEventListener("click", closeModal);
  document.getElementById("traffic-filter").addEventListener("input", renderTraffic);
  document.getElementById("btn-pause-traffic").addEventListener("click", toggleTrafficPause);
  document.getElementById("btn-clear-traffic").addEventListener("click", clearTraffic);
  document.getElementById("btn-toggle-capture").addEventListener("click", toggleCapture);
  document.getElementById("btn-close-detail").addEventListener("click", closeDetail);

  document.getElementById("traffic-tbody").addEventListener("click", (e) => {
    const row = e.target.closest("tr[data-traffic-id]");
    if (row) {
      selectTrafficEntry(parseInt(row.dataset.trafficId));
    }
  });
  document.getElementById("btn-check-update").addEventListener("click", checkForUpdate);
  document.getElementById("about-github-link").addEventListener("click", (e) => {
    e.preventDefault();
    window.__TAURI__.shell.open("https://github.com/bearded-giant/gproxy");
  });
  document.getElementById("btn-start-stop").addEventListener("click", handleStartStop);

  document.getElementById("btn-new-profile").addEventListener("click", createNewProfile);
  document.getElementById("btn-proxyman-auto").addEventListener("click", importProxymanAuto);
  document.getElementById("btn-save-settings").addEventListener("click", saveSettings);

  document.getElementById("setting-launch-at-login").addEventListener("change", async (e) => {
    try {
      await invoke("set_launch_at_login", { enabled: e.target.checked });
    } catch (err) {
      console.error("set launch-at-login failed:", err);
      e.target.checked = !e.target.checked;
    }
  });

  document.getElementById("rule-modal").addEventListener("click", (e) => {
    if (e.target.id === "rule-modal") closeModal();
  });
}

document.addEventListener("DOMContentLoaded", init);
