# Automated builds

The [CI workflow](https://github.com/cube-one-ber/orbit-launcher/actions/workflows/checks.yml)
runs on every push, pull request, and manual **Run workflow** invocation. It checks
Rust formatting, runs core tests and Clippy on each platform, builds the full GUI
in release mode, and runs the offscreen interaction checks against each binary.
Platform jobs run independently so one failure does not cancel the others.

| Artifact | Runner | Rust target |
| --- | --- | --- |
| `orbit-linux-x86_64` | Ubuntu 24.04 | `x86_64-unknown-linux-gnu` |
| `orbit-windows-x86_64` | Windows Server 2022 / MSVC | `x86_64-pc-windows-msvc` |
| `orbit-macos-x86_64` | macOS 15 / Intel | `x86_64-apple-darwin` |
| `orbit-macos-arm64` | macOS 15 / Apple Silicon | `aarch64-apple-darwin` |
| `orbit-roblox-browser-extension` | Ubuntu 24.04 / Node 22 | Chrome/Chromium and Firefox source archives |

Open a successful workflow run and download its **Artifacts**. Windows builds
contain a ZIP; Linux and macOS builds contain a tar.gz archive that preserves the
executable permission. Artifacts are kept for 14 days. Each archive contains the
binary, README, license, and this guide.

The separate Roblox job runs the browser sync fixtures and uploads both extension
variants. Orbit binaries also embed the Chrome/Chromium setup files. The Firefox
archive is unsigned and intended for temporary development loading; see the
[Roblox setup guide](roblox.md) for installation and signing limitations.

These are development binaries. **Qt and Kirigami runtime libraries are not
bundled.** CI uses Qt **6.8.3** (including Qt SVG and Shader Tools) and KDE
Kirigami **6.13.0**, built with matching architectures and compilers. Running a
downloaded binary requires compatible Qt/Kirigami libraries and QML imports on
your machine. Set `QML_IMPORT_PATH` to Kirigami's QML installation directory when
it is outside Qt's default import paths; expose its shared libraries through
`PATH` on Windows or the appropriate dynamic loader paths on Unix.

macOS compilation and demo UI checks do not imply native game discovery support:
the providers currently use Linux paths on non-Windows systems. Configure custom
games or JSON providers on macOS until native discovery is implemented. The
workflow does not create GitHub Releases, signed installers, or macOS app bundles.

Qt downloads, Kirigami installs, and Rust compilation are cached. To change the
dependency versions, update `QT_VERSION` and `KDE_VERSION` in the workflow; to
change the Kirigami build options, also increment the `kirigami-v1` cache prefix.
