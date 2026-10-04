const api = globalThis.browser ?? chrome;
async function render() {
  const state = await api.storage.local.get(["enabled", "lastSync", "username", "reportPath", "error"]);
  document.getElementById("enabled").checked = state.enabled !== false;
  document.getElementById("status").textContent = state.lastSync ? `@${state.username} · Synced ${new Date(state.lastSync * 1000).toLocaleString()}` : "No report yet. Open Roblox and sync.";
  document.getElementById("path").value = state.reportPath || "";
  document.getElementById("error").textContent = state.error || "";
}
document.getElementById("enabled").addEventListener("change", event => api.storage.local.set({ enabled: event.target.checked }));
document.getElementById("sync").addEventListener("click", async event => {
  event.target.disabled = true;
  try {
    const result = await api.runtime.sendMessage({ type: "manual-sync" });
    await render();
    if (result?.error) document.getElementById("error").textContent = result.error;
  } catch { document.getElementById("error").textContent = "Sync could not start. Reload your Roblox tab and try again."; }
  finally { event.target.disabled = false; }
});
api.storage.onChanged.addListener(render);
render();
