# Orbit development plan

Priority order: Windows foundation → clean dark/light UI → additional integrations → release verification.

Status: `[x]` implementation complete with local or CI verification of the applicable code; `[ ]` outstanding. Windows layouts and parsers have Linux fixture coverage. Native Windows core, release GUI build, and offscreen interaction checks now pass in GitHub Actions; real game launches and clean-machine deployment remain separate release gates.

## P0 — Windows support first

- [x] Separate platform paths and executable resolution from providers.
  - `%APPDATA%\Orbit` configuration, `%USERPROFILE%` home, Linux XDG paths and `ORBIT_CONFIG_DIR`.
  - Fixtures for spaces, Unicode, escaped drive paths and UNC paths.
- [x] Detect Steam from HKCU/HKLM registry entries in both views and standard locations.
  - Follow additional libraries and resolve `steam.exe` independently of PATH.
- [x] Discover Prism's roaming data, installed executables and recognized portable folders.
  - Preserve `--dir`, custom instance directories and IDs as single arguments.
- [x] Show provider availability and notes; mark Lutris unavailable on Windows.
- [x] Replace settings with a same-directory temporary file; test repeated writes and legacy migration.
- [x] Keep launches shell-free; use the Windows GUI subsystem for release builds.
- [x] Make tests portable and configure Windows/Linux core CI.
  - Add an isolated Windows registry test without modifying launcher-owned keys.
- [x] Add PowerShell build/deployment tools and a matched Qt/Kirigami/MSVC setup guide.
- [x] Run and record Windows CI; native core, release GUI build and offscreen checks pass.
- [ ] Validate the native GUI and staged folder on clean Windows 10/11 machines.
  - Check Qt platform plugins, Kirigami modules, KDE dependency DLLs and SVG support.
  - Launch real Steam games and Prism instances with spaces and non-ASCII profile paths.
  - Test UNC libraries and redirected roaming configuration folders.
  - Record Windows, Qt, Kirigami, Rust and MSVC versions before declaring a Windows release ready.

## P1 — Clean dark and light interface

- [x] Replace arbitrary accents with complete, persistent dark and light themes.
- [x] Centralize page, card, navigation, input, popup, border and focus colors.
- [x] Remove promotional content and decorative landscapes; use neutral cover placeholders.
- [x] Use compact navigation, one toolbar, consistent spacing and useful empty states.
- [x] Draw portable icons without a desktop icon theme.
- [x] Retain search, sorting, favorites, recents, grid/list views, refresh and custom entries.
- [x] Add browse controls, inline validation and preserved input on failed submissions.
- [x] List integrations dynamically with source-specific errors and configuration.
- [x] Scroll the growing launcher list while keeping Sources and Appearance visible at small window sizes.
- [x] Exercise both palettes, dialogs and compact navigation offscreen; save dark/light screenshots.
- [ ] Manually verify keyboard-only use and screen-reader announcements on Linux and Windows.
  - Check tab order, focus, Escape, popup navigation and file/folder pickers.
  - Review accessible labels for all icon-only actions and theme choices.
- [ ] Verify Windows 125%, 150% and 200% scaling, high contrast and monitor transitions.
- [ ] Review long names, translated text and libraries containing thousands of games.

## P2 — Additional integrations

- [x] Add Roblox's personal weekly top five from a browser-owned Screen time report.
  - Bundle a Chrome/Chromium MV3 extension with automatic sync on Roblox pages and a manual retry.
  - Prepare extension files and open the browser setup page from Sources; explain the required Load unpacked step.
  - Keep cookies and authentication tickets in the browser; export only account/game IDs, titles and weekly playtime.
  - Watch one bounded local report, rank by playtime, preserve account-specific IDs, and report stale snapshots.
  - Fetch exact Roblox landscape thumbnails with icon fallback and offline caching; exclude unrelated Steam covers.
  - Dispatch direct experience joins with a strict single-place-ID protocol; support compatible client overrides.
  - Add Rust/browser fixtures, demo/navigation checks and CI extension archives.
