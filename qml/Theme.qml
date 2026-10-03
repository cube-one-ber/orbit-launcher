import QtQuick

QtObject {
    property bool dark: true
    readonly property color window: dark ? "#111216" : "#f5f6f8"
    readonly property color sidebar: dark ? "#17181d" : "#ffffff"
    readonly property color surface: dark ? "#1c1e24" : "#ffffff"
    readonly property color elevated: dark ? "#24272f" : "#eef1f6"
    readonly property color hover: dark ? "#2b2f39" : "#e8edf6"
    readonly property color border: dark ? "#30343e" : "#dde2eb"
    readonly property color text: dark ? "#edf0f7" : "#222938"
    readonly property color muted: dark ? "#a2aabb" : "#5d687c"
    readonly property color faint: dark ? "#7d879b" : "#737e91"
    readonly property color accent: dark ? "#b4caff" : "#315fc4"
    readonly property color accentText: dark ? "#16294f" : "#ffffff"
    readonly property color selection: dark ? "#29344d" : "#eaf0ff"
    readonly property color warning: dark ? "#f4c995" : "#875314"
}
