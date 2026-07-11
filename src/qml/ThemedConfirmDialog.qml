pragma ComponentBehavior: Bound

import QtQuick 2.15
import QtQuick.Controls 2.15
import QtQuick.Layouts 1.15
import "."

ThemedPopup {
    id: root

    property string title: ""
    property string message: ""
    property string confirmText: "Confirm"
    property bool danger: false

    signal confirmed()

    popupMinWidth: 420
    popupMinHeight: 230
    popupMaxWidth: 520
    popupMaxHeight: 320
    popupPreferredWidth: 480
    popupPreferredHeight: 260

    Theme {
        id: theme
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 18
        spacing: 14

        Label {
            Layout.fillWidth: true
            text: root.title
            color: theme.vmTextPrimary
            font.pixelSize: 18
            font.bold: true
            wrapMode: Text.WordWrap
        }

        Label {
            Layout.fillWidth: true
            Layout.fillHeight: true
            text: root.message
            color: theme.vmTextMeta
            font.pixelSize: 13
            lineHeight: 1.35
            wrapMode: Text.WordWrap
            verticalAlignment: Text.AlignTop
        }

        RowLayout {
            Layout.fillWidth: true
            spacing: 10

            Item {
                Layout.fillWidth: true
            }

            PanelButton {
                text: "Cancel"
                Layout.preferredWidth: 96
                onClicked: root.close()
            }

            PanelButton {
                text: root.confirmText
                variant: root.danger ? "danger" : "primary"
                Layout.preferredWidth: 112
                onClicked: {
                    root.confirmed();
                    root.close();
                }
            }
        }
    }
}