- [ ] Validate the extension with real signed-in Chrome/Chromium accounts on Linux and Windows.
  - Check screen-time availability, actual response shape, account switches, and no-cookie report exports.
  - Verify automatic download overwrite, redirected Downloads, browser permission prompts and download failures.
  - Verify Orbit detects changes, top-five ordering matches Roblox, and disabling sync retains the last snapshot.
- [ ] Validate actual Roblox Windows and compatible Linux client joins with minimal launcher UI.
  - Check cold/already-running clients, login/update prompts, root-place mapping, and protocol handler errors.
- [ ] Publish/sign browser extension packages for a simpler permanent installation flow.
  - Chrome Web Store publication is required before replacing unpacked setup with a store installation link.
  - Mozilla signing is required for the optional Firefox variant; its unsigned archive is for temporary development loading.

- [x] Add Epic Games installed-manifest discovery on Windows.
  - Skip incomplete/missing installs, DLC and engine plugins; report malformed files independently.
  - Encode catalog namespace, item and app identity in the URI; support command overrides.
- [x] Add GOG Galaxy discovery from registry directories and local metadata.
  - Deduplicate IDs, exclude DLC, support additional folders and executable overrides.
  - Launch with explicit game ID and installation path, preserving spaces as one argument.
- [x] Add fixtures, stable IDs, availability notes and setup instructions for Epic and GOG.
- [ ] Validate real Epic and Galaxy installs and launches on Windows in non-default locations.
  - Verify Epic entitlement/login prompts and already-running launcher behavior.
  - Verify GOG offline installs, Galaxy-managed libraries and multiple metadata versions.
- [x] Add Heroic/Legendary with Linux/Windows discovery and metadata fixtures.
  - Read Heroic Epic, GOG and Amazon installations, with optional cached titles/artwork.
  - Keep standalone Legendary configurations separate and preserve each folder during dispatch.
  - Request Heroic's hidden-window launch modes and launch Legendary through its CLI.
  - Suppress extra console windows for Windows command-line launchers.
- [ ] Validate real Heroic/Legendary launches on Linux and Windows.
  - Check Heroic cold/already-running launches across supported versions and all three stores.
  - Check login/error prompts, Wine/Proton settings, cloud saves and AppImage/portable overrides.
  - Check multiple Legendary configurations, paths with spaces/Unicode, and no console flashes.
- [x] Add Battle.net installed Windows games from registry and protobuf `product.db`.
  - Bound metadata reads, skip unknown components/test clients and missing installs, deduplicate product IDs.
  - Dispatch the selected catalog product with one `--exec=launch <id>` argument; preserve executable overrides.
  - Prefer catalog landscape artwork, then exact store matches and cached/local covers.
- [x] Add Ubisoft Connect installed Windows games from registry and protobuf install-state markers.
  - Read custom game/library folders, validate positive numeric IDs and prefer marker titles.
  - Dispatch a restricted `uplay://launch/<id>/0` link or pass it to the configured executable.
- [x] Add itch.io and kitch installed games/apps on Linux and Windows.
  - Query only public game metadata and install locations in a read-only SQLite connection.
  - Skip morphing/missing/external installs, preserve custom install locations and deduplicate games per database.
  - Use recent `itch-setup --run-game` for headless native games, retaining upstream sandbox/preferences/prerequisite handling and app fallback.
  - Preserve Linux XDG configuration; reject unsupported Windows data relocation and mark macOS unavailable.
- [ ] Validate actual Battle.net, Ubisoft and itch.io game launches on native Windows/Linux.
  - Record launcher versions; check cold and already-running requests, login/update/error prompts, spaces/Unicode and moved libraries.
  - For itch.io, check current itch-setup, older butler fallback, native/HTML games, multiple installed uploads, sandbox defaults and prerequisites.
  - Check Ubisoft install-state formats against real installations; registry discovery remains the fallback for default installs.
  - Maintain Battle.net's product catalog and verify regional IDs, WoW Classic variants and newer products before adding mappings.
