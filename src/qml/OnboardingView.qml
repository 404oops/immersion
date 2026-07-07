pragma ComponentBehavior: Bound

import QtQuick 2.15
import QtQuick.Controls 2.15
import QtQuick.Layouts 1.15
import "."

Rectangle {
    id: root
    color: theme.appBackground

    Theme {
        id: theme
    }

    signal openSettingsRequested

    ColumnLayout {
        anchors.centerIn: parent
        anchors.margins: 40
        spacing: 32
        width: Math.min(600, parent.width - 80)

        ColumnLayout {
            Layout.fillWidth: true
            spacing: 16

            Text {
                text: "Welcome to Immersion"
                font.pixelSize: 48
                font.weight: Font.Bold
                color: theme.textPrimary
                horizontalAlignment: Text.AlignHCenter
                Layout.fillWidth: true
            }

            Text {
                text: "Open Settings to choose the folder where all your project files sit, and we'll take care of the rest."
                font.pixelSize: 16
                color: theme.textSecondary
                wrapMode: Text.Wrap
                horizontalAlignment: Text.AlignHCenter
                Layout.fillWidth: true
                lineHeight: 1.5
            }
        }

        Button {
            id: button
            text: "Open Settings"
            Layout.fillWidth: true
            Layout.preferredHeight: 48
            font.pixelSize: 14
            font.weight: Font.Medium

            background: Rectangle {
                color: button.hovered || button.pressed ? theme.buttonFillHover : theme.buttonFill
                radius: 4
                Behavior on color {
                    ColorAnimation {
                        duration: 150
                    }
                }
            }

            contentItem: Text {
                text: button.text
                color: theme.buttonLabel
                font: button.font
                horizontalAlignment: Text.AlignHCenter
                verticalAlignment: Text.AlignVCenter
            }

            onClicked: root.openSettingsRequested()
        }
    }
}
