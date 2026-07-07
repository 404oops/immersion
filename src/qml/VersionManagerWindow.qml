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

    readonly property int popupMinWidth: 720
    readonly property int popupMinHeight: 480
    readonly property int popupMaxWidth: 1120
    readonly property int popupMaxHeight: 700

    anchors.centerIn: parent
    width: {
        if (!parent)
            return popupMaxWidth;
        const available = Math.max(0, parent.width - 48);
        const preferred = Math.min(popupMaxWidth, Math.max(900, available));
        return Math.min(available, Math.max(popupMinWidth, preferred));
    }
    height: {
        if (!parent)
            return popupMaxHeight;
        const available = Math.max(0, parent.height - 48);
        const preferred = Math.min(popupMaxHeight, Math.max(560, available));
        return Math.min(available, Math.max(popupMinHeight, preferred));
    }

    background: Rectangle {
        color: theme.vmPanel
        radius: 10
        border.color: theme.vmBorder
        border.width: 1
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
        transformOrigin: Item.Center

        enter: Transition {
            ParallelAnimation {
                NumberAnimation {
                    property: "opacity"
                    from: 0.0
                    to: 1.0
                    duration: 180
                    easing.type: Easing.OutCubic
                }
                NumberAnimation {
                    property: "scale"
                    from: 0.98
                    to: 1.0
                    duration: 180
                    easing.type: Easing.OutCubic
                }
            }
        }

        exit: Transition {
            NumberAnimation {
                property: "opacity"
                from: 1.0
                to: 0.0
                duration: 120
                easing.type: Easing.InCubic
            }
        }

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
                                    root.refreshVersionGraph();
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
                                        root.refreshVersionGraph();
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
