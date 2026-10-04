# Desktop games

**Desktop games** imports standalone Linux games and emulators from application menus, including native packages, AppImages with desktop entries, Flatpak and Snap. Examples include SuperTuxKart, OpenTTD and RetroArch. A store integration is unnecessary; an installed `.desktop` entry categorized as `Game` is enough.

Orbit scans the user application directory, `XDG_DATA_DIRS` application directories and standard Flatpak/Snap exports. In **Sources → Desktop games → Configure**, add additional application folders or absolute `.desktop` file paths. Additional paths take priority, followed by user entries and system entries. Desktop file IDs stay stable, and a hidden entry masks matching entries in lower-priority directories.

Orbit skips hidden entries, entries marked `NoDisplay`, non-game categories, missing executables, missing `TryExec` programs, known store launchers and game links already handled by those launchers. Directory scans and file reads are bounded; directory symlinks are not followed. Local absolute icons and standard hicolor/pixmap icons are used without cover cropping. Public artwork and custom covers follow the normal library preferences.

Play uses `gio launch <absolute-desktop-file-path>` from GLib. Install `gio` if it is not already available on the desktop. GIO interprets the entry's arguments, quoting, `%` field codes, working folder, terminal preference and Flatpak command. Orbit does not turn the `Exec` value into a shell command. A command override replaces `gio launch` and receives the file path as one argument; use a desktop-file launcher, for example `["dex"]`.

The source can be disabled in Sources. It is unavailable on Windows and macOS; custom executable entries and Steam non-Steam shortcuts remain available there where Steam is supported. Windows Start menu import and emulator ROM/profile libraries are separate future work.

Tests cover filtering, masking/deduplication, stable IDs, icon selection, custom dispatch and quoted executable paths. On Linux with GIO installed, a fixture game verifies real dispatch of arguments, field codes and the working folder. No real game is launched by these tests.

Upstream references: [Desktop Entry Specification](https://specifications.freedesktop.org/desktop-entry/latest-single/), [GIO's desktop-file launch command](https://mail.gnome.org/archives/commits-list/2020-December/msg02950.html), [GIO launch semantics](https://docs.gtk.org/gio/method.AppInfo.launch.html).
