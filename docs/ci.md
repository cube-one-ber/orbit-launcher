# Automated prerelease builds

The [CI workflow](https://github.com/cube-one-ber/orbit-launcher/actions/workflows/checks.yml)
runs on every push, pull request, and manual **Run workflow** invocation. It checks
Rust formatting, runs core tests and Clippy on each platform, builds the full GUI
in release mode, and runs the offscreen interaction checks against each binary.
Platform jobs run independently so one failure does not cancel the others. Every
successful branch push publishes a separate [GitHub prerelease](https://github.com/cube-one-ber/orbit-launcher/releases)
tagged `ci-<full-commit-SHA>`. New commits do not cancel older push builds.
Pull requests build and test packages without publishing. Manual runs can also
publish the selected branch's commit; rerunning the same commit updates its
existing prerelease rather than creating duplicates.

| Artifact | Runner | Rust target |
| --- | --- | --- |
| `orbit-linux-x86_64` | Ubuntu 24.04 | `x86_64-unknown-linux-gnu` |
| `orbit-windows-x86_64` | Windows Server 2022 / MSVC | `x86_64-pc-windows-msvc` |
| `orbit-macos-x86_64` | macOS 15 / Intel | `x86_64-apple-darwin` |
| `orbit-macos-arm64` | macOS 15 / Apple Silicon | `aarch64-apple-darwin` |
| `orbit-roblox-browser-extension` | Ubuntu 24.04 / Node 22 | Chrome/Chromium and Firefox source archives |

Download assets from the commit's **prerelease**. Windows builds contain a ZIP;
Linux and macOS builds contain a tar.gz archive that preserves executable
permissions and symlinks. Releases include `SHA256SUMS`, browser extensions, and
all four application packages. Workflow artifacts are also kept for 14 days;
prerelease assets persist until the release is removed. Packages include a
`BUILD-INFO.json` recording the exact commit, architecture, and dependency versions,
dependency license texts, upstream attributions and source locations.

The separate Roblox job runs the browser sync fixtures and uploads both extension
variants. Orbit binaries also embed the Chrome/Chromium setup files. The Firefox
archive is unsigned and intended for temporary development loading; see the
[Roblox setup guide](roblox.md) for installation and signing limitations.

**Windows:** extract the entire ZIP and double-click `orbit.exe`. Qt **6.8.3**,
Kirigami **6.13.0**, their QML modules, native plugins, and official redistributable
Microsoft C++/OpenMP runtime DLLs are included. Rust, Visual Studio, Craft and Qt
installations are unnecessary. Keep the DLLs and subfolders beside the executable.
Windows 10/11 x64 is required. Game clients and browsers remain separate installs.

**Linux:** on Ubuntu 24.04 or a compatible newer distribution, extract the tar.gz
and run `./orbit`. Libraries and QML imports resolve relative to the package. Qt,
Kirigami, QML modules, plugins and native dependencies are included; glibc and
graphics drivers come from your distribution.

**macOS:** extract the archive for Intel or Apple Silicon and open `Orbit.app`.
Qt frameworks, Kirigami and QML/plugins are bundled. macOS 15+ is required. These
prerelease apps are ad-hoc signed, without Developer ID notarization; macOS may
require you to approve opening the downloaded app in Privacy & Security.

After building, CI extracts each actual upload on a **separate fresh runner**
with no Qt/Kirigami/Rust setup. It moves the package into a path containing spaces,
clears development library/import paths, verifies the embedded commit, and runs
interaction checks, a native-window smoke test, and a light-theme smoke test.
Windows also checks required bundled DLLs; macOS verifies the app signature.
Publishing requires formatting, browser tests, all platform builds and all four
extracted-package checks to succeed. Uploads go to a draft before publication,
so failed initial uploads do not expose partial prereleases.

macOS compilation and demo UI checks do not imply native game discovery support:
the providers currently use Linux paths on non-Windows systems. Configure custom
games or JSON providers on macOS until native discovery is implemented. The
packages are prerelease builds; native game launches still need testing with real
installed clients. The Windows package is portable rather than an installer.

Qt downloads, Kirigami installs, and Rust compilation are cached. To change the
dependency versions, update `QT_VERSION` and `KDE_VERSION` in the workflow; to
change the Kirigami build options, also increment the `kirigami-v1` cache prefix.
