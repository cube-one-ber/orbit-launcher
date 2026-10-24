pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic as Controls
import QtQuick.Layouts
import QtQuick.Dialogs as Dialogs

Controls.Dialog {
    id: editor
    required property var host
    required property var backendObject
    required property var theme
    readonly property var view: JSON.parse(backendObject.instance_view)
    property string tab: "mods"
    property bool browsing: false
    readonly property var content: (view.content || {})[tab] || {items:[], updates:0, error:"", unavailable:""}
    readonly property var results: view.search && view.search.kind === tab ? view.search : null
    readonly property bool working: backendObject.instance_busy
    readonly property bool preview: !!view.demo
    readonly property bool settingsTab: tab === "settings"
    readonly property string kindLabel: ({mods:"mods", resourcepacks:"resource packs", shaderpacks:"shader packs"})[tab] || "content"
    property var pendingRemoval: null
    parent: Controls.Overlay.overlay
    anchors.centerIn: parent
    width: Math.min(host.width - 32, 980)
    height: Math.min(host.height - 32, 720)
    modal: true
    padding: 20
    closePolicy: Controls.Popup.CloseOnEscape
    background: Rectangle { radius: 12; color: editor.theme.window; border.color: editor.theme.border }
    Controls.Overlay.modal: Rectangle { color: editor.theme.dark ? "#99000000" : "#663e4655" }
    function show() {
        tab = "mods";
        browsing = false;
        search.text = "";
        loadSettings();
        open();
    }
    function selectTab(value) {
        tab = value;
        browsing = false;
        search.text = "";
    }
    function browse() {
        browsing = true;
        if (!content.inspected) backendObject.inspect_content(tab);
        backendObject.search_content(tab, search.text, 0);
    }
    function loadSettings() {
        const s = view.settings;
        if (!s) return;
        instanceName.text = s.name;
        notes.text = s.notes;
        memoryOverride.checked = s.override_memory;
        minimumMemory.text = s.min_memory.toString();
        maximumMemory.text = s.max_memory.toString();
        argsOverride.checked = s.override_java_args;
        javaArgs.text = s.java_args;
    }
    function saveSettings() {
        return backendObject.save_instance_settings(JSON.stringify({
            name:instanceName.text, notes:notes.text,
            override_memory:memoryOverride.checked,
            min_memory:Number(minimumMemory.text), max_memory:Number(maximumMemory.text),
            override_java_args:argsOverride.checked, java_args:javaArgs.text
        }));
    }
    function openFolder(kind) {
        const url = backendObject.instance_folder(kind);
        if (url) Qt.openUrlExternally(url);
    }
    onClosed: { removeDialog.close(); localPicker.close(); backendObject.close_instance(); }

    component Label: Controls.Label {
        color: editor.theme.text
        font.pixelSize: 13
        textFormat: Text.PlainText
    }
    component Button: Controls.Button {
        id: button
        property bool primary: false
        implicitHeight: 36
        implicitWidth: Math.max(72, implicitContentWidth + 24)
        horizontalPadding: 12
        hoverEnabled: true
        opacity: enabled ? 1 : 0.45
        contentItem: Label { text: button.text; color: button.primary ? editor.theme.accentText : editor.theme.text; horizontalAlignment: Text.AlignHCenter; verticalAlignment: Text.AlignVCenter }
        background: Rectangle { radius: 7; color: button.primary ? editor.theme.accent : button.checked ? editor.theme.selection : button.hovered ? editor.theme.hover : editor.theme.surface; border.color: button.activeFocus ? editor.theme.accent : editor.theme.border; border.width: button.activeFocus ? 2 : 1 }
    }
    component Field: Controls.TextField {
        id: field
        implicitHeight: 38
        leftPadding: 10; rightPadding: 10
        color: editor.theme.text
        font.pixelSize: 13
        placeholderTextColor: editor.theme.faint
        selectionColor: editor.theme.accent
        selectedTextColor: editor.theme.accentText
        background: Rectangle { radius: 6; color: editor.theme.surface; border.color: field.activeFocus ? editor.theme.accent : editor.theme.border }
    }
    component Check: Controls.CheckBox {
        id: check
        contentItem: Label { text: check.text; leftPadding: check.indicator.width + check.spacing; verticalAlignment: Text.AlignVCenter }
    }
    component TextArea: Controls.TextArea {
        id: area
        color: editor.theme.text
        font.pixelSize: 13
        wrapMode: TextEdit.Wrap
        selectByMouse: true
        padding: 10
        selectionColor: editor.theme.accent
        selectedTextColor: editor.theme.accentText
        background: Rectangle { radius: 6; color: editor.theme.surface; border.color: area.activeFocus ? editor.theme.accent : editor.theme.border }
    }

    contentItem: Rectangle {
      color: editor.theme.window
      ColumnLayout {
        anchors.fill: parent
        spacing: 12
        RowLayout {
            Layout.fillWidth: true
            ColumnLayout {
                Layout.fillWidth: true
                Label { text: editor.view.title || "Manage instance"; font { pixelSize: 22; weight: Font.DemiBold } elide: Text.ElideRight; Layout.fillWidth: true }
                Label { text: editor.view.summary || ""; color: editor.theme.muted; wrapMode: Text.Wrap; Layout.fillWidth: true }
            }
            Button { text: "Open folder"; enabled: !editor.preview; onClicked: editor.openFolder("") }
            Button { text: "Close"; onClicked: editor.close() }
        }
        Label { text: editor.preview ? "Preview mode · Sample content. Changes are disabled." : "Close Minecraft before changing this instance."; color: editor.theme.muted; wrapMode: Text.Wrap; Layout.fillWidth: true }
        Label { visible: !!editor.view.modpack; text: "Modpack: " + (editor.view.modpack || "") + ". Pack updates may replace your changes."; color: editor.theme.warning; wrapMode: Text.Wrap; Layout.fillWidth: true }
        RowLayout {
            spacing: 6
            Repeater {
                model: [{id:"mods",title:"Mods"}, {id:"resourcepacks",title:"Resource packs"}, {id:"shaderpacks",title:"Shader packs"}, {id:"settings",title:"Settings"}]
                Button {
                    required property var modelData
                    text: modelData.title
                    checked: editor.tab === modelData.id
                    onClicked: editor.selectTab(modelData.id)
                    Layout.fillWidth: true
                }
            }
        }
        Controls.ScrollView {
            visible: !!editor.view.error || !!editor.view.status
            Layout.fillWidth: true
            Layout.preferredHeight: Math.min(statusLabel.implicitHeight, 76)
            contentWidth: availableWidth
            clip: true
            Label { id: statusLabel; width: parent.width; text: [editor.view.error, editor.view.status].filter(Boolean).join("\n"); color: editor.view.error ? editor.theme.warning : editor.theme.muted; wrapMode: Text.Wrap }
        }
        Controls.ProgressBar { visible: editor.working; indeterminate: true; Layout.fillWidth: true; Layout.preferredHeight: 4; Accessible.name: "Instance task in progress" }
        Label { visible: editor.working; text: "This task continues if you close this window."; color: editor.theme.faint; font.pixelSize: 12 }
        ColumnLayout {
            visible: !editor.settingsTab
            Layout.fillWidth: true
            Layout.fillHeight: true
            spacing: 10
            RowLayout {
                Layout.fillWidth: true
                Button { text: editor.browsing ? "Installed" : "Browse Modrinth"; onClicked: { if (editor.browsing) editor.browsing = false; else editor.browse(); } }
                Item { Layout.fillWidth: true }
                Button { text: "Add file"; enabled: !editor.working && !editor.preview; onClicked: { localPicker.contentKind = editor.tab; localPicker.nameFilters = editor.tab === "mods" ? ["Mods (*.jar *.zip)"] : ["Packs (*.zip)"]; localPicker.open(); } }
                Button { text: "Open folder"; enabled: !editor.preview; onClicked: editor.openFolder(editor.tab) }
            }
            Label { visible: !!editor.content.error; text: editor.content.error; color: editor.theme.warning; wrapMode: Text.Wrap; Layout.fillWidth: true }
            Label { visible: !!editor.content.unavailable; text: editor.content.unavailable; color: editor.theme.muted; wrapMode: Text.Wrap; Layout.fillWidth: true }
            RowLayout {
                visible: !editor.browsing
                Layout.fillWidth: true
                Label { text: editor.content.items.length + " " + editor.kindLabel; color: editor.theme.muted; Layout.fillWidth: true }
                Button { text: "Check for updates"; enabled: !editor.working && !editor.content.unavailable; onClicked: editor.backendObject.inspect_content(editor.tab) }
                Button { text: "Update all (" + editor.content.updates + ")"; visible: editor.content.updates > 0; enabled: !editor.working && !editor.preview; onClicked: editor.backendObject.update_content(editor.tab, "") }
            }
            RowLayout {
                visible: editor.browsing
                Layout.fillWidth: true
                Field { id: search; Layout.fillWidth: true; placeholderText: "Search " + editor.kindLabel + " on Modrinth"; Accessible.name: "Search Modrinth"; onAccepted: editor.browse() }
                Button { text: "Search"; enabled: !editor.working && !editor.content.unavailable; onClicked: editor.browse() }
            }
            Label { visible: editor.browsing; text: "Results match this Minecraft version and loader. Required mod dependencies are added when available."; color: editor.theme.faint; font.pixelSize: 12; wrapMode: Text.Wrap; Layout.fillWidth: true }
            Label { visible: editor.browsing && !!editor.results && !!editor.results.error; text: editor.results ? editor.results.error : ""; color: editor.theme.warning; wrapMode: Text.Wrap; Layout.fillWidth: true }
            ListView {
                id: installedList
                visible: !editor.browsing
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                spacing: 6
                model: editor.content.items
                Controls.ScrollBar.vertical: Controls.ScrollBar {}
                delegate: Rectangle {
                    id: installedRow
                    required property var modelData
                    width: installedList.width
                    height: 78
                    radius: 7
                    color: editor.theme.surface
                    border.color: editor.theme.border
                    RowLayout {
                        anchors { fill: parent; margins: 12 }
                        spacing: 8
                        ColumnLayout {
                            Layout.fillWidth: true
                            Layout.minimumWidth: 0
                            Label { text: installedRow.modelData.title; elide: Text.ElideRight; Layout.fillWidth: true; font.weight: Font.DemiBold; opacity: installedRow.modelData.enabled ? 1 : 0.5 }
                            Label { text: [installedRow.modelData.version, installedRow.modelData.file_name].filter(Boolean).join(" · "); color: editor.theme.faint; font.pixelSize: 11; elide: Text.ElideMiddle; Layout.fillWidth: true }
                            Label { visible: !!installedRow.modelData.update; text: "Update: " + installedRow.modelData.update; color: editor.theme.accent; font.pixelSize: 11 }
                        }
                        Button { text: installedRow.modelData.enabled ? "Disable" : "Enable"; visible: !installedRow.modelData.folder; enabled: !editor.working && !editor.preview; onClicked: editor.backendObject.toggle_content(editor.tab, installedRow.modelData.file_name) }
                        Button { text: "Update"; visible: !!installedRow.modelData.update; enabled: !editor.working && !editor.preview; onClicked: editor.backendObject.update_content(editor.tab, installedRow.modelData.file_name) }
                        Button { text: installedRow.modelData.folder ? "Folder" : "Remove"; enabled: !installedRow.modelData.folder && !editor.working && !editor.preview; onClicked: { editor.pendingRemoval = {kind:editor.tab, file:installedRow.modelData.file_name}; removeDialog.open(); } }
                    }
                }
                Label { anchors.centerIn: parent; visible: installedList.count === 0; text: "No " + editor.kindLabel + " installed.\nBrowse Modrinth or add a local file."; color: editor.theme.muted; horizontalAlignment: Text.AlignHCenter }
            }
            ListView {
                id: searchList
                visible: editor.browsing
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                spacing: 6
                model: editor.results ? editor.results.hits : []
                Controls.ScrollBar.vertical: Controls.ScrollBar {}
                delegate: Rectangle {
                    id: searchRow
                    required property var modelData
                    width: searchList.width
                    height: 100
                    radius: 7
                    color: editor.theme.surface
                    border.color: editor.theme.border
                    RowLayout {
                        anchors { fill: parent; margins: 12 }
                        spacing: 12
                        Image { source: searchRow.modelData.icon || ""; Layout.preferredWidth: 40; Layout.preferredHeight: 40; fillMode: Image.PreserveAspectFit; visible: status === Image.Ready }
                        ColumnLayout {
                            Layout.fillWidth: true
                            Layout.minimumWidth: 0
                            Label { text: searchRow.modelData.title; font.weight: Font.DemiBold; elide: Text.ElideRight; Layout.fillWidth: true }
                            Label { text: "by " + searchRow.modelData.author; color: editor.theme.faint; font.pixelSize: 11; elide: Text.ElideRight; Layout.fillWidth: true }
                            Label { text: searchRow.modelData.description; color: editor.theme.muted; wrapMode: Text.Wrap; maximumLineCount: 2; elide: Text.ElideRight; Layout.fillWidth: true; font.pixelSize: 12 }
                        }
                        Button { text: searchRow.modelData.installed ? "Installed" : "Install"; primary: !searchRow.modelData.installed; enabled: !searchRow.modelData.installed && !editor.working && !editor.preview && !editor.content.unavailable; onClicked: editor.backendObject.install_content(editor.tab, searchRow.modelData.project_id) }
                    }
                }
                Label { anchors.centerIn: parent; visible: searchList.count === 0 && !editor.working; text: editor.results ? "No matching results." : "Search Modrinth for compatible content."; color: editor.theme.muted }
            }
            RowLayout {
                visible: editor.browsing && !!editor.results && editor.results.total > 0
                Layout.fillWidth: true
                Button { text: "Previous"; enabled: !editor.working && editor.results && editor.results.offset > 0; onClicked: editor.backendObject.search_content(editor.tab, editor.results.query, Math.max(0, editor.results.offset - 20)) }
                Label { text: editor.results ? (editor.results.offset + 1) + "–" + (editor.results.offset + editor.results.hits.length) + " of " + editor.results.total : ""; color: editor.theme.muted; horizontalAlignment: Text.AlignHCenter; Layout.fillWidth: true }
                Button { text: "Next"; enabled: !editor.working && editor.results && editor.results.offset + editor.results.hits.length < editor.results.total; onClicked: editor.backendObject.search_content(editor.tab, editor.results.query, editor.results.offset + 20) }
            }
        }
        Controls.ScrollView {
            id: settingsScroll
            visible: editor.settingsTab
            Layout.fillWidth: true
            Layout.fillHeight: true
            contentWidth: availableWidth
            clip: true
            ColumnLayout {
                width: settingsScroll.availableWidth
                spacing: 12
                Label { visible: !editor.view.settings; text: editor.view.settings_error || ("Change instance settings in " + (editor.view.launcher_name || "the launcher") + "."); wrapMode: Text.Wrap; Layout.fillWidth: true; color: editor.theme.muted }
                ColumnLayout {
                    visible: !!editor.view.settings
                    Layout.fillWidth: true
                    enabled: !editor.preview && !editor.working
                    spacing: 10
                    Label { text: "Instance name" }
                    Field { id: instanceName; Layout.fillWidth: true; Accessible.name: "Instance name" }
                    Label { text: "Notes" }
                    TextArea { id: notes; Layout.fillWidth: true; Layout.preferredHeight: 80; Accessible.name: "Instance notes" }
                    Check { id: memoryOverride; text: "Use instance memory settings" }
                    RowLayout {
                        enabled: memoryOverride.checked
                        Label { text: "Minimum (MiB)" }
                        Field { id: minimumMemory; Layout.fillWidth: true; inputMethodHints: Qt.ImhDigitsOnly; validator: IntValidator { bottom: 128; top: 1048576 } Accessible.name: "Minimum memory in MiB" }
                        Label { text: "Maximum (MiB)" }
                        Field { id: maximumMemory; Layout.fillWidth: true; inputMethodHints: Qt.ImhDigitsOnly; validator: IntValidator { bottom: 128; top: 1048576 } Accessible.name: "Maximum memory in MiB" }
                    }
                    Check { id: argsOverride; text: "Use instance Java arguments" }
                    Field { id: javaArgs; enabled: argsOverride.checked; Layout.fillWidth: true; Accessible.name: "Instance Java arguments" }
                    Label { text: "Unchecked overrides use the launcher's global settings. Restart the launcher after saving if it is already open."; color: editor.theme.faint; wrapMode: Text.Wrap; Layout.fillWidth: true }
                    Button { text: "Save settings"; primary: true; onClicked: editor.saveSettings() }
                }
            }
        }
      }
    }
    Controls.Dialog {
        id: removeDialog
        parent: Controls.Overlay.overlay
        anchors.centerIn: parent
        width: Math.min(editor.host.width - 64, 440)
        modal: true
        title: "Remove file?"
        padding: 20
        background: Rectangle { radius: 10; color: editor.theme.surface; border.color: editor.theme.border }
        header: Label { text: removeDialog.title; font { pixelSize: 19; weight: Font.DemiBold } padding: 20 }
        contentItem: ColumnLayout {
            Label { text: editor.pendingRemoval ? "Delete " + editor.pendingRemoval.file + " from this instance? You can disable it to keep a copy." : ""; wrapMode: Text.Wrap; Layout.fillWidth: true }
            RowLayout {
                Layout.topMargin: 12
                Item { Layout.fillWidth: true }
                Button { text: "Cancel"; onClicked: removeDialog.close() }
                Button { text: "Remove"; enabled: !editor.working && !editor.preview; onClicked: { editor.backendObject.remove_content(editor.pendingRemoval.kind, editor.pendingRemoval.file); removeDialog.close(); } }
            }
        }
    }
    Dialogs.FileDialog {
        id: localPicker
        property string contentKind: "mods"
        title: "Add a file to this instance"
        onAccepted: editor.backendObject.add_content_file(contentKind, editor.backendObject.local_path(selectedFile.toString()))
    }
}
