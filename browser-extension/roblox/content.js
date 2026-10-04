(() => {
  const api = globalThis.browser ?? chrome;
  async function json(url, authenticated = false) {
    // Keep the full collection below Chrome's idle service-worker timeout.
    const response = await fetch(url, { credentials: authenticated ? "include" : "omit", cache: "no-store", signal: AbortSignal.timeout(7000) });
    if (!response.ok) {
      if ([401, 403].includes(response.status)) throw Error("Sign in and check that Roblox Screen time → Top experiences is available for your account.");
      if (response.status === 429) throw Error("Roblox is rate limiting requests. Automatic sync will retry later.");
      throw Error(`Roblox request failed (${response.status}). The previous report was kept.`);
    }
    const body = await response.text();
    if (body.length > 1024 * 1024) throw Error("Roblox returned an unexpectedly large response.");
    return JSON.parse(body);
  }
  async function collect() {
    const [user, time] = await Promise.all([
      json("https://users.roblox.com/v1/users/authenticated", true),
      json("https://apis.roblox.com/parental-controls-api/v1/parental-controls/get-top-weekly-screentime-by-universe", true)
    ]);
    const entries = OrbitRoblox.entries(time);
    const metadata = entries.length ? await json(`https://games.roblox.com/v1/games?universeIds=${entries.map(e => e.universeId).join(",")}`) : { data: [] };
    // Avoid combining data if the user changed accounts while requests were running.
    const current = await json("https://users.roblox.com/v1/users/authenticated", true);
    if (current.id !== user.id) throw Error("Roblox account changed during sync. Try again.");
    return OrbitRoblox.makeReport(user, time, metadata);
  }
  api.runtime.onMessage.addListener((message, _sender, respond) => {
    if (message?.type !== "collect") return;
    collect().then(report => respond({ report }), error => respond({ error: error.message }));
    return true;
  });
  function request() { api.runtime.sendMessage({ type: "request-sync" }).catch(() => {}); }
  request();
  setInterval(request, 15 * 60 * 1000);
  document.addEventListener("visibilitychange", () => { if (!document.hidden) request(); });
})();
