// Deliberately export only playtime and public game/account identifiers.
// No cookies, authorization headers, or arbitrary URLs enter the report.
globalThis.OrbitRoblox = (() => {
  const id = value => Number.isSafeInteger(value) && value > 0;
  const text = (value, limit) => typeof value === "string" && value.trim().length > 0 && value.length <= limit && !/[\u0000-\u001f\u007f]/u.test(value);
  function entries(response) {
    if (!Array.isArray(response?.universeWeeklyScreentimes)) throw Error("Roblox changed its screen-time response. Update Orbit Roblox Sync.");
    const seen = new Set();
    const result = response.universeWeeklyScreentimes
      .filter(e => id(e?.universeId) && Number.isFinite(e.weeklyMinutes) && e.weeklyMinutes > 0 && e.weeklyMinutes <= 10080)
      .sort((a, b) => b.weeklyMinutes - a.weeklyMinutes || a.universeId - b.universeId)
      .filter(e => !seen.has(e.universeId) && seen.add(e.universeId)).slice(0, 20);
    if (response.universeWeeklyScreentimes.length && !result.length) throw Error("Roblox returned no usable playtime values. The previous report was kept.");
    return result;
  }
  function makeReport(user, screenTime, metadata, now = Date.now()) {
    if (!id(user?.id) || !text(user.name, 100)) throw Error("Sign in to Roblox to sync your weekly top games.");
    if (!Array.isArray(metadata?.data)) throw Error("Roblox game details are unavailable. Try syncing again.");
    const games = new Map(metadata.data.filter(g => id(g?.id) && id(g.rootPlaceId) && text(g.name, 300)).map(g => [g.id, g]));
    const ranked = entries(screenTime);
    const playable = ranked.filter(e => games.has(e.universeId));
    if (ranked.length && !playable.length) throw Error("No playable game details were returned. The previous report was kept.");
    return {
      version: 1, exported_at: Math.floor(now / 1000), user_id: user.id, username: user.name,
      games: playable.map(e => ({ universe_id: e.universeId, place_id: games.get(e.universeId).rootPlaceId,
        title: games.get(e.universeId).name, weekly_minutes: Math.max(1, Math.round(e.weeklyMinutes)) }))
    };
  }
  function validReport(report) {
    return report?.version === 1 && id(report.user_id) && id(report.exported_at) && text(report.username, 100)
      && Array.isArray(report.games) && report.games.length <= 20
      && report.games.every(g => id(g.universe_id) && id(g.place_id) && text(g.title, 300)
        && Number.isSafeInteger(g.weekly_minutes) && g.weekly_minutes > 0 && g.weekly_minutes <= 10080);
  }
  return { entries, makeReport, validReport };
})();