- [ ] Add EA app and Rockstar Games Launcher installed-game discovery.
  - Verify current installation manifests/registry and direct-game requests using upstream evidence and real fixtures.
  - Keep launchers' account handling; do not invent hidden-window flags or guess game executables.
- [ ] Add additional Minecraft launchers with local metadata fixtures and direct instance launch where supported.
- [x] Add standalone Linux desktop game/emulator discovery, including Flatpak/Snap exports.
  - Respect desktop IDs, user masking, categories, hidden entries and installed executables; skip existing store launchers/game links.
  - Dispatch through GIO to preserve desktop-entry semantics; verify arguments, field codes and working folders with a fixture game.
- [x] Import Steam non-Steam shortcuts and improve the Steam experience.
  - Bound binary/text metadata reads, select the recent account, preserve custom grid art and use stable unsigned shortcut IDs.
  - Launch installed games with tray-mode/direct requests; launch shortcuts through Steam to preserve Proton/options.
  - Add installed/shortcut filters and exclude stale installs and support tools.
- [ ] Validate real desktop/Flatpak game launches and Steam cold/already-running launches on Linux and Windows.
  - Check account switching, custom shortcut artwork and Proton launch options.
- [ ] Add an opt-in Windows Start menu application source.
- [ ] Add emulator profiles, per-game arguments and custom entry editing.

## P2a — Modrinth, Minecraft and artwork (0.3)

- [x] Discover installed Modrinth profiles on Linux and Windows without modifying launcher databases.
  - Detect current `instances`/content-set/link tables and legacy `profiles` tables.
  - Follow `custom_dir`, extra folders, explicit `app.db`, environment overrides and Linux Flatpak data.
  - Deduplicate databases and preserve game IDs across database layout changes.
  - Skip unfinished/missing profiles; report malformed rows and unsupported schemas independently.
- [x] Launch current Modrinth instances through encoded instance links or command overrides.
  - Restrict accepted links to instance launch operations.
  - Explain legacy launcher limitations, label the action Open launcher, and avoid recording a game launch for that action.
- [x] Start Prism with `--dir` and `--launch`, skipping the main window.
  - Preserve arguments containing spaces and Unicode; retain Prism's authentication, Java and loader handling.
  - Explain behavior in Sources, instance details and documentation.
- [x] Discover Minecraft versions from Prism components and Modrinth's applied content set.
- [x] Default Minecraft covers to official game-drop/update artwork for the installed version.
  - Match known release/hotfix ranges; use exact official patch-note metadata for unknown versions and snapshots.
  - Offer Modrinth featured-gallery preference with update-art fallback.
  - Keep different Minecraft versions from sharing unrelated covers.
- [x] Improve automatic art for all sources.
  - Prefer Steam custom grid art and landscape images, including nested library caches.
  - Separate Lutris banners/covers from icons; discover Epic/GOG local art and public GOG images.
  - Use unambiguous exact Steam title matches when another source has no cover.
- [x] Add background enrichment, incremental GUI updates and an offline artwork cache.
  - Limit requests, redirects, time, response size, decoder allocation and normalized image dimensions.
  - Reuse metadata offline; delay retries of failed resources; cancel enrichment on refresh or preference changes.
- [x] Share artwork presentation across grid, list and game details.
  - Fade in covers, preserve small icons' proportions, and handle missing/broken image fallbacks.
  - Show Minecraft version badges and artwork source information.
- [x] Add per-game cover selection/reset and persistent online-artwork/Minecraft preferences.
- [x] Add fixtures for both Modrinth schemas, read-only queries, active versions, encoded links and Prism direct launch.
- [x] Test cover priorities, normalization, offline reuse, cancellation, malformed/oversized images, unsafe URLs and version-specific sharing.
- [x] Exercise public Steam, GOG, Modrinth and Mojang artwork endpoints with real image decoding/cache writes.
- [x] Update dark/light preview screenshots, the artwork guide, provider setup and Windows instructions.
- [ ] Validate the new Modrinth integration on native Windows with both supported database generations.
  - Verify default/custom storage, spaces/Unicode, URI registration and command overrides.
  - Test an already-running launcher and confirm that legacy entries clearly request profile selection.
