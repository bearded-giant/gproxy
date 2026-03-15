const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

let rules = [];
let matchCounts = {};

async function init() {
  try {
    const status = await invoke("get_status");
    updateStatus(status);
    renderRules(status.rules || []);
    await loadProfiles();
  } catch (e) {
    console.error("init failed:", e);
  }

  setupEventListeners();
  startEventStream();
}

function renderRules(ruleList) {
  rules = ruleList;
  const container = document.getElementById("rules-list");

  if (!rules.length) {
    container.innerHTML = '<div class="empty-state">No rules loaded</div>';
    return;
  }

  container.innerHTML = rules
    .map(
      (rule) => `
    <div class="rule-row" data-id="${rule.id}">
      <div class="rule-info">
        <label class="rule-toggle">
          <input type="checkbox" ${rule.enabled ? "checked" : ""}
                 data-rule-id="${rule.id}">
        </label>
        <div class="rule-details">
          <div class="rule-name">${rule.id}</div>
          <div class="rule-target">&rarr; target</div>
        </div>
      </div>
      <span class="match-count ${matchCounts[rule.id] ? "active" : ""}"
            id="count-${rule.id}">
        ${matchCounts[rule.id] || 0}
      </span>
    </div>
  `
    )
    .join("");

  container.querySelectorAll('input[type="checkbox"]').forEach((cb) => {
    cb.addEventListener("change", () => toggleRule(cb.dataset.ruleId));
  });
}

async function toggleRule(ruleId) {
  try {
    await invoke("toggle_rule", { ruleId });
  } catch (e) {
    console.error("toggle failed:", e);
    const status = await invoke("get_status");
    renderRules(status.rules || []);
  }
}

async function loadProfiles() {
  try {
    const resp = await invoke("get_profiles");
    const profiles = resp.profiles || [];
    const select = document.getElementById("profile-select");
    select.innerHTML = '<option value="">Switch Profile...</option>';
    profiles.forEach((p) => {
      const opt = document.createElement("option");
      opt.value = p;
      opt.textContent = p;
      select.appendChild(opt);
    });
  } catch (e) {
    console.error("load profiles failed:", e);
  }
}

async function switchProfile(profile) {
  if (!profile) return;
  try {
    await invoke("switch_profile", { profile });
    const status = await invoke("get_status");
    updateStatus(status);
    renderRules(status.rules || []);
    matchCounts = {};
  } catch (e) {
    console.error("switch profile failed:", e);
  }
}

function updateStatus(status) {
  const running = status.running || status.profile;
  document.getElementById("proxy-status").textContent = running
    ? "running"
    : "stopped";
  document.getElementById("proxy-status").className =
    "status-indicator " + (running ? "running" : "stopped");
  document.getElementById("profile-name").textContent =
    status.profile || "--";

  const addr = status.listen_addr || "";
  const port = addr.split(":").pop() || "--";
  document.getElementById("listen-port").textContent = ":" + port;
}

function handleProxyEvent(event) {
  switch (event.type) {
    case "RequestMatched":
      if (event.data && event.data.rule_id) {
        const rid = event.data.rule_id;
        matchCounts[rid] = (matchCounts[rid] || 0) + 1;
        const el = document.getElementById("count-" + rid);
        if (el) {
          el.textContent = matchCounts[rid];
          el.classList.add("active");
        }
      }
      break;
    case "RuleToggled":
      if (event.data) {
        const cb = document.querySelector(
          `input[data-rule-id="${event.data.rule_id}"]`
        );
        if (cb) cb.checked = event.data.enabled;
      }
      break;
    case "ProfileSwitched":
    case "ProxyStarted":
    case "ProxyStopped":
      invoke("get_status").then((s) => {
        updateStatus(s);
        renderRules(s.rules || []);
      });
      break;
  }
}

async function startEventStream() {
  await listen("proxy-event", (event) => {
    handleProxyEvent(event.payload);
  });

  await listen("ws-status", (event) => {
    const overlay = document.getElementById("reconnecting");
    overlay.style.display = event.payload.connected ? "none" : "flex";
  });

  await listen("copy-to-clipboard", (event) => {
    navigator.clipboard.writeText(event.payload).catch(() => {});
  });
}

function setupEventListeners() {
  document
    .getElementById("profile-select")
    .addEventListener("change", (e) => {
      switchProfile(e.target.value);
      e.target.value = "";
    });
}

document.addEventListener("DOMContentLoaded", init);
