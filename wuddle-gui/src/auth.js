// GitHub authentication: status, token management, health display
import { state } from "./state.js";
import { $ } from "./utils.js";
import { safeInvoke } from "./commands.js";
import { log } from "./logs.js";
import { withBusy } from "./ui.js";
import { openUrl } from "./repos.js";

let _refreshAll = async () => {};
export function setAuthCallbacks(cbs) {
  if (cbs.refreshAll) _refreshAll = cbs.refreshAll;
}

export function setGithubAuthStatus(message, kind = "") {
  const statusLine = $("ghAuthStatus");
  statusLine.classList.remove("status-ok", "status-warn");
  if (kind) statusLine.classList.add(kind);
  statusLine.textContent = message;
}

export function renderGithubAuth(status) {
  state.githubAuth = status;
  const statusKnown = status && typeof status === "object";
  const keychainAvailable = !!status?.keychainAvailable;
  const tokenStored = !!status?.tokenStored;
  const envTokenPresent = !!status?.envTokenPresent;

  const input = $("githubToken");
  const saveBtn = $("btnSaveGithubToken");
  const clearBtn = $("btnClearGithubToken");
  const hint = $("ghAuthHint");
  const badge = $("ghAuthBadge");

  badge.className = "auth-badge";

  // Keep auth controls available even while probing keychain.
  input.disabled = false;
  saveBtn.disabled = false;
  clearBtn.disabled = false;

  if (!statusKnown) {
    hint.textContent = "Optional: add a GitHub token to avoid anonymous API rate limits.";
    badge.textContent = "Checking…";
    badge.classList.add("info");
    setGithubAuthStatus("Checking keychain availability…");
    renderGithubAuthHealth();
    return;
  }

  if (!keychainAvailable) {
    hint.textContent =
      "System keychain unavailable. Saving still works for this session (env token).";
    if (envTokenPresent) {
      badge.textContent = "Env token";
      badge.classList.add("info");
    } else {
      badge.textContent = "No token";
      badge.classList.add("warn");
    }
    setGithubAuthStatus(
      envTokenPresent
        ? "Environment token detected (authenticated requests enabled)."
        : "No token detected.",
      envTokenPresent ? "status-ok" : "status-warn",
    );
    state.loggedNoTokenAutoSkip = false;
    renderGithubAuthHealth();
    return;
  }

  const portableMode = !!status?.portableMode;
  hint.textContent = "Optional: add a GitHub token to avoid anonymous API rate limits.";
  if (tokenStored) {
    badge.textContent = portableMode ? "Saved locally" : "Keychain token";
    badge.classList.add("ok");
    setGithubAuthStatus(
      portableMode ? "Token saved locally (portable mode)." : "Token saved in system keychain.",
      "status-ok",
    );
    state.loggedNoTokenAutoSkip = false;
  } else if (envTokenPresent) {
    badge.textContent = "Env token";
    badge.classList.add("info");
    setGithubAuthStatus("Environment token detected (not stored in keychain).", "status-ok");
    state.loggedNoTokenAutoSkip = false;
  } else {
    badge.textContent = "No token";
    badge.classList.add("warn");
    setGithubAuthStatus("No token saved (optional).", "status-warn");
  }
  renderGithubAuthHealth();
}

export function hasGithubAuthToken() {
  return !!state.githubAuth?.tokenStored || !!state.githubAuth?.envTokenPresent;
}

export function detectGithubAuthIssue() {
  if (!hasGithubAuthToken()) return null;
  const authIssuePattern =
    /bad credentials|requires authentication|http\s*401|http\s*403|rate[\s-]?limit|forbidden/i;
  for (const plan of state.plans) {
    const error = String(plan?.error || "").trim();
    if (!error) continue;
    if (authIssuePattern.test(error)) {
      return error;
    }
  }
  return null;
}

export function renderGithubAuthHealth() {
  const el = $("ghAuthHealth");
  if (!el) return;
  if (state.tab !== "options") return;

  const issue = detectGithubAuthIssue();
  if (!issue) {
    el.textContent = "";
    el.className = "auth-health hidden";
    state.authHealthActiveIssue = "";
    return;
  }

  const issueKey = issue.toLowerCase().trim();
  if (!state.authHealthActiveIssue) {
    if (state.authHealthSeenSession) {
      el.textContent = "";
      el.className = "auth-health hidden";
      return;
    }
    state.authHealthSeenSession = true;
    state.authHealthActiveIssue = issueKey;
  } else if (state.authHealthActiveIssue !== issueKey) {
    // Keep a stable, non-repeating banner within the current session.
    el.textContent = "";
    el.className = "auth-health hidden";
    return;
  }

  const preview = issue.length > 190 ? `${issue.slice(0, 187)}...` : issue;
  el.className = "auth-health warn";
  el.textContent =
    "Token is configured, but GitHub still reports auth/rate-limit errors. Re-save token or create a new classic token (no scopes). Latest: "
    + preview;
}

export async function refreshGithubAuthStatus() {
  try {
    const status = await safeInvoke("wuddle_github_auth_status", {}, { timeoutMs: 5000 });
    renderGithubAuth(status);
  } catch (e) {
    renderGithubAuth(null);
    $("ghAuthHint").textContent = "Could not read GitHub auth status.";
    setGithubAuthStatus(e.message);
  }
}

export async function saveGithubToken() {
  const token = $("githubToken").value.trim();
  if (!token) {
    log("ERROR auth: GitHub token is empty.");
    return;
  }

  await withBusy(async () => {
    try {
      await safeInvoke("wuddle_github_auth_set_token", { token }, { timeoutMs: 8000 });
      $("githubToken").value = "";
      await refreshGithubAuthStatus();
      if (state.githubAuth?.tokenStored) {
        log("GitHub token saved to system keychain.");
      } else if (state.githubAuth?.envTokenPresent) {
        log("GitHub token set for current session (keychain unavailable).");
      } else {
        log("GitHub token saved.");
      }
      await _refreshAll({ forceCheck: true });
    } catch (e) {
      log(`ERROR auth save: ${e.message}`);
    }
  });
}

export async function clearGithubToken() {
  await withBusy(async () => {
    try {
      await safeInvoke("wuddle_github_auth_clear_token", {}, { timeoutMs: 8000 });
      $("githubToken").value = "";
      await refreshGithubAuthStatus();
      log("GitHub token cleared.");
    } catch (e) {
      log(`ERROR auth clear: ${e.message}`);
    }
  });
}

export async function connectGithub() {
  await openUrl("https://github.com/settings/tokens");
}
