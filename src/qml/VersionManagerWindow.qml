pragma ComponentBehavior: Bound

import QtQuick 2.15
import QtQuick.Controls 2.15
import QtQuick.Layouts 1.15
import "."

// Modal overlay on the main window (stays above it on macOS/Windows/Linux).
Popup {
    id: root
    property var backend: null
    readonly property bool hasBackend: backend !== null && backend !== undefined
    property var designGraph: buildEdgeCaseGraph()

    parent: Overlay.overlay
    modal: true
    focus: true
    padding: 0
    closePolicy: Popup.CloseOnEscape

    anchors.centerIn: parent
    width: parent ? Math.min(1120, Math.max(900, parent.width - 48)) : 1120
    height: parent ? Math.min(700, Math.max(560, parent.height - 48)) : 700

    background: Rectangle {
        color: theme.panelSurfaceAlt
        radius: 10
        border.color: theme.border
        border.width: 1
    }

    function buildEdgeCaseGraph() {
        const graph = [];
        let row = 0;

        // 1x2x2x2x2: one root, each level branches by 2 for four levels.
        const depthX = [80, 240, 400, 560, 720];
        const rowStep = 70;

        graph.push({
            id: "1",
            label: "v1",
            fullLabel: "v1",
            x: depthX[0],
            y: 60 + row * rowStep,
            parentId: "",
            note: "root 1",
            timestamp: "preview",
            isCurrent: false
        });
        ++row;

        function addChildren(parentId, depth, childCount) {
            if (depth >= depthX.length)
                return;
            for (let childIndex = 1; childIndex <= childCount; ++childIndex) {
                const childId = parentId + "." + childIndex;
                graph.push({
                    id: childId,
                    label: "." + childIndex,
                    fullLabel: "v" + childId,
                    x: depthX[depth],
                    y: 60 + row * rowStep,
                    parentId: parentId,
                    note: "node " + childId,
                    timestamp: "preview",
                    isCurrent: false
                });
                ++row;
                addChildren(childId, depth + 1, 2);
            }
        }

        addChildren("1", 1, 2);

        // Pick a deep preview node as current.
        for (let i = 0; i < graph.length; ++i) {
            if (graph[i].id === "1.2.2.2.2") {
                graph[i].isCurrent = true;
                break;
            }
        }

        return graph;
    }

    Theme {
        id: theme
    }

    property var versionGraph: []
    property string selectedVersionId: ""
    property real graphExtentWidth: 0
    property real graphExtentHeight: 0

    Shortcut {
        sequence: "Escape"
        enabled: root.opened
        onActivated: root.close()
    }

    function nodeById(versionId) {
        for (let i = 0; i < versionGraph.length; ++i) {
            if (versionGraph[i].id === versionId)
                return versionGraph[i];
        }
        return null;
    }

    function currentVersionNode() {
        for (let i = 0; i < versionGraph.length; ++i) {
            if (versionGraph[i].isCurrent)
                return versionGraph[i];
        }
        return null;
    }

    function refreshVersionGraph() {
        versionGraph = hasBackend ? root.backend.selectedProjectVersionGraph : designGraph;
        if (versionGraph.length === 0) {
            selectedVersionId = "";
        } else if (!selectedVersionId || !nodeById(selectedVersionId)) {
            selectedVersionId = versionGraph[0].id;
        }

        if (versionGraph.length === 0) {
            graphExtentWidth = graphFlick ? graphFlick.width : 0;
            graphExtentHeight = graphFlick ? graphFlick.height : 0;
        } else {
            let minX = Number(versionGraph[0].x || 0);
            let maxX = minX;
            let minY = Number(versionGraph[0].y || 0);
            let maxY = minY;
            for (let i = 1; i < versionGraph.length; ++i) {
                const node = versionGraph[i];
                const x = Number(node.x || 0);
                const y = Number(node.y || 0);
                minX = Math.min(minX, x);
                maxX = Math.max(maxX, x);
                minY = Math.min(minY, y);
                maxY = Math.max(maxY, y);
            }

            const nodeRadius = 20;
            const labelPad = 20;
            const viewportPad = 24;
            graphExtentWidth = (maxX - minX) + (nodeRadius + viewportPad) * 2;
            graphExtentHeight = (maxY - minY) + (nodeRadius + labelPad + viewportPad) * 2;
        }

        graphCanvas.requestPaint();
    }

    function openForProject(index) {
        if (!hasBackend)
            return;
        root.backend.manageProjectVersions(index);
        refreshVersionGraph();
        projectNoteEdit.text = root.backend.selectedProjectNote;
        versionNoteEdit.text = selectedVersionId && nodeById(selectedVersionId) ? nodeById(selectedVersionId).note : "";
        open();
    }

    Component.onCompleted: {
        refreshVersionGraph();
        if (!hasBackend)
            projectNoteEdit.text = "Preview project note";
    }

    Connections {
        target: root.backend
        function onSelectedProjectVersionGraphChanged() {
            if (root.opened && root.hasBackend) {
                root.refreshVersionGraph();
                versionNoteEdit.text = root.selectedVersionId && root.nodeById(root.selectedVersionId) ? root.nodeById(root.selectedVersionId).note : "";
            }
        }
        function onSelectedProjectNoteChanged() {
            if (root.opened && root.hasBackend) {
                projectNoteEdit.text = root.backend.selectedProjectNote;
            }
        }
    }

    Dialog {
        id: deleteConfirmDialog
        anchors.centerIn: parent
        modal: true
        title: "Delete Version"
        standardButtons: Dialog.Ok | Dialog.Cancel

        Label {
            text: "Delete version v" + root.selectedVersionId + "?\n\nThis removes its snapshot from .musit and cannot be undone."
            wrapMode: Text.WordWrap
        }

        onAccepted: {
            if (root.backend.deleteVersionById(root.selectedVersionId)) {
                root.refreshVersionGraph();
                versionNoteEdit.text = root.selectedVersionId && root.nodeById(root.selectedVersionId) ? root.nodeById(root.selectedVersionId).note : "";
            }
        }
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 14
        spacing: 10

        RowLayout {
            Layout.fillWidth: true
            spacing: 12
            Label {
                text: "Version Manager"
                font.pixelSize: 19
                font.bold: true
                color: theme.textPrimary
            }
            Item {
                Layout.fillWidth: true
            }
            Button {
                text: "Close"
                onClicked: root.close()
            }
        }

        RowLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            spacing: 12

            Rectangle {
                Layout.fillWidth: true
                Layout.fillHeight: true
                color: theme.graphSurface
                radius: 8
                border.width: 1
                border.color: theme.border

                Flickable {
                    id: graphFlick
                    anchors.fill: parent
                    anchors.leftMargin: 8
                    anchors.topMargin: 8
                    anchors.rightMargin: 8 + (graphScrollBarV.visible ? graphScrollBarV.implicitWidth + 4 : 0)
                    anchors.bottomMargin: 8 + (graphScrollBarH.visible ? graphScrollBarH.implicitWidth + 4 : 0)
                    contentWidth: Math.max(width, root.graphExtentWidth)
                    contentHeight: Math.max(height, root.graphExtentHeight)
                    clip: true
                    boundsBehavior: Flickable.StopAtBounds
                    ScrollBar.vertical: ScrollBar {
                        id: graphScrollBarV
                        policy: ScrollBar.AsNeeded
                    }
                    ScrollBar.horizontal: ScrollBar {
                        id: graphScrollBarH
                        policy: ScrollBar.AsNeeded
                    }

                    Item {
                        width: graphFlick.contentWidth
                        height: graphFlick.contentHeight

                        Canvas {
                            id: graphCanvas
                            anchors.fill: parent
                            onPaint: {
                                const ctx = getContext("2d");
                                ctx.reset();
                                ctx.strokeStyle = theme.graphLink;
                                ctx.lineWidth = 2;
                                for (let i = 0; i < root.versionGraph.length; ++i) {
                                    const node = root.versionGraph[i];
                                    if (!node.parentId)
                                        continue;
                                    const parent = root.nodeById(node.parentId);
                                    if (!parent)
                                        continue;
                                    ctx.beginPath();
                                    ctx.moveTo(parent.x, parent.y);
                                    const midX = parent.x + (node.x - parent.x) * 0.55;
                                    ctx.bezierCurveTo(midX, parent.y, midX, node.y, node.x, node.y);
                                    ctx.stroke();
                                }
                            }
                        }

                        Repeater {
                            model: root.versionGraph
                            delegate: Item {
                                id: nodeItem
                                required property var modelData
                                x: nodeItem.modelData.x - 20
                                y: nodeItem.modelData.y - 20
                                width: 40
                                height: 40

                                Rectangle {
                                    anchors.fill: parent
                                    radius: 20
                                    color: root.selectedVersionId === nodeItem.modelData.id ? theme.selection : (nodeItem.modelData.isCurrent ? theme.success : theme.accent)
                                    border.color: nodeItem.modelData.isCurrent ? theme.successBorder : theme.nodeBorder
                                    border.width: root.selectedVersionId === nodeItem.modelData.id ? 2 : 1
                                }

                                Label {
                                    anchors.centerIn: parent
                                    text: nodeItem.modelData.label
                                    color: "white"
                                    font.bold: true
                                    font.pixelSize: 11
                                }

                                MouseArea {
                                    anchors.fill: parent
                                    onClicked: {
                                        root.selectedVersionId = nodeItem.modelData.id;
                                        versionNoteEdit.text = nodeItem.modelData.note || "";
                                    }
                                }

                                Label {
                                    anchors.horizontalCenter: parent.horizontalCenter
                                    anchors.top: parent.bottom
                                    anchors.topMargin: 2
                                    visible: nodeItem.modelData.isCurrent
                                    text: "CURRENT"
                                    color: theme.successStrong
                                    font.pixelSize: 9
                                    font.bold: true
                                }
                            }
                        }
                    }
                }
            }

            Rectangle {
                Layout.preferredWidth: 340
                Layout.fillHeight: true
                color: theme.sidePanelSurface
                radius: 8
                border.width: 1
                border.color: theme.border

                ScrollView {
                    id: sidePanelScroll
                    anchors.fill: parent
                    anchors.margins: 10
                    clip: true
                    ScrollBar.vertical.policy: ScrollBar.AsNeeded
                    ScrollBar.horizontal.policy: ScrollBar.AlwaysOff

                    ColumnLayout {
                        width: sidePanelScroll.availableWidth
                        spacing: 10

                        Label {
                            text: "Project Note"
                            color: theme.textPrimary
                            font.bold: true
                        }

                        TextArea {
                            id: projectNoteEdit
                            Layout.fillWidth: true
                            Layout.preferredHeight: 110
                            placeholderText: "Write a note for this project..."
                            wrapMode: TextEdit.Wrap
                        }

                        Button {
                            text: "Save Project Note"
                            Layout.fillWidth: true
                            Layout.preferredHeight: 36
                            enabled: root.hasBackend
                            onClicked: root.backend.selectedProjectNote = projectNoteEdit.text
                        }

                        Label {
                            color: theme.textPrimary
                            font.bold: true
                            Layout.fillWidth: true
                            wrapMode: Text.WordWrap
                            text: root.selectedVersionId ? ("Version " + (root.nodeById(root.selectedVersionId) ? root.nodeById(root.selectedVersionId).fullLabel : root.selectedVersionId)) : "No version selected"
                        }

                        Label {
                            color: theme.successStrong
                            font.bold: true
                            Layout.fillWidth: true
                            wrapMode: Text.WordWrap
                            text: root.currentVersionNode() ? ("Current in project folder: " + root.currentVersionNode().fullLabel) : "Current in project folder: no matching snapshot yet"
                        }

                        Label {
                            color: theme.textMeta
                            Layout.fillWidth: true
                            wrapMode: Text.WordWrap
                            text: root.selectedVersionId && root.nodeById(root.selectedVersionId) ? ("Time: " + (root.nodeById(root.selectedVersionId).timestamp || "unknown") + "\nCurrent note: " + ((root.nodeById(root.selectedVersionId).note || "").length > 0 ? root.nodeById(root.selectedVersionId).note : "<none>")) : "Select a blob in the graph to inspect metadata."
                        }

                        TextArea {
                            id: versionNoteEdit
                            Layout.fillWidth: true
                            Layout.preferredHeight: 120
                            enabled: root.selectedVersionId.length > 0
                            placeholderText: "Write a note for selected version..."
                            wrapMode: TextEdit.Wrap
                        }

                        Button {
                            text: "Save Version Note"
                            Layout.fillWidth: true
                            Layout.preferredHeight: 36
                            enabled: root.hasBackend && root.selectedVersionId.length > 0
                            onClicked: {
                                if (root.backend.saveVersionNote(root.selectedVersionId, versionNoteEdit.text))
                                    root.refreshVersionGraph();
                            }
                        }

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 8

                            Button {
                                text: "Open Version"
                                Layout.fillWidth: true
                                Layout.preferredHeight: 36
                                enabled: root.hasBackend && root.selectedVersionId.length > 0
                                onClicked: {
                                    if (root.backend.restoreVersionById(root.selectedVersionId)) {
                                        root.refreshVersionGraph();
                                        root.close();
                                    }
                                }
                            }

                            Button {
                                text: "Delete"
                                Layout.fillWidth: true
                                Layout.preferredHeight: 36
                                enabled: root.hasBackend && root.selectedVersionId.length > 0
                                onClicked: deleteConfirmDialog.open()
                            }
                        }

                        // Breathing room above the scroll bar when the panel scrolls.
                        Item {
                            Layout.preferredHeight: 4
                        }
                    }
                }
            }
        }
    }
}
