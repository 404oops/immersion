pragma ComponentBehavior: Bound

import QtQuick 2.15
import QtQuick.Controls 2.15
import "."

ComboBox {
    id: control

    implicitHeight: 30
    spacing: 6
    leftPadding: 10
    rightPadding: control.indicator.width + 6
    font.pixelSize: 12

    Theme {
        id: theme
    }

    background: Rectangle {
        radius: 5
        color: theme.inputFill
        border.width: control.activeFocus || control.popup.visible ? 1.5 : 1
        border.color: control.activeFocus || control.popup.visible ? theme.accent : theme.inputBorderAccent
        Behavior on border.color {
            ColorAnimation {
                duration: 120
            }
        }
    }

    contentItem: Text {
        text: control.displayText
        font: control.font
        color: theme.textPrimary
        verticalAlignment: Text.AlignVCenter
        elide: Text.ElideRight
    }

    indicator: Text {
        x: control.width - width - 10
        y: (control.height - height) / 2
        text: "▾"
        color: theme.accent
        font.pixelSize: 11
    }

    popup: Popup {
        y: control.height + 2
        width: control.width
        implicitHeight: Math.min(contentItem.implicitHeight + 6, 220)
        padding: 3

        contentItem: ListView {
            clip: true
            implicitHeight: contentHeight
            model: control.popup.visible ? control.delegateModel : null
            currentIndex: control.highlightedIndex

            ScrollIndicator.vertical: ScrollIndicator { }
        }

        background: Rectangle {
            color: theme.inputFill
            border.color: theme.inputBorderAccent
            border.width: 1
            radius: 5
        }
    }

    delegate: ItemDelegate {
        id: delegateItem
        required property var modelData
        required property int index

        width: control.width - 6
        height: 28

        contentItem: Text {
            text: delegateItem.modelData
            font: control.font
            color: theme.textPrimary
            elide: Text.ElideRight
            verticalAlignment: Text.AlignVCenter
            leftPadding: 8
        }

        background: Rectangle {
            radius: 4
            color: delegateItem.highlighted
                   ? theme.selection
                   : (delegateItem.hovered ? theme.rowEven : "transparent")
        }
    }
}
