if (!globalThis.OrbitRoblox && typeof importScripts === "function") importScripts("report.js");
const api = globalThis.browser ?? chrome;
let syncing = false;
const interval = 15 * 60 * 1000;
function robloxTab(sender) {
  try { const url = new URL(sender.url); return sender.tab && sender.frameId === 0 && url.origin === "https://www.roblox.com"; }
  catch { return false; }
}
async function sync(tabId, manual) {
  if (syncing) return;
  syncing = true;
  try {
    const state = await api.storage.local.get(["enabled", "lastAttempt"]);
    if ((!manual && state.enabled === false) || (!manual && Date.now() - (state.lastAttempt ?? 0) < interval)) return;
    await api.storage.local.set({ lastAttempt: Date.now(), error: "" });
    const result = await api.tabs.sendMessage(tabId, { type: "collect" });
    if (result?.error) throw Error(result.error);
    if (!OrbitRoblox.validReport(result?.report)) throw Error("Invalid sync report. The previous report was kept.");
    const report = result.report;
    const downloadId = await api.downloads.download({
      url: "data:application/json;charset=utf-8," + encodeURIComponent(JSON.stringify(report, null, 2)),
      filename: "orbit-roblox-top-games.json", conflictAction: "overwrite", saveAs: false
    });
    await api.storage.local.set({ pending: { downloadId, exportedAt: report.exported_at, username: report.username } });
    await checkDownload(downloadId);
  } catch (error) {
    await api.storage.local.set({ error: error.message || "Sync failed. The previous report was kept." });
  } finally { syncing = false; }
}
async function checkDownload(id) {
  const { pending } = await api.storage.local.get("pending");
  if (!pending || pending.downloadId !== id) return;
  const [download] = await api.downloads.search({ id });
  if (download?.state === "complete") {
    await api.storage.local.set({ lastSync: pending.exportedAt, username: pending.username, reportPath: download.filename, error: "" });
    await api.storage.local.remove("pending");
  } else if (download?.state === "interrupted") {
    await api.storage.local.set({ error: "Your browser blocked the report download. Allow downloads for this extension, then sync again." });
    await api.storage.local.remove("pending");
  }
}
api.downloads.onChanged.addListener(delta => { if (delta.state) checkDownload(delta.id).catch(() => {}); });
api.runtime.onMessage.addListener((message, sender, respond) => {
  if (message?.type === "request-sync" && robloxTab(sender)) {
    sync(sender.tab.id, false).then(() => respond({ ok: true })); return true;
  }
  if (message?.type === "manual-sync" && sender.url === api.runtime.getURL("popup.html")) {
    api.tabs.query({ active: true, currentWindow: true }).then(async ([tab]) => {
      if (!tab?.url?.startsWith("https://www.roblox.com/")) throw Error("Open a Roblox website tab and sign in, then try again.");
      await sync(tab.id, true); respond({ ok: true });
    }).catch(error => respond({ error: error.message })); return true;
  }
});
