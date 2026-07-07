pragma ComponentBehavior: Bound

import QtQuick 2.15
import QtQuick.Controls 2.15
import QtQuick.Layouts 1.15
import QtQuick.Dialogs
import "."

Popup {
    id: root
    property var backend: null
    readonly property bool hasBackend: backend !== null && backend !== undefined

    signal chooseProjectsFolderRequested()

    parent: Overlay.overlay
    modal: true
    focus: true
    padding: 0
    closePolicy: Popup.CloseOnEscape
    transformOrigin: Item.Center
    dim: false

    enter: Transition {
        ParallelAnimation {
            NumberAnimation {
                property: "opacity"
                from: 0.0
                to: 1.0
                duration: 220
                easing.type: Easing.OutCubic
            }
            NumberAnimation {
                property: "scale"
                from: 0.97
                to: 1.0
                duration: 220
                easing.type: Easing.OutCubic
            }
        }
    }

    exit: Transition {
        ParallelAnimation {
            NumberAnimation {
                property: "opacity"
                from: 1.0
                to: 0.0
                duration: 160
                easing.type: Easing.InCubic
            }
            NumberAnimation {
                property: "scale"
                from: 1.0
                to: 0.985
                duration: 160
                easing.type: Easing.InCubic
            }
        }
    }

    readonly property int popupMinWidth: 560
    readonly property int popupMinHeight: 480
    readonly property int popupMaxWidth: 760
    readonly property int popupMaxHeight: 720

    anchors.centerIn: parent
    width: {
        if (!parent)
            return popupMaxWidth;
        const available = Math.max(0, parent.width - 48);
        return Math.min(available, Math.max(popupMinWidth, Math.min(popupMaxWidth, 680)));
    }
    height: {
        if (!parent)
            return popupMaxHeight;
        const available = Math.max(0, parent.height - 48);
        return Math.min(available, Math.max(popupMinHeight, Math.min(popupMaxHeight, 640)));
    }

    Theme {
        id: theme
    }

    readonly property int contentPaddingH: 14

    background: Rectangle {
        color: theme.vmPanel
        radius: 10
        border.color: theme.vmBorder
        border.width: 1
    }

    FileDialog {
        id: exportLogDialog
        title: "Export Activity Log"
        fileMode: FileDialog.SaveFile
        nameFilters: ["Log files (*.log)", "Text files (*.txt)"]
        defaultSuffix: "log"
        onAccepted: {
            if (root.hasBackend)
                root.backend.exportActivityLog(selectedFile);
        }
    }

    Dialog {
        id: resetConfirmDialog
        anchors.centerIn: parent
        modal: true
        title: "Reset Configuration"
        standardButtons: Dialog.Ok | Dialog.Cancel

        Label {
            text: "This removes saved settings, the projects folder choice, and project notes.\n\nProject version history (.musit folders) is not deleted."
            wrapMode: Text.WordWrap
        }

        onAccepted: {
            if (root.hasBackend)
                root.backend.resetConfig();
        }
    }

    function openSettings() {
        open();
    }

    function openExportLogDialog() {
        if (!root.hasBackend)
            return;
        exportLogDialog.currentFolder = root.backend.defaultActivityLogExportFolderUrl();
        exportLogDialog.currentFile = root.backend.defaultActivityLogExportFileUrl();
        exportLogDialog.open();
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 12
        spacing: 10

        RowLayout {
            Layout.fillWidth: true
            spacing: 12

            Label {
                text: "Settings"
                font.pixelSize: 19
                font.bold: true
                color: theme.vmTextPrimary
            }

            Item {
                Layout.fillWidth: true
            }

            PanelButton {
                text: "Close"
                variant: "default"
                Layout.preferredWidth: 88
                onClicked: root.close()
            }
        }

        Rectangle {
            Layout.fillWidth: true
            Layout.fillHeight: true
            radius: 8
            color: theme.vmSidePanel
            border.color: theme.vmBorder
            border.width: 1
            clip: true

            ScrollView {
                id: settingsScroll
                anchors.fill: parent
                anchors.margins: 1
                ScrollBar.horizontal.policy: ScrollBar.AlwaysOff
                ScrollBar.vertical.policy: ScrollBar.AsNeeded
                clip: true

                ColumnLayout {
                    width: settingsScroll.availableWidth
                    spacing: 0

                    Item {
                        Layout.fillWidth: true
                        Layout.preferredHeight: 12
                    }

                    ColumnLayout {
                        Layout.fillWidth: true
                        Layout.leftMargin: root.contentPaddingH
                        Layout.rightMargin: root.contentPaddingH
                        spacing: 14

                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 8

                        Label {
                            text: "Projects folder"
                            font.bold: true
                            font.pixelSize: 13
                            color: theme.vmTextPrimary
                        }

                        Label {
                            Layout.fillWidth: true
                            text: root.hasBackend && root.backend.projectsFolderPath
                                  ? root.backend.projectsFolderPath
                                  : "No folder selected yet"
                            wrapMode: Text.Wrap
                            color: theme.vmTextMeta
                            font.pixelSize: 12
                            lineHeight: 1.35
                        }

                        PanelButton {
                            Layout.fillWidth: true
                            variant: "primary"
                            text: root.hasBackend && root.backend.hasProjectsFolder
                                  ? "Change Projects Folder"
                                  : "Choose Projects Folder"
                            onClicked: root.chooseProjectsFolderRequested()
                        }
                    }

                    Rectangle {
                        Layout.fillWidth: true
                        Layout.preferredHeight: 1
                        color: theme.vmBorder
                    }

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 12
                        visible: root.hasBackend && root.backend.launchAtStartupSupported

                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: 4

                            Label {
                                text: "Launch at login"
                                font.bold: true
                                font.pixelSize: 13
                                color: theme.vmTextPrimary
                            }

                            Label {
                                Layout.fillWidth: true
                                text: "Start Immersion automatically when you sign in."
                                wrapMode: Text.Wrap
                                color: theme.vmTextMeta
                                font.pixelSize: 12
                                lineHeight: 1.35
                            }
                        }

                        ThemedSwitch {
                            id: launchAtStartupSwitch
                            Layout.alignment: Qt.AlignVCenter
                            enabled: root.hasBackend
                            checked: root.hasBackend ? root.backend.launchAtStartup : false
                            onToggled: {
                                if (root.hasBackend)
                                    root.backend.launchAtStartup = checked;
                            }
                        }
                    }

                    Rectangle {
                        Layout.fillWidth: true
                        Layout.preferredHeight: 1
                        color: theme.vmBorder
                        visible: root.hasBackend && root.backend.launchAtStartupSupported
                    }

                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 8

                        Label {
                            text: "Default sort mode"
                            font.bold: true
                            color: theme.vmTextPrimary
                        }

                        ThemedComboBox {
                            id: sortModeCombo
                            Layout.fillWidth: true
                            model: ["Name", "Last Opened"]
                            currentIndex: root.hasBackend && root.backend.sortMode === "Last Opened" ? 1 : 0
                            onActivated: function (index) {
                                if (root.hasBackend)
                                    root.backend.sortMode = sortModeCombo.textAt(index);
                            }
                        }
                    }

                    Rectangle {
                        Layout.fillWidth: true
                        Layout.preferredHeight: 1
                        color: theme.vmBorder
                    }

                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 8

                        Label {
                            text: "Activity log level"
                            font.bold: true
                            color: theme.vmTextPrimary
                        }

                        Label {
                            Layout.fillWidth: true
                            text: "Debug shows scan and watcher details in the activity panel."
                            wrapMode: Text.Wrap
                            color: theme.vmTextMeta
                            font.pixelSize: 12
                        }

                        ThemedComboBox {
                            id: logLevelCombo
                            Layout.fillWidth: true
                            model: ["Info", "Debug"]
                            currentIndex: root.hasBackend && root.backend.logLevel === "Debug" ? 1 : 0
                            onActivated: function (index) {
                                if (root.hasBackend)
                                    root.backend.logLevel = logLevelCombo.textAt(index);
                            }
                        }
                    }

                    Rectangle {
                        Layout.fillWidth: true
                        Layout.preferredHeight: 1
                        color: theme.vmBorder
                    }

                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 8

                        Label {
                            text: "Snapshot retention"
                            font.bold: true
                            color: theme.vmTextPrimary
                        }

                        Label {
                            Layout.fillWidth: true
                            text: "Number of recent versions that keep fast uncompressed copies per file."
                            wrapMode: Text.Wrap
                            color: theme.vmTextMeta
                            font.pixelSize: 12
                        }

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 12

                            ThemedSpinBox {
                                id: retentionSpinBox
                                from: 1
                                to: 50
                                stepSize: 1
                                editable: true
                                enabled: root.hasBackend
                                value: root.hasBackend ? root.backend.snapshotRetention : 5
                                onValueChanged: {
                                    if (root.hasBackend)
                                        root.backend.snapshotRetention = value;
                                }
                            }

                            Label {
                                text: "recent versions"
                                color: theme.vmTextMeta
                                font.pixelSize: 12
                            }
                        }
                    }

                    Rectangle {
                        Layout.fillWidth: true
                        Layout.preferredHeight: 1
                        color: theme.vmBorder
                    }

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 12

                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: 4

                            Label {
                                text: "Save notifications"
                                font.bold: true
                                font.pixelSize: 13
                                color: theme.vmTextPrimary
                            }

                            Label {
                                Layout.fillWidth: true
                                text: "Show an OS notification when a new snapshot is saved."
                                wrapMode: Text.Wrap
                                color: theme.vmTextMeta
                                font.pixelSize: 12
                                lineHeight: 1.35
                            }
                        }

                        ThemedSwitch {
                            id: notificationsSwitch
                            Layout.alignment: Qt.AlignVCenter
                            enabled: root.hasBackend
                            checked: root.hasBackend ? root.backend.notificationsEnabled : true
                            onToggled: {
                                if (root.hasBackend)
                                    root.backend.notificationsEnabled = checked;
                            }
                        }
                    }

                    Rectangle {
                        Layout.fillWidth: true
                        Layout.preferredHeight: 1
                        color: theme.vmBorder
                    }

                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 8

                        Label {
                            text: "Appearance"
                            font.bold: true
                            color: theme.vmTextPrimary
                        }

                        ThemedComboBox {
                            id: colorSchemeCombo
                            Layout.fillWidth: true
                            model: ["System", "Light", "Dark"]
                            currentIndex: {
                                if (!root.hasBackend)
                                    return 0;
                                if (root.backend.colorSchemeMode === "Light")
                                    return 1;
                                if (root.backend.colorSchemeMode === "Dark")
                                    return 2;
                                return 0;
                            }
                            onActivated: function (index) {
                                if (root.hasBackend)
                                    root.backend.colorSchemeMode = colorSchemeCombo.textAt(index);
                            }
                        }
                    }

                    Rectangle {
                        Layout.fillWidth: true
                        Layout.preferredHeight: 1
                        color: theme.vmBorder
                    }

                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 8

                        Label {
                            text: "Theme hue"
                            font.bold: true
                            color: theme.vmTextPrimary
                        }

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 12

                            ThemedSlider {
                                id: hueSlider
                                Layout.fillWidth: true
                                from: 0
                                to: 360
                                stepSize: 1
                                enabled: root.hasBackend
                                value: root.hasBackend ? root.backend.themeHue : theme.hue
                                onMoved: {
                                    if (root.hasBackend)
                                        root.backend.themeHue = value;
                                }
                            }

                            Label {
                                Layout.preferredWidth: 42
                                horizontalAlignment: Text.AlignRight
                                text: Math.round(hueSlider.value)
                                color: theme.vmTextMeta
                                font.pixelSize: 12
                            }
                        }

                        Rectangle {
                            Layout.fillWidth: true
                            Layout.preferredHeight: 22
                            radius: 6
                            color: theme.accent
                            opacity: theme.isDarkMode ? 0.72 : 0.82
                            border.color: theme.vmBorder
                            border.width: 1
                        }
                    }

                    Rectangle {
                        Layout.fillWidth: true
                        Layout.preferredHeight: 1
                        color: theme.vmBorder
                    }

                    Rectangle {
                        Layout.fillWidth: true
                        radius: 10
                        color: theme.vmPanel
                        border.color: theme.vmBorder
                        border.width: 1
                        implicitHeight: supportColumn.implicitHeight + 20

                        ColumnLayout {
                            id: supportColumn
                            anchors.fill: parent
                            anchors.margins: 10
                            spacing: 8

                            Label {
                                Layout.fillWidth: true
                                text: "Support zone"
                                font.bold: true
                                font.pixelSize: 13
                                color: theme.vmTextPrimary
                            }

                            Label {
                                Layout.fillWidth: true
                                text: "Reset app configuration or export the full activity log for troubleshooting."
                                wrapMode: Text.Wrap
                                color: theme.vmTextMeta
                                font.pixelSize: 12
                                lineHeight: 1.35
                            }

                            RowLayout {
                                Layout.fillWidth: true
                                spacing: 10

                                PanelButton {
                                    Layout.fillWidth: true
                                    Layout.preferredHeight: 40
                                    text: "Export Activity Log"
                                    onClicked: root.openExportLogDialog()
                                }

                                PanelButton {
                                    Layout.fillWidth: true
                                    Layout.preferredHeight: 40
                                    text: "Reset Configuration"
                                    variant: "danger"
                                    onClicked: resetConfirmDialog.open()
                                }
                            }
                        }
                    }

                    }

                    Item {
                        Layout.fillWidth: true
                        Layout.preferredHeight: 10
                    }
                }
            }
        }
    }

    Connections {
        target: root.backend
        function onLaunchAtStartupChanged() {
            if (root.hasBackend)
                launchAtStartupSwitch.checked = root.backend.launchAtStartup;
        }
        function onThemeHueChanged() {
            if (root.hasBackend)
                hueSlider.value = root.backend.themeHue;
        }
        function onSortModeChanged() {
            if (root.hasBackend)
                sortModeCombo.currentIndex = root.backend.sortMode === "Last Opened" ? 1 : 0;
        }
        function onLogLevelChanged() {
            if (root.hasBackend)
                logLevelCombo.currentIndex = root.backend.logLevel === "Debug" ? 1 : 0;
        }
        function onSnapshotRetentionChanged() {
            if (root.hasBackend)
                retentionSpinBox.value = root.backend.snapshotRetention;
        }
        function onNotificationsEnabledChanged() {
            if (root.hasBackend)
                notificationsSwitch.checked = root.backend.notificationsEnabled;
        }
        function onColorSchemeModeChanged() {
            if (!root.hasBackend)
                return;
            if (root.backend.colorSchemeMode === "Light")
                colorSchemeCombo.currentIndex = 1;
            else if (root.backend.colorSchemeMode === "Dark")
                colorSchemeCombo.currentIndex = 2;
            else
                colorSchemeCombo.currentIndex = 0;
        }
        function onConfigReset() {
            root.close();
        }
    }
}
