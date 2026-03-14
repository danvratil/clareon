// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as Controls
import org.kde.kirigami as Kirigami
import cz.dvratil.clareon

Kirigami.ScrollablePage {
    id: root

    title: qsTr("Advanced Settings")

    property var config
    property bool isDirty: false

    ColumnLayout {
        width: root.width
        spacing: Kirigami.Units.largeSpacing

        // Warning banner
        Kirigami.InlineMessage {
            Layout.fillWidth: true
            type: Kirigami.MessageType.Warning
            text: qsTr("These settings are for advanced users. Changing them may affect application stability or performance.")
            visible: true
        }

        // Logging Settings
        Kirigami.FormLayout {
            Layout.fillWidth: true

            Kirigami.Separator {
                Kirigami.FormData.isSection: true
                Kirigami.FormData.label: qsTr("Logging")
            }

            Controls.ComboBox {
                id: globalLogLevelCombo
                Kirigami.FormData.label: qsTr("Global log level:")
                model: ["error", "warn", "info", "debug", "trace"]
                currentIndex: {
                    let level = root.config.logging.global || "info"
                    return model.indexOf(level) >= 0 ? model.indexOf(level) : 2
                }
                onActivated: {
                    root.config.logging.global = model[currentIndex]
                }
            }

            Controls.Label {
                text: qsTr("Higher levels produce more detailed logs but may impact performance")
                font.pointSize: Kirigami.Theme.smallFont.pointSize
                color: Kirigami.Theme.disabledTextColor
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
            }

            Controls.CheckBox {
                id: logToFileCheckBox
                Kirigami.FormData.label: qsTr("Log to file:")
                text: qsTr("Save logs to file")
                checked: root.config.logging.logToFile || false
                onToggled: {
                    root.config.logging.logToFile = checked
                }
            }

            Controls.Label {
                text: qsTr("When enabled, logs are saved to ~/.local/share/clareon/clareon.log")
                font.pointSize: Kirigami.Theme.smallFont.pointSize
                color: Kirigami.Theme.disabledTextColor
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
            }
        }

        // Module-specific Logging
        ColumnLayout {
            Layout.fillWidth: true
            spacing: Kirigami.Units.smallSpacing

            Kirigami.Separator {
                Layout.fillWidth: true
            }

            Kirigami.Heading {
                text: qsTr("Module-specific Log Levels")
                level: 3
            }

            Controls.Label {
                text: qsTr("Override log levels for specific modules (e.g., clareon_core, aws_sdk, sqlx)")
                font.pointSize: Kirigami.Theme.smallFont.pointSize
                color: Kirigami.Theme.disabledTextColor
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
            }

            Kirigami.InlineMessage {
                Layout.fillWidth: true
                type: Kirigami.MessageType.Information
                text: qsTr("Module-specific log level configuration will be available in a future version")
                visible: true
            }

            // Module log levels list (placeholder for now)
            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: 150
                color: Kirigami.Theme.alternateBackgroundColor
                border.color: Kirigami.Theme.separatorColor
                border.width: 1
                radius: 4

                ListView {
                    id: moduleLogLevelsList
                    anchors.fill: parent
                    anchors.margins: Kirigami.Units.smallSpacing
                    clip: true

                    // Display current module levels from config
                    model: {
                        let modules = root.config.logging.modules || {}
                        let list = []
                        for (let module in modules) {
                            list.push({module: module, level: modules[module]})
                        }
                        return list
                    }

                    delegate: Controls.ItemDelegate {
                        width: ListView.view.width
                        contentItem: RowLayout {
                            Controls.Label {
                                text: modelData.module
                                Layout.fillWidth: true
                                font.family: "monospace"
                            }
                            Controls.Label {
                                text: modelData.level
                                color: Kirigami.Theme.disabledTextColor
                            }
                        }
                    }

                    Controls.Label {
                        anchors.centerIn: parent
                        text: qsTr("Default module log levels")
                        visible: moduleLogLevelsList.count === 0
                        color: Kirigami.Theme.disabledTextColor
                    }
                }
            }

            RowLayout {
                Layout.fillWidth: true

                Controls.Button {
                    text: qsTr("Add Module")
                    icon.name: "list-add"
                    enabled: false
                    onClicked: {
                        // TODO: Open dialog to add module
                    }
                }

                Controls.Button {
                    text: qsTr("Reset to Defaults")
                    icon.name: "edit-undo"
                    onClicked: {
                        // Reset modules to default
                        root.config.logging.modules = {
                            "clareon": "debug",
                            "clareon_core": "debug",
                            "clareon_cli": "debug",
                            "aws_sdk": "warn",
                            "aws_smithy": "warn",
                            "aws_config": "warn",
                            "sqlx": "warn",
                            "hyper": "warn",
                            "h2": "warn"
                        }
                    }
                }

                Item {
                    Layout.fillWidth: true
                }
            }
        }
    }
}
