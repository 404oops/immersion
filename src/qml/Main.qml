pragma ComponentBehavior: Bound

import QtQuick 2.15
import QtQuick.Controls 2.15
import QtQuick.Layouts 1.15
import QtQuick.Dialogs
import "."

ApplicationWindow {
    id: window
    required property var backend
    width: 1100
    height: 720
    minimumWidth: 880
    minimumHeight: 560
    visible: false
    title: "Immersion"

    onClosing: function (close) {
        close.accepted = false
        window.hide()
    }

    FolderDialog {
        id: folderDialog
        currentFolder: ""
        title: "Select Projects Folder"
        onAccepted: {
            window.backend.loadProjectsFromFolder(selectedFolder)
        }
    }

    // Show onboarding only until the user picks a projects folder. While a NAS
    // scan runs, keep the main view visible and populate the list incrementally.
    readonly property bool isOnboarding: !window.backend.hasProjectsFolder
    readonly property int viewFadeDuration: 280

    function scrollActivityToBottom() {
        if (!mainView || !mainView.activityListView)
            return;
        Qt.callLater(function () {
            mainView.activityListView.positionViewAtEnd();
        });
    }

    Loader {
        id: onboardingLoader
        anchors.fill: parent
        z: 2
        active: window.isOnboarding || opacity > 0
        enabled: window.isOnboarding
        opacity: window.isOnboarding ? 1 : 0
        visible: active
        sourceComponent: OnboardingView {
            onChooseProjectFolderRequested: folderDialog.open()
        }

        Behavior on opacity {
            NumberAnimation {
                duration: window.viewFadeDuration
                easing.type: Easing.OutCubic
            }
        }
    }

    MainView {
        id: mainView
        objectName: "mainView"
        anchors.fill: parent
        z: 1
        opacity: window.isOnboarding ? 0 : 1
        enabled: !window.isOnboarding
        visible: opacity > 0 || !window.isOnboarding
        statusMessage: window.backend.statusMessage
        isScanningProjects: window.backend.isScanningProjects
        projectsModel: window.backend.projects
        activityModel: window.backend.activity
        logLevel: window.backend.logLevel
        searchText: window.backend.searchText
        sortMode: window.backend.sortMode
        useDesignerPlaceholders: false

        onChooseProjectFolderRequested: folderDialog.open()
        onSortModeRequested: function (mode) {
            window.backend.sortMode = mode;
        }
        onSearchTextRequested: function (text) {
            window.backend.searchText = text;
        }
        onOpenProjectRequested: function (index) {
            window.backend.openProject(index);
        }
        onManageProjectRequested: function (index) {
            versionManagerWindow.openForProject(index);
        }
        onLogLevelRequested: function (level) {
            window.backend.logLevel = level;
        }

        Behavior on opacity {
            NumberAnimation {
                duration: window.viewFadeDuration
                easing.type: Easing.OutCubic
            }
        }

        Component.onCompleted: window.scrollActivityToBottom()
    }

    Connections {
        target: window.backend
        function onActivityChanged() {
            window.scrollActivityToBottom();
        }
    }

    Theme {
        id: windowTheme
    }

    // Full-window scrim behind the version manager (Overlay.modal ignores opacity).
    Rectangle {
        id: versionManagerScrim
        parent: Overlay.overlay
        anchors.fill: parent
        color: windowTheme.modalScrim
        opacity: versionManagerWindow.opened ? 1 : 0
        visible: opacity > 0
        z: versionManagerWindow.z - 1

        Behavior on opacity {
            NumberAnimation {
                duration: 220
                easing.type: Easing.OutCubic
            }
        }
    }

    VersionManagerWindow {
        id: versionManagerWindow
        backend: window.backend
    }

    Component.onCompleted: {
        if (window.isOnboarding)
            window.show();
    }
}
