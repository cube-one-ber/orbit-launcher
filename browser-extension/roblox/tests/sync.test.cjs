const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");
const source = name => fs.readFileSync(path.join(__dirname, "..", name), "utf8");
const fixtures = () => ({
  user: { id: 12345, name: "Example_Player", secret: "must-never-export" },
  time: { universeWeeklyScreentimes: Array.from({ length: 8 }, (_, i) => ({ universeId: i + 1, weeklyMinutes: (i + 1) * 60 })) },
  metadata: { data: Array.from({ length: 8 }, (_, i) => ({ id: i + 1, rootPlaceId: i + 101, name: `Game ${i + 1}` })) }
});
function reportModule() {
  const context = vm.createContext({}); vm.runInContext(source("report.js"), context); return context.OrbitRoblox;
}
test("ranks personal minutes and exports only the agreed credential-free schema", () => {
  const f = fixtures(); const report = reportModule().makeReport(f.user, f.time, f.metadata, 1700000000000);
  assert.equal(report.games[0].title, "Game 8"); assert.equal(report.games[0].weekly_minutes, 480);
  assert.equal(report.games.length, 8); // Orbit chooses five after excluding invalid records.
  assert.equal(report.exported_at, 1700000000); assert.ok(!JSON.stringify(report).includes("must-never-export"));
  assert.deepEqual(Object.keys(report).sort(), ["exported_at", "games", "user_id", "username", "version"]);
});
test("rejects changed API formats and distinguishes empty history from failed metadata", () => {
  const f = fixtures(), module = reportModule();
  assert.throws(() => module.entries({ data: [] }), /changed/);
  assert.throws(() => module.entries({ universeWeeklyScreentimes: [{ universeId: 1, weeklyMinutes: "unknown" }] }), /no usable/);
  assert.throws(() => module.makeReport(f.user, f.time, { data: [] }), /previous report/);
  assert.equal(module.makeReport(f.user, { universeWeeklyScreentimes: [] }, { data: [] }).games.length, 0);
});
test("deduplicates experiences and excludes impossible durations and unsafe IDs", () => {
  const f = fixtures(), module = reportModule();
  f.time.universeWeeklyScreentimes.push({ universeId: 8, weeklyMinutes: 500 }, { universeId: 10, weeklyMinutes: 99999 }, { universeId: Number.MAX_SAFE_INTEGER + 1, weeklyMinutes: 100 });
  const entries = module.entries(f.time); assert.equal(entries.length, 8); assert.equal(entries[0].weeklyMinutes, 500);
});

