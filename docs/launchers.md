# Heroic and Legendary

Both sources discover installed games from local files without signing in, running discovery commands, or modifying launcher-owned configuration. Enable or disable them independently in **Sources**. Refresh after installing or moving a game.

## Heroic Games Launcher

Orbit supports Heroic's installed **Epic Games**, **GOG** and **Amazon Games** libraries. It reads:

| Store | Installation records relative to Heroic's configuration folder |
| --- | --- |
| Epic | `legendaryConfig/legendary/installed.json`, with `metadata/<app-name>.json` for artwork |
| GOG | `gog_store/installed.json`, with optional `store_cache/gog_library.json` for titles/artwork |
| Amazon | `nile_config/nile/installed.json`, with `library.json` and optional `store_cache/nile_library.json` for titles/artwork |

Library caches enrich installed entries; an owned game appearing only in a cache is not imported. Missing directories, DLC, preloaded/incomplete Epic installs and GOG installations with `.gogdl-resume` are skipped. Broken records/files are reported independently so another store can still load. If optional title metadata is absent, Orbit uses the installation folder's name.

Default configuration folders:

- Linux: `$XDG_CONFIG_HOME/heroic`, normally `~/.config/heroic`.
- Linux Flatpak: `~/.var/app/com.heroicgameslauncher.hgl/config/heroic`.
- Windows: `%APPDATA%\heroic`.

Add other **Heroic configuration folders** in Sources. Native Linux uses `heroic`; Flatpak uses `flatpak run com.heroicgameslauncher.hgl`. Windows checks local Programs, Program Files and PATH for `Heroic.exe`. AppImages and portable installations should use a command override, such as:

```json
["/home/example/Applications/Heroic.AppImage"]
```

```json
["D:/Apps/Heroic/Heroic.exe"]
```

Orbit appends `--no-gui` and a single encoded launch argument:

```text
heroic://launch?appName=<game-id>&runner=<legendary|gog|nile>&gui=false
```

This requests a hidden main window at startup and, on recent Heroic builds, when an existing instance receives the launch request. Heroic retains account handling, Wine/Proton, environment settings, cloud-save behavior and per-game arguments. Older versions may still show the main window; account/error prompts may appear when needed. Custom configuration folders must belong to the Heroic installation selected by your command override.

## Standalone Legendary

Orbit reads `installed.json` and optional `metadata/<app-name>.json`. The default is `$XDG_CONFIG_HOME/legendary`, or `~/.config/legendary` when unset, **including on Windows**. `LEGENDARY_CONFIG_PATH` overrides discovery. Heroic's embedded Legendary folder is not automatically scanned by the standalone source.

Additional paths can be configuration folders or their `installed.json` files. Each game receives its discovered configuration folder through `LEGENDARY_CONFIG_PATH`, so multiple profiles retain their own launch settings and account context. Arguments are passed separately:

```text
legendary launch -- <app-name>
```

The `--` separator keeps an app name from being interpreted as a launcher option. Orbit does not force offline mode or bypass launch authentication. Install Legendary, sign in and configure its Wine/native launch settings through Legendary first. Use a command override if it is not on PATH:

```json
["D:/Tools/legendary.exe"]
```

Legendary uses its CLI without a launcher window. On Windows, Orbit starts console launchers with `CREATE_NO_WINDOW` to avoid flashing a terminal; Windows ignores that flag for GUI executables. Required game or account UI remains controlled by the launcher.

Game IDs include the source, canonical configuration folder and store/app identity. Adding the same folder twice does not duplicate games. Different configurations remain separate, and explicitly adding a Heroic-managed Legendary folder to the standalone source can intentionally show the same game under both sources.

## Validation and remaining checks

Fixtures cover all three Heroic stores, exact URI encoding, cold/already-running quiet-launch requests, Flatpak/portable command overrides, read-only discovery, corrupt files, missing/incomplete installs, metadata priority and Legendary configuration dispatch. The UI checks exercise both new sources' filters and configuration dialogs. Windows CI also checks that a spawned console tool has no console window.

Real game launches, Heroic version-specific window behavior, account prompts, Wine/Proton/cloud saves and portable Windows installs still need manual validation. See [TODO.md](../TODO.md).

The format and launch behavior were checked against the official [Heroic source](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher), [Heroic protocol handler](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher/blob/main/src/backend/protocol.ts), [Legendary source and CLI documentation](https://github.com/legendary-gl/legendary), and [Windows process-creation documentation](https://learn.microsoft.com/en-us/windows/win32/procthread/process-creation-flags).
