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
            text: qsTr("Accounts & Models")
            level: 1
            Layout.fillWidth: true
        }

        // Backend Selection
        Kirigami.FormLayout {
            Layout.fillWidth: true

            Kirigami.Separator {
                Kirigami.FormData.isSection: true
                Kirigami.FormData.label: qsTr("Backend")
            }

            Controls.ComboBox {
                id: backendComboBox
                model: ["AWS Bedrock", "Anthropic API"]
                currentIndex: 0

                onCurrentIndexChanged: {
                    // Toggle visibility of backend-specific sections
                    bedrockSection.visible = (currentIndex === 0)
                    anthropicSection.visible = (currentIndex === 1)
                }
            }

            Controls.Label {
                text: qsTr("Choose which API to use for Claude models")
                font.pointSize: Kirigami.Theme.smallFont.pointSize
                color: Kirigami.Theme.disabledTextColor
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
            }
        }

        // AWS Bedrock Configuration
        ColumnLayout {
            id: bedrockSection
            Layout.fillWidth: true
            spacing: Kirigami.Units.largeSpacing
            visible: backendComboBox.currentIndex === 0

            Kirigami.FormLayout {
                Layout.fillWidth: true

                Kirigami.Separator {
                    Kirigami.FormData.isSection: true
                    Kirigami.FormData.label: qsTr("AWS Bedrock Configuration")
                }

                Controls.TextField {
                    id: awsRegionField
                    Kirigami.FormData.label: qsTr("AWS Region:")
                    placeholderText: "us-east-1"
                    text: "us-east-1"
                    Layout.fillWidth: true
                }

                Controls.TextField {
                    id: awsProfileField
                    Kirigami.FormData.label: qsTr("AWS Profile:")
                    //: Placeholder text for AWS profile field
                    placeholderText: qsTr("default (leave empty for default)")
                    Layout.fillWidth: true
                }

                Controls.CheckBox {
                    id: promptCachingCheckBox
                    Kirigami.FormData.label: qsTr("Prompt caching:")
                    text: "Enable prompt caching (reduces costs)"
                    checked: true
                }

                Controls.Label {
                    text: qsTr("Prompt caching is only available for Claude Sonnet 3.5+, Opus 4, and Nova models")
                    font.pointSize: Kirigami.Theme.smallFont.pointSize
                    color: Kirigami.Theme.disabledTextColor
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                }
            }

            // Bedrock Model Selection
            Kirigami.FormLayout {
                Layout.fillWidth: true

                Kirigami.Separator {
                    Kirigami.FormData.isSection: true
                    Kirigami.FormData.label: qsTr("Model Selection")
                }

                Controls.TextField {
                    id: bedrockDefaultModelField
                    Kirigami.FormData.label: qsTr("Default model:")
                    placeholderText: "anthropic.claude-sonnet-4-20250514-v1:0"
                    text: "anthropic.claude-sonnet-4-20250514-v1:0"
                    Layout.fillWidth: true
                }

                Controls.TextField {
                    id: bedrockHaikuModelField
                    Kirigami.FormData.label: qsTr("Haiku model:")
                    placeholderText: "anthropic.claude-3-5-haiku-20241022-v1:0"
                    text: "anthropic.claude-3-5-haiku-20241022-v1:0"
                    Layout.fillWidth: true
                }

                Controls.TextField {
                    id: bedrockSonnetModelField
                    Kirigami.FormData.label: qsTr("Sonnet model:")
                    placeholderText: "anthropic.claude-sonnet-4-20250514-v1:0"
                    text: "anthropic.claude-sonnet-4-20250514-v1:0"
                    Layout.fillWidth: true
                }

                Controls.TextField {
                    id: bedrockOpusModelField
                    Kirigami.FormData.label: qsTr("Opus model:")
                    placeholderText: "anthropic.claude-opus-4-20250514-v1:0"
                    text: "anthropic.claude-opus-4-20250514-v1:0"
                    Layout.fillWidth: true
                }

                Controls.TextField {
                    id: bedrockSummarizationModelField
                    //: Model used to generate title of a conversation
                    Kirigami.FormData.label: qsTr("Title generation model:")
                    placeholderText: "anthropic.claude-3-5-haiku-20241022-v1:0"
                    text: "anthropic.claude-3-5-haiku-20241022-v1:0"
                    Layout.fillWidth: true
                }

                Controls.Label {
                    text: qsTr("Model IDs must be in AWS Bedrock format (e.g., anthropic.model-name-version:0)")
                    font.pointSize: Kirigami.Theme.smallFont.pointSize
                    color: Kirigami.Theme.disabledTextColor
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                }
            }
        }

        // Anthropic API Configuration
        ColumnLayout {
            id: anthropicSection
            Layout.fillWidth: true
            spacing: Kirigami.Units.largeSpacing
            visible: backendComboBox.currentIndex === 1

            Kirigami.FormLayout {
                Layout.fillWidth: true

                Kirigami.Separator {
                    Kirigami.FormData.isSection: true
                    Kirigami.FormData.label: qsTr("Anthropic API Configuration")
                }

                RowLayout {
                    Kirigami.FormData.label: qsTr("API Key:")
                    Layout.fillWidth: true

                    Kirigami.PasswordField {
                        id: anthropicApiKeyField
                        placeholderText: qsTr("sk-ant-...")
                        echoMode: TextInput.Password
                        Layout.fillWidth: true
                    }
                }

                Controls.TextField {
                    id: anthropicBaseUrlField
                    //: Base URL for Anthropic API
                    Kirigami.FormData.label: qsTr("Base URL:")
                    placeholderText: qsTr("https://api.anthropic.com (leave empty for default)")
                    Layout.fillWidth: true
                }

                Controls.Label {
                    text: qsTr("Custom base URL is only needed for proxy or custom API endpoints")
                    font.pointSize: Kirigami.Theme.smallFont.pointSize
                    color: Kirigami.Theme.disabledTextColor
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                }
            }

            Kirigami.FormLayout {
                Layout.fillWidth: true

                Kirigami.Separator {
                    Kirigami.FormData.isSection: true
                    Kirigami.FormData.label: qsTr("Model Selection")
                }

                Controls.ComboBox {
                    id: anthropicDefaultModelCombo
                    Kirigami.FormData.label: qsTr("Default model:")
                    model: [
                        qsTr("Claude Opus 4.5"),
                        qsTr("Claude Sonnet 4.5"),
                        qsTr("Claude Haiku 4.5")
                    ]
                    currentIndex: 1
                }

                Controls.Label {
                    text: qsTr("Model selection for specific tasks (Haiku, Opus, title generation) is automatically managed")
                    font.pointSize: Kirigami.Theme.smallFont.pointSize
                    color: Kirigami.Theme.disabledTextColor
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                }
            }
        }
    }
}