function browserHarness({ accountChanged = false, status = 200, enabled = true, invalidTime = false, downloadState = "complete" } = {}) {
  const f = fixtures(); const state = { enabled }; const downloads = []; const requests = [];
  const events = { background: [], content: [], changed: [] }; let authCalls = 0;
  const pageSender = { url: "https://www.roblox.com/home", frameId: 0, tab: { id: 7 } };
  const popupSender = { url: "chrome-extension://fixture/popup.html" };
  const invoke = (listeners, message, sender) => new Promise(resolve => {
    const waiting = listeners.some(fn => fn(message, sender, resolve) === true);
    if (!waiting) resolve(undefined);
  });
  const storage = {
    async get(keys) { if (typeof keys === "string") return { [keys]: state[keys] }; return Object.fromEntries(keys.map(k => [k, state[k]])); },
    async set(values) { Object.assign(state, values); },
    async remove(key) { delete state[key]; }
  };
  const backgroundApi = {
    storage: { local: storage },
    runtime: { getURL: p => `chrome-extension://fixture/${p}`, onMessage: { addListener: fn => events.background.push(fn) } },
    tabs: { async query() { return [{ id: 7, url: pageSender.url }]; }, sendMessage: (_id, msg) => invoke(events.content, msg, { id: "fixture" }) },
    downloads: { async download(options) { downloads.push(options); return 9; }, async search() { return [{ state: downloadState, filename: "/Downloads/orbit-roblox-top-games.json" }]; }, onChanged: { addListener: fn => events.changed.push(fn) } }
  };
  const contentApi = { runtime: { onMessage: { addListener: fn => events.content.push(fn) }, sendMessage: msg => invoke(events.background, msg, pageSender) } };
  const context = vm.createContext({ browser: backgroundApi, URL, Date, console });
  vm.runInContext(source("report.js"), context); vm.runInContext(source("background.js"), context);
  const contentContext = vm.createContext({ browser: contentApi, URL, Date, AbortSignal, document: { addEventListener() {} }, setInterval() {},
    async fetch(url, options) {
      requests.push({ url, options });
      let body;
      if (url.includes("users/authenticated")) { authCalls++; body = { ...f.user, id: accountChanged && authCalls > 1 ? 6789 : f.user.id }; }
      else if (url.includes("screentime")) body = invalidTime ? {} : f.time;
      else body = f.metadata;
      return { ok: status === 200, status, text: async () => JSON.stringify(body) };
    }
  });
  vm.runInContext(source("report.js"), contentContext); vm.runInContext(source("content.js"), contentContext);
  return { state, downloads, requests, events, pageSender, popupSender,
    message: (msg, sender = pageSender) => invoke(events.background, msg, sender) };
}
async function settle() { for (let i = 0; i < 12; i++) await new Promise(resolve => setImmediate(resolve)); }
test("automatic browser sync downloads one report, waits for completion and respects throttling", async () => {
  const h = browserHarness(); await settle();
  assert.equal(h.downloads.length, 1); assert.equal(h.state.username, "Example_Player"); assert.ok(h.state.lastSync);
  assert.equal(h.downloads[0].filename, "orbit-roblox-top-games.json"); assert.equal(h.downloads[0].conflictAction, "overwrite"); assert.equal(h.downloads[0].saveAs, false);
  const report = JSON.parse(decodeURIComponent(h.downloads[0].url.split(",")[1]));
  assert.ok(reportModule().validReport(report)); assert.equal(report.games[0].place_id, 108);
  assert.ok(!JSON.stringify(report).includes("must-never-export"));
  assert.equal(h.requests.filter(r => r.options.credentials === "include").length, 3);
  assert.ok(h.requests.find(r => r.url.includes("games.roblox.com")).options.credentials === "omit");
  await h.message({ type: "request-sync" }); assert.equal(h.downloads.length, 1);
  await h.message({ type: "manual-sync" }, h.popupSender); assert.equal(h.downloads.length, 2);
});
test("disabled sync and untrusted senders cannot trigger automatic account access", async () => {
  const h = browserHarness({ enabled: false }); await settle(); assert.equal(h.requests.length, 0);
  await h.message({ type: "request-sync" }, { url: "https://attacker.example/", tab: { id: 7 }, frameId: 0 });
  assert.equal(h.requests.length, 0);
  await h.message({ type: "manual-sync" }, h.popupSender); assert.equal(h.downloads.length, 1);
});
test("account changes, private API failures and malformed responses preserve the previous report", async () => {
  for (const options of [{ accountChanged: true }, { status: 401 }, { status: 429 }, { invalidTime: true }]) {
    const h = browserHarness(options); await settle(); assert.equal(h.downloads.length, 0); assert.ok(h.state.error);
  }
});
test("interrupted downloads never claim a successful sync", async () => {
  const h = browserHarness({ downloadState: "interrupted" }); await settle();
  assert.equal(h.state.lastSync, undefined); assert.match(h.state.error, /blocked/); assert.equal(h.state.pending, undefined);
});
test("both browser manifests restrict injection and do not request cookie access", () => {
  for (const name of ["manifest.json", "manifest.firefox.json"]) {
    const manifest = JSON.parse(source(name));
    assert.equal(manifest.manifest_version, 3); assert.ok(!manifest.permissions.includes("cookies"));
    assert.deepEqual(manifest.content_scripts[0].matches, ["https://www.roblox.com/*"]);
    assert.ok(!manifest.host_permissions.includes("<all_urls>"));
  }
});
