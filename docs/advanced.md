# Advanced setup and development

For downloading a ready-to-run copy of Orbit and connecting Roblox, start with the [beginner guide](../README.md). This guide covers source folders, command overrides, building from source and provider development.

## Run on Linux

On Arch Linux / CachyOS:

```sh
sudo pacman -S --needed rust base-devel qt6-base qt6-declarative kirigami
QMAKE=/usr/bin/qmake6 cargo run --locked
```

Qt 6.5+, development headers/tools, Kirigami 6 and a C++ toolchain are required. Other distributions need equivalent packages. CXX-Qt generates the Qt bridge at build time. Orbit draws interface icons itself, so a desktop icon theme is unnecessary. The relevant game launchers must be installed and configured to play their games.

```sh
cargo run --locked -- --demo         # Sample library; no launches or configuration writes
cargo run --locked -- --demo --light # Preview the light theme
cargo build --locked --release      # Binary: target/release/orbit
```

Set `QMAKE` to Qt 6's qmake executable for nonstandard installations. Use matching Qt and Kirigami libraries.

## Run on Windows

In an x64 MSVC developer environment with KDE Craft activated:

```powershell
.\scripts\build-windows.ps1 -QMake C:\CraftRoot\bin\qmake.exe
.\target\x86_64-pc-windows-msvc\release\orbit.exe --demo
```

Use your actual Craft path. `-Package` stages Qt/KDE dependencies and runs isolated interaction checks. See the [Windows guide](windows.md) for prerequisites, paths and native release checks.

## Your library

Choose **dark or light mode**, switch between grid and list views, and adjust card size in Appearance. Search, source filters, favorites and recent launches keep the library easy to navigate. Steam has separate filters for installed games and non-Steam shortcuts. Browse controls help configure sources and add a game or application. Invalid forms keep your input for correction.

The default library contains actual installed games and, after connecting browser sync, your Roblox weekly top five. Sample entries appear only with `--demo`. Orbit does not ask for account credentials; Roblox authentication stays in your signed-in browser. A successful launch message confirms dispatch of the request; it does not confirm that the game reached its main menu.

Under **Sources**, enable integrations, add absolute paths, or set a launcher command override:

| Source | Additional path should point to |
| --- | --- |
| Steam | A Steam folder or extra library containing `steamapps`; shortcuts are read from the client’s `userdata` |
| Desktop games | A Linux application folder containing `.desktop` files, or an individual desktop file |
| Lutris | Its data directory or `pga.db` |
| Prism | Its data directory containing `prismlauncher.cfg` and normally `instances` |
| MultiMC | Its data/portable folder containing `multimc.cfg` and normally `instances` |
| PolyMC | Its data folder containing `polymc.cfg` and normally `instances` |
| ATLauncher | Its data/portable folder containing `instances/<folder>/instance.json` |
| Nile | Its `nile` folder containing `installed.json` and `library.json`, or `installed.json` itself |
| Modrinth | Its application data directory containing `app.db`, or `app.db` itself; custom content folders are read from its settings |
| Heroic | Its configuration folder containing `legendaryConfig`, `gog_store` or `nile_config` |
| Legendary | Its configuration folder containing `installed.json`, or `installed.json` itself |
| Roblox | The extension's `orbit-roblox-top-games.json` report, or its download folder; leave empty to use Downloads |
| Battle.net | Its Agent folder containing `product.db`, or the database itself |
| Ubisoft Connect | Game folders or immediate library folders containing `uplay_install.state` |
| itch.io / kitch | An `itch` or `kitch` data folder containing `db/butler.db`, or that database; Windows uses the current roaming profile |
| Epic | A manifest directory containing `.item` files |
| GOG | A game folder or library with `goggame-*.info` in immediate game folders |

Commands are JSON arrays, such as `["C:/Apps/PrismLauncher/prismlauncher.exe"]`, `["/opt/PrismLauncher.AppImage"]` or `["flatpak", "run", "org.prismlauncher.PrismLauncher"]`. Orbit appends game-specific arguments. Ubisoft Connect, Epic and current Modrinth instances normally use their registered URI handlers; a command override receives the URI as one argument.

Steam Play requests tray mode and starts the selected game directly, preserving Steam launch options and Proton settings. Non-Steam shortcuts and custom artwork come from the most recent local account. Hidden shortcuts, support tools and stale recorded installations are skipped. Login and update prompts may still appear. See the [Steam guide](steam.md).

Desktop games detects standalone Linux games and emulators from application menus, including Flatpak and Snap. It uses `gio launch` to preserve desktop-entry quoting, arguments, field codes and working folders, and skips existing store launchers/game links. Install GLib’s `gio` if needed. See the [desktop game guide](desktop-games.md).

