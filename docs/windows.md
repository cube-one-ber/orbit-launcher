# Windows development and validation

Orbit includes native Windows discovery and process launching. GitHub Actions builds the full MSVC GUI, tests the Rust core, and runs offscreen interaction checks; see [automated builds](ci.md) for downloads and dependencies. Standalone deployment and real game launches still require native validation. See [TODO.md](../TODO.md) for the remaining release gates.

## Prerequisites

- Windows 10/11 x64.
- Rust stable with the `x86_64-pc-windows-msvc` target.
- Visual Studio Build Tools with the Desktop development with C++ workload and Windows SDK.
- A matching **MSVC** Qt 6 installation with Qt Declarative/Quick Controls, SVG, and **Kirigami 6**.

Use [KDE Craft's Windows setup](https://develop.kde.org/docs/getting-started/building/craft/) to obtain Qt and KDE libraries from the same toolchain. In the Craft environment, install `kirigami` and `qtsvg`. An ordinary Qt SDK alone does not include Kirigami. Do not mix MSVC and MinGW libraries.

## Build

Open the x64 MSVC developer environment with Craft activated, then run:

```powershell
.\scripts\build-windows.ps1 -QMake C:\CraftRoot\bin\qmake.exe
.\target\x86_64-pc-windows-msvc\release\orbit.exe
```

Use your actual Craft qmake path. The script verifies Qt 6, the MSVC specification, and Kirigami, runs the Rust tests, then builds `orbit.exe`. A release build does not open a console window.

To stage a standalone development bundle:

```powershell
.\scripts\build-windows.ps1 -QMake C:\CraftRoot\bin\qmake.exe -Package
```

This uses [windeployqt](https://doc.qt.io/qt-6/windows-deployment.html), follows KDE plugin DLL dependencies with `dumpbin`, writes a relative `qt.conf`, and runs the interaction checks with development paths removed. It fails if that test cannot complete. A staged folder is **not a signed or release-certified installer**; test it on a clean machine and collect Qt/KDE redistribution notices before distribution.

## Libraries and paths

- Configuration: `%APPDATA%\Orbit\settings.json`, overridden by `ORBIT_CONFIG_DIR`.
- Artwork cache: `%LOCALAPPDATA%\Orbit\cache\artwork`, overridden by `ORBIT_CACHE_DIR` (the override contains the `artwork` folder). `ORBIT_CONFIG_DIR` alone isolates the cache under that configuration folder.
- Steam: HKCU/HKLM `Software\Valve\Steam` (both registry views), then Program Files locations; extra libraries come from `libraryfolders.vdf`.
- Prism: `%APPDATA%\PrismLauncher`, with executable discovery in `%LOCALAPPDATA%\Programs\PrismLauncher`, Program Files, PATH, and configured portable data folders. See [Prism data locations](https://prismlauncher.org/wiki/getting-started/data-location/).
- Modrinth: `%APPDATA%\ModrinthApp` and legacy `%APPDATA%\com.modrinth.theseus`, or an additional folder/`app.db` configured in Sources. Custom profile storage comes from the database settings. Current instances use the registered `modrinth://launch/instance/<id>` handler; a command override receives the URI. Older profiles open the launcher without claiming to start the game.
- Heroic: `%APPDATA%\heroic`; discovery reads installed Epic/GOG/Amazon metadata. Executable discovery checks local Programs, Program Files and PATH; portable installs can use an override. Play requests `--no-gui` and `gui=false`.
- Legendary: `%USERPROFILE%\.config\legendary`, or `LEGENDARY_CONFIG_PATH` / `XDG_CONFIG_HOME` overrides. Add other configuration folders or `installed.json` files in Sources. Install `legendary.exe` on PATH or configure an executable override. Each game is launched with the matching `LEGENDARY_CONFIG_PATH`.
- Epic: `%PROGRAMDATA%\Epic\EpicGamesLauncher\Data\Manifests`, with additional `.item` folders configurable in Sources. Orbit skips incomplete installs, DLC and engine plugins. It launches with the registered `com.epicgames.launcher` protocol; a command override receives that URI as one argument.
- GOG: game folders from HKCU/HKLM `Software\GOG.com\Games` in both views, plus standard `GOG Galaxy\Games` locations in Program Files. Add other game folders or libraries in Sources. Orbit reads `goggame-*.info`, excludes DLC and deduplicates IDs. Galaxy's executable is resolved from its installation registry path or Program Files; a command override can select another installation.
- Roblox: the bundled Chrome/Chromium extension exports personal weekly playtime to `%USERPROFILE%\Downloads\orbit-roblox-top-games.json`. Select the report explicitly if the browser downloads elsewhere. Setup detects Chrome in Program Files, Local AppData or PATH, prepares the extension and opens `chrome://extensions`; the browser requires the one-time Load unpacked action. Play joins the root place through Roblox's registered protocol. See the [Roblox guide](roblox.md).
- Lutris: unavailable on Windows; [Lutris supports Linux](https://lutris.net/downloads).
- Battle.net: installed games from uninstall records in both registry views, plus `%PROGRAMDATA%\Battle.net\Agent\product.db`; known products use a direct `--exec=launch <product-id>` request.
- Ubisoft Connect: installation records in `Software\Ubisoft\Launcher\Installs`, plus install-state markers in standard/custom game libraries; uses `uplay://launch/<id>/0`.
- itch.io / kitch: `%APPDATA%\itch\db\butler.db` or `%APPDATA%\kitch\db\butler.db`; recent `itch-setup.exe --run-game` skips the app for native games. The stable executable is resolved under `%LOCALAPPDATA%\itch` / `kitch` or PATH; set a command override for a relocated install binary. Data must remain in the current roaming profile. See [additional launcher setup](common-launchers.md).
- Additional paths must be absolute. Forward slashes (`D:/Games/Steam`) work and are easier to enter in JSON. Backslashes in JSON require escaping (`D:\\Games\\Steam`).
- Custom entries can launch `.exe` files with separately specified argument arrays. Orbit does not evaluate shell command strings.

Example command overrides (each line is a separate example for its provider's command field):

```json
["D:/Apps/PrismLauncher/prismlauncher.exe"]
["D:/Apps/GOG Galaxy/GalaxyClient.exe"]
```

Console launchers are created without an extra console window; GUI launchers use their normal Windows behavior. Heroic requests a hidden main window, while Legendary uses its CLI. Required authentication/error dialogs may still appear.

Prism starts instances with `--dir` and `--launch`, skipping its main window while retaining its account, Java and loader handling. Account, console or error dialogs may still appear.

`[]` restores automatic discovery. Epic's executable override must accept an Epic game URI. Use installed and configured launchers; Orbit does not sign in to accounts itself.

For an isolated preview, run `orbit.exe --demo` or `orbit.exe --demo --light`. Normal theme changes persist; preview changes do not write settings or launch games.

## Native release checklist

1. Run `cargo test --locked --no-default-features` and `scripts/check-ui.ps1 -Executable <path-to-orbit.exe>` on Windows; inspect the configured CI results too.
2. Launch installed Steam, Epic and GOG games, a Prism instance, and a current Modrinth instance, Heroic games from all three stores, a standalone Legendary game, Battle.net and Ubisoft titles, and an itch.io native game; verify non-default locations and paths with spaces and Unicode. Verify Prism skips the main window, an already-running Prism receives the instance request, Heroic stays hidden on cold/already-running launches where supported, Legendary does not flash a console, and older Modrinth profiles open the launcher with a clear message.
3. Check Modrinth default/custom content paths, registered URI dispatch, and command overrides. Test artwork downloads, cached covers after restarting offline, and local covers from paths with spaces and Unicode.
4. Toggle themes, restart, and verify favorites/history/settings survive repeated saves.
5. Check keyboard navigation, file/folder pickers, 125%, 150%, and 200% display scaling.
6. Test the staged bundle on a Windows machine without Craft or Qt on PATH.
7. Confirm Qt/KDE license compliance before producing a signed installer.
