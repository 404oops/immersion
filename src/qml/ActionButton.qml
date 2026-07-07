pragma ComponentBehavior: Bound

import QtQuick 2.15
import QtQuick.Controls 2.15
import "."

// Themed button with explicit sizing. Parent should set Layout.fillHeight for row actions.
Button {
    id: control

    implicitWidth: 96
    implicitHeight: 36
    padding: 8
    font.pixelSize: 13

    Theme {
        id: theme
    }

    background: Rectangle {
        radius: 4
        color: control.pressed || control.hovered ? theme.buttonFillHover : theme.buttonFill
        Behavior on color {
            ColorAnimation {
                duration: 120
            }
        }
    }

    contentItem: Text {
        text: control.text
        font: control.font
        color: theme.buttonLabel
        horizontalAlignment: Text.AlignHCenter
        verticalAlignment: Text.AlignVCenter
        elide: Text.ElideRight
    }
}
