pragma ComponentBehavior: Bound

import QtQuick 2.15
import QtQuick.Controls 2.15
import "."

Slider {
    id: control

    implicitHeight: 28
    padding: 6

    Theme {
        id: theme
    }

    background: Rectangle {
        x: control.leftPadding
        y: control.topPadding + control.availableHeight / 2 - height / 2
        implicitHeight: 5
        width: control.availableWidth
        height: implicitHeight
        radius: height / 2
        color: theme.inputBorder

        Rectangle {
            width: control.visualPosition * parent.width
            height: parent.height
            radius: parent.radius
            color: theme.accent
            opacity: theme.isDarkMode ? 0.45 : 0.55
        }
    }

    handle: Rectangle {
        x: control.leftPadding + control.visualPosition * (control.availableWidth - width)
        y: control.topPadding + control.availableHeight / 2 - height / 2
        implicitWidth: 14
        implicitHeight: 14
        radius: width / 2
        color: theme.inputSurface
        border.color: theme.inputBorderAccent
        border.width: 1
        opacity: control.pressed ? 0.92 : 1.0
    }
}
