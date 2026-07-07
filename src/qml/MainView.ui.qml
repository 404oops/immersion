pragma ComponentBehavior: Bound

import QtQuick 2.15
import QtQuick.Controls 2.15
import QtQuick.Layouts 1.15
import "."

Rectangle {
    id: root
    objectName: "mainView"
    width: 1100
    height: 720
    color: theme.appBackground

    Theme {
        id: theme
    }

    // Semantic hooks for app logic.
    property string statusMessage: "Please select the folder where all of your project files sit."
    property bool isScanningProjects: false
    property var projectsModel: []
    property var activityModel: []
    property alias activityListView: activityList
    property string logLevel: "Info"
    property string searchText: ""
    property string sortMode: "Name"
    // Defaults to designer behavior; runtime explicitly overrides this to false.
    property bool useDesignerPlaceholders: true

    signal chooseProjectFolderRequested
    signal sortModeRequested(string mode)
    signal searchTextRequested(string text)
    signal openProjectRequested(int index)
    signal restoreProjectRequested(int index)
    signal manageProjectRequested(int index)
    signal logLevelRequested(string level)

    ListModel {
        id: placeholderProjectsModel
        ListElement {
            name: "Project 1"
            type: "Bitwig"
            file: "example-v1.bwproject"
            path: "~/Music/Projects/A"
        }
        ListElement {
            name: "Project 2"
            type: "Ableton"
            file: "example-v2.als"
            path: "~/Music/Projects/B"
        }
        ListElement {
            name: "Project 3"
            type: "Logic"
            file: "example-v3.logicx"
            path: "~/Music/Projects/C"
        }
        ListElement {
            name: "Project 4"
            type: "Reaper"
            file: "example-v4.rpp"
            path: "~/Music/Projects/D"
        }
        ListElement {
            name: "Project 5"
            type: "Studio One"
            file: "example-v5.song"
            path: "~/Music/Projects/E"
        }
        ListElement {
            name: "Project 6"
            type: "FL Studio"
            file: "example-v6.flp"
            path: "~/Music/Projects/F"
        }
        ListElement {
            name: "Project 7"
            type: "Cubase"
            file: "example-v7.cpr"
            path: "~/Music/Projects/G"
        }
        ListElement {
            name: "Project 8"
            type: "Renoise"
            file: "example-v8.xrns"
            path: "~/Music/Projects/H"
        }
        ListElement {
            name: "Project 9"
            type: "Reason"
            file: "example-v9.reason"
            path: "~/Music/Projects/I"
        }
        ListElement {
            name: "Project 10"
            type: "Pro Tools"
            file: "example-v10.ptx"
            path: "~/Music/Projects/J"
        }
        ListElement {
            name: "Project 11"
            type: "Bitwig"
            file: "example-v11.bwproject"
            path: "~/Music/Projects/K"
        }
        ListElement {
            name: "Project 12"
            type: "Ableton"
            file: "example-v12.als"
            path: "~/Music/Projects/L"
        }
        ListElement {
            name: "Project 13"
            type: "Logic"
            file: "example-v13.logicx"
            path: "~/Music/Projects/M"
        }
        ListElement {
            name: "Project 14"
            type: "Reaper"
            file: "example-v14.rpp"
            path: "~/Music/Projects/N"
        }
        ListElement {
            name: "Project 15"
            type: "Studio One"
            file: "example-v15.song"
            path: "~/Music/Projects/O"
        }
        ListElement {
            name: "Project 16"
            type: "FL Studio"
            file: "example-v16.flp"
            path: "~/Music/Projects/P"
        }
        ListElement {
            name: "Project 17"
            type: "Cubase"
            file: "example-v17.cpr"
            path: "~/Music/Projects/Q"
        }
        ListElement {
            name: "Project 18"
            type: "Cakewalk"
            file: "example-v18.cwp"
            path: "~/Music/Projects/R"
        }
        ListElement {
            name: "Project 19"
            type: "Reason"
            file: "example-v19.reason"
            path: "~/Music/Projects/S"
        }
        ListElement {
            name: "Project 20"
            type: "Pro Tools"
            file: "example-v20.ptx"
            path: "~/Music/Projects/T"
        }
    }

    property bool hasProjects: root.projectsModel && root.projectsModel.length !== undefined && root.projectsModel.length > 0

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 16
        spacing: 10

        RowLayout {
            Layout.fillWidth: true
            spacing: 12

            Label {
                objectName: "statusLabel"
                text: root.statusMessage
                color: theme.textStatus
                wrapMode: Text.WordWrap
                Layout.fillWidth: true
            }

            ActionButton {
                id: chooseProjectFolderButton
                objectName: "chooseProjectFolderButton"
                text: "Open Projects"
                Layout.preferredHeight: 36
                Layout.preferredWidth: 160

                Connections {
                    target: chooseProjectFolderButton
                    function onClicked() {
                        root.chooseProjectFolderRequested();
                    }
                }
            }
        }

        SplitView {
            Layout.fillWidth: true
            Layout.fillHeight: true
            orientation: Qt.Vertical

            handle: Rectangle {
                implicitWidth: parent ? parent.width : 0
                implicitHeight: 20
                color: "transparent"
                Rectangle {
                    anchors.centerIn: parent
                    width: 6
                    height: 6
                    radius: 3
                    color: theme.textSecondary
                    opacity: SplitHandle.pressed ? 1.0 : (SplitHandle.hovered ? 0.8 : 0.5)
                }
            }

            Rectangle {
                SplitView.fillWidth: true
                SplitView.fillHeight: true
                SplitView.preferredHeight: 480
                color: theme.panelSurface
                radius: 8

                ColumnLayout {
                    anchors.fill: parent
                    anchors.margins: 10
                    spacing: 8

                    RowLayout {
                        Layout.fillWidth: true

                        Label {
                            objectName: "projectListHeader"
                            text: "Discovered Projects"
                            color: theme.textPrimary
                        }

                        Item {
                            Layout.fillWidth: true
                        }

                        RowLayout {
                            Layout.alignment: Qt.AlignVCenter

                            Label {
                                text: "Sort:"
                                color: theme.textSecondary
                            }

                            ThemedComboBox {
                                id: sortCombo
                                objectName: "sortModeComboBox"
                                model: ["Name", "Last Opened"]
                                currentIndex: root.sortMode === "Last Opened" ? 1 : 0
                                Layout.preferredWidth: 150

                                Connections {
                                    target: sortCombo
                                    function onActivated(index) {
                                        root.sortModeRequested(sortCombo.textAt(index));
                                    }
                                }
                            }
                        }
                    }

                    ThemedTextField {
                        id: projectSearchField
                        objectName: "projectSearchField"
                        placeholderText: "Search projects..."
                        Layout.fillWidth: true

                        Connections {
                            target: projectSearchField
                            function onTextEdited() {
                                root.searchTextRequested(projectSearchField.text);
                            }
                        }
                    }

                    Item {
                        Layout.fillWidth: true
                        Layout.fillHeight: true

                        ListView {
                            id: projectList
                            objectName: "projectList"
                            anchors.fill: parent
                            anchors.rightMargin: projectListScrollBar.visible ? projectListScrollBar.implicitWidth : 0
                            model: root.hasProjects ? root.projectsModel : (root.useDesignerPlaceholders ? placeholderProjectsModel : [])
                            clip: true
                            spacing: 8
                            boundsBehavior: Flickable.StopAtBounds
                            ScrollBar.vertical: projectListScrollBar

                            delegate: Rectangle {
                                    id: projectDelegate
                                    objectName: "projectItem"
                                    required property int index
                                    required property var modelData
                                    width: ListView.view.width
                                    height: 86
                                    radius: 6
                                    color: projectDelegate.index % 2 === 0 ? theme.rowEven : theme.rowOdd

                                    RowLayout {
                                        anchors.fill: parent
                                        anchors.margins: 8
                                        spacing: 10

                                        Rectangle {
                                            Layout.preferredWidth: 48
                                            Layout.preferredHeight: 48
                                            Layout.alignment: Qt.AlignVCenter
                                            radius: 6
                                            color: theme.accent
                                            Label {
                                                anchors.centerIn: parent
                                                text: (projectDelegate.modelData && projectDelegate.modelData.type) ? projectDelegate.modelData.type : "DAW"
                                                color: "white"
                                                font.pixelSize: 10
                                                horizontalAlignment: Text.AlignHCenter
                                                wrapMode: Text.WordWrap
                                                width: 42
                                            }
                                        }

                                        ColumnLayout {
                                            Layout.fillWidth: true
                                            Layout.alignment: Qt.AlignVCenter
                                            spacing: 2

                                            Label {
                                                text: projectDelegate.modelData && projectDelegate.modelData.name ? projectDelegate.modelData.name : ("Project " + (projectDelegate.index + 1))
                                                color: theme.textPrimary
                                                font.bold: true
                                            }

                                            Label {
                                                text: "Project file: " + ((projectDelegate.modelData && projectDelegate.modelData.file) ? projectDelegate.modelData.file : "example.bwproject")
                                                color: theme.textSecondary
                                            }

                                            Label {
                                                text: "Path: " + ((projectDelegate.modelData && projectDelegate.modelData.path) ? projectDelegate.modelData.path : "~/Music/Projects")
                                                color: theme.textMuted
                                                elide: Text.ElideRight
                                                Layout.fillWidth: true
                                            }
                                        }

                                        // Full-height action column beside project metadata.
                                        Item {
                                            Layout.preferredWidth: 210
                                            Layout.fillHeight: true
                                            Layout.alignment: Qt.AlignVCenter

                                            Row {
                                                anchors.fill: parent
                                                spacing: 8

                                                ActionButton {
                                                    id: openProjectButton
                                                    objectName: "openProjectButton"
                                                    width: (parent.width - parent.spacing) / 2
                                                    height: parent.height
                                                    text: "Open"

                                                    Connections {
                                                        target: openProjectButton
                                                        function onClicked() {
                                                            root.openProjectRequested(projectDelegate.index);
                                                        }
                                                    }
                                                }

                                                ActionButton {
                                                    id: manageProjectButton
                                                    objectName: "manageProjectButton"
                                                    width: (parent.width - parent.spacing) / 2
                                                    height: parent.height
                                                    text: "Manage"

                                                    Connections {
                                                        target: manageProjectButton
                                                        function onClicked() {
                                                            root.manageProjectRequested(projectDelegate.index);
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                        ScrollBar {
                            id: projectListScrollBar
                            anchors.top: parent.top
                            anchors.bottom: parent.bottom
                            anchors.right: parent.right
                            anchors.rightMargin: -4
                            policy: ScrollBar.AsNeeded
                            orientation: Qt.Vertical
                        }

                        Label {
                            objectName: "noProjectsLoadedLabel"
                            anchors.centerIn: parent
                            visible: !root.useDesignerPlaceholders && !root.hasProjects
                            text: root.isScanningProjects ? "Scanning for projects…" : "No projects loaded"
                            color: theme.textStatus
                            font.pixelSize: 20
                            font.bold: true
                        }
                    }
                }
            }

            Rectangle {
                SplitView.fillWidth: true
                SplitView.fillHeight: true
                SplitView.preferredHeight: 150
                color: theme.panelSurface
                radius: 8

                ColumnLayout {
                    anchors.fill: parent
                    anchors.margins: 10
                    spacing: 8

                    RowLayout {
                        Layout.fillWidth: true

                        Label {
                            objectName: "activityHeader"
                            text: "Activity"
                            color: theme.textPrimary
                        }

                        Item {
                            Layout.fillWidth: true
                        }

                        Label {
                            text: "Log level:"
                            color: theme.textSecondary
                        }

                        ThemedComboBox {
                            id: activityLogLevelCombo
                            objectName: "activityLogLevelComboBox"
                            model: ["Info", "Debug"]
                            currentIndex: root.logLevel === "Debug" ? 1 : 0
                            Layout.preferredWidth: 120

                            Connections {
                                target: activityLogLevelCombo
                                function onActivated(index) {
                                    root.logLevelRequested(activityLogLevelCombo.textAt(index));
                                }
                            }
                        }
                    }

                    Item {
                        Layout.fillWidth: true
                        Layout.fillHeight: true

                        ListView {
                            id: activityList
                            objectName: "activityList"
                            anchors.fill: parent
                            anchors.rightMargin: activityListScrollBar.visible ? activityListScrollBar.implicitWidth : 0
                            model: root.activityModel
                            clip: true
                            boundsBehavior: Flickable.StopAtBounds
                            ScrollBar.vertical: activityListScrollBar

                            delegate: Label {
                                required property var modelData
                                text: modelData
                                color: theme.textActivity
                                elide: Text.ElideRight
                                width: ListView.view.width
                            }
                        }

                        ScrollBar {
                            id: activityListScrollBar
                            anchors.top: parent.top
                            anchors.bottom: parent.bottom
                            anchors.right: parent.right
                            anchors.rightMargin: -4
                            policy: ScrollBar.AsNeeded
                            orientation: Qt.Vertical
                        }
                    }
                }
            }
        }
    }
}
