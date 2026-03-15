const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

let currentRules = [];
let logEntries = [];
let logsPaused = false;
let editingRuleId = null;

async function init() {
  setupTabs();
  setupEventListeners();
  startEventStream();
  await loadRulesTab();
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
        case "rules": loadRulesTab(); break;
        case "profiles": loadProfilesTab(); break;
        case "settings": loadSettingsTab(); break;
      }
    });
  });
}

async function loadRulesTab() {
  try {
    const status = await invoke("get_status");
    currentRules = status.rules || [];
    renderRulesTable();
  } catch (e) {
    console.error("load rules failed:", e);
  }
}

function renderRulesTable() {
  const tbody = document.getElementById("rules-tbody");
  const empty = document.getElementById("rules-empty");

  if (!currentRules.length) {
    tbody.innerHTML = "";
    empty.style.display = "flex";
    return;
  }

  empty.style.display = "none";
  const filter = (document.getElementById("rule-search").value || "").toLowerCase();

  tbody.innerHTML = currentRules
    .filter(r => !filter || r.id.toLowerCase().includes(filter))
    .map(rule => `
      <tr data-id="${rule.id}">
        <td class="col-check">
          <input type="checkbox" ${rule.enabled ? "checked" : ""} data-rule-id="${rule.id}">
        </td>
        <td>${rule.id}</td>
        <td class="mono">${rule.id}</td>
        <td class="mono">target</td>
        <td class="col-actions">
          <button class="action-btn small" onclick="editRule('${rule.id}')">Edit</button>
          <button class="action-btn small danger" onclick="deleteRule('${rule.id}')">Del</button>
        </td>
      </tr>
    `).join("");

  tbody.querySelectorAll('input[type="checkbox"]').forEach(cb => {
    cb.addEventListener("change", () => toggleRule(cb.dataset.ruleId));
  });
}

async function toggleRule(ruleId) {
  try {
    await invoke("toggle_rule", { ruleId });
    await loadRulesTab();
  } catch (e) {
    console.error("toggle failed:", e);
  }
}

async function deleteRule(ruleId) {
  if (!confirm("Delete rule " + ruleId + "?")) return;
  try {
    await invoke("delete_rule", { ruleId });
    await loadRulesTab();
  } catch (e) {
    console.error("delete failed:", e);
  }
}

async function editRule(ruleId) {
  editingRuleId = ruleId;
  document.getElementById("modal-title").textContent = "Edit Rule";
  try {
    const rule = await invoke("get_rule", { ruleId });
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
    console.error("load rule failed:", e);
  }
}

