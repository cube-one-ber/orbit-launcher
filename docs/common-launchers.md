# Battle.net, Ubisoft Connect and itch.io

These sources join Orbit's installed library in version 0.6. Enable, disable or configure each in **Sources**. Discovery reads local metadata without running the launcher, signing in or modifying its database. Refresh after installing or moving a game.

## Battle.net — Windows

Orbit checks Windows uninstall records in HKLM/HKCU, in both registry views, and `%PROGRAMDATA%\Battle.net\Agent\product.db`. Additional paths can be Agent folders or explicit `product.db` files. The protobuf reader limits each file to 16 MiB and imports recognized installed game products, excluding client components, unrecognized test/beta/PTR variants and missing installation folders. Recognized historical IDs such as Hearthstone’s `hs_beta` map to their current retail game.

The bundled [product catalog](../src/data/battlenet.json) covers World of Warcraft, Diablo III/IV, Diablo II: Resurrected, Diablo Immortal, Overwatch 2, Hearthstone, StarCraft, StarCraft II, Heroes of the Storm, Warcraft III: Reforged and other recognized Battle.net products. Unknown IDs are skipped rather than guessing a title or launch code. WoW Classic variants and legacy standalone Blizzard executables are not mapped separately yet.

Play starts Battle.net with one argument:

```text
--exec=launch <product-id>
```

The executable is resolved from Program Files, the Battle.net uninstall record or PATH. For another installation, set a command override such as `["D:/Apps/Battle.net/Battle.net.exe"]`. Orbit appends the game request; do not supply shell quotes around it yourself.

This requests the game directly. Battle.net can still show its main window, updates or account prompts; Orbit does not forcibly close it. Local covers take priority, followed by catalog landscape artwork and exact store title matches. Images are cached for offline use.

## Ubisoft Connect — Windows

Default discovery reads `Software\Ubisoft\Launcher\Installs` registry entries and standard `Ubisoft\Ubisoft Game Launcher\games` libraries. Add a game folder or a library of immediate game folders under Sources. Additional discovery reads each game's protobuf `uplay_install.state`, using fields 22 (title) and 28 (game ID). IDs must be positive integers. When a marker is missing or cannot be decoded, valid registry installs remain available with a folder-name title fallback.

Play dispatches the registered Windows protocol handler:

```text
uplay://launch/<game-id>/0
```

A command override, such as `["D:/Apps/Ubisoft/UbisoftConnect.exe"]`, receives that complete URI as one argument instead. Orbit accepts only the direct game-launch operation for this protocol. Connect still manages authentication, updates and any required launcher UI.

Local covers are preferred. Without them, Orbit can use an unambiguous exact store title match, or you can choose your own cover. Marker and registry formats are reverse engineered; verify them with real installs when Connect updates.

## itch.io and kitch — Linux and Windows

Orbit queries `caves`, `games` and `install_locations` in the app's read-only `db/butler.db`. Only installed records with an existing directory are shown; morphing installs, external-only entries and uninstalled library games are excluded. Custom installation folders and normal library locations are supported. Multiple installed uploads appear once per game/database; itch-setup chooses the app's most recently used install when launching.

Default data folders:

| Platform | itch | Canary kitch |
| --- | --- | --- |
| Linux | `$XDG_CONFIG_HOME/itch`, normally `~/.config/itch` | `$XDG_CONFIG_HOME/kitch` |
| Windows | `%APPDATA%\itch` | `%APPDATA%\kitch` |

Additional paths can be an `itch` or `kitch` data folder or its `db/butler.db`. Linux launches receive that folder's parent through `XDG_CONFIG_HOME`, preserving the selected configuration. Windows itch-setup resolves roaming data through the OS known-folder API, so Orbit rejects arbitrary relocated data folders instead of launching a different profile. The install binary can still be relocated with a command override.

Use **itch-setup 1.31.0 or newer with `--run-game` support**. Orbit resolves the stable launcher at `~/.itch/itch-setup` / `~/.kitch/itch-setup` on Linux or `%LOCALAPPDATA%\itch\itch-setup.exe` / `kitch\itch-setup.exe` on Windows, with PATH as fallback. An override must point to itch-setup itself, not the app's shell launch script or a versioned app executable. For example:

```json
["D:/Apps/itch/itch-setup.exe"]
```

Orbit appends separate arguments:

```text
--appname <itch|kitch> --run-game <game-id>
```

Current itch-setup runs native games without the app's main window using its matching bundled butler. It preserves global/per-game preferences, sandboxing, prerequisites and playtime handling. HTML/shell/URL games, required license/prerequisite prompts, missing profiles or an older butler fall back to the app. An older **itch-setup** that does not recognize `--run-game` must be updated; it cannot provide that fallback itself. Orbit confirms dispatch, so a later child-process failure is not currently reported.

The upstream command is not supported on macOS; this source is marked unavailable there. Native games/app fallback still need manual validation on Linux and Windows. See the [detailed TODO](../TODO.md).

The headless command was introduced in [itch-setup 1.31.0](https://github.com/itchio/itch-setup/releases/tag/v1.31.0).

## Format references

- [Original Playnite Battle.net installation reader](https://github.com/JosefNemec/PlayniteExtensions/blob/master/source/Libraries/BattleNetLibrary/BattleNetLibrary.cs), [product metadata](https://github.com/JosefNemec/PlayniteExtensions/blob/master/source/Libraries/BattleNetLibrary/BattleNetGames.cs) and [game launch controller](https://github.com/JosefNemec/PlayniteExtensions/blob/master/source/Libraries/BattleNetLibrary/BattleNetGameController.cs).
- [Original Playnite Ubisoft reader](https://github.com/JosefNemec/PlayniteExtensions/blob/master/source/Libraries/UplayLibrary/UplayLibrary.cs) and [NiceDeck's install-state reader and direct launch](https://github.com/mateussouzaweb/nicedeck/blob/master/src/platforms/launcher/ubisoft.go).
- [Official itch-setup headless launch documentation](https://github.com/itchio/itch-setup#launching-games-in-headless-mode), [butler cave schema](https://github.com/itchio/butler/blob/master/database/models/cave.go) and [headless/app fallback implementation](https://github.com/itchio/itch-setup/blob/master/rungame/rungame.go).
