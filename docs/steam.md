# Steam

Orbit imports installed Steam games and non-Steam shortcuts on Linux and Windows. Native and Flatpak Steam data folders on Linux, and registry/default Steam installations on Windows, are discovered automatically. Add other Steam folders or libraries containing `steamapps` in **Sources → Steam → Configure**.

Steam's view has **All Steam games**, **Installed games** and **Non-Steam shortcuts** filters. The normal library, favorites and recent views retain all sources. Steam's support runtimes and Proton tools are excluded; manifests with missing installation state or a missing recorded install folder are skipped. A malformed manifest or shortcut file reports an error without hiding healthy installations.

Play requests `-silent -applaunch <appid>` for installed games and `-silent steam://rungameid/<shortcut-game-id>` for non-Steam shortcuts. Flatpak and executable/argument overrides retain the same behavior. Steam owns launch options, compatibility settings, accounts, updates and cloud saves. Its main window normally stays in the tray; required login/update/error dialogs may appear. Orbit confirms dispatch, not that the game reached its menu.

Shortcuts and custom grid covers come from the account marked `MostRecent` in `config/loginusers.vdf`. If that file is unavailable, Orbit selects the account with the newest local configuration/shortcut metadata. Hidden shortcuts are excluded. Launching through Steam preserves Proton and shortcut launch options rather than reconstructing the shortcut's executable command. Orbit reads `userdata/<account>/config/shortcuts.vdf` and grid artwork without changing Steam's files. Saved Orbit favorites and recent launches use stable shortcut IDs.

Steam custom grid artwork takes priority over its cached library covers. Shortcuts support both the 32-bit shortcut app ID and the 64-bit game ID used in grid filenames. A shortcut icon is displayed without cropping if no cover exists. Orbit's **Choose cover** remains available in game details.

Fixture tests cover binary KeyValues, signed/unsigned shortcut IDs, account selection, hidden shortcuts, malformed/truncated metadata, Flatpak arguments, custom commands, installed-state checks and stale paths. Cold and already-running native Steam launches, account switching and real Windows shortcut launches still need manual validation.

Upstream references: [Steam's silent launch option](https://help.steampowered.com/en/faqs/view/0188-6BB7-D467-08E1), [Steam command-line options](https://developer.valvesoftware.com/wiki/Command_Line_Options#Steam), [ValvePython binary KeyValues implementation](https://github.com/ValvePython/vdf/blob/master/vdf/__init__.py), [shortcut game-ID conversion](https://gist.github.com/sonic2kk/934fc97d27d9d8c4ac9c1d817e163bf1).
