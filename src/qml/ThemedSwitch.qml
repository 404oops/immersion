pragma ComponentBehavior: Bound

import QtQuick 2.15
import QtQuick.Controls 2.15
import "."

Switch {
    id: control

    implicitWidth: indicator.implicitWidth
    implicitHeight: indicator.implicitHeight

    contentItem: Item {
        implicitWidth: 0
        implicitHeight: 0
        visible: false
    }

    Theme {
        id: theme
    }

    indicator: Item {
        implicitWidth: 46
        implicitHeight: 26

        Rectangle {
            id: track
            anchors.fill: parent
            radius: height / 2
            color: control.checked ? theme.accent : theme.inputBorder
            opacity: control.enabled
                   ? (control.checked ? (theme.isDarkMode ? 0.5 : 0.58) : 1.0)
                   : 0.45

            Behavior on color {
                ColorAnimation {
                    duration: 140
                    easing.type: Easing.OutCubic
                }
            }
        }

        Rectangle {
            id: thumb
            x: control.checked ? track.width - width - 3 : 3
            anchors.verticalCenter: parent.verticalCenter
            width: 20
            height: 20
            radius: width / 2
            color: theme.inputSurface
            border.color: theme.inputBorderAccent
            border.width: 1

            Behavior on x {
                NumberAnimation {
                    duration: 140
                    easing.type: Easing.OutCubic
                }
            }
        }
    }
}