Prism uses `--dir <data-folder> --launch <instance-id>` to start the instance while skipping its main window. Prism still handles authentication, Java and mod loaders, and may show account or error dialogs. Existing Prism settings control console windows and reopening after the game exits.

MultiMC and PolyMC follow custom instance/icon directories and use `--dir <data-folder> --launch <instance-id>`. ATLauncher reads `instance.json` and selects the instance with `--working-dir` and `--launch`. All three retain launcher-managed accounts, Java, loaders and console preferences. Nile reads standalone Amazon installations and starts its CLI with the discovered configuration; Wine/prefix options can be supplied in its command override. See the [additional Minecraft and Nile guide](additional-launchers.md) for default folders, portable setup and platform limits.

Modrinth discovery supports both legacy `profiles` and current `instances` databases, follows custom content folders, and skips unfinished or missing installations. Current builds launch via `modrinth://launch/instance/<id>`. Older builds lack direct instance launch support, so their entries show **Open launcher** and ask you to select the profile there. Configure a command override if your Linux desktop has no Modrinth URI handler.

Heroic reads installed Epic, GOG and Amazon records rather than importing every owned game. Play requests `--no-gui` and `gui=false` so current Heroic builds keep the main window hidden, including when already running. Heroic still manages Wine/Proton, accounts and launch settings. Older builds or account/error prompts may show UI.

Legendary discovers standalone installations and starts `legendary launch -- <app-name>` with the discovered folder in `LEGENDARY_CONFIG_PATH`. Install Legendary and sign in through its CLI first. Heroic-managed Epic games belong to the Heroic source; standalone Legendary discovery does not automatically scan Heroic folders. See the [Heroic and Legendary guide](launchers.md) for paths and command overrides.

Battle.net and Ubisoft Connect discover installed Windows games automatically and request the selected game directly. Their launchers may still show login, update or other required windows. itch.io reads installed games/apps from its local database and uses `itch-setup --run-game`; install a recent itch-setup with that command. Native games skip the main window, while HTML games, required prompts and older butler builds fall back to the app. See the [additional launcher guide](common-launchers.md) for supported products, paths, overrides and platform limits.

For Roblox, use **Sources → Roblox → Configure → Set up browser sync**. Orbit prepares the embedded Chrome/Chromium extension and opens the Extensions page. Enable Developer mode and **Load unpacked** once; Chrome requires this browser step for an unpublished extension. Visit Roblox while signed in to sync automatically, at most every 15 minutes. Orbit watches the local report and shows your top five **from the last week**, with playtime and account name. Play joins the selected experience directly, skipping Roblox's home screen. See the [Roblox setup guide](roblox.md) for redirected downloads, offline use, command overrides and current release limitations.

Settings live in `$XDG_CONFIG_HOME/orbit/settings.json` (normally `~/.config/orbit/settings.json`) on Linux and `%APPDATA%\Orbit\settings.json` on Windows. `ORBIT_CONFIG_DIR` overrides the directory. Writes atomically replace the file. Legacy accent-based settings retain favorites, source paths and history, and default to dark mode. Malformed settings are reported and protected from automatic overwrite; correct the file or move it aside, then restart. `--light` selects and saves light mode unless used with `--demo`.

Keyboard shortcuts: **Ctrl+F** search, **Ctrl+R** refresh, **Ctrl+N** add, **Ctrl+,** appearance, **Escape** close dialogs / clear search.

## Artwork and offline use

Covers load from installed launchers and a background artwork cache. Small application icons keep their proportions; missing images use a quiet initials placeholder. Open a game's details and select **Choose cover** to use a local image, or **Reset** to restore automatic selection.

Under **Appearance**, **Download missing covers** controls networking and is enabled by default. Downloads use public artwork services and may send a game title or product/project ID; Orbit does not request account credentials. Disable this option to use local and previously cached artwork offline.

**Minecraft covers** defaults to **Game drops & updates**, matching the installed Minecraft version rather than the newest release. Select **Modpack galleries** to prefer featured Modrinth artwork for linked packs; missing galleries fall back to update art.

The cache lives in `~/.cache/orbit/artwork` on Linux (respecting `XDG_CACHE_HOME`) and `%LOCALAPPDATA%\Orbit\cache\artwork` on Windows. `ORBIT_CACHE_DIR` changes the cache root; setting only `ORBIT_CONFIG_DIR` isolates it under `<config>/cache/artwork`.

Demo mode reads existing cached covers without downloading. To preview the sample library with real artwork:

```sh
cargo run --locked --no-default-features --example artwork-preview -- /tmp/orbit-preview-cache/artwork
ORBIT_CACHE_DIR=/tmp/orbit-preview-cache cargo run --locked -- --demo
```

The first command downloads and validates public covers without launching games. See the [artwork guide](artwork.md) for selection priorities, cache behavior, upstream references and provider hints.

