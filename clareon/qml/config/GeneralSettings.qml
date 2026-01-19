// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as Controls
import org.kde.kirigami as Kirigami
import cz.dvratil.clareon
import cc.clareon.core

Kirigami.ScrollablePage {
    id: root

    title: qsTr("General Settings")

    // Local config state - loaded from ConfigManagerQt
    property var config

    Component.onCompleted: {
        console.log("GeneralSettings: Loaded config:", root.config)
    }

    ColumnLayout {
        width: root.width
        spacing: Kirigami.Units.largeSpacing

        // User Identity Section
        Kirigami.FormLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true

            Kirigami.Separator {
                Kirigami.FormData.isSection: true
                Kirigami.FormData.label: qsTr("User Identity")
            }

            Controls.TextField {
                id: userNameField
                Kirigami.FormData.label: qsTr("What should Claude call you:")
                placeholderText: qsTr("Your name")
                Layout.fillWidth: true
                text: root.config.systemPrompt.userName || ""
                onTextChanged: {
                    if (text !== root.config.systemPrompt.userName) {
                        root.config.systemPrompt.userName = text || null
                    }
                }
            }

            Controls.TextArea {
                id: personalPrefsField
                Kirigami.FormData.label: qsTr("Personal preferences:")
                placeholderText: qsTr("Tell Claude about your preferences in responses (tone, style, verbosity, etc.)")
                Layout.fillWidth: true
                Layout.fillHeight: true
                wrapMode: TextEdit.Wrap
                text: root.config.systemPrompt.personalPreferences || ""
                onTextChanged: {
                    if (text !== root.config.systemPrompt.personalPreferences) {
                        root.config.systemPrompt.personalPreferences = text || null
                    }
                }
            }
        }

        // System Integration Section
        Kirigami.FormLayout {
            Layout.fillWidth: true

            Kirigami.Separator {
                Kirigami.FormData.isSection: true
                Kirigami.FormData.label: qsTr("System Integration")
            }

            Controls.CheckBox {
                id: minimizeToTrayCheckbox
                Kirigami.FormData.label: qsTr("Minimize to tray:")
                text: qsTr("Minimize to tray instead of closing")
                checked: root.config.ui.minimizeToTray || false
                onCheckedChanged: {
                    if (checked !== root.config.ui.minimizeToTray) {
                        root.config.ui.minimizeToTray = checked
                    }
                }
            }

            Controls.CheckBox {
                id: startMinimizedCheckbox
                Kirigami.FormData.label: qsTr("Start minimized:")
                text: qsTr("Start minimized to tray")
                checked: root.config.ui.startMinimized || false
                onCheckedChanged: {
                    if (checked !== root.config.ui.startMinimized) {
                        root.config.ui.startMinimized = checked
                    }
                }
            }

            Controls.CheckBox {
                id: autoStartCheckbox
                Kirigami.FormData.label: qsTr("Auto-start:")
                text: qsTr("Auto-start on login")
                checked: root.config.ui.autoStart || false
                onCheckedChanged: {
                    if (checked !== root.config.ui.autoStart) {
                        root.config.ui.autoStart = checked
                    }
                }
            }
        }
    }
}
