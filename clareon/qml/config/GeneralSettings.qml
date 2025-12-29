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
            text: qsTr("General Settings")
            level: 1
            Layout.fillWidth: true
        }

        // User Identity Section
        Kirigami.FormLayout {
            Layout.fillWidth: true

            Kirigami.Separator {
                Kirigami.FormData.isSection: true
                Kirigami.FormData.label: qsTr("User Identity")
            }

            Controls.TextField {
                id: userNameField
                Kirigami.FormData.label: qsTr("What should Claude call you:")
                placeholderText: qsTr("Your name")
                Layout.fillWidth: true
            }

            Controls.TextArea {
                id: personalPrefsField
                Kirigami.FormData.label: qsTr("Personal preferences:")
                placeholderText: qsTr("Tell Claude about your preferences in responses (tone, style, verbosity, etc.)")
                Layout.fillWidth: true
                Layout.preferredHeight: 100
                Layout.minimumHeight: 100
                wrapMode: TextEdit.Wrap
            }
        }

        // Response Settings Section
        Kirigami.FormLayout {
            Layout.fillWidth: true

            Kirigami.Separator {
                Kirigami.FormData.isSection: true
                Kirigami.FormData.label: qsTr("Response Settings")
            }

            Controls.SpinBox {
                id: maxTokensField
                Kirigami.FormData.label: qsTr("Max output tokens:")
                from: 1024
                to: 8192
                value: 4096
                stepSize: 512
                editable: true
            }

            Controls.Label {
                text: qsTr("Higher values allow longer responses but cost more")
                font.pointSize: Kirigami.Theme.smallFont.pointSize
                color: Kirigami.Theme.disabledTextColor
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
            }
       }

        // Application Behavior Section
        Kirigami.FormLayout {
            Layout.fillWidth: true

            Kirigami.Separator {
                Kirigami.FormData.isSection: true
                Kirigami.FormData.label: qsTr("Application Behavior")
            }

            Controls.CheckBox {
                id: autostartCheckBox
                Kirigami.FormData.label: qsTr("Autostart:")
                text: qsTr("Launch Clareon on login")
                checked: false
            }

            Controls.CheckBox {
                id: systemTrayCheckBox
                Kirigami.FormData.label: qsTr("System tray:")
                text: qsTr("Close to system tray instead of quitting")
                checked: false
            }

            Controls.CheckBox {
                id: startMinimizedCheckBox
                Kirigami.FormData.label: qsTr("Start minimized:")
                text: qsTr("Start minimized to system tray")
                checked: false
                enabled: systemTrayCheckBox.checked
            }
        }
    }
}
