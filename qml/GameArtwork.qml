pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic as Controls

Rectangle {
    id: artwork
    required property var game
    required property var theme
    property bool thumbnail: false
    property bool fallbackUsed: false
    readonly property string primaryUrl: game ? game.artwork || "" : ""
    readonly property string iconUrl: game && game.art && game.art.icon && (game.art.icon.startsWith("file:") || game.art.icon.startsWith("qrc:") || game.art.icon.startsWith("data:image/")) ? game.art.icon : ""
    readonly property bool iconMode: !!(fallbackUsed || (game && game.art && game.art.kind === "icon"))
    readonly property bool hasImage: picture.status === Image.Ready
    readonly property bool usingFallback: !hasImage
    readonly property bool minecraft: !!(game && (game.provider === "prism" || game.provider === "modrinth" || (game.art && game.art.minecraft_version)))
    onPrimaryUrlChanged: fallbackUsed = false
    color: theme.elevated
    radius: thumbnail ? 6 : 8
    clip: true
    Accessible.ignored: true
    gradient: Gradient {
        GradientStop { position: 0; color: artwork.theme.elevated }
        GradientStop { position: 1; color: Qt.tint(artwork.theme.elevated, Qt.rgba(0.35, 0.47, 0.7, artwork.theme.dark ? 0.13 : 0.06)) }
    }
    AppIcon {
        anchors.centerIn: parent
        width: artwork.thumbnail ? 20 : 46; height: width
        name: artwork.minecraft ? "cube" : "app"
        ink: artwork.theme.faint
        opacity: artwork.thumbnail ? 0.8 : 0.28
        visible: !artwork.hasImage
    }
    Controls.Label {
        anchors { horizontalCenter: parent.horizontalCenter; top: parent.verticalCenter; topMargin: 30 }
        visible: !artwork.hasImage && !artwork.thumbnail
        text: artwork.game ? artwork.game.title.split(" ").filter(w=>w.length).slice(0,2).map(w=>w[0]).join("").toUpperCase() : ""
        color: artwork.theme.faint
        font { pixelSize: 13; weight: Font.Medium; letterSpacing: 2 }
    }
    Image {
        id: picture
        anchors.centerIn: parent
        width: artwork.iconMode ? Math.min(artwork.width * 0.55, 100) : artwork.width
        height: artwork.iconMode ? Math.min(artwork.height * 0.65, 100) : artwork.height
        source: artwork.fallbackUsed ? artwork.iconUrl : artwork.primaryUrl
        asynchronous: true
        fillMode: artwork.iconMode ? Image.PreserveAspectFit : Image.PreserveAspectCrop
        smooth: !artwork.iconMode || !artwork.minecraft
        opacity: status === Image.Ready ? 1 : 0
        Behavior on opacity { NumberAnimation { duration: 160 } }
        onStatusChanged: {
            if (status === Image.Error && !artwork.fallbackUsed && artwork.iconUrl !== "" && artwork.iconUrl !== artwork.primaryUrl)
                artwork.fallbackUsed = true;
        }
    }
}