function openNewRuleModal() {
  editingRuleId = null;
  document.getElementById("modal-title").textContent = "New Rule";
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

function closeModal() {
  document.getElementById("rule-modal").style.display = "none";
  editingRuleId = null;
}

async function saveRule() {
  const rule = {
    id: document.getElementById("rule-id").value,
    enabled: true,
    match_rule: {
      host: document.getElementById("rule-host-pattern").value || null,
      path: document.getElementById("rule-path-pattern").value || null,
      not_path: document.getElementById("rule-exclude-path").value || null,
      method: document.getElementById("rule-method").value || null,
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

  const rulePayload = {
    id: rule.id,
    enabled: rule.enabled,
    match: rule.match_rule,
    target: rule.target,
    preserve_host: rule.preserve_host,
    priority: rule.priority,
  };

  try {
    if (editingRuleId) {
      await invoke("update_rule", { ruleId: editingRuleId, rule: rulePayload });
    } else {
      await invoke("add_rule", { rule: rulePayload });
    }
    closeModal();
    await loadRulesTab();
  } catch (e) {
    console.error("save rule failed:", e);
    alert("Failed to save rule: " + e);
  }
}

async function loadProfilesTab() {
  try {
    const resp = await invoke("get_profiles");
    const profiles = resp.profiles || [];
    const status = await invoke("get_status");
    const activeProfile = status.profile;
    const container = document.getElementById("profiles-list");

    if (!profiles.length) {
      container.innerHTML = '<div class="empty-state">No profiles found</div>';
      return;
    }

    container.innerHTML = profiles.map(p => `
      <div class="profile-card ${p === activeProfile ? 'active' : ''}" data-profile="${p}">
        <div class="profile-name">${p}</div>
        <div class="profile-meta">${p === activeProfile ? 'Active' : 'Click to activate'}</div>
      </div>
    `).join("");

    container.querySelectorAll(".profile-card").forEach(card => {
      card.addEventListener("click", () => switchProfile(card.dataset.profile));
    });
  } catch (e) {
    console.error("load profiles failed:", e);
  }
}

async function switchProfile(profile) {
  try {
    await invoke("switch_profile", { profile });
    await loadProfilesTab();
    await loadRulesTab();
  } catch (e) {
    console.error("switch profile failed:", e);
  }
}

function renderLogs() {
  const tbody = document.getElementById("logs-tbody");
  const empty = document.getElementById("logs-empty");
  const filter = (document.getElementById("log-filter").value || "").toLowerCase();

  const filtered = logEntries.filter(l =>
    !filter || l.url.toLowerCase().includes(filter) || l.rule_id.toLowerCase().includes(filter)
  );

  if (!filtered.length) {
    tbody.innerHTML = "";
    empty.style.display = "flex";
    return;
  }

  empty.style.display = "none";
  tbody.innerHTML = filtered.slice(0, 200).map(l => `
    <tr>
      <td class="col-time mono">${l.time}</td>
      <td class="col-method">${l.method || "GET"}</td>
      <td class="mono">${l.url}</td>
      <td class="col-action">redirect</td>
      <td class="col-rule">${l.rule_id}</td>
    </tr>
  `).join("");
}

function toggleLogsPause() {
  logsPaused = !logsPaused;
  document.getElementById("btn-pause-logs").textContent = logsPaused ? "Resume" : "Pause";
}

function clearLogs() {
  logEntries = [];
  renderLogs();
}

async function loadSettingsTab() {
  try {
    const status = await invoke("get_status");
    const addr = status.listen_addr || "127.0.0.1:8080";
    const port = addr.split(":").pop();
    document.getElementById("setting-listen-port").value = port;
    document.getElementById("setting-routing-mode").value = status.routing_mode || "manual";
  } catch (e) {
    console.error("load settings failed:", e);
  }
}

async function startEventStream() {
  await listen("proxy-event", (event) => {
    const data = event.payload;
    switch (data.type) {
      case "RequestMatched":
        if (!logsPaused && data.data) {
          const now = new Date();
          logEntries.unshift({
            time: now.toLocaleTimeString(),
            url: data.data.url || "",
            rule_id: data.data.rule_id || "",
            method: "",
          });
          if (logEntries.length > 1000) logEntries.length = 1000;
          renderLogs();
        }
        break;
      case "RuleToggled":
      case "ProfileSwitched":
      case "ProxyStarted":
      case "ProxyStopped":
        loadRulesTab();
        break;
    }
  });
}

function setupEventListeners() {
  document.getElementById("btn-new-rule").addEventListener("click", openNewRuleModal);
  document.getElementById("btn-modal-save").addEventListener("click", saveRule);
  document.getElementById("btn-modal-cancel").addEventListener("click", closeModal);
  document.getElementById("btn-modal-close").addEventListener("click", closeModal);
  document.getElementById("rule-search").addEventListener("input", renderRulesTable);
  document.getElementById("log-filter").addEventListener("input", renderLogs);
  document.getElementById("btn-pause-logs").addEventListener("click", toggleLogsPause);
  document.getElementById("btn-clear-logs").addEventListener("click", clearLogs);

  document.getElementById("rule-modal").addEventListener("click", (e) => {
    if (e.target.id === "rule-modal") closeModal();
  });
}

document.addEventListener("DOMContentLoaded", init);
