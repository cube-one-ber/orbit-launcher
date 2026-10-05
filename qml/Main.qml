pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic as Controls
import QtQuick.Layouts
import QtQuick.Dialogs as Dialogs
import org.kde.kirigami as Kirigami
import app.orbit

Kirigami.ApplicationWindow {
    id: root
    width: 1280; height: 820
    minimumWidth: 720; minimumHeight: 600
    title: "Orbit"
    color: colors.window
    Theme { id: colors; dark: root.prefs.theme !== "light" }
    Backend { id: backend }
    property var library: JSON.parse(backend.snapshot)
    property var prefs: JSON.parse(backend.preferences)
    property string section: "library"
    property string sourceFilter: "all"
    property string steamFilter: "all"
    onSourceFilterChanged: { if (sourceFilter === "roblox") sortBy = "Weekly playtime"; else if (sortBy === "Weekly playtime") sortBy = "Name"; }
    property string query: ""
    property string sortBy: "Name"
    property var selectedGame: null
    onLibraryChanged: {
        if (selectedGame) selectedGame = library.games.find(g => g.id === selectedGame.id) || selectedGame;
    }
    readonly property bool darkMode: colors.dark
    readonly property color pageColor: colors.window
    readonly property bool compactNavigation: width < 980
    readonly property bool navigationFits: appearanceNavigation.y + appearanceNavigation.height <= appearanceNavigation.parent.height
    readonly property string sectionTitle: section === "sources" ? "Sources" : section === "appearance" ? "Appearance" : section === "favorites" ? "Favorites" : section === "recent" ? "Recently played" : sourceFilter !== "all" ? providerName(sourceFilter) : "Library"
    property var visibleGames: {
        let games = library.games.filter(g => (sourceFilter === "all" || g.provider === sourceFilter)
            && (sourceFilter !== "steam" || steamFilter === "all" || (steamFilter === "shortcuts") === g.id.startsWith("steam:shortcut:"))
            && (section !== "favorites" || g.favorite)
            && (section !== "recent" || g.last_played > 0)
            && (g.title + " " + g.subtitle).toLowerCase().includes(query.toLowerCase()));
        games.sort(sortBy === "Recently played" || section === "recent" ? (a,b)=>b.last_played-a.last_played : sortBy === "Weekly playtime" ? (a,b)=>(a.source_rank || 255)-(b.source_rank || 255) : (a,b)=>a.title.localeCompare(b.title));
        return games;
    }
    function save(values) { return backend.configure(JSON.stringify(values)); }
    function changeSection(value) { section = value; sourceFilter = "all"; }
    function providerName(id) {
        const provider = library.providers.find(p=>p.id === id);
        return provider ? provider.name : ({custom:"Custom games",epic:"Epic Games",gog:"GOG Galaxy",modrinth:"Modrinth Launcher",multimc:"MultiMC",polymc:"PolyMC",atlauncher:"ATLauncher",nile:"Nile (Amazon Games)",heroic:"Heroic Games Launcher",legendary:"Legendary",battlenet:"Battle.net",ubisoft:"Ubisoft Connect",itch:"itch.io",desktop:"Desktop games"})[id] || id;
    }
    function openGame(game) { selectedGame = game; details.open(); }
    function closeGame() { details.close(); }
    function openAdd() { addDialog.error = ""; addDialog.open(); }
    function closeAdd() { addDialog.close(); }
    function openSource(id) {
        sourceDialog.sourceId = id;
        sourceDialog.error = "";
        sourceDialog.extensionPath = "";
        const cfg = prefs.sources[id] || {enabled:true, paths:[], command:[]};
        sourcePaths.text = cfg.paths.join("\n"); sourceCommand.text = JSON.stringify(cfg.command);
        sourceDialog.open();
    }
    function closeSource() { sourceDialog.close(); }

    palette.window: colors.window
    palette.windowText: colors.text
    palette.base: colors.surface
    palette.alternateBase: colors.elevated
    palette.text: colors.text
    palette.button: colors.elevated
    palette.buttonText: colors.text
    palette.highlight: colors.accent
    palette.highlightedText: colors.accentText
    palette.placeholderText: colors.faint
    palette.light: colors.border
    palette.mid: colors.border
    palette.dark: colors.border
    palette.toolTipBase: colors.elevated
    palette.toolTipText: colors.text
    Kirigami.Theme.inherit: false
    Kirigami.Theme.backgroundColor: colors.window
    Kirigami.Theme.textColor: colors.text
    Kirigami.Theme.disabledTextColor: colors.faint
    Kirigami.Theme.highlightColor: colors.accent
    Kirigami.Theme.highlightedTextColor: colors.accentText
    pageStack.globalToolBar.style: Kirigami.ApplicationHeaderStyle.None

    Component.onCompleted: {
        if (Qt.application.arguments.includes("--light")) save({theme:"light"});
        backend.refresh();
    }
    Timer { interval: 100; repeat: true; running: backend.busy || backend.artwork_busy; onTriggered: backend.poll() }
    Timer { interval: 30000; repeat: true; running: !backend.demo; onTriggered: backend.check_roblox_report() }
    Timer { interval: 2400; running: Qt.application.arguments.includes("--smoke-test"); onTriggered: Qt.quit() }
    Timer {
        interval: 1400; running: Qt.application.arguments.includes("--screenshot")
        onTriggered: {
            const path = Qt.application.arguments[Qt.application.arguments.indexOf("--screenshot") + 1];
            if (path) libraryPage.grabToImage(result => { if (!result.saveToFile(path)) console.error("Screenshot failed: " + path); });
        }
    }
    UiChecks { host: root; backendObject: backend; running: Qt.application.arguments.includes("--ui-test") }
    Shortcut { sequence: "Ctrl+F"; onActivated: { root.changeSection("library"); search.forceActiveFocus(); } }
    Shortcut { sequence: "Ctrl+R"; onActivated: backend.refresh() }
    Shortcut { sequence: "Ctrl+,"; onActivated: root.changeSection("appearance") }
    Shortcut { sequence: "Ctrl+N"; onActivated: root.openAdd() }
    Shortcut { sequence: "Escape"; onActivated: { if (!details.opened && !addDialog.opened && !sourceDialog.opened) search.clear(); } }

    component Button: Controls.Button {
        id: button
        property string symbol: ""
        property bool primary: false
        property bool quiet: false
        implicitHeight: 38; implicitWidth: Math.max(38, buttonRow.implicitWidth + 26)
        leftPadding: 13; rightPadding: 13
        hoverEnabled: true
        opacity: enabled ? 1 : 0.45
        background: Rectangle {
            radius: 7
            color: button.primary ? colors.accent : button.checked ? colors.selection : button.hovered ? colors.hover : button.quiet ? "transparent" : colors.surface
            border.width: button.activeFocus ? 2 : 1
            border.color: button.activeFocus ? colors.accent : button.primary || button.quiet ? "transparent" : colors.border
        }
        contentItem: RowLayout {
            id: buttonRow
            spacing: 8
            AppIcon { visible: button.symbol !== ""; name: button.symbol; ink: button.primary ? colors.accentText : button.checked ? colors.accent : colors.muted; Layout.preferredWidth: 17; Layout.preferredHeight: 17 }
            Controls.Label { visible: button.text !== ""; text: button.text; color: button.primary ? colors.accentText : button.checked ? colors.accent : colors.text; font { pixelSize: 12; weight: Font.Medium } Layout.alignment: Qt.AlignCenter }
        }
    }
    component Field: Controls.TextField {
        id: field
        implicitHeight: 40; leftPadding: 12; rightPadding: 12
        color: colors.text; placeholderTextColor: colors.faint; selectionColor: colors.accent; selectedTextColor: colors.accentText
        font.pixelSize: 13
        background: Rectangle { radius: 7; color: colors.surface; border.color: field.activeFocus ? colors.accent : colors.border; border.width: field.activeFocus ? 2 : 1 }
    }
    component Combo: Controls.ComboBox {
        id: combo
        implicitHeight: 38; implicitWidth: 152; leftPadding: 12; rightPadding: 30
        background: Rectangle { radius: 7; color: combo.hovered ? colors.hover : colors.surface; border.color: combo.activeFocus ? colors.accent : colors.border }
        contentItem: Controls.Label { text: combo.displayText; color: colors.text; verticalAlignment: Text.AlignVCenter; elide: Text.ElideRight; font.pixelSize: 12 }
        indicator: AppIcon { name: "chevron"; ink: colors.muted; x: combo.width - 26; y: (combo.height-height)/2; width: 18; height: 18 }
        delegate: Controls.ItemDelegate {
            id: comboItem
            required property var modelData
            required property int index
            width: combo.width
            contentItem: Controls.Label { text: comboItem.modelData; color: colors.text; font.pixelSize: 12 }
            background: Rectangle { color: comboItem.highlighted ? colors.selection : colors.surface }
            highlighted: combo.highlightedIndex === index
        }
        popup: Controls.Popup {
            y: combo.height + 5; width: combo.width; padding: 5
            implicitHeight: contentItem.implicitHeight + 10
            background: Rectangle { radius: 8; color: colors.surface; border.color: colors.border }
            contentItem: ListView { clip: true; implicitHeight: contentHeight; model: combo.popup.visible ? combo.delegateModel : null; currentIndex: combo.highlightedIndex }
        }
    }
    component NavItem: Controls.AbstractButton {
        id: nav
        property string symbol: "grid"
        property bool selected: false
        property string badge: ""
        Layout.fillWidth: true; Layout.minimumHeight: 42; implicitHeight: 42; hoverEnabled: true
        Accessible.name: text
        background: Rectangle { radius: 7; color: nav.selected ? colors.selection : nav.hovered ? colors.hover : "transparent"; border.color: nav.activeFocus ? colors.accent : "transparent" }
        contentItem: RowLayout {
            spacing: 12
            AppIcon { name: nav.symbol; ink: nav.selected ? colors.accent : colors.muted; Layout.preferredWidth: 18; Layout.preferredHeight: 18; Layout.leftMargin: 12 }
            Controls.Label { id: navLabel; visible: !root.compactNavigation; text: nav.text; elide: Text.ElideRight; color: nav.selected ? colors.accent : colors.muted; font { pixelSize: 13; weight: nav.selected ? Font.DemiBold : Font.Normal } Layout.fillWidth: true; Layout.minimumWidth: 0 }
            Controls.Label { visible: !root.compactNavigation && nav.badge !== ""; text: nav.badge; color: colors.faint; font.pixelSize: 11; Layout.rightMargin: 12 }
        }
        Controls.ToolTip.visible: hovered && (root.compactNavigation || navLabel.truncated)
        Controls.ToolTip.text: text
    }
    component Panel: Rectangle { color: colors.surface; radius: 10; border.color: colors.border }
    component Modal: Controls.Dialog {
        id: modal
        parent: Controls.Overlay.overlay
        anchors.centerIn: parent
        width: Math.min(root.width - 48, 560)
        modal: true; padding: 24
        closePolicy: Controls.Popup.CloseOnEscape
        background: Rectangle { radius: 13; color: colors.surface; border.color: colors.border }
        header: Controls.Label { text: modal.title; color: colors.text; font { pixelSize: 21; weight: Font.DemiBold } padding: 24; bottomPadding: 16; elide: Text.ElideRight }
        Controls.Overlay.modal: Rectangle { color: colors.dark ? "#99000000" : "#663e4655" }
    }

    pageStack.initialPage: Kirigami.Page {
        id: libraryPage
        padding: 0
        background: Rectangle { color: colors.window }
        RowLayout {
            anchors.fill: parent; spacing: 0
            Rectangle {
                id: sidebar
                Layout.preferredWidth: root.compactNavigation ? 66 : 214
                Layout.fillHeight: true; color: colors.sidebar
                Rectangle { anchors.right: parent.right; width: 1; height: parent.height; color: colors.border }
                ColumnLayout {
                    anchors { fill: parent; margins: root.compactNavigation ? 10 : 16 }
                    spacing: 4
                    RowLayout {
                        Layout.topMargin: 12; Layout.bottomMargin: 30; Layout.leftMargin: root.compactNavigation ? 2 : 10
                        Item {
                            implicitWidth: 26; implicitHeight: 26
                            Rectangle { width: 20; height: 20; radius: 10; anchors.centerIn: parent; color: "transparent"; border.width: 1.8; border.color: colors.text }
                            Rectangle { width: 28; height: 8; radius: 4; anchors.centerIn: parent; rotation: -35; color: "transparent"; border.width: 1.6; border.color: colors.text }
                        }
                        Controls.Label { visible: !root.compactNavigation; text: "Orbit"; color: colors.text; font { pixelSize: 23; weight: Font.DemiBold } Layout.leftMargin: 8 }
                    }
                    NavItem { text: "Library"; symbol: "grid"; selected: root.section === "library" && root.sourceFilter === "all"; badge: root.library.games.length.toString(); onClicked: root.changeSection("library") }
                    NavItem { text: "Favorites"; symbol: "star"; selected: root.section === "favorites"; badge: root.library.games.filter(g=>g.favorite).length.toString(); onClicked: root.changeSection("favorites") }
                    NavItem { text: "Recently played"; symbol: "clock"; selected: root.section === "recent"; onClicked: root.changeSection("recent") }
                    Controls.Label { visible: !root.compactNavigation; text: "LIBRARIES"; color: colors.faint; font { pixelSize: 10; weight: Font.Medium; letterSpacing: 1.2 } Layout.leftMargin: 12; Layout.topMargin: 28; Layout.bottomMargin: 8 }
                    Controls.ScrollView {
                        id: libraryNavigation
                        Layout.fillWidth: true; Layout.fillHeight: true; Layout.minimumHeight: 80
                        clip: true; contentWidth: availableWidth
                        Controls.ScrollBar.horizontal.policy: Controls.ScrollBar.AlwaysOff
                        ColumnLayout {
                            width: libraryNavigation.availableWidth; spacing: 4
                            Repeater {
                                model: root.library.providers.filter(p=>p.available && p.enabled)
                                delegate: NavItem {
                                    required property var modelData
                                    text: modelData.name; symbol: "app"; badge: modelData.count.toString()
                                    selected: root.section === "library" && root.sourceFilter === modelData.id
                                    onClicked: { root.section = "library"; root.sourceFilter = modelData.id; }
                                }
                            }
                        }
                    }
                    NavItem { text: "Sources"; symbol: "folder"; selected: root.section === "sources"; onClicked: root.changeSection("sources") }
                    NavItem { id: appearanceNavigation; text: "Appearance"; symbol: "settings"; selected: root.section === "appearance"; onClicked: root.changeSection("appearance") }
                    Controls.Label { visible: !root.compactNavigation; text: backend.demo ? "Preview mode" : "Orbit " + Qt.application.version; color: colors.faint; font.pixelSize: 11; Layout.leftMargin: 12; Layout.topMargin: 14; Layout.bottomMargin: 5 }
                }
            }
            ColumnLayout {
                Layout.fillWidth: true; Layout.fillHeight: true
                Layout.margins: root.compactNavigation ? 24 : 32
                spacing: 22
                RowLayout {
                    id: headerRow
                    Layout.fillWidth: true; spacing: 10
                    ColumnLayout {
                        spacing: 7
                        Controls.Label { text: root.sectionTitle; color: colors.text; font { pixelSize: 28; weight: Font.DemiBold; letterSpacing: -0.5 } }
                        Controls.Label { text: root.section === "sources" ? "Connect and manage your launchers." : root.section === "appearance" ? "A simple space that feels like yours." : root.visibleGames.length + (root.visibleGames.length === 1 ? " game" : " games") + (root.sourceFilter === "all" ? " in your library" : " from " + root.providerName(root.sourceFilter)); color: colors.muted; font.pixelSize: 12 }
                    }
                    Item { Layout.fillWidth: true }
                    Controls.BusyIndicator { running: backend.busy || backend.artwork_busy; visible: running; hoverEnabled: true; implicitWidth: 26; implicitHeight: 26; Accessible.name: backend.busy ? "Scanning library" : "Fetching artwork"; Controls.ToolTip.visible: hovered; Controls.ToolTip.text: Accessible.name }
                    Button { symbol: colors.dark ? "sun" : "moon"; quiet: true; Accessible.name: colors.dark ? "Switch to light theme" : "Switch to dark theme"; onClicked: root.save({theme:colors.dark ? "light" : "dark"}); Controls.ToolTip.visible: hovered; Controls.ToolTip.text: Accessible.name }
                    Button { symbol: "refresh"; quiet: true; enabled: !backend.busy; Accessible.name: "Refresh library"; onClicked: backend.refresh(); Controls.ToolTip.visible: hovered; Controls.ToolTip.text: "Refresh · Ctrl+R" }
                    Button { id: addButton; text: "Add game"; symbol: "plus"; primary: true; onClicked: root.openAdd() }
                }
                Panel {
                    visible: backend.message !== ""; Layout.fillWidth: true; implicitHeight: messageRow.implicitHeight + 22
                    RowLayout {
                        id: messageRow
                        anchors { left: parent.left; right: parent.right; top: parent.top; margins: 11 }
                        Controls.Label { text: backend.message; color: colors.muted; wrapMode: Text.Wrap; Layout.fillWidth: true; font.pixelSize: 12 }
                        Button { symbol: "close"; quiet: true; Accessible.name: "Dismiss message"; onClicked: backend.message = "" }
                    }
                }
                StackLayout {
                    Layout.fillWidth: true; Layout.fillHeight: true
                    currentIndex: root.section === "sources" ? 1 : root.section === "appearance" ? 2 : 0
                    ColumnLayout {
                        spacing: 22
                        RowLayout {
                            Layout.fillWidth: true; spacing: 10
                            Field {
                                id: search; Layout.fillWidth: true; leftPadding: 37
                                placeholderText: "Search games"; Accessible.name: "Search games"
                                onTextChanged: root.query = text
                                AppIcon { x: 12; anchors.verticalCenter: parent.verticalCenter; width: 16; height: 16; name: "search"; ink: colors.faint }
                            }
                            Combo { model: root.sourceFilter === "roblox" ? ["Weekly playtime", "Name", "Recently played"] : ["Name", "Recently played"]; currentIndex: model.indexOf(root.sortBy); onActivated: root.sortBy = currentText; Accessible.name: "Sort games" }
                            RowLayout {
                                spacing: 2
                                Button { symbol: "grid"; quiet: true; checked: root.prefs.view === "grid"; Accessible.name: "Grid view"; onClicked: root.save({view:"grid"}); Controls.ToolTip.visible: hovered; Controls.ToolTip.text: "Grid view" }
                                Button { symbol: "list"; quiet: true; checked: root.prefs.view === "list"; Accessible.name: "List view"; onClicked: root.save({view:"list"}); Controls.ToolTip.visible: hovered; Controls.ToolTip.text: "List view" }
                            }
                        }
                        Combo {
                            visible: root.sourceFilter === "steam"
                            model: ["All Steam games", "Installed games", "Non-Steam shortcuts"]
                            implicitWidth: 205
                            Accessible.name: "Steam game type"
                            currentIndex: ["all", "installed", "shortcuts"].indexOf(root.steamFilter)
                            onActivated: root.steamFilter = ["all", "installed", "shortcuts"][currentIndex]
                        }
                        Item {
                            Layout.fillWidth: true; Layout.fillHeight: true
                            GridView {
                                id: grid
                                anchors.fill: parent; clip: true
                                model: root.visibleGames
                                property int columns: Math.max(2, Math.floor(width / (root.prefs.density === "compact" ? 180 : 220)))
                                cellWidth: root.prefs.view === "list" ? width : width / columns
                                cellHeight: root.prefs.view === "list" ? 77 : root.prefs.density === "compact" ? 220 : 250
                                boundsBehavior: Flickable.StopAtBounds
                                Controls.ScrollBar.vertical: Controls.ScrollBar { }
                                delegate: Loader {
                                    id: gameLoader
                                    required property var modelData
                                    width: grid.cellWidth - (root.prefs.view === "list" ? 0 : 16)
                                    height: grid.cellHeight - 16
                                    sourceComponent: root.prefs.view === "list" ? listComponent : cardComponent
                                    Component {
                                        id: cardComponent
                                        GameCard { game: gameLoader.modelData; theme: colors; onClicked: root.openGame(game); onFavoriteRequested: backend.favorite(game.id); onPlayRequested: backend.launch(game.id) }
                                    }
                                    Component {
                                        id: listComponent
                                        Controls.AbstractButton {
                                            id: listRow
                                            hoverEnabled: true
                                            Accessible.name: gameLoader.modelData.title
                                            onClicked: root.openGame(gameLoader.modelData)
                                            background: Rectangle { color: listRow.hovered ? colors.elevated : colors.surface; radius: 8; border.color: listRow.activeFocus ? colors.accent : colors.border }
                                            contentItem: RowLayout {
                                                spacing: 14
                                                GameArtwork { game: gameLoader.modelData; theme: colors; thumbnail: true; Layout.preferredWidth: 38; Layout.preferredHeight: 38; Layout.leftMargin: 12 }
                                                ColumnLayout { Layout.fillWidth: true; spacing: 4
                                                    Controls.Label { text: gameLoader.modelData.title; color: colors.text; font { pixelSize: 13; weight: Font.Medium } elide: Text.ElideRight; Layout.fillWidth: true }
                                                    Controls.Label { text: gameLoader.modelData.subtitle.split(" · ").slice(0, 2).join(" · "); visible: gameLoader.modelData.provider === "roblox" || gameLoader.modelData.id.startsWith("steam:shortcut:"); color: colors.muted; font.pixelSize: 11; elide: Text.ElideRight; Layout.fillWidth: true }
                                                }
                                                Controls.Label { text: root.providerName(gameLoader.modelData.provider); color: colors.muted; font.pixelSize: 12; visible: root.width > 900 }
                                                Button { symbol: "star"; quiet: true; checked: gameLoader.modelData.favorite; Accessible.name: "Toggle favorite"; onClicked: backend.favorite(gameLoader.modelData.id) }
                                                Button { symbol: "play"; quiet: true; Accessible.name: (gameLoader.modelData.launch_notice ? "Open launcher for " : "Launch ") + gameLoader.modelData.title; onClicked: backend.launch(gameLoader.modelData.id); Layout.rightMargin: 10 }
                                            }
                                        }
                                    }
                                }
                            }
                            ColumnLayout {
                                anchors.centerIn: parent; width: Math.min(parent.width-48, 380); spacing: 14
                                visible: root.visibleGames.length === 0 && !backend.busy
                                AppIcon { name: root.section === "favorites" ? "star" : "grid"; ink: colors.faint; Layout.preferredWidth: 36; Layout.preferredHeight: 36; Layout.alignment: Qt.AlignHCenter }
                                Controls.Label { text: root.query !== "" ? "No matching games" : root.section === "favorites" ? "Your favorites belong here" : root.section === "recent" ? "Nothing played yet" : "Welcome to your library"; color: colors.text; font { pixelSize: 20; weight: Font.Medium } Layout.alignment: Qt.AlignHCenter }
                                Controls.Label { text: root.query !== "" ? "Try another title or clear your search." : root.section === "favorites" ? "Use the star on a game to keep it close." : root.section === "recent" ? "Games you launch will appear here." : "Connect a launcher or add a game to get started."; color: colors.muted; wrapMode: Text.Wrap; horizontalAlignment: Text.AlignHCenter; Layout.fillWidth: true }
                                Button { text: root.query !== "" ? "Clear search" : "Manage sources"; Layout.alignment: Qt.AlignHCenter; Layout.topMargin: 7; onClicked: { if (root.query !== "") search.clear(); else root.changeSection("sources"); } }
                            }
                        }
                    }
                    Controls.ScrollView {
                        id: sourcesScroll
                        clip: true; contentWidth: availableWidth
                        ColumnLayout {
                            width: sourcesScroll.availableWidth; spacing: 14
                            Repeater {
                                model: root.library.providers
                                delegate: Panel {
                                    id: providerPanel
                                    required property var modelData
                                    Layout.fillWidth: true; implicitHeight: sourceColumn.implicitHeight + 36
                                    ColumnLayout {
                                        id: sourceColumn
                                        anchors { left: parent.left; right: parent.right; top: parent.top; margins: 18 } spacing: 10
                                        RowLayout {
                                            spacing: 14
                                            Rectangle { color: colors.elevated; radius: 8; implicitWidth: 40; implicitHeight: 40
                                                AppIcon { anchors.centerIn: parent; name: "app"; ink: colors.muted; width: 20; height: 20 }
                                            }
                                            ColumnLayout {
                                                Layout.fillWidth: true; spacing: 5
                                                Controls.Label { text: providerPanel.modelData.name; color: colors.text; font { pixelSize: 15; weight: Font.DemiBold } }
                                                Controls.Label { text: !providerPanel.modelData.available ? "Unavailable on this platform" : !providerPanel.modelData.enabled ? "Disabled" : providerPanel.modelData.count + " installed games"; color: colors.muted; font.pixelSize: 12 }
                                            }
                                            Button { text: "Configure"; visible: providerPanel.modelData.available; enabled: !backend.busy; onClicked: root.openSource(providerPanel.modelData.id) }
                                            Controls.Switch {
                                                id: sourceSwitch
                                                checked: providerPanel.modelData.enabled; enabled: providerPanel.modelData.available && !backend.busy
                                                Accessible.name: "Enable " + providerPanel.modelData.name
                                                indicator: Rectangle { implicitWidth: 38; implicitHeight: 22; x: sourceSwitch.leftPadding; y: (sourceSwitch.height-height)/2; radius: 11; color: sourceSwitch.checked ? colors.accent : colors.border; opacity: sourceSwitch.enabled ? 1 : 0.5
                                                    Rectangle { x: sourceSwitch.checked ? 19 : 3; y: 3; width: 16; height: 16; radius: 8; color: sourceSwitch.checked ? colors.accentText : colors.surface }
                                                }
                                                onToggled: {
                                                    let sources = JSON.parse(JSON.stringify(root.prefs.sources));
                                                    if (!sources[providerPanel.modelData.id]) sources[providerPanel.modelData.id] = {paths:[],command:[]};
                                                    sources[providerPanel.modelData.id].enabled = checked;
                                                    root.save({sources:sources});
                                                }
                                            }
                                        }
                                        Controls.Label { text: providerPanel.modelData.note; visible: text !== ""; Layout.fillWidth: true; wrapMode: Text.Wrap; color: colors.muted; font.pixelSize: 12 }
                                        Controls.Label { text: providerPanel.modelData.errors.join("\n"); visible: text !== ""; Layout.fillWidth: true; wrapMode: Text.WrapAnywhere; color: colors.warning; font.pixelSize: 12 }
                                    }
                                }
                            }
                            Panel {
                                Layout.fillWidth: true; implicitHeight: extensionColumn.implicitHeight + 40
                                ColumnLayout {
                                    id: extensionColumn
                                    anchors { left: parent.left; right: parent.right; top: parent.top; margins: 20 } spacing: 12
                                    Controls.Label { text: "Add another integration"; color: colors.text; font { pixelSize: 16; weight: Font.DemiBold } }
                                    Controls.Label { text: "Add a JSON provider manifest to your providers folder, then refresh. Custom games can also launch desktop apps and emulators."; Layout.fillWidth: true; wrapMode: Text.Wrap; color: colors.muted }
                                    Controls.Label { text: backend.config_path() + "/providers"; Layout.fillWidth: true; wrapMode: Text.WrapAnywhere; color: colors.faint; font.pixelSize: 11 }
                                    Button { text: "Open providers folder"; symbol: "folder"; onClicked: { const url = backend.prepare_config(); if (url) Qt.openUrlExternally(url + "providers/"); } }
                                }
                            }
                        }
                    }
                    Controls.ScrollView {
                        id: appearanceScroll
                        clip: true; contentWidth: availableWidth
                        ColumnLayout {
                            width: appearanceScroll.availableWidth; spacing: 24
                            Panel {
                                Layout.fillWidth: true; implicitHeight: themeColumn.implicitHeight + 48
                                ColumnLayout {
                                    id: themeColumn
                                    anchors { left: parent.left; right: parent.right; top: parent.top; margins: 24 } spacing: 17
                                    Controls.Label { text: "Theme"; color: colors.text; font { pixelSize: 16; weight: Font.DemiBold } }
                                    RowLayout {
                                        spacing: 16; Layout.fillWidth: true
                                        Repeater {
                                            model: ["dark", "light"]
                                            delegate: Controls.AbstractButton {
                                                id: themeChoice
                                                required property string modelData
                                                Layout.fillWidth: true; implicitHeight: 146
                                                Accessible.name: modelData === "dark" ? "Dark theme" : "Light theme"
                                                onClicked: root.save({theme:modelData})
                                                background: Rectangle { radius: 9; color: colors.surface; border.width: root.prefs.theme === themeChoice.modelData || themeChoice.activeFocus ? 2 : 1; border.color: root.prefs.theme === themeChoice.modelData || themeChoice.activeFocus ? colors.accent : colors.border }
                                                contentItem: ColumnLayout {
                                                    spacing: 11; anchors { fill: parent; margins: 12 }
                                                    Rectangle {
                                                        Layout.fillWidth: true; Layout.fillHeight: true; radius: 5; color: themeChoice.modelData === "dark" ? "#111216" : "#f5f6f8"; border.color: themeChoice.modelData === "dark" ? "#30343e" : "#dde2eb"
                                                        Rectangle { x: 0; height: parent.height; width: parent.width*0.23; color: themeChoice.modelData === "dark" ? "#24272f" : "#e9edf4" }
                                                        Row { x: parent.width*0.3; y: 20; spacing: 7
                                                            Repeater { model: 3; Rectangle { width: (themeChoice.width-75)/5; height: 42; radius: 3; color: themeChoice.modelData === "dark" ? "#2e3341" : "#d9e1f1" } }
                                                        }
                                                    }
                                                    RowLayout { Layout.fillWidth: true
                                                        AppIcon { name: themeChoice.modelData === "dark" ? "moon" : "sun"; ink: colors.muted; Layout.preferredWidth: 16; Layout.preferredHeight: 16 }
                                                        Controls.Label { text: themeChoice.modelData === "dark" ? "Dark" : "Light"; color: colors.text; font.pixelSize: 13; Layout.fillWidth: true }
                                                        Controls.Label { text: root.prefs.theme === themeChoice.modelData ? "Selected" : ""; color: colors.accent; font.pixelSize: 11 }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            Panel {
                                Layout.fillWidth: true; implicitHeight: 108
                                RowLayout {
                                    anchors { fill: parent; margins: 24 } spacing: 18
                                    ColumnLayout { Layout.fillWidth: true; spacing: 8
                                        Controls.Label { text: "Card size"; color: colors.text; font { pixelSize: 16; weight: Font.DemiBold } }
                                        Controls.Label { text: "Choose how much fits in your library."; color: colors.muted; font.pixelSize: 12 }
                                    }
                                    Combo { model: ["Comfortable", "Compact"]; currentIndex: root.prefs.density === "compact" ? 1 : 0; Accessible.name: "Card size"; onActivated: root.save({density:currentIndex === 1 ? "compact" : "comfortable"}) }
                                }
                            }
                            Panel {
                                Layout.fillWidth: true; implicitHeight: artworkSettings.implicitHeight + 48
                                ColumnLayout {
                                    id: artworkSettings
                                    anchors { left: parent.left; right: parent.right; top: parent.top; margins: 24 }
                                    spacing: 12
                                    RowLayout {
                                        Layout.fillWidth: true
                                        ColumnLayout {
                                            Layout.fillWidth: true; spacing: 7
                                            Controls.Label { text: "Library artwork"; color: colors.text; font { pixelSize: 16; weight: Font.DemiBold } }
                                            Controls.Label { text: "Download missing covers"; color: colors.muted; font.pixelSize: 12 }
                                        }
                                        Controls.CheckBox { checked: root.prefs.online_artwork; Accessible.name: "Download missing artwork"; onToggled: root.save({online_artwork:checked}) }
                                    }
                                    Controls.Label { text: "Uses launcher artwork, Modrinth galleries and official Minecraft update art. Downloads are cached for offline use. You can choose a cover in any game's details."; Layout.fillWidth: true; wrapMode: Text.Wrap; color: colors.faint; font.pixelSize: 12 }
                                    RowLayout {
                                        Layout.fillWidth: true
                                        Controls.Label { text: "Minecraft covers"; color: colors.muted; font.pixelSize: 12; Layout.fillWidth: true }
                                        Combo { model: ["Game drops & updates", "Modpack galleries"]; implicitWidth: 210; currentIndex: root.prefs.minecraft_artwork === "modpacks" ? 1 : 0; Accessible.name: "Minecraft artwork preference"; onActivated: root.save({minecraft_artwork:currentIndex === 1 ? "modpacks" : "updates"}) }
                                    }
                                }
                            }
                            Controls.Label { text: "Changes are saved automatically.\n\nCtrl+F  Search     Ctrl+R  Refresh     Ctrl+N  Add game     Ctrl+,  Appearance"; Layout.fillWidth: true; wrapMode: Text.Wrap; color: colors.faint; font.pixelSize: 12 }
                        }
                    }
                }
            }
        }
    }

    Modal {
        id: details
        title: root.selectedGame ? root.selectedGame.title : "Game details"
        contentItem: ColumnLayout {
            spacing: 16
            Controls.Label { text: root.selectedGame ? root.selectedGame.subtitle : ""; color: colors.muted; Layout.fillWidth: true; wrapMode: Text.Wrap }
            Controls.Label { text: root.selectedGame && root.selectedGame.provider === "steam" ? "Starts this game with Steam in the background, keeping your launch options and compatibility settings. Login or update prompts may still appear." : root.selectedGame && root.selectedGame.provider === "desktop" ? "Starts this game using its desktop entry, keeping its arguments and working folder." : root.selectedGame && ["prism", "multimc", "polymc", "atlauncher"].indexOf(root.selectedGame.provider) >= 0 ? "Starts the instance directly through " + root.providerName(root.selectedGame.provider) + ". Its account, Java, console and mod loader settings still apply." : root.selectedGame && root.selectedGame.provider === "roblox" ? "Joins the experience directly, skipping Roblox's home screen. Login or update prompts may still appear." : root.selectedGame && root.selectedGame.provider === "itch" ? "Native games start without itch’s main window on recent versions. Required prompts and other game types open the app." : root.selectedGame && ["battlenet", "ubisoft"].indexOf(root.selectedGame.provider) >= 0 ? "Requests this game directly. Launcher login, updates or other required UI may still appear." : "Opens with " + (root.selectedGame ? root.providerName(root.selectedGame.provider) : ""); color: colors.faint; font.pixelSize: 12; Layout.fillWidth: true; wrapMode: Text.Wrap }
            Controls.Label { text: root.selectedGame ? root.selectedGame.launch_notice || "" : ""; visible: text !== ""; color: colors.muted; font.pixelSize: 12; Layout.fillWidth: true; wrapMode: Text.Wrap }
            GameArtwork { game: root.selectedGame; theme: colors; Layout.fillWidth: true; Layout.preferredHeight: 180 }
            RowLayout {
                Layout.fillWidth: true
                Controls.Label { text: root.selectedGame && root.selectedGame.art ? root.selectedGame.art.source : ""; color: colors.faint; font.pixelSize: 11; Layout.fillWidth: true; elide: Text.ElideRight }
                Button { text: "Choose cover"; symbol: "folder"; quiet: true; onClicked: { filePicker.target = "cover"; filePicker.gameId = root.selectedGame.id; filePicker.nameFilters = ["Images (*.png *.jpg *.jpeg *.webp *.svg *.ico)"]; filePicker.open(); } }
                Button { text: "Reset"; quiet: true; visible: root.selectedGame !== null && root.prefs.artwork_overrides[root.selectedGame.id] !== undefined; onClicked: backend.set_artwork(root.selectedGame.id, "") }
            }
            RowLayout { spacing: 10
                Button { text: root.selectedGame && root.selectedGame.launch_notice ? "Open launcher" : "Play"; symbol: "play"; primary: true; onClicked: { backend.launch(root.selectedGame.id); details.close(); } }
                Button { text: root.selectedGame && root.library.games.some(g=>g.id === root.selectedGame.id && g.favorite) ? "Favorited" : "Favorite"; symbol: "star"; onClicked: backend.favorite(root.selectedGame.id) }
                Item { Layout.fillWidth: true }
                Button { visible: root.selectedGame !== null && root.selectedGame.provider === "custom"; text: "Remove"; onClicked: { backend.remove_game(root.selectedGame.id); details.close(); } }
                Button { text: "Close"; quiet: true; onClicked: details.close() }
            }
        }
    }
    Modal {
        id: sourceDialog
        property string sourceId: ""
        property string error: ""
        property string extensionPath: ""
        title: "Configure " + root.providerName(sourceId)
        height: Math.min(root.height - 48, implicitHeight)
        contentItem: Controls.ScrollView {
            id: sourceScroll
            implicitHeight: sourceForm.implicitHeight
            contentWidth: availableWidth
            ColumnLayout {
                id: sourceForm
                width: sourceScroll.availableWidth
                spacing: 12
                Controls.Label { text: ({steam:"Add Steam folders or extra libraries containing steamapps. Installed games and the most recent account’s non-Steam shortcuts are imported automatically. Play keeps Steam in the background; login and updates may still need a window.",desktop:"Find standalone games and emulators in Linux application menus, including Flatpak. Add application folders or individual .desktop files. Existing store launchers and their game links are skipped.",lutris:"Add Lutris data folders or pga.db files.",prism:"Add Prism data folders containing prismlauncher.cfg. Play skips Prism's main window.",multimc:"Add MultiMC data folders containing multimc.cfg and instances. Portable installs can use the MultiMC executable in that folder or a command override. Play starts the instance directly.",polymc:"Add PolyMC data folders containing polymc.cfg. Native and Flatpak data folders are detected automatically; portable installs can use a command override. Play starts the instance directly.",atlauncher:"Add ATLauncher data folders containing instances and instance.json files. Portable executable or JAR installs can use a command override. Play starts the instance directly; setup and error prompts may appear.",nile:"Add a Nile data folder named nile containing installed.json and library.json, or installed.json itself. Sign in with Nile and sync its library first. For Wine/prefix options, use an override such as [\"nile\",\"launch\",\"--wine-prefix\",\"/path/to/prefix\"].",modrinth:"Add Modrinth data folders containing app.db, or select an app.db path. Custom app directories are read from the database.",heroic:"Add Heroic configuration folders containing legendaryConfig, gog_store or nile_config. Select a command override for AppImages or portable installations.",legendary:"Add Legendary configuration folders or installed.json files. Orbit preserves the selected configuration when launching.",roblox:"Connect the bundled browser extension to sync your top five from last week. Your login stays in Chrome or Chromium. Play skips Roblox’s home screen.",epic:"Add Epic Games Launcher manifest folders (.item files).",gog:"Add GOG game folders or libraries containing goggame-*.info files.",battlenet:"Windows installations are detected automatically. Add Battle.net Agent folders or product.db files for other locations. Play requests the selected game directly; login and update prompts may appear.",ubisoft:"Windows installations are detected automatically. Add game folders or libraries containing uplay_install.state. Play requests the selected game directly; login and update prompts may appear.",itch:"Add itch or kitch data folders containing db/butler.db, or that database file. Requires recent itch-setup with --run-game support. Native games skip the main window; older butler versions and required prompts open the app."})[sourceDialog.sourceId] || "This provider reads its JSON manifest."; Layout.fillWidth: true; wrapMode: Text.Wrap; color: colors.muted; font.pixelSize: 12 }
                Button { text: "Set up browser sync"; visible: sourceDialog.sourceId === "roblox"; onClicked: sourceDialog.extensionPath = backend.prepare_roblox_extension() }
                Controls.Label { text: "In Chrome or Chromium, enable Developer mode on the Extensions page, choose Load unpacked, and select this folder. Then visit Roblox while signed in."; visible: sourceDialog.extensionPath !== ""; Layout.fillWidth: true; wrapMode: Text.Wrap; color: colors.muted; font.pixelSize: 12 }
                RowLayout { visible: sourceDialog.extensionPath !== ""; Layout.fillWidth: true
                    Field { id: extensionFolder; text: sourceDialog.extensionPath; readOnly: true; Layout.fillWidth: true; Accessible.name: "Prepared extension folder" }
                    Button { text: "Copy path"; onClicked: { extensionFolder.selectAll(); extensionFolder.copy(); extensionFolder.deselect(); } }
                }
                Controls.Label { text: sourceDialog.sourceId === "roblox" ? "Report path (optional)" : "Additional paths"; color: colors.text; font.pixelSize: 12 }
                Controls.TextArea {
                    id: sourcePaths; Layout.fillWidth: true; implicitHeight: 100
                    Accessible.name: sourceDialog.sourceId === "roblox" ? "Roblox report path" : "Additional library paths"
                    color: colors.text; placeholderTextColor: colors.faint; font.pixelSize: 12
                    placeholderText: "One absolute path per line"; wrapMode: Text.WrapAnywhere; padding: 12
                    background: Rectangle { radius: 7; color: colors.window; border.color: sourcePaths.activeFocus ? colors.accent : colors.border }
                }
                Button { text: sourceDialog.sourceId === "roblox" ? "Choose report" : "Browse folder"; symbol: "folder"; onClicked: { if (sourceDialog.sourceId === "roblox") { filePicker.target = "roblox-report"; filePicker.nameFilters = ["Roblox report (*.json)"]; filePicker.open(); } else { folderPicker.target = "source"; folderPicker.open(); } } }
                Controls.Label { text: "Leave the path empty to watch Downloads/orbit-roblox-top-games.json. If downloads are redirected, choose the report shown in the extension's popup. Orbit refreshes when it changes."; visible: sourceDialog.sourceId === "roblox"; Layout.fillWidth: true; wrapMode: Text.Wrap; color: colors.faint; font.pixelSize: 11 }
                Controls.Label { text: "Launcher command (optional)"; color: colors.text; font.pixelSize: 12 }
                Field { id: sourceCommand; Layout.fillWidth: true; placeholderText: '["C:/Apps/Launcher.exe"]'; Accessible.name: "Launcher command" }
                Controls.Label { text: sourceDialog.sourceId === "desktop" ? "Leave [] to use gio launch. An override receives the desktop file path as one argument." : "Use a JSON array of executable and arguments. Leave [] to detect the launcher automatically."; Layout.fillWidth: true; wrapMode: Text.Wrap; color: colors.faint; font.pixelSize: 11 }
                Controls.Label { text: sourceDialog.error; visible: text !== ""; Layout.fillWidth: true; wrapMode: Text.Wrap; color: colors.warning }
                RowLayout {
                    Item { Layout.fillWidth: true }
                    Button { text: "Cancel"; onClicked: sourceDialog.close() }
                    Button { text: "Save"; primary: true; onClicked: {
                        try {
                            const command = JSON.parse(sourceCommand.text || "[]");
                            if (!Array.isArray(command) || !command.every(s=>typeof s === "string")) throw Error("Command must be a JSON array of strings.");
                            const sources = JSON.parse(JSON.stringify(root.prefs.sources));
                            sources[sourceDialog.sourceId] = {enabled:sources[sourceDialog.sourceId] ? sources[sourceDialog.sourceId].enabled : true, paths:sourcePaths.text.split("\n").map(p=>p.trim()).filter(p=>p.length), command:command};
                            if (root.save({sources:sources})) sourceDialog.close(); else sourceDialog.error = backend.message;
                        } catch (e) { sourceDialog.error = e.toString(); }
                    } }
                }
            }
        }
    }
    Modal {
        id: addDialog
        property string error: ""
        title: "Add a game or app"
        contentItem: ColumnLayout {
            spacing: 10
            Controls.Label { text: "Name"; color: colors.text; font.pixelSize: 12 }
            Field { id: gameTitle; Layout.fillWidth: true; placeholderText: "Game or application name"; Accessible.name: "Name" }
            Controls.Label { text: "Executable"; color: colors.text; font.pixelSize: 12 }
            RowLayout { Layout.fillWidth: true
                Field { id: gameExecutable; Layout.fillWidth: true; placeholderText: "Executable path or command"; Accessible.name: "Executable" }
                Button { symbol: "folder"; Accessible.name: "Choose executable"; onClicked: { filePicker.target = "executable"; filePicker.nameFilters = ["All files (*)"]; filePicker.open(); } }
            }
            Controls.Label { text: "Arguments (optional)"; color: colors.text; font.pixelSize: 12 }
            Field { id: gameArgs; Layout.fillWidth: true; text: "[]"; placeholderText: '["--fullscreen"]'; Accessible.name: "Arguments" }
            Controls.Label { text: "Working folder (optional)"; color: colors.text; font.pixelSize: 12 }
            RowLayout { Layout.fillWidth: true
                Field { id: gameDirectory; Layout.fillWidth: true; placeholderText: "Use the default folder"; Accessible.name: "Working folder" }
                Button { symbol: "folder"; Accessible.name: "Choose working folder"; onClicked: { folderPicker.target = "working"; folderPicker.open(); } }
            }
            Controls.Label { text: "Cover image (optional)"; color: colors.text; font.pixelSize: 12 }
            RowLayout { Layout.fillWidth: true
                Field { id: gameArtwork; Layout.fillWidth: true; placeholderText: "Local image path"; Accessible.name: "Cover image" }
                Button { symbol: "folder"; Accessible.name: "Choose cover"; onClicked: { filePicker.target = "artwork"; filePicker.nameFilters = ["Images (*.png *.jpg *.jpeg *.webp *.svg *.ico)", "All files (*)"]; filePicker.open(); } }
            }
            Controls.Label { text: "Arguments are a JSON array, such as [\"--fullscreen\"]."; Layout.fillWidth: true; wrapMode: Text.Wrap; color: colors.faint; font.pixelSize: 11 }
            Controls.Label { text: addDialog.error; visible: text !== ""; Layout.fillWidth: true; wrapMode: Text.Wrap; color: colors.warning }
            RowLayout {
                Layout.topMargin: 8
                Item { Layout.fillWidth: true }
                Button { text: "Cancel"; onClicked: addDialog.close() }
                Button { text: "Add"; primary: true; onClicked: {
                    try {
                        const args = JSON.parse(gameArgs.text || "[]");
                        if (!Array.isArray(args) || !args.every(s=>typeof s === "string")) throw Error("Arguments must be a JSON array of strings.");
                        if (!backend.add_game(JSON.stringify({title:gameTitle.text,command:[gameExecutable.text.trim()].concat(args),directory:gameDirectory.text.trim(),artwork:gameArtwork.text.trim()}))) { addDialog.error = backend.message; return; }
                        gameTitle.clear(); gameExecutable.clear(); gameArgs.text = "[]"; gameDirectory.clear(); gameArtwork.clear();
                        addDialog.close();
                    } catch(e) { addDialog.error = e.toString(); }
                } }
            }
        }
    }
    Dialogs.FileDialog {
        id: filePicker
        property string target: "executable"
        property string gameId: ""
        title: target === "artwork" || target === "cover" ? "Choose a cover image" : target === "roblox-report" ? "Choose Roblox playtime report" : "Choose an executable"
        onAccepted: { const path = backend.local_path(selectedFile.toString()); if (target === "cover") backend.set_artwork(gameId, path); else if (target === "artwork") gameArtwork.text = path; else if (target === "roblox-report") sourcePaths.text = path; else gameExecutable.text = path; }
    }
    Dialogs.FolderDialog {
        id: folderPicker
        property string target: "source"
        title: target === "working" ? "Choose a working folder" : "Choose a library folder"
        onAccepted: {
            const path = backend.local_path(selectedFolder.toString());
            if (target === "working") gameDirectory.text = path;
            else sourcePaths.text += (sourcePaths.text.trim() ? "\n" : "") + path;
        }
    }
}
