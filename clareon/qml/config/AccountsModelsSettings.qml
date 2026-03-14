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

    readonly property string selectedProvider: providerComboBox.currentValue || "openai"
    readonly property bool isOpenAiBacked: ["openai", "openrouter", "litellm"].indexOf(selectedProvider) !== -1

    ColumnLayout {
        width: root.width
        spacing: Kirigami.Units.largeSpacing

        // Provider Selection
        Kirigami.FormLayout {
            Layout.fillWidth: true

            Kirigami.Separator {
                Kirigami.FormData.isSection: true
                Kirigami.FormData.label: qsTr("Provider")
            }

            Controls.ComboBox {
                id: providerComboBox
                Kirigami.FormData.label: qsTr("Provider:")
                model: ["openai", "openrouter", "litellm", "anthropic", "bedrock"]
                displayText: {
                    const labels = {
                        "openai": "OpenAI",
                        "openrouter": "OpenRouter",
                        "litellm": "LiteLLM",
                        "anthropic": "Anthropic",
                        "bedrock": "AWS Bedrock"
                    }
                    return labels[currentValue] || currentValue
                }
                delegate: Controls.ItemDelegate {
                    required property string modelData
                    required property int index
                    width: parent.width
                    text: {
                        const labels = {
                            "openai": "OpenAI",
                            "openrouter": "OpenRouter",
                            "litellm": "LiteLLM",
                            "anthropic": "Anthropic",
                            "bedrock": "AWS Bedrock"
                        }
                        return labels[modelData] || modelData
                    }
                    highlighted: providerComboBox.highlightedIndex === index
                }
                Component.onCompleted: {
                    let provider = root.config.defaultProvider || "openai"
                    currentIndex = model.indexOf(provider)
                }
                onActivated: {
                    root.config.defaultProvider = currentValue
                }
            }

            Controls.Label {
                text: qsTr("Choose your LLM provider. OpenAI, OpenRouter and LiteLLM use the OpenAI-compatible API.")
                font.pointSize: Kirigami.Theme.smallFont.pointSize
                color: Kirigami.Theme.disabledTextColor
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
            }
        }

        // OpenAI Configuration
        ColumnLayout {
            Layout.fillWidth: true
            spacing: Kirigami.Units.largeSpacing
            visible: selectedProvider === "openai"

            Kirigami.FormLayout {
                Layout.fillWidth: true

                Kirigami.Separator {
                    Kirigami.FormData.isSection: true
                    Kirigami.FormData.label: qsTr("OpenAI Configuration")
                }

                Controls.TextField {
                    Kirigami.FormData.label: qsTr("API Key:")
                    placeholderText: qsTr("sk-...")
                    text: root.config.providers.openai.apiKey || ""
                    echoMode: TextInput.Password
                    Layout.fillWidth: true
                    onTextChanged: {
                        let currentValue = root.config.providers.openai.apiKey || ""
                        if (text !== currentValue) {
                            root.config.providers.openai.apiKey = text || null
                        }
                    }
                }

                Controls.TextField {
                    Kirigami.FormData.label: qsTr("Base URL:")
                    placeholderText: qsTr("https://api.openai.com/v1 (leave empty for default)")
                    text: root.config.providers.openai.baseUrl || ""
                    Layout.fillWidth: true
                    onTextChanged: {
                        let currentValue = root.config.providers.openai.baseUrl || ""
                        if (text !== currentValue) {
                            root.config.providers.openai.baseUrl = text || null
                        }
                    }
                }
            }
        }

        // OpenRouter Configuration
        ColumnLayout {
            Layout.fillWidth: true
            spacing: Kirigami.Units.largeSpacing
            visible: selectedProvider === "openrouter"

            Kirigami.FormLayout {
                Layout.fillWidth: true

                Kirigami.Separator {
                    Kirigami.FormData.isSection: true
                    Kirigami.FormData.label: qsTr("OpenRouter Configuration")
                }

                Controls.TextField {
                    Kirigami.FormData.label: qsTr("API Key:")
                    placeholderText: qsTr("sk-or-...")
                    text: root.config.providers.openrouter.apiKey || ""
                    echoMode: TextInput.Password
                    Layout.fillWidth: true
                    onTextChanged: {
                        let currentValue = root.config.providers.openrouter.apiKey || ""
                        if (text !== currentValue) {
                            root.config.providers.openrouter.apiKey = text || null
                        }
                    }
                }

                Controls.TextField {
                    Kirigami.FormData.label: qsTr("Base URL:")
                    placeholderText: qsTr("https://openrouter.ai/api/v1 (leave empty for default)")
                    text: root.config.providers.openrouter.baseUrl || ""
                    Layout.fillWidth: true
                    onTextChanged: {
                        let currentValue = root.config.providers.openrouter.baseUrl || ""
                        if (text !== currentValue) {
                            root.config.providers.openrouter.baseUrl = text || null
                        }
                    }
                }
            }
        }

        // LiteLLM Configuration
        ColumnLayout {
            Layout.fillWidth: true
            spacing: Kirigami.Units.largeSpacing
            visible: selectedProvider === "litellm"

            Kirigami.FormLayout {
                Layout.fillWidth: true

                Kirigami.Separator {
                    Kirigami.FormData.isSection: true
                    Kirigami.FormData.label: qsTr("LiteLLM Configuration")
                }

                Controls.TextField {
                    Kirigami.FormData.label: qsTr("API Key:")
                    placeholderText: qsTr("sk-...")
                    text: root.config.providers.litellm.apiKey || ""
                    echoMode: TextInput.Password
                    Layout.fillWidth: true
                    onTextChanged: {
                        let currentValue = root.config.providers.litellm.apiKey || ""
                        if (text !== currentValue) {
                            root.config.providers.litellm.apiKey = text || null
                        }
                    }
                }

                Controls.TextField {
                    Kirigami.FormData.label: qsTr("Base URL:")
                    placeholderText: qsTr("http://localhost:4000/v1")
                    text: root.config.providers.litellm.baseUrl || ""
                    Layout.fillWidth: true
                    onTextChanged: {
                        let currentValue = root.config.providers.litellm.baseUrl || ""
                        if (text !== currentValue) {
                            root.config.providers.litellm.baseUrl = text || null
                        }
                    }
                }

                Controls.Label {
                    text: qsTr("Base URL is required for LiteLLM. Point it to your LiteLLM proxy server.")
                    font.pointSize: Kirigami.Theme.smallFont.pointSize
                    color: Kirigami.Theme.disabledTextColor
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                }
            }
        }

        // Anthropic API Configuration
        ColumnLayout {
            Layout.fillWidth: true
            spacing: Kirigami.Units.largeSpacing
            visible: selectedProvider === "anthropic"

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
                    checked: root.config.providers.anthropic.apiKeyInKeyring !== undefined ? root.config.providers.anthropic.apiKeyInKeyring : true
                    onToggled: {
                        root.config.providers.anthropic.apiKeyInKeyring = checked
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
                    Kirigami.FormData.label: qsTr("Base URL:")
                    placeholderText: qsTr("https://api.anthropic.com (leave empty for default)")
                    text: root.config.providers.anthropic.baseUrl || ""
                    Layout.fillWidth: true
                    onTextChanged: {
                        let currentValue = root.config.providers.anthropic.baseUrl || ""
                        if (text !== currentValue) {
                            root.config.providers.anthropic.baseUrl = text || null
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
        }

        // AWS Bedrock Configuration
        ColumnLayout {
            Layout.fillWidth: true
            spacing: Kirigami.Units.largeSpacing
            visible: selectedProvider === "bedrock"

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
                    text: root.config.providers.bedrock.region || "us-east-1"
                    Layout.fillWidth: true
                    onTextChanged: {
                        if (text !== root.config.providers.bedrock.region) {
                            root.config.providers.bedrock.region = text
                        }
                    }
                }

                Controls.TextField {
                    id: awsProfileField
                    Kirigami.FormData.label: qsTr("AWS Profile:")
                    placeholderText: qsTr("default (leave empty for default)")
                    text: root.config.providers.bedrock.profile || ""
                    Layout.fillWidth: true
                    onTextChanged: {
                        let currentValue = root.config.providers.bedrock.profile || ""
                        if (text !== currentValue) {
                            root.config.providers.bedrock.profile = text || null
                        }
                    }
                }

                Controls.CheckBox {
                    id: promptCachingCheckBox
                    Kirigami.FormData.label: qsTr("Prompt caching:")
                    text: qsTr("Enable prompt caching (reduces costs)")
                    checked: root.config.providers.bedrock.enablePromptCaching !== undefined ? root.config.providers.bedrock.enablePromptCaching : true
                    onToggled: {
                        root.config.providers.bedrock.enablePromptCaching = checked
                    }
                }

                Controls.Label {
                    text: qsTr("Prompt caching is only available for select models (Claude Sonnet 3.5+, Opus 4, Nova)")
                    font.pointSize: Kirigami.Theme.smallFont.pointSize
                    color: Kirigami.Theme.disabledTextColor
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                }
            }
        }

        // Default Model section
        Kirigami.FormLayout {
            Layout.fillWidth: true

            Kirigami.Separator {
                Kirigami.FormData.isSection: true
                Kirigami.FormData.label: qsTr("Default Model")
            }

            // Browse mode (OpenAI-backed providers)
            RowLayout {
                Kirigami.FormData.label: qsTr("Default model:")
                visible: isOpenAiBacked
                Layout.fillWidth: true

                Controls.Label {
                    text: root.config.defaultModel || qsTr("(none selected)")
                    Layout.fillWidth: true
                    elide: Text.ElideRight
                }
                Controls.Button {
                    text: qsTr("Browse Models...")
                    icon.name: "view-list-details"
                    onClicked: defaultModelSheet.open()
                }
            }

            // Fallback text field (Anthropic/Bedrock)
            Controls.TextField {
                Kirigami.FormData.label: qsTr("Default model:")
                visible: !isOpenAiBacked
                placeholderText: "claude-sonnet-4-5-20250929"
                text: root.config.defaultModel || ""
                Layout.fillWidth: true
                onTextChanged: {
                    if (text !== root.config.defaultModel) {
                        root.config.defaultModel = text
                    }
                }
            }

            // Same pattern for title generation model
            RowLayout {
                Kirigami.FormData.label: qsTr("Title generation model:")
                visible: isOpenAiBacked
                Layout.fillWidth: true

                Controls.Label {
                    text: root.config.models.titleGeneration || qsTr("(none selected)")
                    Layout.fillWidth: true
                    elide: Text.ElideRight
                }
                Controls.Button {
                    text: qsTr("Browse Models...")
                    icon.name: "view-list-details"
                    onClicked: titleModelSheet.open()
                }
            }

            Controls.TextField {
                Kirigami.FormData.label: qsTr("Title generation model:")
                visible: !isOpenAiBacked
                placeholderText: "claude-haiku-3-5-20241022"
                text: root.config.models.titleGeneration || ""
                Layout.fillWidth: true
                onTextChanged: {
                    if (text !== root.config.models.titleGeneration) {
                        root.config.models.titleGeneration = text
                    }
                }
            }

            Controls.Label {
                text: qsTr("Use the model ID as shown by your provider")
                visible: !isOpenAiBacked
                font.pointSize: Kirigami.Theme.smallFont.pointSize
                color: Kirigami.Theme.disabledTextColor
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
            }
        }
    }

    ModelSelectorSheet {
        id: defaultModelSheet
        provider: selectedProvider
        onModelSelected: function(modelId, contextWindow, maxOutputTokens) {
            root.config.defaultModel = modelId
        }
    }

    ModelSelectorSheet {
        id: titleModelSheet
        provider: selectedProvider
        onModelSelected: function(modelId, contextWindow, maxOutputTokens) {
            root.config.models.titleGeneration = modelId
        }
    }
}
