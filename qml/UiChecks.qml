import QtQuick

// Explicit opt-in interaction checks against the running application, using demo data.
Item {
    id: checks
    required property var host
    required property var backendObject
    property bool running: false
    property int step: 0
    visible: false
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
                    checks.verify(app.library.games.length === 8, "Demo library loaded");
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
                    checks.verify(app.library.games.length === 8, "Custom game removal works");
                    checks.verify(app.compactNavigation, "Narrow window uses compact navigation");
                    app.changeSection("appearance");
                    app.openSource("steam");
                    break;
                case 10:
                    app.closeSource();
                    app.openAdd();
                    break;
                case 11:
                    app.closeAdd();
                    console.log("ORBIT_UI_TEST_PASS: dark/light contrast, search, favorites, views, source filters, dialogs, custom games, validation, small window, preview safety");
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
