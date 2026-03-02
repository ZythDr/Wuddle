import { state, LOG_LEVEL_KEY, LOG_WRAP_KEY, LOG_AUTOSCROLL_KEY } from "./state.js";
import { $, formatTime } from "./utils.js";

export function log(line) {
  const text = String(line ?? "");
  const level = /(^|\s)ERROR\b/i.test(text) ? "error" : "info";
  state.logLines.push({ at: new Date(), text, level });
  if (state.logLines.length > 4000) {
    state.logLines.shift();
  }
  renderLog();
}

export function logOperationResult(result) {
  if (typeof result === "string") {
    log(result);
    return result;
  }
  if (!result || typeof result !== "object") {
    const msg = String(result ?? "");
    if (msg) log(msg);
    return msg;
  }
  const steps = Array.isArray(result.steps) ? result.steps : [];
  for (const step of steps) {
    if (step) log(step);
  }
  const msg = String(result.message ?? "");
  if (msg) log(msg);
  return msg;
}

function filteredLogLines() {
  const query = state.logQuery.trim().toLowerCase();
  return state.logLines.filter((entry) => {
    if (state.logLevel !== "all" && entry.level !== state.logLevel) return false;
    if (!query) return true;
    return entry.text.toLowerCase().includes(query);
  });
}

function renderLogLevelButtons() {
  document.querySelectorAll(".filter-btn[data-log-level]").forEach((btn) => {
    const level = btn.getAttribute("data-log-level");
    btn.classList.toggle("active", level === state.logLevel);
  });
}

export function renderLog() {
  if (state.tab !== "logs") {
    state.logDirty = true;
    return;
  }
  if (state.logRenderQueued) {
    state.logDirty = true;
    return;
  }
  state.logRenderQueued = true;
  requestAnimationFrame(() => {
    state.logRenderQueued = false;
    if (state.tab !== "logs") {
      state.logDirty = true;
      return;
    }
    const el = $("log");
    const lines = filteredLogLines();
    el.textContent = lines.map((entry) => `[${formatTime(entry.at)}] ${entry.text}`).join("\n");
    el.classList.toggle("wrap", state.logWrap);
    renderLogLevelButtons();
    if (state.logAutoScroll) {
      el.scrollTop = el.scrollHeight;
    }
    state.logDirty = false;
  });
}

export function setLogLevel(level) {
  const allowed = new Set(["all", "info", "error"]);
  state.logLevel = allowed.has(level) ? level : "all";
  localStorage.setItem(LOG_LEVEL_KEY, state.logLevel);
  renderLog();
}

export function setLogQuery(value) {
  state.logQuery = String(value ?? "");
  renderLog();
}

export function setLogWrap(enabled) {
  state.logWrap = !!enabled;
  localStorage.setItem(LOG_WRAP_KEY, state.logWrap ? "true" : "false");
  renderLog();
}

export function setLogAutoscroll(enabled) {
  state.logAutoScroll = !!enabled;
  localStorage.setItem(LOG_AUTOSCROLL_KEY, state.logAutoScroll ? "true" : "false");
  renderLog();
}

export async function copyLogToClipboard() {
  const text = $("log").textContent || "";
  if (!text.trim()) {
    log("Log is empty.");
    return;
  }

  try {
    if (navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(text);
    } else {
      const ta = document.createElement("textarea");
      ta.value = text;
      ta.setAttribute("readonly", "");
      ta.style.position = "fixed";
      ta.style.opacity = "0";
      document.body.appendChild(ta);
      ta.focus();
      ta.select();
      const ok = document.execCommand("copy");
      ta.remove();
      if (!ok) {
        throw new Error("Clipboard copy command failed");
      }
    }
    log("Copied log to clipboard.");
  } catch (e) {
    log(`ERROR copy log: ${e.message || e}`);
  }
}
