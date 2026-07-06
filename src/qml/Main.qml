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
    visible: true
    title: "Immersion"

    FolderDialog {
        id: folderDialog
        currentFolder: ""
        title: "Select Projects Folder"
        onAccepted: {
            window.backend.loadProjectsFromFolder(selectedFolder)
        }
    }

    // Show onboarding only when nothing was discovered at all. The `projects`
    // list is search-filtered, so it must not drive this (an unmatched search
    // would otherwise kick the user back to the welcome screen).
    readonly property bool isOnboarding: !window.backend.hasDiscoveredProjects

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
        visible: window.isOnboarding
        active: window.isOnboarding
        sourceComponent: OnboardingView {
            onChooseProjectFolderRequested: folderDialog.open()
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

        Component.onCompleted: window.scrollActivityToBottom()
    }

    Connections {
        target: window.backend
        function onActivityChanged() {
            window.scrollActivityToBottom();
        }
    }

    VersionManagerWindow {
        id: versionManagerWindow
        backend: window.backend
    }
}
