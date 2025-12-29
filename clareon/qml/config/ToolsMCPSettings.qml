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
            text: qsTr("Tools & MCP")
            level: 1
            Layout.fillWidth: true
        }

        // Tool Execution Settings
        Kirigami.FormLayout {
            Layout.fillWidth: true

            Kirigami.Separator {
                Kirigami.FormData.isSection: true
                Kirigami.FormData.label: qsTr("Tool Execution")
            }

            Controls.CheckBox {
                id: enableToolsCheckBox
                Kirigami.FormData.label: qsTr("Enable tools:")
                text: qsTr("Allow Claude to use tools")
                checked: true
            }

            Controls.CheckBox {
                id: autoExecuteCheckBox
                Kirigami.FormData.label: qsTr("Auto-execute:")
                text: qsTr("Automatically execute tools without approval")
                checked: true
                enabled: enableToolsCheckBox.checked
            }

            Controls.Label {
                text: qsTr("When disabled, you'll be prompted to approve each tool use")
                font.pointSize: Kirigami.Theme.smallFont.pointSize
                color: Kirigami.Theme.disabledTextColor
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
            }

            Controls.SpinBox {
                id: toolTimeoutSpinBox
                Kirigami.FormData.label: qsTr("Tool timeout:")
                from: 5
                to: 300
                value: 30
                stepSize: 5
                enabled: enableToolsCheckBox.checked
            }

            Controls.Label {
                text: qsTr("Maximum time (in seconds) to wait for tool execution")
                font.pointSize: Kirigami.Theme.smallFont.pointSize
                color: Kirigami.Theme.disabledTextColor
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
            }
        }

        // Sandboxing Settings
        Kirigami.FormLayout {
            Layout.fillWidth: true

            Kirigami.Separator {
                Kirigami.FormData.isSection: true
                Kirigami.FormData.label: qsTr("Sandboxing")
            }

            Controls.ComboBox {
                id: sandboxModeCombo
                Kirigami.FormData.label: qsTr("Sandbox mode:")
                model: [qsTr("Strict (Recommended)"), qsTr("Basic"), qsTr("None (dangerous)")]
                currentIndex: 0
                enabled: enableToolsCheckBox.checked
            }

            Controls.Label {
                text: qsTr("Strict mode isolates tool execution using bubblewrap. " +
                           "Basic mode provides limited isolation. " +
                           "None mode is only for development and testing.")
                font.pointSize: Kirigami.Theme.smallFont.pointSize
                color: Kirigami.Theme.disabledTextColor
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
            }
        }

        // Built-in Tools
        Kirigami.FormLayout {
            Layout.fillWidth: true

            Kirigami.Separator {
                Kirigami.FormData.isSection: true
                Kirigami.FormData.label: qsTr("Built-in Tools")
            }

            Controls.CheckBox {
                id: enableReadFileCheckBox
                Kirigami.FormData.label: qsTr("read_file:")
                text: qsTr("Allow reading files")
                checked: true
                enabled: enableToolsCheckBox.checked
            }

            Controls.CheckBox {
                id: enableWriteFileCheckBox
                Kirigami.FormData.label: qsTr("write_file:")
                text: qsTr("Allow writing files")
                checked: true
                enabled: enableToolsCheckBox.checked
            }

            Controls.CheckBox {
                id: enableListDirectoryCheckBox
                Kirigami.FormData.label: qsTr("list_directory:")
                text: qsTr("Allow listing directories")
                checked: true
                enabled: enableToolsCheckBox.checked
            }
        }

        // Workspace Settings
        Kirigami.FormLayout {
            Layout.fillWidth: true

            Kirigami.Separator {
                Kirigami.FormData.isSection: true
                Kirigami.FormData.label: qsTr("Workspace Management")
            }

            Controls.SpinBox {
                id: maxWorkspaceSizeSpinBox
                Kirigami.FormData.label: qsTr("Max workspace size:")
                from: 50
                to: 5000
                value: 500
                stepSize: 50
                enabled: enableToolsCheckBox.checked
            }

            Controls.Label {
                text: qsTr("Maximum size (in MB) for each conversation workspace")
                font.pointSize: Kirigami.Theme.smallFont.pointSize
                color: Kirigami.Theme.disabledTextColor
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
            }

            Controls.SpinBox {
                id: maxUploadSizeSpinBox
                Kirigami.FormData.label: qsTr("Max upload size:")
                from: 10
                to: 1000
                value: 100
                stepSize: 10
                enabled: enableToolsCheckBox.checked
            }

            Controls.Label {
                text: qsTr("Maximum size (in MB) for individual file uploads")
                font.pointSize: Kirigami.Theme.smallFont.pointSize
                color: Kirigami.Theme.disabledTextColor
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
            }

            Controls.SpinBox {
                id: workspaceRetentionSpinBox
                Kirigami.FormData.label: qsTr("Workspace retention:")
                from: 1
                to: 365
                value: 30
                stepSize: 1
                enabled: enableToolsCheckBox.checked
            }

            Controls.Label {
                text: qsTr("Days to keep inactive workspace files before cleanup")
                font.pointSize: Kirigami.Theme.smallFont.pointSize
                color: Kirigami.Theme.disabledTextColor
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
            }
        }

        // MCP Servers
        ColumnLayout {
            Layout.fillWidth: true
            spacing: Kirigami.Units.smallSpacing

            Kirigami.Separator {
                Layout.fillWidth: true
                Kirigami.FormData.isSection: true
            }

            Kirigami.Heading {
                text: qsTr("MCP Servers")
                level: 3
            }

            Controls.Label {
                text: qsTr("Model Context Protocol (MCP) servers provide additional tools and resources to Claude")
                font.pointSize: Kirigami.Theme.smallFont.pointSize
                color: Kirigami.Theme.disabledTextColor
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
            }

            // MCP Server List
            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: 200
                color: Kirigami.Theme.alternateBackgroundColor
                border.color: Kirigami.Theme.separatorColor
                border.width: 1
                radius: 4

                ColumnLayout {
                    anchors.fill: parent
                    anchors.margins: Kirigami.Units.smallSpacing
                    spacing: 0

                    Controls.Label {
                        text: qsTr("No MCP servers configured")
                        Layout.alignment: Qt.AlignCenter
                        Layout.fillHeight: true
                        color: Kirigami.Theme.disabledTextColor
                        horizontalAlignment: Text.AlignHCenter
                        verticalAlignment: Text.AlignVCenter
                    }
                }
            }

            RowLayout {
                Layout.fillWidth: true

                Controls.Button {
                    text: qsTr("Add Server")
                    icon.name: "list-add"
                    enabled: enableToolsCheckBox.checked
                    onClicked: {
                        // TODO: Open MCP server configuration dialog
                    }
                }

                Controls.Button {
                    text: qsTr("Import from File")
                    icon.name: "document-open"
                    enabled: enableToolsCheckBox.checked
                    onClicked: {
                        // TODO: Import MCP server configuration
                    }
                }

                Item {
                    Layout.fillWidth: true
                }
            }
        }
    }
}
