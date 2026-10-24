# Managing Minecraft instances

Open a Minecraft instance's details in Orbit and choose **Manage instance**. This is available for Prism Launcher, Modrinth App, MultiMC and PolyMC instances, including custom and portable data folders.

Close Minecraft before changing its files. If a launcher is updating the instance or its modpack, let that finish first. Modpack updates can replace manual changes.

## Mods, resource packs and shader packs

Each tab lists the files already in that instance, including local files that Modrinth does not recognize.

- **Browse Modrinth** searches content matching the installed Minecraft version and mod loader. Enter a search or browse popular results, then choose **Install**. Orbit prefers stable releases and adds missing required mod dependencies. Required downloads are verified before files are added. Missing or conflicting dependencies produce an error so you can resolve them in the owning launcher.
- **Add file** copies a local mod or pack into the instance. Existing files are preserved; Orbit does not verify the compatibility of local files.
- **Disable / Enable** changes the `.disabled` suffix understood by these launchers. Disabled files remain in the instance.
- **Remove** asks before deleting a file and its Prism index metadata. Disable a file if you want to retain it.
- **Check for updates** identifies installed files using Modrinth hashes and displays compatible newer versions. Choose **Update** for one file or **Update all** for the current tab. Updates keep files disabled if they were disabled, add missing required mod dependencies, and verify replacements before changing the original. If dependencies conflict, resolve them in the owning launcher and retry.
- **Open folder** opens the selected content folder. Unpacked resource and shader pack folders are listed; manage their contents there.

Vanilla instances can install resource packs and shader packs. To add mods, install a loader in the owning launcher first. Using shader packs also requires an appropriate shader mod or loader. Orbit does not change Minecraft or loader versions, world settings, accounts or Java installations.

Search and update checks use the public [Modrinth API](https://docs.modrinth.com/api/). Downloads use Modrinth's CDN and are checked against their size and SHA-512 hash. Your existing local files remain usable offline. The artwork preference controls downloaded icons independently of explicit content searches and downloads.

## Instance settings

For Prism, MultiMC and PolyMC, the **Settings** tab edits the instance name, notes, memory allocation and Java arguments. Check an override to use instance values; otherwise the owning launcher's global settings apply. Choose **Save settings** to apply changes. If the owning launcher is open, restart it after saving so its cached configuration does not replace your changes.

Modrinth keeps these settings in its database; edit them in Modrinth App. Orbit changes Modrinth content files without writing `app.db`. The app reconciles those files when it scans the instance. Prism downloads also receive `.index/*.pw.toml` metadata for its content manager. Unknown launcher settings are preserved when Orbit saves `instance.cfg`.

In `--demo` mode, the editor displays sample files and search results. Changes, downloads and opening instance folders are disabled.

## Development checks

`cargo test --no-default-features` covers launcher discovery, settings formats, file operations, Modrinth compatibility and dependencies, verified downloads, failed updates and session state. For an isolated live API check, run:

```sh
cargo test --no-default-features --test instances -- --ignored --nocapture
```

The live check installs Iris, Sodium and a shader pack into a temporary fixture, not a discovered user instance. `scripts/check-ui.sh` exercises the editor in both palettes and a narrow window with demo-only data. Native Windows edits and reconciliation with running launcher versions still need manual verification; fixture checks do not replace those checks.
