pragma ComponentBehavior: Bound

import QtQuick 2.15
import QtQuick.Controls 2.15
import "."

TextField {
    id: control

    implicitHeight: 30
    leftPadding: 10
    rightPadding: 10
    color: theme.textPrimary
    placeholderTextColor: theme.textMuted
    selectionColor: theme.selection
    selectedTextColor: theme.textPrimary
    font.pixelSize: 12

    Theme {
        id: theme
    }

    background: Rectangle {
        radius: 5
        color: theme.inputFill
        border.width: control.activeFocus ? 1.5 : 1
        border.color: control.activeFocus ? theme.accent : theme.inputBorderAccent
        Behavior on border.color {
            ColorAnimation {
                duration: 120
            }
        }
    }
}
