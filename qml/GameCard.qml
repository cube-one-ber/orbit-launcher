pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic as Controls

Controls.AbstractButton {
    id: card
    required property var game
    required property var theme
    signal favoriteRequested()
    signal playRequested()
    hoverEnabled: true
    Accessible.name: game.title + ", " + game.provider
    background: Rectangle {
        radius: 12; color: card.hovered ? card.theme.elevated : card.theme.surface
        border.width: card.activeFocus ? 2 : 1
        border.color: card.activeFocus ? card.theme.accent : card.theme.border
        Behavior on color { ColorAnimation { duration: 120 } }
    }
    contentItem: Item {
        Rectangle {
            id: cover
            anchors { left: parent.left; right: parent.right; top: parent.top; margins: 8 }
            height: parent.height - 78; radius: 7
            color: card.theme.elevated; clip: true
            Controls.Label {
                anchors.centerIn: parent
                text: card.game.title.split(" ").filter(w=>w.length).slice(0,2).map(w=>w[0]).join("").toUpperCase()
                color: card.theme.faint; opacity: 0.6
                font { pixelSize: 32; weight: Font.Medium; letterSpacing: 3 }
            }
            Image { id: art; anchors.fill: parent; source: card.game.artwork; asynchronous: true; fillMode: Image.PreserveAspectCrop }
            Controls.AbstractButton {
                anchors { top: parent.top; right: parent.right; margins: 7 }
                width: 30; height: 30; hoverEnabled: true
                Accessible.name: card.game.favorite ? "Remove favorite" : "Add favorite"
                background: Rectangle { radius: 6; color: card.theme.surface; opacity: 0.94; border.color: card.theme.border }
                contentItem: Item { AppIcon { name: "star"; ink: card.game.favorite ? card.theme.accent : card.theme.muted; anchors.centerIn: parent; width: 16; height: 16 } }
                onClicked: card.favoriteRequested()
                Controls.ToolTip.visible: hovered
                Controls.ToolTip.text: Accessible.name
            }
            Controls.AbstractButton {
                anchors.centerIn: parent; width: 44; height: 44
                visible: card.hovered || card.activeFocus || activeFocus
                Accessible.name: "Launch " + card.game.title
                background: Rectangle { color: card.theme.accent; radius: 22 }
                contentItem: Item { AppIcon { name: "play"; ink: card.theme.accentText; anchors.centerIn: parent; width: 21; height: 21 } }
                onClicked: card.playRequested()
            }
        }
        Column {
            anchors { left: parent.left; right: parent.right; bottom: parent.bottom; margins: 15 }
            spacing: 7
            Controls.Label { width: parent.width; text: card.game.title; elide: Text.ElideRight; color: card.theme.text; font { pixelSize: 14; weight: Font.DemiBold } }
            Controls.Label { width: parent.width; text: ({steam:"Steam",prism:"Prism Launcher",lutris:"Lutris",epic:"Epic Games",gog:"GOG Galaxy",custom:"Custom"})[card.game.provider] || card.game.provider; color: card.theme.muted; font.pixelSize: 12 }
        }
    }
}
