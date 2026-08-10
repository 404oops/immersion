pragma ComponentBehavior: Bound

import QtQuick 2.15
import QtQuick.Controls 2.15
import QtQuick.Layouts 1.15
import "."

ThemedPopup {
    id: root
    property var backend: null
    readonly property bool hasBackend: backend !== null && backend !== undefined
    property string folderPath: ""
    property bool awaitingScan: false
    property string scanError: ""
    property bool scanSucceeded: false
    property bool scanWhenOpened: false

    popupMinWidth: 520
    popupMinHeight: 420
    popupMaxWidth: 640
    popupMaxHeight: 560
    popupPreferredWidth: 600
    popupPreferredHeight: 500

    Theme {
        id: theme
    }

    function folderPathString() {
        return root.folderPath;
    }

    function openForFolder(folderValue) {
        root.folderPath = root.hasBackend
                ? root.backend.displayLocalPath(folderValue)
                : folderValue.toString();
        root.awaitingScan = false;
        root.scanError = "";
        root.scanSucceeded = false;
        root.scanWhenOpened = true;
        if (root.hasBackend)
            root.selectedLayout = root.backend.projectsFolderLayoutForPath(root.folderPath);
        else
            root.selectedLayout = "Bundles";
        root.open();
    }

    function openForCurrentFolder() {
        if (!root.hasBackend || root.backend.projectsFolderPath.length === 0)
            return;
        root.folderPath = root.backend.projectsFolderPath;
        root.awaitingScan = false;
        root.scanError = "";
        root.scanSucceeded = false;
        root.scanWhenOpened = false;
        root.selectedLayout = root.backend.projectsFolderLayout;
        root.open();
    }

    onOpened: {
        if (!root.scanWhenOpened || root.awaitingScan)
            return;
        root.scanWhenOpened = false;
        root.confirmSelection();
    }

    function confirmSelection() {
        if (!root.hasBackend)
            return;

        const localPath = root.folderPath;
        if (localPath.length === 0) {
            root.scanError = "The selected folder path is invalid.";
            return;
        }

        root.scanError = "";
        root.scanSucceeded = false;
        root.awaitingScan = true;
        root.backend.confirmProjectsFolder(localPath, root.selectedLayout);
    }

    function selectAndScan(layout) {
        if (root.awaitingScan)
            return;
        root.selectedLayout = layout;
        root.confirmSelection();
    }

    property string selectedLayout: "Bundles"

    Connections {
        target: root.backend

        function onProjectsFolderScanFinished(foundProjects) {
            if (!root.opened || !root.awaitingScan)
                return;

            root.awaitingScan = false;
            if (foundProjects) {
                root.scanSucceeded = true;
                root.scanError = "Folder layout updated and projects reloaded.";
            } else {
                root.scanSucceeded = false;
                root.scanError = "No supported project files were found for this layout. Choose the other layout or verify the selected folder.";
            }
        }
    }

    ScrollView {
        id: pickerScroll
        anchors.fill: parent
        anchors.margins: 1
        clip: true
        ScrollBar.horizontal.policy: ScrollBar.AlwaysOff
        ScrollBar.vertical.policy: ScrollBar.AsNeeded

        ColumnLayout {
            width: pickerScroll.availableWidth
            spacing: 12

            Item {
                Layout.fillWidth: true
                Layout.preferredHeight: 12
            }

            ColumnLayout {
                Layout.fillWidth: true
                Layout.leftMargin: 14
                Layout.rightMargin: 14
                spacing: 12

                Label {
                    Layout.fillWidth: true
                    text: "How are your projects organized?"
                    font.pixelSize: 19
                    font.bold: true
                    color: theme.vmTextPrimary
                    wrapMode: Text.WordWrap
                }

                Label {
                    Layout.fillWidth: true
                    text: "This choice is saved in .immersion/settings.json inside the folder you selected."
                    wrapMode: Text.WordWrap
                    color: theme.vmTextMeta
                    font.pixelSize: 12
                    lineHeight: 1.35
                }

                Label {
                    Layout.fillWidth: true
                    visible: root.folderPathString().length > 0
                    text: root.folderPathString()
                    wrapMode: Text.Wrap
                    color: theme.vmTextMeta
                    font.pixelSize: 11
                    opacity: 0.9
                }

                Rectangle {
                    Layout.fillWidth: true
                    Layout.preferredHeight: bundlesCardColumn.implicitHeight + 20
                    radius: theme.modalPanelRadius
                    color: root.selectedLayout === "Bundles" ? theme.vmSidePanel : theme.vmPanel
                    border.color: root.selectedLayout === "Bundles"
                                  ? theme.modalSelectedBorder
                                  : theme.modalOptionBorder
                    border.width: 1

                    ColumnLayout {
                        id: bundlesCardColumn
                        anchors.left: parent.left
                        anchors.right: parent.right
                        anchors.top: parent.top
                        anchors.margins: 10
                        spacing: 6

                        Label {
                            Layout.fillWidth: true
                            text: "Project folders with bundles"
                            font.bold: true
                            font.pixelSize: 13
                            color: theme.vmTextPrimary
                        }

                        Label {
                            Layout.fillWidth: true
                            text: "Each project lives in its own subfolder and uses bundle-style project files like .logicx or .band."
                            wrapMode: Text.WordWrap
                            color: theme.vmTextMeta
                            font.pixelSize: 12
                            lineHeight: 1.35
                        }
                    }

                    MouseArea {
                        anchors.fill: parent
                        enabled: !root.awaitingScan
                        cursorShape: Qt.PointingHandCursor
                        onClicked: root.selectAndScan("Bundles")
                    }
                }

                Rectangle {
                    Layout.fillWidth: true
                    Layout.preferredHeight: filesCardColumn.implicitHeight + 20
                    radius: theme.modalPanelRadius
                    color: root.selectedLayout === "Files" ? theme.vmSidePanel : theme.vmPanel
                    border.color: root.selectedLayout === "Files"
                                  ? theme.modalSelectedBorder
                                  : theme.modalOptionBorder
                    border.width: 1

                    ColumnLayout {
                        id: filesCardColumn
                        anchors.left: parent.left
                        anchors.right: parent.right
                        anchors.top: parent.top
                        anchors.margins: 10
                        spacing: 6

                        Label {
                            Layout.fillWidth: true
                            text: "Loose project files"
                            font.bold: true
                            font.pixelSize: 13
                            color: theme.vmTextPrimary
                        }

                        Label {
                            Layout.fillWidth: true
                            text: "Project files like .als and .flp sit directly in this folder without extra subfolders."
                            wrapMode: Text.WordWrap
                            color: theme.vmTextMeta
                            font.pixelSize: 12
                            lineHeight: 1.35
                        }
                    }

                    MouseArea {
                        anchors.fill: parent
                        enabled: !root.awaitingScan
                        cursorShape: Qt.PointingHandCursor
                        onClicked: root.selectAndScan("Files")
                    }
                }

                Label {
                    Layout.fillWidth: true
                    visible: root.awaitingScan || root.scanError.length > 0
                    text: root.awaitingScan ? "Scanning the folder…" : root.scanError
                    wrapMode: Text.WordWrap
                    color: root.awaitingScan
                           ? theme.vmTextMeta
                           : (root.scanSucceeded ? theme.success : theme.buttonDangerLabel)
                    font.pixelSize: 12
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 10

                    Item {
                        Layout.fillWidth: true
                    }

                    PanelButton {
                        text: "Close"
                        Layout.preferredWidth: 96
                        onClicked: root.close()
                    }
                }
            }

            Item {
                Layout.fillWidth: true
                Layout.preferredHeight: 12
            }
        }
    }
}
