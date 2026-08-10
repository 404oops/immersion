pragma ComponentBehavior: Bound

import QtQuick 2.15
import QtQuick.Controls 2.15
import QtQuick.Layouts 1.15
import "."

ThemedPopup {
    id: root
    property var backend: null
    readonly property bool hasBackend: backend !== null && backend !== undefined
    readonly property bool childDialogOpened: deleteConfirmDialog.opened
    property var designGraph: buildEdgeCaseGraph()

    popupMinWidth: 720
    popupMinHeight: 480
    popupMaxWidth: 1120
    popupMaxHeight: 700
    popupPreferredWidth: 900
    popupPreferredHeight: 560

    Theme {
        id: theme
    }

    function buildEdgeCaseGraph() {
        const graph = [];
        let row = 0;

        // 1x2x2x2x2: one root, each level branches by 2 for four levels.
        const depthX = [88, 264, 440, 616, 792];
        const rowStep = 92;

        graph.push({
            id: "1",
            label: "v1",
            fullLabel: "v1",
            x: depthX[0],
            y: 72 + row * rowStep,
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
                    y: 72 + row * rowStep,
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

    function defaultSelectedVersionId() {
        const current = currentVersionNode();
        if (current)
            return current.id;

        if (versionGraph.length === 0)
            return "";

        let latest = versionGraph[0];
        for (let i = 1; i < versionGraph.length; ++i) {
            const node = versionGraph[i];
            const nodeTime = node.timestamp || "";
            const latestTime = latest.timestamp || "";
            if (nodeTime > latestTime)
                latest = node;
        }
        return latest.id;
    }

    function applyVersionSelection(selectLatest) {
        if (versionGraph.length === 0) {
            selectedVersionId = "";
            versionNoteEdit.text = "";
            return;
        }

        if (selectLatest || !selectedVersionId || !nodeById(selectedVersionId))
            selectedVersionId = defaultSelectedVersionId();

        const selected = nodeById(selectedVersionId);
        versionNoteEdit.text = selected ? (selected.note || "") : "";
    }

    function ancestorIdSet(versionId) {
        const ids = {};
        let currentId = versionId;
        while (currentId) {
            ids[currentId] = true;
            const node = nodeById(currentId);
            currentId = node && node.parentId ? node.parentId : "";
        }
        return ids;
    }

    function refreshVersionGraph(selectLatest) {
        versionGraph = hasBackend ? root.backend.selectedProjectVersionGraph : designGraph;
        applyVersionSelection(!!selectLatest);

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

            const nodeHalfW = 38;
            const nodeHalfH = 23;
            const labelPad = 28;
            const viewportPad = 32;
            graphExtentWidth = (maxX - minX) + (nodeHalfW + viewportPad) * 2;
            graphExtentHeight = (maxY - minY) + (nodeHalfH + labelPad + viewportPad) * 2;
        }

        graphCanvas.requestPaint();
    }

    onSelectedVersionIdChanged: graphCanvas.requestPaint()

    function openForProject(index) {
        if (!hasBackend)
            return;
        root.backend.manageProjectVersions(index);
        refreshVersionGraph(true);
        projectNoteEdit.text = root.backend.selectedProjectNote;
        syncPrimaryFileCombo();
        open();
    }

    function syncPrimaryFileCombo() {
        if (!root.hasBackend) {
            primaryFileCombo.model = [];
            primaryFileCombo.currentIndex = -1;
            return;
        }

        const files = root.backend.selectedProjectFiles;
        primaryFileCombo.model = files;
        const currentFile = root.backend.selectedProjectPrimaryFile;
        let index = -1;
        for (let i = 0; i < files.length; ++i) {
            if (files[i] === currentFile) {
                index = i;
                break;
            }
        }
        primaryFileCombo.currentIndex = index >= 0 ? index : (files.length > 0 ? 0 : -1);
    }

    Component.onCompleted: {
        refreshVersionGraph(true);
        if (!hasBackend)
            projectNoteEdit.text = "Preview project note";
    }

    Connections {
        target: root.backend
        function onSelectedProjectVersionGraphChanged() {
            if (root.opened && root.hasBackend) {
                root.refreshVersionGraph(false);
            }
        }
        function onSelectedProjectNoteChanged() {
            if (root.opened && root.hasBackend) {
                projectNoteEdit.text = root.backend.selectedProjectNote;
            }
        }
        function onSelectedProjectFilesChanged() {
            if (root.opened && root.hasBackend) {
                root.syncPrimaryFileCombo();
            }
        }
        function onSelectedProjectPrimaryFileChanged() {
            if (root.opened && root.hasBackend) {
                root.syncPrimaryFileCombo();
            }
        }
    }

    ThemedConfirmDialog {
        id: deleteConfirmDialog
        z: 40
        title: "Delete Version"
        message: "Delete version v" + root.selectedVersionId
                 + "?\n\nThis removes its snapshot from .musit and cannot be undone."
        confirmText: "Delete"
        danger: true
        onConfirmed: {
            if (root.backend.deleteVersionById(root.selectedVersionId)) {
                root.refreshVersionGraph(false);
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
                color: theme.vmTextPrimary
            }
            Item {
                Layout.fillWidth: true
            }
            PanelButton {
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
                color: theme.vmGraph
                radius: 8
                border.width: 1
                border.color: theme.vmBorder

                Label {
                    anchors.centerIn: parent
                    visible: root.versionGraph.length === 0
                    text: "No versions yet"
                    color: theme.vmTextMeta
                    font.pixelSize: 15
                }

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

                                const gridStep = 28;
                                ctx.fillStyle = theme.vmGraphGrid;
                                for (let gx = gridStep; gx < width; gx += gridStep) {
                                    for (let gy = gridStep; gy < height; gy += gridStep) {
                                        ctx.beginPath();
                                        ctx.arc(gx, gy, 1.1, 0, Math.PI * 2);
                                        ctx.fill();
                                    }
                                }

                                const halfW = 38;
                                const halfH = 23;
                                const ancestors = ancestorIdSet(root.selectedVersionId);

                                function drawLink(parent, child, highlighted) {
                                    const px = parent.x;
                                    const py = parent.y;
                                    const cx = child.x;
                                    const cy = child.y;
                                    const x1 = px + halfW;
                                    const y1 = py;
                                    const x2 = cx - halfW;
                                    const y2 = cy;
                                    const midX = x1 + (x2 - x1) * 0.5;

                                    ctx.beginPath();
                                    ctx.moveTo(x1, y1);
                                    ctx.lineTo(midX, y1);
                                    ctx.lineTo(midX, y2);
                                    ctx.lineTo(x2, y2);
                                    ctx.strokeStyle = highlighted ? theme.vmGraphLinkActive : theme.vmGraphLink;
                                    ctx.lineWidth = highlighted ? 2.5 : 1.75;
                                    ctx.lineCap = "round";
                                    ctx.lineJoin = "round";
                                    ctx.stroke();
                                }

                                for (let i = 0; i < root.versionGraph.length; ++i) {
                                    const node = root.versionGraph[i];
                                    if (!node.parentId)
                                        continue;
                                    const parent = root.nodeById(node.parentId);
                                    if (!parent)
                                        continue;
                                    const highlighted = ancestors[node.parentId] && ancestors[node.id];
                                    drawLink(parent, node, highlighted);
                                }
                            }
                        }

                        Repeater {
                            model: root.versionGraph
                            delegate: VersionGraphNode {
                                required property var modelData

                                node: modelData
                                selected: root.selectedVersionId === modelData.id
                                current: !!modelData.isCurrent

                                onClicked: {
                                    root.selectedVersionId = modelData.id;
                                    versionNoteEdit.text = modelData.note || "";
                                }
                            }
                        }
                    }
                }
            }

            Rectangle {
                Layout.preferredWidth: 340
                Layout.fillHeight: true
                color: theme.vmSidePanel
                radius: 8
                border.width: 1
                border.color: theme.vmBorder

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
                            color: theme.vmTextPrimary
                            font.bold: true
                        }

                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: 6
                            visible: root.hasBackend && root.backend.selectedProjectFiles.length > 1

                            Label {
                                text: "Main project file"
                                color: theme.vmTextPrimary
                                font.bold: true
                            }

                            Label {
                                Layout.fillWidth: true
                                text: "Choose which file Immersion opens and versions when several project files share this folder."
                                wrapMode: Text.WordWrap
                                color: theme.vmTextMeta
                                font.pixelSize: 12
                                lineHeight: 1.35
                            }

                            ThemedComboBox {
                                id: primaryFileCombo
                                Layout.fillWidth: true
                                enabled: root.hasBackend
                                onActivated: function (index) {
                                    if (root.hasBackend && index >= 0)
                                        root.backend.selectedProjectPrimaryFile = primaryFileCombo.textAt(index);
                                }
                            }
                        }

                        ThemedTextArea {
                            id: projectNoteEdit
                            Layout.fillWidth: true
                            Layout.preferredHeight: 110
                            placeholderText: "Write a note for this project..."
                        }

                        PanelButton {
                            text: "Save Project Note"
                            variant: "primary"
                            Layout.fillWidth: true
                            Layout.preferredHeight: 36
                            enabled: root.hasBackend
                            onClicked: root.backend.selectedProjectNote = projectNoteEdit.text
                        }

                        Label {
                            color: theme.vmTextPrimary
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
                            color: theme.vmTextMeta
                            Layout.fillWidth: true
                            wrapMode: Text.WordWrap
                            text: root.selectedVersionId && root.nodeById(root.selectedVersionId) ? ("Time: " + (root.nodeById(root.selectedVersionId).timestamp || "unknown") + "\nCurrent note: " + ((root.nodeById(root.selectedVersionId).note || "").length > 0 ? root.nodeById(root.selectedVersionId).note : "<none>")) : "Select a blob in the graph to inspect metadata."
                        }

                        ThemedTextArea {
                            id: versionNoteEdit
                            Layout.fillWidth: true
                            Layout.preferredHeight: 120
                            enabled: root.selectedVersionId.length > 0
                            placeholderText: "Write a note for selected version..."
                        }

                        PanelButton {
                            text: "Save Version Note"
                            variant: "primary"
                            Layout.fillWidth: true
                            Layout.preferredHeight: 36
                            enabled: root.hasBackend && root.selectedVersionId.length > 0
                            onClicked: {
                                if (root.backend.saveVersionNote(root.selectedVersionId, versionNoteEdit.text))
                                    root.refreshVersionGraph(false);
                            }
                        }

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 8

                            PanelButton {
                                text: "Open Version"
                                variant: "primary"
                                Layout.fillWidth: true
                                Layout.preferredHeight: 36
                                enabled: root.hasBackend && root.selectedVersionId.length > 0
                                onClicked: {
                                    if (root.backend.restoreVersionById(root.selectedVersionId)) {
                                        root.refreshVersionGraph(true);
                                        root.close();
                                    }
                                }
                            }

                            PanelButton {
                                text: "Delete"
                                variant: "danger"
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
