pragma ComponentBehavior: Bound

import QtQuick 2.15
import QtQuick.Controls 2.15
import "."

SpinBox {
    id: control

    implicitHeight: 32
    implicitWidth: 118
    leftPadding: 30
    rightPadding: 30
    font.pixelSize: 12

    Theme {
        id: theme
    }

    contentItem: TextInput {
        z: 2
        text: control.textFromValue(control.value, control.locale)
        font: control.font
        color: theme.textPrimary
        selectionColor: theme.selection
        selectedTextColor: theme.textPrimary
        horizontalAlignment: Qt.AlignHCenter
        verticalAlignment: Qt.AlignVCenter
        readOnly: !control.editable
        validator: control.validator
        inputMethodHints: Qt.ImhFormattedNumbersOnly
    }

    background: Rectangle {
        radius: 6
        color: theme.inputFill
        border.width: control.activeFocus ? 1.5 : 1
        border.color: control.activeFocus ? theme.accent : theme.inputBorderAccent
        opacity: control.enabled ? 1.0 : 0.55

        Behavior on border.color {
            ColorAnimation {
                duration: 120
            }
        }
    }

    up.indicator: Rectangle {
        x: control.width - width - 4
        y: 4
        width: 24
        height: control.height - 8
        radius: 4
        color: up.pressed
               ? theme.buttonSoftFillHover
               : (up.hovered ? theme.buttonSoftFill : "transparent")
        opacity: control.enabled ? 1.0 : 0.45

        Label {
            anchors.centerIn: parent
            text: "+"
            color: theme.textSecondary
            font.pixelSize: 13
        }
    }

    down.indicator: Rectangle {
        x: 4
        y: 4
        width: 24
        height: control.height - 8
        radius: 4
        color: down.pressed
               ? theme.buttonSoftFillHover
               : (down.hovered ? theme.buttonSoftFill : "transparent")
        opacity: control.enabled ? 1.0 : 0.45

        Label {
            anchors.centerIn: parent
            text: "−"
            color: theme.textSecondary
            font.pixelSize: 13
        }
    }
}
