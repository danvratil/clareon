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

    title: qsTr("Accounts & Models")

    property var config
    property bool isDirty: false

    ColumnLayout {
        width: root.width
        spacing: Kirigami.Units.largeSpacing

        // Backend Selection
        Kirigami.FormLayout {
            Layout.fillWidth: true

            Kirigami.Separator {
                Kirigami.FormData.isSection: true
                Kirigami.FormData.label: qsTr("Backend")
            }

            Controls.ComboBox {
                id: backendComboBox
                Kirigami.FormData.label: qsTr("Default backend:")
                model: ["openai", "bedrock", "anthropic"]
                currentIndex: {
                    let backend = root.config.defaultBackend || "openai"
                    return model.indexOf(backend) >= 0 ? model.indexOf(backend) : 0
                }
                onActivated: {
                    root.config.defaultBackend = model[currentIndex]
                }
            }

            Controls.Label {
                text: qsTr("Choose which API backend to use. OpenAI-compatible supports LiteLLM, OpenRouter, and other providers.")
                font.pointSize: Kirigami.Theme.smallFont.pointSize
                color: Kirigami.Theme.disabledTextColor
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
            }
        }

        // OpenAI-compatible API Configuration
        ColumnLayout {
            id: openaiSection
            Layout.fillWidth: true
            spacing: Kirigami.Units.largeSpacing
            visible: backendComboBox.currentIndex === 0

            Kirigami.FormLayout {
                Layout.fillWidth: true

                Kirigami.Separator {
                    Kirigami.FormData.isSection: true
                    Kirigami.FormData.label: qsTr("OpenAI-compatible API Configuration")
                }

                Controls.TextField {
                    id: openaiApiKeyField
                    Kirigami.FormData.label: qsTr("API Key:")
                    placeholderText: qsTr("sk-...")
                    text: root.config.backends.openai.apiKey || ""
                    echoMode: TextInput.Password
                    Layout.fillWidth: true
                    onTextChanged: {
                        let currentValue = root.config.backends.openai.apiKey || ""
                        if (text !== currentValue) {
                            root.config.backends.openai.apiKey = text || null
                        }
                    }
                }

                Controls.TextField {
                    id: openaiBaseUrlField
                    Kirigami.FormData.label: qsTr("Base URL:")
                    placeholderText: qsTr("https://api.openai.com/v1 (leave empty for default)")
                    text: root.config.backends.openai.baseUrl || ""
                    Layout.fillWidth: true
                    onTextChanged: {
                        let currentValue = root.config.backends.openai.baseUrl || ""
                        if (text !== currentValue) {
                            root.config.backends.openai.baseUrl = text || null
                        }
                    }
                }

                Controls.Label {
                    text: qsTr("Set the base URL to point to your LiteLLM, OpenRouter, or other OpenAI-compatible API endpoint.")
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
                    Kirigami.FormData.label: qsTr("Default Model")
                }

                Controls.TextField {
                    id: openaiDefaultModelField
                    Kirigami.FormData.label: qsTr("Default model:")
                    placeholderText: "gpt-4o"
                    text: root.config.defaultModel || "gpt-4o"
                    Layout.fillWidth: true
                    onTextChanged: {
                        if (text !== root.config.defaultModel) {
                            root.config.defaultModel = text
                        }
                    }
                }

                Controls.TextField {
                    id: openaiTitleModelField
                    Kirigami.FormData.label: qsTr("Title generation model:")
                    placeholderText: "gpt-4o-mini"
                    text: root.config.models.titleGeneration || "gpt-4o-mini"
                    Layout.fillWidth: true
                    onTextChanged: {
                        if (text !== root.config.models.titleGeneration) {
                            root.config.models.titleGeneration = text
                        }
                    }
                }

                Controls.Label {
                    text: qsTr("Use the model ID as shown by your API provider (e.g., gpt-4o, claude-sonnet-4-5-20250929, etc.)")
                    font.pointSize: Kirigami.Theme.smallFont.pointSize
                    color: Kirigami.Theme.disabledTextColor
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                }
            }
        }

        // AWS Bedrock Configuration
        ColumnLayout {
            id: bedrockSection
            Layout.fillWidth: true
            spacing: Kirigami.Units.largeSpacing
            visible: backendComboBox.currentIndex === 1

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
                    text: root.config.backends.bedrock.region || "us-east-1"
                    Layout.fillWidth: true
                    onTextChanged: {
                        if (text !== root.config.backends.bedrock.region) {
                            root.config.backends.bedrock.region = text
                        }
                    }
                }

                Controls.TextField {
                    id: awsProfileField
                    Kirigami.FormData.label: qsTr("AWS Profile:")
                    placeholderText: qsTr("default (leave empty for default)")
                    text: root.config.backends.bedrock.profile || ""
                    Layout.fillWidth: true
                    onTextChanged: {
                        let currentValue = root.config.backends.bedrock.profile || ""
                        if (text !== currentValue) {
                            root.config.backends.bedrock.profile = text || null
                        }
                    }
                }

                Controls.CheckBox {
                    id: promptCachingCheckBox
                    Kirigami.FormData.label: qsTr("Prompt caching:")
                    text: qsTr("Enable prompt caching (reduces costs)")
                    checked: root.config.backends.bedrock.enablePromptCaching !== undefined ? root.config.backends.bedrock.enablePromptCaching : true
                    onToggled: {
                        root.config.backends.bedrock.enablePromptCaching = checked
                    }
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
                    Kirigami.FormData.label: qsTr("Default Model")
                }

                Controls.TextField {
                    id: bedrockDefaultModelField
                    Kirigami.FormData.label: qsTr("Default model:")
                    placeholderText: "anthropic.claude-sonnet-4-20250514-v1:0"
                    text: root.config.defaultModel || "anthropic.claude-sonnet-4-20250514-v1:0"
                    Layout.fillWidth: true
                    onTextChanged: {
                        if (text !== root.config.defaultModel) {
                            root.config.defaultModel = text
                        }
                    }
                }

                Controls.TextField {
                    id: bedrockTitleModelField
                    Kirigami.FormData.label: qsTr("Title generation model:")
                    placeholderText: "anthropic.claude-3-5-haiku-20241022-v1:0"
                    text: root.config.models.titleGeneration || "anthropic.claude-3-5-haiku-20241022-v1:0"
                    Layout.fillWidth: true
                    onTextChanged: {
                        if (text !== root.config.models.titleGeneration) {
                            root.config.models.titleGeneration = text
                        }
                    }
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
            visible: backendComboBox.currentIndex === 2

            Kirigami.FormLayout {
                Layout.fillWidth: true

                Kirigami.Separator {
                    Kirigami.FormData.isSection: true
                    Kirigami.FormData.label: qsTr("Anthropic API Configuration")
                }

                Controls.CheckBox {
                    id: apiKeyInKeyringCheckBox
                    Kirigami.FormData.label: qsTr("API Key storage:")
                    text: qsTr("Store API key in system keyring")
                    checked: root.config.backends.anthropic.apiKeyInKeyring !== undefined ? root.config.backends.anthropic.apiKeyInKeyring : true
                    onToggled: {
                        root.config.backends.anthropic.apiKeyInKeyring = checked
                    }
                }

                Controls.Label {
                    text: qsTr("When enabled, API key is stored securely in your system keyring. When disabled, set ANTHROPIC_API_KEY environment variable.")
                    font.pointSize: Kirigami.Theme.smallFont.pointSize
                    color: Kirigami.Theme.disabledTextColor
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                }

                Controls.TextField {
                    id: anthropicBaseUrlField
                    Kirigami.FormData.label: qsTr("Base URL:")
                    placeholderText: qsTr("https://api.anthropic.com (leave empty for default)")
                    text: root.config.backends.anthropic.baseUrl || ""
                    Layout.fillWidth: true
                    onTextChanged: {
                        let currentValue = root.config.backends.anthropic.baseUrl || ""
                        if (text !== currentValue) {
                            root.config.backends.anthropic.baseUrl = text || null
                        }
                    }
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
                    Kirigami.FormData.label: qsTr("Default Model")
                }

                Controls.TextField {
                    id: anthropicDefaultModelField
                    Kirigami.FormData.label: qsTr("Default model:")
                    placeholderText: "claude-sonnet-4-5-20250514"
                    text: root.config.defaultModel || "anthropic.claude-sonnet-4-20250514-v1:0"
                    Layout.fillWidth: true
                    onTextChanged: {
                        if (text !== root.config.defaultModel) {
                            root.config.defaultModel = text
                        }
                    }
                }

                Controls.TextField {
                    id: anthropicTitleModelField
                    Kirigami.FormData.label: qsTr("Title generation model:")
                    placeholderText: "claude-3-5-haiku-20241022"
                    text: root.config.models.titleGeneration || "anthropic.claude-3-5-haiku-20241022-v1:0"
                    Layout.fillWidth: true
                    onTextChanged: {
                        if (text !== root.config.models.titleGeneration) {
                            root.config.models.titleGeneration = text
                        }
                    }
                }

                Controls.Label {
                    text: qsTr("Model IDs for Anthropic API (e.g., claude-opus-4-5-20251101)")
                    font.pointSize: Kirigami.Theme.smallFont.pointSize
                    color: Kirigami.Theme.disabledTextColor
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                }
            }
        }
    }
}
