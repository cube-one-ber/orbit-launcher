# Additional Minecraft launchers and Nile

Orbit imports **MultiMC**, **PolyMC**, **ATLauncher** and standalone **Nile (Amazon Games)** libraries on Linux and Windows. Enable each integration under Sources and add custom data folders or launcher commands there. Discovery reads local metadata without starting the launcher or modifying its files. Native macOS discovery for these sources is not implemented.

## MultiMC and PolyMC

Add the launcher's data folder, containing `multimc.cfg` or `polymc.cfg` and normally an `instances` directory. Orbit follows `InstanceDir` and `IconsDir`, including absolute custom folders. Instances need `instance.cfg`; Minecraft versions come from `mmc-pack.json`, with `IntendedVersion` as a legacy fallback. Local covers/icons, Modrinth project hints and recorded last-launch times are preserved.

On Linux, Orbit checks the XDG data folder's `multimc`, `PolyMC` and legacy `polymc` directories. PolyMC Flatpak data is read from `~/.var/app/org.polymc.PolyMC/data/PolyMC` or its legacy lowercase directory. On Windows, PolyMC uses `%APPDATA%\PolyMC`; detected portable executable folders with `portable.txt` are also scanned. Official MultiMC archives are portable: add their extracted data folder explicitly. A configured MultiMC executable on PATH can expose its adjacent data folder.

Play sends `--dir <data-folder> --launch <instance-folder-name>`. The selected launcher still owns Microsoft authentication, Java, mod loaders, console settings and post-game behavior. Startup/error prompts can appear. PolyMC's post-game preferences can reopen its window.

Orbit resolves `MultiMC.exe` or the Linux `MultiMC` wrapper inside the configured portable folder, and resolves Windows PolyMC executables or Linux portable `bin/polymc`. Other installations can use commands such as:

```json
["D:/Minecraft/MultiMC/MultiMC.exe"]
["/opt/PolyMC.AppImage"]
["flatpak", "run", "org.polymc.PolyMC"]
```

Prism's provider IDs and launch commands remain compatible with existing favorites and history. Identical folder names in separate data folders have distinct IDs.

References: [MultiMC CLI manual](https://github.com/MultiMC/Launcher/blob/develop/doc/multimc.1.txt), [MultiMC portable installation](https://github.com/MultiMC/Launcher/wiki/Getting-Started), [PolyMC CLI and post-game behavior](https://polymc.org/wiki/getting-started/command-line-interface/), [PolyMC data-path implementation](https://github.com/PolyMC/PolyMC/blob/develop/launcher/Application.cpp).

## ATLauncher

Add the data folder containing `instances/<folder>/instance.json`. Linux packages normally use `$XDG_DATA_HOME/atlauncher` (default `~/.local/share/atlauncher`), Flatpak uses `~/.var/app/com.atlauncher.ATLauncher/data`, and Windows installer builds use `%APPDATA%\ATLauncher`. Portable/JAR installations can keep data elsewhere; add their actual folder.

Orbit reads `launcher.name`, the installed Minecraft version and linked Modrinth project. Local covers and `instance.png` are used when present. Bad instance files are reported individually so healthy instances remain visible.

Play passes `--working-dir=<data-folder>` and `--launch=<instance-name>` as individual arguments. ATLauncher starts the instance directly and handles authentication, Java and loaders; initial setup, splash screens, error dialogs and configured consoles may appear. Orbit does not force ATLauncher to close after Minecraft starts.

Windows portable executables and an adjacent Linux `ATLauncher.jar` are resolved automatically. A JAR elsewhere can use:

```json
["java", "-jar", "/opt/ATLauncher/ATLauncher.jar"]
```

For the standard Flatpak package, Orbit invokes the bundled Java/JAR directly because its shell wrapper expands launch arguments without quoting. This preserves names and paths containing spaces; the selected data folder retains the existing launcher configuration. A command override takes priority.

References: [ATLauncher instance reader](https://github.com/ATLauncher/ATLauncher/blob/master/src/main/java/com/atlauncher/managers/InstanceManager.java), [ATLauncher direct-launch CLI](https://github.com/ATLauncher/ATLauncher/blob/master/src/main/java/com/atlauncher/App.java), [Linux package wrapper](https://github.com/ATLauncher/ATLauncher/blob/master/packaging/linux/deb/atlauncher), [Windows installer](https://github.com/ATLauncher/ATLauncher/blob/master/packaging/windows-setup/installer.iss), [Flatpak wrapper](https://github.com/flathub/com.atlauncher.ATLauncher/blob/master/atlauncher.sh).

## Nile (Amazon Games)

Install and sign in through Nile first, then sync its library. Orbit does not read account tokens. It imports installed records from `installed.json`, matches public product metadata from `library.json`, and requires an existing installation folder with `fuel.json`. Missing library records are reported with a suggestion to run `nile library sync`.

Linux defaults to `~/.config/nile` and Windows to `%APPDATA%\nile`. Nile honors `NILE_CONFIG_PATH` first, then `XDG_CONFIG_HOME`; both designate a **parent folder**, to which it appends `nile`. In Sources, add the resulting `nile` folder or its `installed.json`. The folder must be named `nile` because the upstream CLI cannot select an arbitrarily named folder.

Play uses `nile launch -- <product-id>` (or `nile.exe` on Windows). Orbit sets `NILE_CONFIG_PATH` only for that child process to preserve the selected library. Configurations containing the same product ID receive distinct Orbit IDs. Heroic-managed Amazon games stay in the Heroic source unless you explicitly add their configuration to Nile.

Nile handles the Amazon SDK, game launch instructions and Wine on Linux. For a custom Wine prefix or wrapper, include the `launch` subcommand and its options in the command override; Orbit appends `-- <product-id>`:

```json
["nile", "launch", "--wine-prefix", "/home/example/Games/Amazon prefix"]
["nile", "launch", "--wine", "/opt/wine/bin/wine"]
```

A command containing only the executable receives `launch` automatically. Local artwork and public Amazon image URLs use Orbit's normal cache and offline preferences.

References: [Nile project](https://github.com/imLinguin/nile), [configuration paths](https://github.com/imLinguin/nile/blob/master/nile/constants.py), [CLI arguments](https://github.com/imLinguin/nile/blob/master/nile/arguments.py), [installed/library matching](https://github.com/imLinguin/nile/blob/master/nile/cli.py), [game launching](https://github.com/imLinguin/nile/blob/master/nile/utils/launch.py).

## Verification

Fixtures cover custom folders, Unicode/spaces, portable executable resolution, Flatpak command selection, corrupt metadata, duplicate records, source settings, reserved IDs, read-only discovery and real child-process argument/environment dispatch. The UI checks cover source navigation, filtering and configuration dialogs. Actual signed-in game launches on Linux and Windows remain on the [release checklist](../TODO.md).
