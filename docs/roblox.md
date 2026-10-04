# Roblox weekly top five

Orbit imports **your five most played experiences from the last week**, ranked by personal playtime. It uses Roblox's signed-in Screen time report, rather than public popularity, favorites or recently played games. If you played fewer than five experiences, Orbit shows those available. Roblox documents this weekly report in [Managing Screen Time](https://en.help.roblox.com/hc/en-us/articles/30428328969492-Managing-Screen-Time); this integration does not claim an all-time ranking.

## Chrome / Chromium setup

1. Open **Sources → Roblox → Configure → Set up browser sync** in Orbit.
2. Orbit prepares the bundled extension under its configuration folder at `extensions/roblox/chromium` and opens Chrome/Chromium's Extensions page if the browser is discoverable. Copy the displayed extension path.
3. On `chrome://extensions`, enable **Developer mode**, select **Load unpacked** and choose that folder. This one-time browser action is required by [Chrome's unpacked extension installation flow](https://developer.chrome.com/docs/extensions/get-started/tutorial/hello-world#load-unpacked). Orbit cannot silently install an unpublished extension into regular Chrome.
4. Visit `https://www.roblox.com/` and sign in normally. If the tab was already open when the extension was installed, reload it. Sync runs automatically on Roblox pages, at most once every 15 minutes. You can use **Sync now** in the extension popup to retry immediately.
5. Orbit checks the report every 30 seconds while idle. Choose the Roblox source to see the weekly ranking, playtime and account name. Favorites and launch history remain available.

The setup button works with installed Chrome on Windows, Chrome/Chromium on PATH on Linux, and standard Chrome/Chromium application locations on macOS. If detection fails, the extension is still prepared; open `chrome://extensions` yourself. Setup does not change browser profiles, enterprise policies, startup settings or security preferences. Updating the prepared files requires **Reload** on the browser's Extensions page and a Roblox tab reload.

## Where the report goes

The extension overwrites `orbit-roblox-top-games.json` in your browser's download directory. Orbit defaults to `%USERPROFILE%\Downloads` on Windows and the XDG download folder on Linux, falling back to `~/Downloads`. If your browser downloads elsewhere, use the extension popup's **Report path**, then **Choose report** in Orbit's Roblox configuration. Select a single file or a folder containing that filename. Use one report to avoid combining accounts.

The report contains only a schema version, export timestamp, Roblox user ID/username, game universe/place IDs, game titles and weekly minutes. Login cookies, authorization headers and authentication tickets stay in the browser. Orbit never reads browser cookie databases. The extension has Roblox host, downloads and local-storage permissions; it does not request the cookies permission or use an external sync server.

The browser must have an open Roblox page for automatic sync. Closing Roblox, signing out or disabling automatic sync leaves the last successful report available offline. A later successful sync replaces it with the currently signed-in account's games. IDs include that account to keep favorites/history separate. Reports older than a day show a message under Sources. Disabled or blocked downloads, rate limits, unavailable Screen time and API changes appear in the extension popup; failed requests keep the previous report. An empty successful report clears the Roblox games.

To disconnect, disable Roblox in Orbit and turn off or remove the extension. Remove the local report if you want its stored playtime deleted. Disabling artwork networking in Orbit does not disable browser sync; use the extension's **Sync automatically on Roblox** toggle for that.

## Artwork and direct launching

Orbit requests Roblox's public landscape experience thumbnails, matched by universe ID, with the matching game icon as fallback. Images use the same validated local cache and offline artwork setting as other sources. Roblox entries do not borrow similarly named Steam covers. Local custom covers still take priority.

Play sends `roblox://experiences/start?placeId=<root-place-id>` directly to the registered client, skipping its home screen. Only this exact join format, with one positive place ID, is accepted. No browser or game-details page is opened during Play. Required login, update and error prompts may still appear. This protocol is described by [Bloxstrap's bootstrapper documentation](https://github.com/bloxstraplabs/bloxstrap/wiki/A-deep-dive-on-how-the-Roblox-bootstrapper-works).

Install Roblox and complete its sign-in on Windows first. Linux requires a compatible client/protocol handler; [Sober](https://sober.vinegarhq.org/) is an unofficial, experimental option, with [protocol setup documented by its maintainers](https://vinegarhq.org/Sober/Troubleshooting.html). An optional command override receives the join URI as a single argument, for example:

```json
["flatpak", "run", "org.vinegarhq.Sober"]
```

Orbit does not install Roblox/Sober, copy browser login into the game client or verify a successful game connection after dispatch.

## Development and release checks

The browser source is in [`browser-extension/roblox`](../browser-extension/roblox); Rust embeds the required files, so shipped Orbit binaries can prepare the extension without a checkout or network download. Build separate source archives with:

```sh
python scripts/package-roblox-extension.py target/artifacts
node --test browser-extension/roblox/tests/sync.test.cjs
```

The Firefox variant is provided for development using **about:debugging → This Firefox → Load Temporary Add-on** with its `manifest.json`. Its unsigned archive is not a permanent installer; [Mozilla signing is required for distribution](https://extensionworkshop.com/documentation/publish/self-distribution/). Orbit's setup UI targets Chrome/Chromium.

Rust fixtures cover top-five ordering, duplicate/invalid entries, report size/schema checks, account isolation, stale reports, paths, protocol validation, artwork fallback and offline cache use. Browser tests simulate automatic syncing, account changes, rate limits, downloads, disabled syncing and report privacy. Public Roblox metadata/artwork can be checked without signing in. A real signed-in sync, Chrome download overwrite behavior and real Windows/Sober game joins remain manual release checks; the Screen time endpoint is not a supported Open Cloud/OAuth contract and may change.
