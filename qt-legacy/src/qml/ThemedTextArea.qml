pragma ComponentBehavior: Bound

import QtQuick 2.15
import QtQuick.Controls 2.15
import "."

TextArea {
    id: control

    implicitHeight: Math.max(80, contentHeight + topPadding + bottomPadding)
    leftPadding: 10
    rightPadding: 10
    topPadding: 8
    bottomPadding: 8
    color: theme.vmTextPrimary
    placeholderTextColor: theme.textMuted
    selectionColor: theme.selection
    selectedTextColor: theme.vmTextPrimary
    font.pixelSize: 13
    wrapMode: TextEdit.Wrap

    Theme {
        id: theme
    }

    background: Rectangle {
        radius: 6
        color: theme.vmInputSurface
        border.width: control.activeFocus ? 1.5 : 1
        border.color: control.activeFocus ? theme.inputBorderAccent : theme.vmBorder
        Behavior on border.color {
            ColorAnimation {
                duration: 120
            }
        }
    }
}