- [ ] Validate real Prism direct launches on Linux and Windows, including authentication/error/console prompts and post-game behavior.
- [ ] Verify artwork on Windows offline, with proxy/certificate configurations and custom image paths.
- [ ] Maintain the Minecraft release/drop catalog as upstream adds versions and changes metadata.
- [ ] Add a cache size limit and an explicit artwork-cache cleanup action.
- [ ] Add an optional stronger game-identity mapping for editions whose store titles differ across providers.

## P3 — Release quality

- [x] Run formatting, core/full-GUI linting, provider tests and native offscreen UI checks on Linux.
- [x] Test legacy migration while preserving favorites, sources and history.
- [x] Update README, extension documentation, platform matrix, Windows guide and screenshots.
- [ ] Add a Windows installer, code signing and update strategy after native runtime validation.
- [ ] Review Qt/KDE redistribution licenses and ship third-party notices with packages.
- [ ] Report immediate provider process failures; currently only dispatch is confirmed.
- [ ] Add single-instance handling or settings coordination before supporting concurrent processes.
- [ ] Add discovery cancellation and progress reporting for slow/network libraries.
- [x] Configure full Linux/Windows/macOS GUI release CI with downloadable build artifacts.

## Verification recorded for this pass

- Linux 0.6: 92 Rust tests pass, including Battle.net/Ubisoft protobuf fixtures, strict Ubisoft URI validation, itch.io database/configuration cases and platform executable resolution. Core and GUI-enabled tests pass; the Qt bridge is owned by the library so GUI test binaries link correctly. Clippy passes with `-D warnings`; Kirigami checks exercise Battle.net, Ubisoft, itch.io, desktop games and Steam installed/shortcut filters.
- 0.6 artwork: real Diablo IV, Assassin’s Creed Odyssey and A Short Hike covers decode and cache successfully. New integrations still require real game launch validation; fixture/CI success does not confirm launcher window behavior.


- Linux 0.4: 60 Rust unit/integration tests pass; core and full-GUI Clippy pass with `-D warnings`.
- Heroic/Legendary: fixtures cover three Heroic stores, incomplete/DLC installs, URI encoding, quiet launch flags, Flatpak/portable commands, read-only files, configuration identity and actual child-process environment dispatch. Native Windows CI includes console suppression coverage.
- Kirigami: offscreen checks cover dark/light contrast, search, favorites, filters, views, dialogs, custom games, validation, compact navigation, image/icon fallback, artwork preferences and demo isolation.
- Artwork: real Steam, GOG, Modrinth galleries and several official Minecraft drop downloads decode and cache successfully. An isolated non-demo library checks background discovery/enrichment without launching a game.
- Windows: the 0.3 release's native MSVC compilation, Rust tests, core Clippy and release offscreen UI checks passed in GitHub Actions. New Heroic/Legendary code has Linux fixture coverage; native game launches, quiet-launch behavior, scaling and clean-machine deployment remain open.

## Acceptance criteria

1. Existing Linux discovery and metadata behavior passes tests.
2. Windows paths and launch resolution are implemented, core CI is configured and unverified runtime work remains visible.
3. Both themes cover the UI and persist through repeated saves.
4. Epic/GOG discovery and commands have fixtures, with real Windows launching tracked separately.
5. This checklist distinguishes implemented work from release blockers.
6. Modrinth uses supported launch behavior for its database generation, and Prism skips the main window while retaining launcher-managed authentication.
7. Minecraft artwork matches the installed version; online enrichment never blocks initial library display, and cached/local artwork remains usable offline.
8. Every added launcher uses its quietest supported game-launch mode; required prompts remain available, and version/platform limits are documented.
