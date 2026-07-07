pragma ComponentBehavior: Bound

import QtQuick 2.15
import QtQuick.Controls 2.15
import "."

// Soft tinted button for panels/modals. Not as loud as ActionButton.
Button {
    id: control

    property string variant: "default" // default | primary | danger

    implicitHeight: 38
    padding: 12
    font.pixelSize: 13
    font.weight: variant === "danger" ? Font.Medium : Font.Normal

    Theme {
        id: theme
    }

    readonly property color fillColor: {
        if (variant === "primary") {
            return control.pressed || control.hovered ? theme.buttonPrimaryFillHover : theme.buttonPrimaryFill
        }
        if (variant === "danger") {
            return control.pressed || control.hovered ? theme.buttonDangerFillHover : theme.buttonDangerFill
        }
        return control.pressed || control.hovered ? theme.buttonSoftFillHover : theme.buttonSoftFill
    }

    readonly property color labelColor: {
        if (variant === "primary")
            return theme.buttonPrimaryLabel
        if (variant === "danger")
            return theme.buttonDangerLabel
        return theme.buttonSoftLabel
    }

    readonly property color borderColor: {
        if (variant === "primary")
            return theme.buttonPrimaryBorder
        if (variant === "danger")
            return theme.buttonDangerBorder
        return theme.buttonSoftBorder
    }

    background: Rectangle {
        radius: 8
        color: control.fillColor
        border.width: control.variant === "default" && !theme.isDarkMode ? 1 : (theme.isDarkMode ? 0 : 1)
        border.color: control.borderColor
        opacity: control.enabled ? 1.0 : 0.55
        Behavior on color {
            ColorAnimation {
                duration: 140
                easing.type: Easing.OutCubic
            }
        }
    }

    contentItem: Text {
        text: control.text
        font: control.font
        color: control.labelColor
        horizontalAlignment: Text.AlignHCenter
        verticalAlignment: Text.AlignVCenter
        elide: Text.ElideRight
        opacity: control.enabled ? 1.0 : 0.65
    }
}
