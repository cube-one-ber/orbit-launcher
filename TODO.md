# Orbit development plan

Priority order: Windows foundation → clean dark/light UI → additional integrations → release verification.

Status: `[x]` implementation complete with local verification of the applicable code; `[ ]` outstanding. Windows layouts and parsers have Linux fixture coverage. Native Windows execution is a separate release gate: Windows CI is configured but has not been run in this workspace.

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
- [ ] Run and record Windows CI; address native compiler or fixture failures.
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
- [x] Exercise both palettes, dialogs and compact navigation offscreen; save dark/light screenshots.
- [ ] Manually verify keyboard-only use and screen-reader announcements on Linux and Windows.
  - Check tab order, focus, Escape, popup navigation and file/folder pickers.
  - Review accessible labels for all icon-only actions and theme choices.
- [ ] Verify Windows 125%, 150% and 200% scaling, high contrast and monitor transitions.
- [ ] Review long names, translated text and libraries containing thousands of games.

## P2 — Additional integrations

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
- [ ] Add Heroic/Legendary with Linux/Windows discovery and metadata fixtures.
- [ ] Add an opt-in application source for Windows Start menu shortcuts and Linux desktop files.
  - Resolve platform shortcut/desktop-entry semantics without arbitrary shell evaluation.
  - Distinguish applications from launchers and avoid duplicate entries.
- [ ] Add emulator profiles, per-game arguments and custom entry/cover editing.

## P3 — Release quality

- [x] Run formatting, core/full-GUI linting, provider tests and native offscreen UI checks on Linux.
- [x] Test legacy migration while preserving favorites, sources and history.
- [x] Update README, extension documentation, platform matrix, Windows guide and screenshots.
- [ ] Add a Windows installer, code signing and update strategy after native runtime validation.
- [ ] Review Qt/KDE redistribution licenses and ship third-party notices with packages.
- [ ] Report immediate provider process failures; currently only dispatch is confirmed.
- [ ] Add single-instance handling or settings coordination before supporting concurrent processes.
- [ ] Add discovery cancellation and progress reporting for slow/network libraries.
- [ ] Add full Linux/Windows GUI release CI after dependency deployment is validated.

## Verification recorded for this pass

- Linux: 28 Rust unit/integration tests pass; core and full-GUI Clippy pass with `-D warnings`.
- Kirigami: offscreen checks cover dark/light contrast, search, favorites, filters, views, dialogs, custom games, validation, compact navigation and demo isolation.
- Windows: source, registry adapters, fixtures, CI and deployment scripts are present. Native compilation, CI execution, actual launches, scaling and clean-machine deployment remain open.

## Acceptance criteria

1. Existing Linux discovery and metadata behavior passes tests.
2. Windows paths and launch resolution are implemented, core CI is configured and unverified runtime work remains visible.
3. Both themes cover the UI and persist through repeated saves.
4. Epic/GOG discovery and commands have fixtures, with real Windows launching tracked separately.
5. This checklist distinguishes implemented work from release blockers.
