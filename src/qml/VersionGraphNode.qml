pragma ComponentBehavior: Bound

import QtQuick 2.15
import QtQuick.Controls 2.15
import "."

Item {
    id: nodeRoot

    property var node: null
    property bool selected: false
    property bool current: false

    readonly property int nodeWidth: 76
    readonly property int nodeHeight: 46

    width: nodeWidth
    height: nodeHeight
    x: (node ? Number(node.x) : 0) - nodeWidth / 2
    y: (node ? Number(node.y) : 0) - nodeHeight / 2

    scale: mouseArea.pressed ? 0.97 : (mouseArea.containsMouse ? 1.03 : 1.0)

    Behavior on scale {
        NumberAnimation {
            duration: 130
            easing.type: Easing.OutCubic
        }
    }

    Theme {
        id: theme
    }

    Rectangle {
        anchors.fill: parent
        anchors.topMargin: 2
        radius: 12
        color: theme.vmNodeShadow
        opacity: selected ? 0.35 : 0.22
    }

    Rectangle {
        id: body
        anchors.fill: parent
        radius: 12
        color: current ? theme.vmNodeCurrentFill
                       : (selected ? theme.vmNodeSelectedFill : theme.vmNodeFill)
        border.color: current ? theme.successBorder
                              : (selected ? theme.vmNodeSelectedBorder : theme.nodeBorder)
        border.width: selected ? 2 : 1

        Behavior on color {
            ColorAnimation {
                duration: 140
            }
        }
    }

    Column {
        anchors.centerIn: parent
        spacing: 1

        Label {
            anchors.horizontalCenter: parent.horizontalCenter
            text: nodeRoot.node && nodeRoot.node.label ? nodeRoot.node.label : "?"
            color: current || selected ? "#ffffff" : theme.vmNodeLabel
            font.bold: true
            font.pixelSize: 12
        }

        Label {
            anchors.horizontalCenter: parent.horizontalCenter
            visible: nodeRoot.node && nodeRoot.node.fullLabel && nodeRoot.node.fullLabel !== nodeRoot.node.label
            text: nodeRoot.node && nodeRoot.node.fullLabel ? nodeRoot.node.fullLabel : ""
            color: current || selected ? "#eef2ff" : theme.vmTextMeta
            font.pixelSize: 9
            opacity: 0.92
        }
    }

    Rectangle {
        visible: current
        anchors.horizontalCenter: body.horizontalCenter
        anchors.bottom: body.top
        anchors.bottomMargin: 4
        width: currentBadge.implicitWidth + 10
        height: 16
        radius: 8
        color: theme.successStrong

        Label {
            id: currentBadge
            anchors.centerIn: parent
            text: "CURRENT"
            color: "#ffffff"
            font.pixelSize: 8
            font.bold: true
        }
    }

    signal clicked()

    MouseArea {
        id: mouseArea
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: nodeRoot.clicked()
    }
}
