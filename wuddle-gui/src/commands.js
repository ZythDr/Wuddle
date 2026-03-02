const invoke = window.__TAURI__?.core?.invoke;
if (!invoke) {
  document.body.innerHTML =
    "<pre style='padding:16px'>ERROR: window.__TAURI__ missing. Start with: npm run tauri dev</pre>";
  throw new Error("window.__TAURI__ missing");
}

export async function safeInvoke(cmd, args = {}, opts = {}) {
  const timeoutMs =
    typeof opts.timeoutMs === "number" && opts.timeoutMs > 0 ? opts.timeoutMs : null;
  let timer = null;
  try {
    if (!timeoutMs) {
      return await invoke(cmd, args);
    }
    const timeout = new Promise((_, reject) => {
      timer = window.setTimeout(() => {
        reject(new Error(`Request timed out (${cmd})`));
      }, timeoutMs);
    });
    return await Promise.race([invoke(cmd, args), timeout]);
  } catch (e) {
    const msg = typeof e === "string" ? e : (e?.message ?? JSON.stringify(e));
    throw new Error(msg);
  } finally {
    if (timer !== null) {
      window.clearTimeout(timer);
    }
  }
}
