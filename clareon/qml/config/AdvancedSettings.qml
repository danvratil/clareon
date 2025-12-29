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
            text: qsTr("Advanced Settings")
            level: 1
            Layout.fillWidth: true
        }

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
                model: ["Error", "Warn", "Info", "Debug", "Trace"]
                currentIndex: 2  // Info
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
                checked: false
            }

            Controls.TextField {
                id: logFilePathField
                Kirigami.FormData.label: qsTr("Log file path:")
                placeholderText: "~/.local/share/clareon/clareon.log"
                enabled: logToFileCheckBox.checked
                Layout.fillWidth: true
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

            // Module log levels list
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

                    model: ListModel {
                        ListElement { module: "clareon_core"; level: "debug" }
                        ListElement { module: "aws_sdk"; level: "warn" }
                        ListElement { module: "sqlx"; level: "warn" }
                    }

                    delegate: Controls.ItemDelegate {
                        width: ListView.view.width
                        contentItem: RowLayout {
                            Controls.Label {
                                text: model.module
                                Layout.fillWidth: true
                                font.family: "monospace"
                            }
                            Controls.ComboBox {
                                model: ["error", "warn", "info", "debug", "trace"]
                                currentIndex: {
                                    const levels = ["error", "warn", "info", "debug", "trace"]
                                    return Math.max(0, levels.indexOf(model.level))
                                }
                                Layout.preferredWidth: 120
                            }
                            Controls.Button {
                                icon.name: "list-remove"
                                flat: true
                                onClicked: {
                                    moduleLogLevelsList.model.remove(index)
                                }
                            }
                        }
                    }
                }
            }

            RowLayout {
                Layout.fillWidth: true

                Controls.Button {
                    //: Add logging module button
                    text: qsTr("Add Module")
                    icon.name: "list-add"
                    onClicked: {
                        // TODO: Open dialog to add module
                    }
                }

                Item {
                    Layout.fillWidth: true
                }
            }
        }

        // Network & Performance
        Kirigami.FormLayout {
            Layout.fillWidth: true

            Kirigami.Separator {
                Kirigami.FormData.isSection: true
                Kirigami.FormData.label: qsTr("Network & Performance")
            }

            Controls.SpinBox {
                id: requestTimeoutSpinBox
                Kirigami.FormData.label: qsTr("Request timeout:")
                from: 30
                to: 600
                value: 120
                stepSize: 10
            }

            Controls.Label {
                text: qsTr("Maximum time (in seconds) to wait for API responses")
                font.pointSize: Kirigami.Theme.smallFont.pointSize
                color: Kirigami.Theme.disabledTextColor
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
            }

            Controls.SpinBox {
                id: maxRetriesSpinBox
                Kirigami.FormData.label: qsTr("Max retries:")
                from: 0
                to: 10
                value: 3
                stepSize: 1
            }

            Controls.Label {
                text: qsTr("Number of retry attempts for failed requests")
                font.pointSize: Kirigami.Theme.smallFont.pointSize
                color: Kirigami.Theme.disabledTextColor
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
            }

            Controls.TextField {
                id: httpProxyField
                Kirigami.FormData.label: qsTr("HTTP proxy:")
                placeholderText: "http://proxy.example.com:8080"
                Layout.fillWidth: true
            }

            Controls.Label {
                text: qsTr("HTTP/HTTPS proxy for API requests (leave empty for system default)")
                font.pointSize: Kirigami.Theme.smallFont.pointSize
                color: Kirigami.Theme.disabledTextColor
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
            }
        }
   }
}
