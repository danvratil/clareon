// SPDX-FileContributor: Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as Controls
import org.kde.kirigami as Kirigami
import cz.dvratil.clareon

Kirigami.ScrollablePage {
    id: root

    title: qsTr("Notifications Settings")

    property var config
    property bool isDirty: false

    ColumnLayout {
        width: root.width
        spacing: Kirigami.Units.largeSpacing

        Kirigami.InlineMessage {
            Layout.fillWidth: true
            type: Kirigami.MessageType.Information
            text: qsTr("Notification settings are not yet implemented. This page is a placeholder for future functionality.")
            visible: true
        }

        // General Notification Settings (Disabled placeholder)
        Kirigami.FormLayout {
            Layout.fillWidth: true

            Kirigami.Separator {
                Kirigami.FormData.isSection: true
                Kirigami.FormData.label: qsTr("General")
            }

            Controls.CheckBox {
                id: enableNotificationsCheckBox
                Kirigami.FormData.label: qsTr("Enable notifications:")
                text: qsTr("Show desktop notifications")
                checked: true
                enabled: false
            }

            Controls.CheckBox {
                id: notifyWhenMinimizedCheckBox
                Kirigami.FormData.label: qsTr("When minimized:")
                text: qsTr("Only notify when window is minimized")
                checked: false
                enabled: false
            }
        }

        // Response Notifications (Disabled placeholder)
        Kirigami.FormLayout {
            Layout.fillWidth: true

            Kirigami.Separator {
                Kirigami.FormData.isSection: true
                Kirigami.FormData.label: qsTr("Response Events")
            }

            Controls.CheckBox {
                id: notifyResponseCompleteCheckBox
                Kirigami.FormData.label: qsTr("Response complete:")
                text: qsTr("Notify when Claude finishes responding")
                checked: true
                enabled: false
            }

            Controls.CheckBox {
                id: notifyErrorCheckBox
                Kirigami.FormData.label: qsTr("Errors:")
                text: qsTr("Notify when an error occurs")
                checked: true
                enabled: false
            }
        }

        // Tool Execution Notifications (Disabled placeholder)
        Kirigami.FormLayout {
            Layout.fillWidth: true

            Kirigami.Separator {
                Kirigami.FormData.isSection: true
                Kirigami.FormData.label: qsTr("Tool Execution")
            }

            Controls.CheckBox {
                id: notifyToolRequestCheckBox
                Kirigami.FormData.label: qsTr("Tool requests:")
                text: qsTr("Notify when Claude wants to use a tool")
                checked: false
                enabled: false
            }

            Controls.CheckBox {
                id: notifyToolCompleteCheckBox
                Kirigami.FormData.label: qsTr("Tool completion:")
                text: qsTr("Notify when tool execution completes")
                checked: false
                enabled: false
            }

            Controls.CheckBox {
                id: notifyToolErrorCheckBox
                Kirigami.FormData.label: qsTr("Tool errors:")
                text: qsTr("Notify when tool execution fails")
                checked: true
                enabled: false
            }
        }

        // Notification Appearance (Disabled placeholder)
        Kirigami.FormLayout {
            Layout.fillWidth: true

            Kirigami.Separator {
                Kirigami.FormData.isSection: true
                Kirigami.FormData.label: qsTr("Appearance")
            }

            Controls.CheckBox {
                id: showPreviewCheckBox
                Kirigami.FormData.label: qsTr("Preview:")
                text: qsTr("Show message preview in notifications")
                checked: true
                enabled: false
            }

            Controls.SpinBox {
                id: previewLengthSpinBox
                Kirigami.FormData.label: qsTr("Preview length:")
                from: 50
                to: 500
                value: 150
                stepSize: 50
                enabled: false
            }

            Controls.Label {
                text: qsTr("Maximum number of characters to show in notification preview")
                font.pointSize: Kirigami.Theme.smallFont.pointSize
                color: Kirigami.Theme.disabledTextColor
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
            }
        }
    }
}
