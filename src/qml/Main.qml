pragma ComponentBehavior: Bound

import QtQuick 2.15
import QtQuick.Controls 2.15
import QtQuick.Layouts 1.15
import "."

ApplicationWindow {
    id: window
    required property var backend
    width: 1100
    height: 720
    visible: true
    title: "musit"

    // Show onboarding when no projects folder is selected (empty projects list)
    readonly property bool isOnboarding: !window.backend.projects || window.backend.projects.length === 0

    function scrollActivityToBottom() {
        if (!mainView || !mainView.activityListView)
            return
        Qt.callLater(function() {
            mainView.activityListView.positionViewAtEnd()
        })
    }

    Loader {
        id: onboardingLoader
        anchors.fill: parent
        visible: window.isOnboarding
        sourceComponent: window.isOnboarding ? onboardingComponent : null
    }

    Component {
        id: onboardingComponent
        Rectangle {
            color: theme.appBackground

            Theme {
                id: theme
            }

            ColumnLayout {
                anchors.centerIn: parent
                anchors.margins: 40
                spacing: 32
                width: Math.min(600, parent.width - 80)

                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 16

                    Text {
                        text: "Welcome to Musit"
                        font.pixelSize: 48
                        font.weight: Font.Bold
                        color: theme.textPrimary
                        horizontalAlignment: Text.AlignHCenter
                        Layout.fillWidth: true
                    }

                    Text {
                        text: "To start, pick a folder where all your project files sit, and we'll take care of the rest."
                        font.pixelSize: 16
                        color: theme.textSecondary
                        wrapMode: Text.Wrap
                        horizontalAlignment: Text.AlignHCenter
                        Layout.fillWidth: true
                        lineHeight: 1.5
                    }
                }

                Button {
                    text: "Open Projects Folder"
                    Layout.fillWidth: true
                    Layout.preferredHeight: 48
                    font.pixelSize: 14
                    font.weight: Font.Medium

                    background: Rectangle {
                        color: button.hovered || button.pressed ? theme.selection : theme.accent
                        radius: 4
                        Behavior on color { ColorAnimation { duration: 150 } }
                    }

                    contentItem: Text {
                        text: button.text
                        color: "white"
                        font: button.font
                        horizontalAlignment: Text.AlignHCenter
                        verticalAlignment: Text.AlignVCenter
                    }

                    onClicked: window.backend.chooseProjectFolder()
                    id: button
                }
            }
        }
    }

    MainView {
        id: mainView
        objectName: "mainView"
        anchors.fill: parent
        visible: !window.isOnboarding
        statusMessage: window.backend.statusMessage
        projectsModel: window.backend.projects
        activityModel: window.backend.activity
        logLevel: window.backend.logLevel
        searchText: window.backend.searchText
        sortMode: window.backend.sortMode
        useDesignerPlaceholders: false

        onChooseProjectFolderRequested: window.backend.chooseProjectFolder()
        onSortModeRequested: function(mode) {
            window.backend.sortMode = mode
        }
        onSearchTextRequested: function(text) {
            window.backend.searchText = text
        }
        onOpenProjectRequested: function(index) {
            window.backend.openProject(index)
        }
        onRestoreProjectRequested: function(index) {
            window.backend.restoreProject(index)
        }
        onManageProjectRequested: function(index) {
            versionManagerWindow.openForProject(index)
        }
        onLogLevelRequested: function(level) {
            window.backend.logLevel = level
        }

        Component.onCompleted: window.scrollActivityToBottom()
    }

    Connections {
        target: window.backend
        function onActivityChanged() {
            window.scrollActivityToBottom()
        }
    }

    VersionManagerWindow {
        id: versionManagerWindow
        backend: window.backend
        hostWindow: window
    }
}
