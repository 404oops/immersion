pragma ComponentBehavior: Bound

import QtQuick 2.15
import QtQuick.Controls 2.15
import "."

Popup {
    id: root

    property int popupMinWidth: 560
    property int popupMinHeight: 480
    property int popupMaxWidth: 760
    property int popupMaxHeight: 720
    property int popupPreferredWidth: 680
    property int popupPreferredHeight: 640

    parent: Overlay.overlay
    modal: true
    focus: true
    padding: 0
    closePolicy: Popup.CloseOnEscape
    transformOrigin: Item.Center
    dim: false

    Theme {
        id: theme
    }

    enter: Transition {
        ParallelAnimation {
            NumberAnimation {
                property: "opacity"
                from: 0.0
                to: 1.0
                duration: theme.modalEnterDuration
                easing.type: Easing.OutCubic
            }
            NumberAnimation {
                property: "scale"
                from: theme.modalEnterScale
                to: 1.0
                duration: theme.modalEnterDuration
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
                duration: theme.modalExitDuration
                easing.type: Easing.InCubic
            }
            NumberAnimation {
                property: "scale"
                from: 1.0
                to: theme.modalExitScale
                duration: theme.modalExitDuration
                easing.type: Easing.InCubic
            }
        }
    }

    anchors.centerIn: parent
    width: {
        if (!parent)
            return popupMaxWidth;
        const available = Math.max(0, parent.width - theme.modalEdgePadding);
        return Math.min(available, Math.max(popupMinWidth, Math.min(popupMaxWidth, popupPreferredWidth)));
    }
    height: {
        if (!parent)
            return popupMaxHeight;
        const available = Math.max(0, parent.height - theme.modalEdgePadding);
        return Math.min(available, Math.max(popupMinHeight, Math.min(popupMaxHeight, popupPreferredHeight)));
    }

    background: Rectangle {
        color: theme.vmPanel
        radius: theme.modalPanelRadius
        border.color: theme.modalBorder
        border.width: 1
    }

    default property alias content: contentHost.data

    Item {
        id: contentHost
        anchors.fill: parent
    }
}
