import QtQuick

// Explicit opt-in interaction checks against the running application, using demo data.
Item {
    id: checks
    required property var host
    required property var backendObject
    property bool running: false
    property int step: 0
    readonly property string testImage: "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGNwTNr4HwAEkAJU0oHd0AAAAABJRU5ErkJggg=="
    visible: false
    Theme { id: probeTheme; dark: checks.host.darkMode }
    GameArtwork { id: artworkProbe; width: 200; height: 120; theme: probeTheme; game: ({title:"Fixture app", provider:"custom", artwork:checks.testImage, art:{kind:"icon",icon:""}}) }
    function verify(condition, message) {
        if (!condition) throw new Error(message);
    }
    Timer {
        interval: 180; repeat: true; running: checks.running
        onTriggered: {
            try {
                const app = checks.host;
                const backend = checks.backendObject;
                switch (checks.step++) {
                case 0:
                    checks.verify(backend.demo, "UI checks require --demo");
                    checks.verify(app.library.games.length === 15, "Demo library loaded");
                    checks.verify(app.navigationFits, "Settings navigation fits with all providers");
                    checks.verify(app.library.providers.some(p=>p.id === "modrinth"), "Modrinth is available in navigation");
                    checks.verify(app.library.providers.some(p=>p.id === "heroic") && app.library.providers.some(p=>p.id === "legendary"), "Heroic and Legendary are available in navigation");
                    app.query = "hollow";
                    break;
                case 1:
                    checks.verify(app.visibleGames.length === 1 && app.visibleGames[0].title === "Hollow Knight", "Search filters titles");
                    app.query = "";
                    backend.favorite("demo:1");
                    app.changeSection("favorites");
                    break;
                case 2:
                    checks.verify(app.visibleGames.some(g => g.id === "demo:1"), "Favorites persist in the model");
                    backend.configure(JSON.stringify({theme:"light", view:"list", density:"compact"}));
                    app.changeSection("library");
                    break;
                case 3:
                    checks.verify(app.prefs.theme === "light" && !app.darkMode && app.prefs.view === "list", "Appearance and view settings update");
                    checks.verify(app.pageColor.toString() === "#f5f6f8" && app.palette.highlightedText.toString() === "#ffffff", "Light background and primary text have the intended contrast");
                    app.sourceFilter = "prism";
                    break;
                case 4:
                    checks.verify(app.visibleGames.length === 1 && app.visibleGames[0].title === "Minecraft", "Provider filter works");
                    app.openGame(app.visibleGames[0]);
                    break;
                case 5:
                    app.closeGame();
                    backend.launch("demo:2");
                    checks.verify(backend.message.indexOf("Preview mode") >= 0, "Preview mode cannot launch games");
                    app.changeSection("sources");
                    backend.configure(JSON.stringify({theme:"dark"}));
                    break;
                case 6:
                    checks.verify(app.darkMode && app.prefs.theme === "dark", "Dark theme restored");
                    checks.verify(app.pageColor.toString() === "#111216" && app.palette.highlightedText.toString() === "#16294f", "Dark background and primary text have the intended contrast");
                    app.openSource("steam");
                    break;
                case 7:
                    app.closeSource();
                    checks.verify(!backend.add_game(JSON.stringify({title:"", command:["example"]})), "Missing names are rejected");
                    checks.verify(!backend.configure(JSON.stringify({theme:"invalid"})), "Invalid themes are rejected");
                    checks.verify(backend.add_game(JSON.stringify({title:"Custom test game", command:["example", "argument with spaces"]})), "Custom game accepted");
                    app.changeSection("library");
                    break;
                case 8:
                    checks.verify(app.library.games.some(g => g.title === "Custom test game"), "Custom game appears in library");
                    backend.remove_game(app.library.games.find(g => g.title === "Custom test game").id);
                    app.width = 780; app.height = 650;
                    backend.configure(JSON.stringify({view:"grid"}));
                    break;
                case 9:
                    checks.verify(app.library.games.length === 15, "Custom game removal works");
                    checks.verify(app.compactNavigation, "Narrow window uses compact navigation");
                    checks.verify(app.navigationFits, "Settings navigation stays visible in a narrow window");
                    app.changeSection("appearance");
                    app.openSource("steam");
                    break;
                case 10:
                    app.closeSource();
                    app.openAdd();
                    break;
                case 11:
                    app.closeAdd();
                    checks.verify(artworkProbe.hasImage && artworkProbe.iconMode, "Small icons load without cover cropping");
                    artworkProbe.game = {title:"Fixture app", provider:"custom", artwork:"file:///orbit-ui-check-intentionally-missing.png", art:{kind:"cover",icon:checks.testImage}};
                    break;
                case 12:
                    checks.verify(artworkProbe.hasImage && artworkProbe.fallbackUsed && artworkProbe.iconMode, "Broken covers fall back to an icon");
                    artworkProbe.game = {title:"Fixture app", provider:"custom", artwork:checks.testImage, art:{kind:"cover",icon:""}};
                    checks.verify(!backend.set_artwork("demo:2", "missing-cover.png"), "Cover picker rejects missing images");
                    checks.verify(!backend.configure(JSON.stringify({minecraft_artwork:"invalid"})), "Invalid artwork preferences are rejected");
                    backend.configure(JSON.stringify({online_artwork:false, minecraft_artwork:"modpacks"}));
                    break;
                case 13:
                    checks.verify(artworkProbe.hasImage && !artworkProbe.iconMode && !artworkProbe.fallbackUsed, "New covers replace an icon fallback");
                    checks.verify(app.prefs.online_artwork === false, "Offline artwork preference updates");
                    checks.verify(app.prefs.minecraft_artwork === "modpacks", "Minecraft artwork preference updates");
                    app.changeSection("library");
                    app.sourceFilter = "heroic";
                    break;
                case 14:
                    checks.verify(app.visibleGames.length === 1 && app.visibleGames[0].title === "Disco Elysium", "Heroic source filter works");
                    app.openSource("heroic");
                    break;
                case 15:
                    app.closeSource();
                    app.sourceFilter = "legendary";
                    break;
                case 16:
                    checks.verify(app.visibleGames.length === 1 && app.visibleGames[0].title === "The Outer Worlds", "Legendary source filter works");
                    app.openSource("legendary");
                    break;
                case 17:
                    app.closeSource();
                    app.sourceFilter = "roblox";
                    break;
                case 18:
                    checks.verify(app.visibleGames.length === 1 && app.visibleGames[0].title === "DOORS", "Roblox source filter works");
                    checks.verify(app.sortBy === "Weekly playtime" && app.visibleGames[0].source_rank === 1, "Roblox sorts by weekly ranking");
                    checks.verify(backend.prepare_roblox_extension() === "", "Demo cannot install browser extensions");
                    app.openSource("roblox");
                    break;
                case 19:
                    app.closeSource();
                    app.sourceFilter = "steam";
                    checks.verify(app.sortBy === "Name", "Weekly sorting does not leak to other providers");
                    app.sourceFilter = "battlenet";
                    break;
                case 20:
                    checks.verify(app.visibleGames.length === 1 && app.visibleGames[0].title === "Diablo IV", "Battle.net filter works");
                    app.openSource("battlenet");
                    break;
                case 21:
                    app.closeSource(); app.sourceFilter = "ubisoft";
                    break;
                case 22:
                    checks.verify(app.visibleGames.length === 1 && app.visibleGames[0].title === "Assassin’s Creed Odyssey", "Ubisoft filter works");
                    app.openSource("ubisoft");
                    break;
                case 23:
                    app.closeSource(); app.sourceFilter = "itch";
                    break;
                case 24:
                    checks.verify(app.visibleGames.length === 1 && app.visibleGames[0].title === "A Short Hike", "itch.io filter works");
                    app.openSource("itch");
                    break;
                case 25:
                    app.closeSource(); app.changeSection("library"); app.sourceFilter = "desktop";
                    checks.verify(app.visibleGames.length === 1 && app.visibleGames[0].title === "SuperTuxKart", "Desktop game filter works");
                    app.openSource("desktop");
                    break;
                case 26:
                    app.closeSource(); app.sourceFilter = "steam"; app.steamFilter = "shortcuts";
                    checks.verify(app.visibleGames.length === 1 && app.visibleGames[0].title === "RetroArch", "Steam non-Steam shortcuts can be isolated");
                    app.openGame(app.visibleGames[0]);
                    break;
                case 27:
                    app.closeGame(); app.steamFilter = "installed";
                    checks.verify(app.visibleGames.length === 4, "Installed Steam filter excludes shortcuts");
                    app.steamFilter = "all";
                    checks.verify(app.visibleGames.length === 5, "All Steam games includes shortcuts");
                    app.sourceFilter = "desktop";
                    checks.verify(app.visibleGames.length === 1, "Steam type filter does not leak to desktop games");
                    app.changeSection("library");
                    checks.verify(app.visibleGames.length === 15, "Full library returns after Steam filters");
                    console.log("ORBIT_UI_TEST_PASS: Desktop games, Steam installed/shortcut filtering, Battle.net, Ubisoft Connect, itch.io, dark/light contrast, artwork and icon fallbacks, offline preference, Modrinth, Heroic, Legendary, Roblox weekly ranking and setup isolation, search, favorites, views, source filters, dialogs, custom games, validation, small window, preview safety");
                    checks.running = false;
                    Qt.quit();
                    break;
                }
            } catch (error) {
                console.error("ORBIT_UI_TEST_FAIL: " + error);
                checks.running = false;
                Qt.exit(1);
            }
        }
    }
}