## Extend Orbit

### JSON providers

Create a `providers` folder inside Orbit's configuration directory, add a manifest such as [`examples/local-games.json`](../examples/local-games.json), and refresh. Sources can open this directory. Each provider appears in navigation and Sources and can be disabled. IDs must be unique ASCII letters, digits or hyphens; `steam`, `lutris`, `prism`, `multimc`, `polymc`, `atlauncher`, `modrinth`, `heroic`, `legendary`, `nile`, `roblox`, `epic`, `gog`, `battlenet`, `ubisoft`, `itch`, `desktop` and `custom` are reserved. Game IDs are namespaced automatically.

```json
{
  "id": "native",
  "name": "Native games",
  "games": [{
    "id": "supertuxkart",
    "title": "SuperTuxKart",
    "provider": "native",
    "subtitle": "Open-source kart racing",
    "artwork": "",
    "command": ["supertuxkart"],
    "directory": null
  }]
}
```

`artwork` accepts a local `file:///...` URL or HTTPS image URL downloaded through Orbit’s cache. Optional `art` hints describe icons, remote covers, Minecraft versions and Modrinth projects; see the [artwork guide](artwork.md). `directory` sets the working folder. Optional `environment` supplies environment variables to the launched command without changing Orbit’s own environment. Arguments pass directly without shell evaluation. Reading a manifest does not execute it; pressing Play does. Optional `launch_uri` takes precedence over `command` and accepts Epic game-launch links on Windows Ubisoft direct game links on Windows, or Modrinth instance-launch and Roblox experience-join links on Linux/Windows. Ubisoft, Modrinth and Roblox links are restricted to the supported launch operation. Most extensions should use `command`.

### Rust providers

Implement `Provider` in `src/providers/`, returning `Game` records, discovery errors and platform availability. Register it in `providers::discover`; stable IDs preserve favorites and history. Discovery runs on a worker thread and publishes results on the GUI thread. `model`, `store`, `platform`, `providers` and `artwork` work independently of Qt with `--no-default-features`.

QML owns presentation; [`qml/Theme.qml`](../qml/Theme.qml) defines semantic colors and [`src/bridge.rs`](../src/bridge.rs) exposes Rust operations through CXX-Qt.

## Linux desktop installation

After a release build:

```sh
install -Dm755 target/release/orbit ~/.local/bin/orbit
install -Dm644 packaging/app.orbit.Launcher.svg ~/.local/share/icons/hicolor/scalable/apps/app.orbit.Launcher.svg
install -Dm644 packaging/app.orbit.Launcher.desktop ~/.local/share/applications/app.orbit.Launcher.desktop
```

Ensure `~/.local/bin` is on the desktop session's `PATH`, or use its absolute path in the desktop file's `Exec` entry.

## Validation

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --no-default-features
cargo test --locked # Includes the Qt-enabled library; requires GUI build dependencies
node --test browser-extension/roblox/tests/sync.test.cjs
QMAKE=/usr/bin/qmake6 cargo build --locked
scripts/check-ui.sh
QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software target/debug/orbit --demo --smoke-test
```

Rust tests cover Steam libraries and escaped Windows paths, read-only Lutris queries, custom Prism directories, Epic/GOG fixtures, both Modrinth schemas, MultiMC/PolyMC/ATLauncher instances, Heroic’s three stores, Legendary/Nile configuration and metadata, quiet launch arguments, artwork selection/cache/offline behavior, Minecraft version matching, extensions, protocols, metadata migration and repeated settings replacement. Windows CI also exercises a temporary test-owned registry key. UI checks cover both palettes, search, favorites, filters, views, dialogs, custom games, validation, compact navigation, image/icon error fallbacks, artwork preferences and demo isolation.

The Rust suite includes **104 tests on Linux** and **8 browser sync tests**, plus core/full-GUI Clippy and offscreen UI checks. New coverage includes MultiMC/PolyMC custom folders, ATLauncher metadata/launch arguments, Nile configuration and child-process dispatch, Battle.net/Ubisoft protobuf parsing, itch.io databases, Steam binary shortcuts/account selection/stale installs, desktop game filtering and real GIO dispatch of a fixture game. Roblox coverage includes personal rankings, account isolation, bounded reports, direct-join validation, public artwork and cached offline use. Windows CI includes a console-window suppression check. Real signed-in Roblox sync, native Windows game launches and clean-machine deployment remain on the [release checklist](../TODO.md).

The smoke test loads the Kirigami window and exits automatically. Add `--screenshot /absolute/path/preview.png` to capture the rendered page, or `--light` for its light theme. On Windows, run `scripts/check-ui.ps1 -Executable <path-to-orbit.exe>` after building. See [TODO.md](../TODO.md) for native Windows, accessibility and packaging work.
