# Orbit

Bring your games and apps together in one searchable library. Orbit finds games from supported launchers, lets you add your own apps, and can show your most played Roblox experiences.

**[Download Orbit](https://github.com/cube-one-ber/orbit-launcher/releases)** · [Set up Roblox](#add-roblox-support) · [Troubleshooting](#troubleshooting)

## Download and open Orbit

You do not need to install Rust, Qt or developer tools to use a release download.

### 1. Get the right download

1. Open the **[Releases page](https://github.com/cube-one-ber/orbit-launcher/releases)**.
2. Choose the newest release at the top. Releases marked **Pre-release** are early builds and may have bugs.
3. Scroll to **Assets** below the release description. Click **Assets** to expand the list if it is collapsed.
4. Click the file that matches your computer:

| Your computer | File to download |
| --- | --- |
| Windows 10 or 11, 64-bit Intel/AMD | `orbit-windows-x86_64.zip` |
| Linux, 64-bit Intel/AMD | `orbit-linux-x86_64.tar.gz` |
| Mac with Apple Silicon, such as M1, M2 or M3 | `orbit-macos-arm64.tar.gz` |
| Mac with an Intel processor | `orbit-macos-x86_64.tar.gz` |

On a Mac, **Apple menu → About This Mac** tells you which chip or processor you have.

Choose one of the application files above. **Source code** is for developers, `SHA256SUMS` is for checking downloads, and the `orbit-roblox-…` files are browser extensions rather than the Orbit app. The Roblox extension is already included inside Orbit.

### 2. Extract and run it

**Windows**

1. Find the ZIP in your **Downloads** folder.
2. Right-click it and select **Extract All**, then finish extracting.
3. Open the extracted folder and double-click **`orbit.exe`**.

Keep the entire extracted folder together, including its DLL files and subfolders. Run the extracted copy; moving only `orbit.exe` will leave required files behind. Orbit is portable, so there is no installation wizard.

**Linux**

1. Find the `.tar.gz` file in your **Downloads** folder.
2. Use your archive manager to extract it.
3. Open a terminal in the extracted folder and run:

```sh
./orbit
```

The Linux package targets Ubuntu 24.04 or a compatible newer distribution. Qt and Kirigami are bundled; your system supplies glibc and graphics drivers.

**macOS**

1. Download the archive for your Mac's processor. macOS 15 or newer is required.
2. Double-click the `.tar.gz` file to extract it.
3. Open **`Orbit.app`** from the extracted files.

Prerelease apps are not notarized. If macOS blocks opening the app, review the message in **System Settings → Privacy & Security** and approve it if you trust the download. Native macOS game discovery is still in development; use **Add game** or a custom provider for now.

For package requirements and build details, see [automated builds and downloads](docs/ci.md).

## Find and play your games

1. Install your game launcher and the games you want to play. Sign in through that launcher first.
2. Open Orbit and select **Sources**.
3. Enable the launchers you use. Orbit checks their usual installation folders automatically.
4. Return to your library, find a game, and select **Play**. Press **Ctrl+R** to refresh after installing more games.

If a launcher uses a custom folder, select its **Configure** button under **Sources**, use **Browse folder**, and save. The guides below explain which folder each source needs.

Orbit brings existing installations together. Your original launcher still manages game downloads, accounts, updates and settings, and may open when a game needs them.

### Supported libraries

These integrations are available in this repository; an older release may have fewer sources.

| Library | Linux | Windows | Setup help |
| --- | --- | --- | --- |
| Steam, including non-Steam shortcuts | Yes | Yes | [Steam](docs/steam.md) |
| Prism Launcher, MultiMC, PolyMC, ATLauncher | Yes | Yes | [Minecraft launchers](docs/additional-launchers.md) |
| Modrinth Launcher | Yes | Yes | [Source configuration](docs/advanced.md#your-library) |
| Heroic and Legendary | Yes | Yes | [Epic, GOG and Amazon libraries](docs/launchers.md) |
| Nile (Amazon Games) | Yes | Yes | [Nile setup](docs/additional-launchers.md) |
| itch.io / kitch | Yes | Yes | [Additional launchers](docs/common-launchers.md) |
| Lutris | Yes | — | [Source configuration](docs/advanced.md#your-library) |
| Desktop games and emulators | Yes | — | [Desktop games](docs/desktop-games.md) |
| Epic Games, GOG Galaxy, Battle.net, Ubisoft Connect | — | Yes | [Windows guide](docs/windows.md) |
| Roblox | Report import; compatible client needed to play | Yes; installed Roblox client needed | [Tutorial below](#add-roblox-support) |
| Games and apps you add yourself | Yes | Yes | [Add a game or app](#add-a-game-or-app-yourself) |

## Add Roblox support

Orbit can show **your five most played Roblox experiences from the last week**, with your playtime and account name. It uses the account signed in to your browser. If you played fewer than five experiences, it shows the available ones.

You will need **Chrome or Chromium**, a Roblox account, and Orbit running normally. To play on Windows, install Roblox and sign in to its game client too. On Linux, importing the list works, but playing requires a compatible Roblox client and protocol handler; see the [Roblox guide](docs/roblox.md#artwork-and-direct-launching). Native macOS game discovery is not yet supported.

### 1. Prepare the browser extension

1. In Orbit, open **Sources** and enable **Roblox**.
2. Next to Roblox, select **Configure → Set up browser sync**.
3. Orbit creates the included extension files and displays a **Prepared extension folder**. Select **Copy path**.
4. Leave this dialog open while you complete the browser steps.

You do not need to download a separate Roblox extension ZIP from Releases for this setup.

### 2. Load it in Chrome or Chromium

1. Orbit should open your browser's Extensions page. If it does not, open Chrome or Chromium, type `chrome://extensions` in the address bar, and press Enter.
2. Turn on **Developer mode** on that page.
3. Select **Load unpacked**.
4. In the folder picker, navigate to or paste the folder path you copied from Orbit, then select that folder.
5. Check that **Orbit Roblox Sync** appears in the extension list and is enabled.

Choose the extension folder itself, which contains `manifest.json`, rather than a ZIP or the report file. This browser step is needed because the extension is not published in the Chrome Web Store.

### 3. Sync your Roblox games

1. Open **[roblox.com](https://www.roblox.com/)** in the same browser and sign in normally. If Roblox was already open when you installed the extension, reload that tab.
2. Click the browser's **Extensions** button beside the address bar, open **Orbit Roblox Sync**, and select **Sync now**.
3. The extension downloads a file named **`orbit-roblox-top-games.json`**. This is the report Orbit reads; you do not need to open or edit it.
4. Return to Orbit's Roblox configuration and select **Save**. Leave **Report path (optional)** empty if your browser downloads to your usual Downloads folder.
5. Select **Roblox** in Orbit's sidebar. Press **Ctrl+R** to refresh, or wait up to 30 seconds for an automatic check.
6. Choose an experience and select **Play** to open it in your installed Roblox client.

Automatic sync runs while a Roblox page is open, at most once every 15 minutes. The last successful report remains available when your browser is closed or offline. Orbit does not install the Roblox game client, and you do not enter your Roblox password into Orbit. Login cookies stay in your browser.

### If your browser saves downloads somewhere else

1. Open **Orbit Roblox Sync** in your browser and look at **Report path** to find the saved file.
2. In Orbit, open **Sources → Roblox → Configure → Choose report**.
3. Select **`orbit-roblox-top-games.json`**, then select **Save** and refresh.

For Firefox's temporary development extension, custom launch commands, updates and more help, see the [full Roblox guide](docs/roblox.md).

## Add a game or app yourself

1. Select **Add game**, or press **Ctrl+N**.
2. Enter a name and use the folder button beside **Executable** to choose the program you want to run.
3. Leave **Arguments (optional)** as `[]` unless the program needs launch arguments. Leave other optional fields empty if you do not need them.
4. Select **Add**. The entry appears in your library.

## Make the library yours

- Use search (**Ctrl+F**), source filters and favorites to find games quickly.
- Open **Appearance** to choose dark or light mode, grid or list view, and card size.
- Open a game's details and select **Choose cover** to use your own picture.
- In **Appearance**, turn off **Download missing covers** to use local and previously cached images. **Minecraft covers** lets you choose game update artwork or modpack galleries.

![Orbit in dark mode](docs/dark.png)

![Orbit in light mode](docs/light.png)

## Troubleshooting

| Problem | What to try |
| --- | --- |
| Windows reports missing DLLs | Extract the entire ZIP again and run `orbit.exe` inside that folder, with its DLLs and subfolders beside it. |
| A library is empty | Check that the source is enabled, install games through its launcher, then refresh with **Ctrl+R**. Configure the source folder if you use a custom location. |
| Roblox has no games | Keep a signed-in Roblox tab open, reload it, and use **Sync now**. Check the extension popup for errors and confirm Orbit can find the report. The list uses last week's playtime, so it may be empty if there is no qualifying activity. |
| Roblox shows the wrong account | Sign in to the intended account on Roblox in the browser and select **Sync now**. The next successful report replaces the previous account's list. |
| Roblox appears but Play fails | Install and sign in to a compatible Roblox client. Importing the browser report and opening a game are separate steps. |
| Games appear only as examples | Restart Orbit without `--demo`. Demo mode shows a sample library and does not launch games or save configuration. |

To update Orbit, download and extract a newer release, then use its app. Keep the new package's files together. If you use Roblox, select **Set up browser sync** again, click **Reload** for the extension on `chrome://extensions`, and reload your Roblox tab.

Still stuck? **[Open an issue](https://github.com/cube-one-ber/orbit-launcher/issues)** with your operating system, Orbit release and what happened.

For source folders, command overrides, settings, building from source and custom JSON/Rust providers, see [Advanced setup and development](docs/advanced.md). See the [development checklist](TODO.md) for planned work.

Orbit is written in Rust with Qt 6 and KDE Kirigami. [MIT licensed](LICENSE).
