# Artwork and Minecraft instances

Orbit selects covers in Rust and sends local file URLs to the shared Kirigami artwork component. The library appears before network enrichment; cards and open game details update as cached images arrive. Grid, list and details use the same selection and error fallback behavior in both themes.

## Choose your artwork

Open a game's details and choose **Choose cover** to select a local image. **Reset** removes that override. Orbit references your original file, so keep it in a stable location. Automatic selection uses:

1. Your per-game cover override, then provider/local covers.
2. Minecraft drop/update artwork matching the installed version, or a Modrinth project's featured gallery if **Appearance → Minecraft covers → Modpack galleries** is selected. Either preference falls back to the other source if unavailable.
3. Provider cover hints, GOG product images, or an unambiguous exact Steam store title match for entries without a cover. Orbit does not select a similarly named sequel or edition.
4. The application or instance icon, shown at its own proportions rather than stretched like a cover.
5. A neutral placeholder with initials. An image that cannot be displayed falls back to a local icon or the placeholder.

Steam discovery checks custom user grid images and both flat and nested library caches, preferring landscape images for cards. Lutris checks covers and banners separately from icons. GOG and Epic check installation folders. Heroic uses cached Epic/GOG/Amazon image metadata; Legendary uses its local Epic key images, with landscape artwork preferred. Heroic GOG entries also retain their product ID for public GOG artwork lookup. Identically named installed games can share a cover across sources; Minecraft instances must also have the same version.

## Minecraft drops and updates

Prism's `mmc-pack.json` identifies the `net.minecraft` version; older instances can use `IntendedVersion`. Modrinth's legacy database supplies `game_version`, and its current database uses the **applied** content set rather than an update waiting to install. Prism's managed Modrinth pack ID and Modrinth's linked project ID enable gallery artwork without accessing account credentials.

[`src/data/minecraft-artwork.json`](../src/data/minecraft-artwork.json) contains official launcher image paths and version ranges from the Nether Update through Wilderness Bound. Hotfixes keep their own drop's artwork: for example, 1.21.5 uses Spring to Life, 1.21.6–1.21.8 use Chase the Skies, and 26.1.x uses Tiny Takeover. Unknown versions and snapshots use an exact version match in Mojang's current Java patch-note feed. Orbit never substitutes the newest drop for an unknown installed version.

The catalog stores metadata only. Images are downloaded from Mojang when needed and retained in your local cache. Future drops can be added by extending the catalog and its version-matching tests.

Prism Play passes `--dir <root> --launch <instance-id>` to Prism, which starts the instance without its main window. Prism continues to manage authentication, Java and mod loaders. Its own settings determine console visibility and behavior after the game exits. Modrinth instances use the launcher's registered instance link where supported; legacy profiles explicitly open the launcher for selection.

## Offline use and storage

**Appearance → Download missing covers** controls networking and is enabled by default. Turning it off cancels enrichment and keeps existing cache/local images available. Enabling it may send a game's title or project/product ID to public metadata services. No account-library or credential requests are made.

Cache locations:

| Platform | Default folder |
| --- | --- |
| Linux | `$XDG_CACHE_HOME/orbit/artwork`, normally `~/.cache/orbit/artwork` |
| Windows | `%LOCALAPPDATA%\Orbit\cache\artwork` |
| Isolated configuration | `<ORBIT_CONFIG_DIR>/cache/artwork` when only `ORBIT_CONFIG_DIR` is set |
| Explicit cache | `<ORBIT_CACHE_DIR>/artwork` |

Remote images are validated and normalized to PNG with a maximum dimension of 1,280 pixels. Downloads require HTTPS and have byte, image-allocation, timeout, redirect and per-scan request limits. Failed resources retry after a 15-minute cooldown; metadata refreshes after 24 hours, with stale metadata available offline. Refresh cancels the previous enrichment pass. You can delete the artwork cache while Orbit is closed; it is rebuilt when networking is enabled.

Demo mode reads existing cached images and never downloads artwork. To prepare the sample screenshots with real covers, explicitly run:

```sh
cargo run --locked --no-default-features --example artwork-preview -- /tmp/orbit-preview-cache/artwork
ORBIT_CACHE_DIR=/tmp/orbit-preview-cache cargo run --locked -- --demo
```

This example downloads public artwork and checks the real image decoder/cache. It never launches a game.

## Provider hints

Older JSON provider records remain valid. Extensions can optionally supply:

```json
{
  "artwork": "",
  "art": {
    "kind": "cover",
    "icon": "file:///home/example/Pictures/icon.png",
    "remote": ["https://example.org/game/banner.webp"],
    "minecraft_version": "1.21.5",
    "modrinth_project": "1KVo5zza",
    "gog_product": null
  }
}
```

Omit fields that do not apply. Use `kind: "icon"` when `artwork` is a small icon rather than a full cover. HTTPS images pass through the Rust cache rather than Qt's network image loader. Preferences persist as `online_artwork`, `minecraft_artwork` (`updates` or `modpacks`) and `artwork_overrides` keyed by stable game ID.

## Upstream references and attribution

- [Prism command-line interface](https://prismlauncher.org/wiki/getting-started/command-line-interface/) and [Prism source](https://github.com/PrismLauncher/PrismLauncher).
- [Modrinth storage locations](https://support.modrinth.com/en/articles/8797641-modrinth-app-storage-folder-location), [official launcher source and migrations](https://github.com/modrinth/code/tree/main/packages/app-lib), and [project gallery API](https://docs.modrinth.com/api/operations/getproject/).
- [Minecraft update timeline](https://www.minecraft.net/en-us/updates/minecraft-updates-timeline-and-evolution), [official launcher news](https://launchercontent.mojang.com/v2/news.json), and [Java patch notes](https://launchercontent.mojang.com/v2/javaPatchNotes.json).

Artwork belongs to its respective creators and publishers. Orbit's MIT license covers its code, not cached game artwork. This repository ships the selection metadata and screenshot previews; it does not bundle the downloaded cover cache.
