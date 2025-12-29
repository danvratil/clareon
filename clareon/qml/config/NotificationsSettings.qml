// SPDX-FileContributor: Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as Controls
import org.kde.kirigami as Kirigami

Controls.ScrollView {
    id: root

    contentWidth: availableWidth

    ColumnLayout {
        width: root.width
        spacing: Kirigami.Units.largeSpacing

        // Page header
        Kirigami.Heading {
            text: qsTr("Notifications")
            level: 1
            Layout.fillWidth: true
        }

        // General Notification Settings
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
            }

            Controls.CheckBox {
                id: notifyWhenMinimizedCheckBox
                Kirigami.FormData.label: qsTr("When minimized:")
                text: qsTr("Only notify when window is minimized")
                checked: false
                enabled: enableNotificationsCheckBox.checked
            }
        }

        // Response Notifications
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
                enabled: enableNotificationsCheckBox.checked
            }

            Controls.CheckBox {
                id: notifyErrorCheckBox
                Kirigami.FormData.label: qsTr("Errors:")
                text: qsTr("Notify when an error occurs")
                checked: true
                enabled: enableNotificationsCheckBox.checked
            }
        }

        // Tool Execution Notifications
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
                enabled: enableNotificationsCheckBox.checked
            }

            Controls.CheckBox {
                id: notifyToolCompleteCheckBox
                Kirigami.FormData.label: qsTr("Tool completion:")
                text: qsTr("Notify when tool execution completes")
                checked: false
                enabled: enableNotificationsCheckBox.checked
            }

            Controls.CheckBox {
                id: notifyToolErrorCheckBox
                Kirigami.FormData.label: qsTr("Tool errors:")
                text: qsTr("Notify when tool execution fails")
                checked: true
                enabled: enableNotificationsCheckBox.checked
            }
        }

        // Notification Appearance
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
                enabled: enableNotificationsCheckBox.checked
            }

            Controls.SpinBox {
                id: previewLengthSpinBox
                Kirigami.FormData.label: qsTr("Preview length:")
                from: 50
                to: 500
                value: 150
                stepSize: 50
                enabled: enableNotificationsCheckBox.checked && showPreviewCheckBox.checked
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
