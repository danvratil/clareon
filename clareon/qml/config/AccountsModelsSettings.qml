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
                model: ["bedrock", "anthropic"]
                currentIndex: {
                    let backend = root.config.default_backend || "bedrock"
                    return model.indexOf(backend) >= 0 ? model.indexOf(backend) : 0
                }
                onActivated: {
                    root.config.default_backend = model[currentIndex]
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
                    text: root.config.backends.bedrock.region || "us-east-1"
                    Layout.fillWidth: true
                    onTextChanged: {
                        if (text !== root.config.backends.bedrock.region) {
                            root.config.backends.bedrock.region = text
                        }
                    }
                }

                Controls.CheckBox {
                    id: promptCachingCheckBox
                    Kirigami.FormData.label: qsTr("Prompt caching:")
                    text: qsTr("Enable prompt caching (reduces costs)")
                    checked: root.config.backends.bedrock.enable_prompt_caching !== undefined ? root.config.backends.bedrock.enable_prompt_caching : true
                    onToggled: {
                        root.config.backends.bedrock.enable_prompt_caching = checked
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

            // Authentication Configuration
            Kirigami.FormLayout {
                Layout.fillWidth: true

                Kirigami.Separator {
                    Kirigami.FormData.isSection: true
                    Kirigami.FormData.label: qsTr("Authentication")
                }

                Controls.ComboBox {
                    id: authMethodComboBox
                    Kirigami.FormData.label: qsTr("Authentication method:")
                    model: [
                        { value: "profile", text: qsTr("AWS Profile") },
                        { value: "sso", text: qsTr("AWS SSO") },
                        { value: "bearer_token", text: qsTr("Bedrock API Key (Bearer Token)") },
                        { value: "environment_variables", text: qsTr("Environment Variables") }
                    ]
                    textRole: "text"
                    valueRole: "value"
                    currentIndex: {
                        let method = root.config.backends.bedrock.auth_method || "profile"
                        return model.findIndex(item => item.value === method)
                    }
                    onActivated: {
                        root.config.backends.bedrock.auth_method = model[currentIndex].value
                    }
                }

                Controls.Label {
                    text: qsTr("Choose how Clareon authenticates with AWS Bedrock")
                    font.pointSize: Kirigami.Theme.smallFont.pointSize
                    color: Kirigami.Theme.disabledTextColor
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                }

                // Profile field (show for Profile and SSO methods)
                Controls.TextField {
                    id: awsProfileField
                    Kirigami.FormData.label: qsTr("AWS Profile:")
                    placeholderText: qsTr("default (leave empty for default)")
                    text: root.config.backends.bedrock.profile || ""
                    Layout.fillWidth: true
                    visible: authMethodComboBox.currentIndex === 0 || authMethodComboBox.currentIndex === 1
                    onTextChanged: {
                        let currentValue = root.config.backends.bedrock.profile || ""
                        if (text !== currentValue) {
                            root.config.backends.bedrock.profile = text || null
                        }
                    }
                }

                // SSO-specific fields
                Controls.TextField {
                    id: ssoRefreshCommandField
                    Kirigami.FormData.label: qsTr("SSO refresh command:")
                    placeholderText: qsTr("aws sso login --profile myprofile")
                    text: root.config.backends.bedrock.sso_refresh_command || ""
                    Layout.fillWidth: true
                    visible: authMethodComboBox.currentIndex === 1
                    onTextChanged: {
                        let currentValue = root.config.backends.bedrock.sso_refresh_command || ""
                        if (text !== currentValue) {
                            root.config.backends.bedrock.sso_refresh_command = text || null
                        }
                    }
                }

                Controls.CheckBox {
                    id: autoRefreshCheckBox
                    text: qsTr("Automatically refresh expired credentials")
                    checked: root.config.backends.bedrock.auto_refresh_credentials !== undefined ? root.config.backends.bedrock.auto_refresh_credentials : true
                    visible: authMethodComboBox.currentIndex === 1
                    onToggled: {
                        root.config.backends.bedrock.auto_refresh_credentials = checked
                    }
                }

                Controls.Label {
                    text: qsTr("When credentials expire, Clareon will automatically run the refresh command")
                    font.pointSize: Kirigami.Theme.smallFont.pointSize
                    color: Kirigami.Theme.disabledTextColor
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                    visible: authMethodComboBox.currentIndex === 1
                }

                // Bearer token fields
                Controls.CheckBox {
                    id: bearerTokenInEnvCheckBox
                    text: qsTr("Read token from AWS_BEARER_TOKEN_BEDROCK environment variable")
                    checked: root.config.backends.bedrock.bearer_token_in_env !== undefined ? root.config.backends.bedrock.bearer_token_in_env : false
                    visible: authMethodComboBox.currentIndex === 2
                    onToggled: {
                        root.config.backends.bedrock.bearer_token_in_env = checked
                    }
                }

                Controls.Label {
                    text: qsTr("When disabled, the token is stored securely in the system keyring")
                    font.pointSize: Kirigami.Theme.smallFont.pointSize
                    color: Kirigami.Theme.disabledTextColor
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                    visible: authMethodComboBox.currentIndex === 2
                }

                RowLayout {
                    Layout.fillWidth: true
                    visible: authMethodComboBox.currentIndex === 2 && !bearerTokenInEnvCheckBox.checked
                    spacing: Kirigami.Units.smallSpacing

                    Controls.Button {
                        text: qsTr("Set Bearer Token...")
                        icon.name: "password-show-on"
                        onClicked: {
                            bearerTokenDialog.open()
                        }
                    }

                    Controls.Button {
                        text: qsTr("Clear Token")
                        icon.name: "edit-clear"
                        enabled: serviceController.hasBedrockBearerToken
                        onClicked: {
                            serviceController.deleteBedrockBearerToken()
                        }
                    }

                    Item {
                        Layout.fillWidth: true
                    }
                }

                Kirigami.InlineMessage {
                    Layout.fillWidth: true
                    type: Kirigami.MessageType.Information
                    text: qsTr("To generate a Bedrock API key, visit the AWS Bedrock console and navigate to API Keys under Settings")
                    visible: authMethodComboBox.currentIndex === 2
                    showCloseButton: false
                }

                // Environment variables info
                Kirigami.InlineMessage {
                    Layout.fillWidth: true
                    type: Kirigami.MessageType.Information
                    text: qsTr("Set AWS_ACCESS_KEY_ID and AWS_SECRET_ACCESS_KEY environment variables before starting Clareon")
                    visible: authMethodComboBox.currentIndex === 3
                    showCloseButton: false
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
                    text: root.config.default_model || "anthropic.claude-sonnet-4-20250514-v1:0"
                    Layout.fillWidth: true
                    onTextChanged: {
                        if (text !== root.config.default_model) {
                            root.config.default_model = text
                        }
                    }
                }

                Controls.TextField {
                    id: bedrockTitleModelField
                    Kirigami.FormData.label: qsTr("Title generation model:")
                    placeholderText: "anthropic.claude-3-5-haiku-20241022-v1:0"
                    text: root.config.models.title_generation || "anthropic.claude-3-5-haiku-20241022-v1:0"
                    Layout.fillWidth: true
                    onTextChanged: {
                        if (text !== root.config.models.title_generation) {
                            root.config.models.title_generation = text
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
            visible: backendComboBox.currentIndex === 1

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
                    checked: root.config.backends.anthropic.api_key_in_keyring !== undefined ? root.config.backends.anthropic.api_key_in_keyring : true
                    onToggled: {
                        root.config.backends.anthropic.api_key_in_keyring = checked
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
                    text: root.config.backends.anthropic.base_url || ""
                    Layout.fillWidth: true
                    onTextChanged: {
                        let currentValue = root.config.backends.anthropic.base_url || ""
                        if (text !== currentValue) {
                            root.config.backends.anthropic.base_url = text || null
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
                    text: root.config.default_model || "anthropic.claude-sonnet-4-20250514-v1:0"
                    Layout.fillWidth: true
                    onTextChanged: {
                        if (text !== root.config.default_model) {
                            root.config.default_model = text
                        }
                    }
                }

                Controls.TextField {
                    id: anthropicTitleModelField
                    Kirigami.FormData.label: qsTr("Title generation model:")
                    placeholderText: "claude-3-5-haiku-20241022"
                    text: root.config.models.title_generation || "anthropic.claude-3-5-haiku-20241022-v1:0"
                    Layout.fillWidth: true
                    onTextChanged: {
                        if (text !== root.config.models.title_generation) {
                            root.config.models.title_generation = text
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

    // Bearer Token Dialog
    Kirigami.PromptDialog {
        id: bearerTokenDialog
        title: qsTr("Set Bedrock Bearer Token")
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel

        ColumnLayout {
            spacing: Kirigami.Units.largeSpacing

            Controls.Label {
                text: qsTr("Enter your AWS Bedrock API key (bearer token):")
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
            }

            Controls.TextField {
                id: bearerTokenInput
                Layout.fillWidth: true
                placeholderText: qsTr("Paste your bearer token here")
                echoMode: Controls.TextField.Password
            }

            Kirigami.InlineMessage {
                Layout.fillWidth: true
                type: Kirigami.MessageType.Warning
                text: qsTr("The token will be stored securely in your system keyring")
                visible: true
                showCloseButton: false
            }
        }

        onAccepted: {
            if (bearerTokenInput.text.length > 0) {
                serviceController.storeBedrockBearerToken(bearerTokenInput.text)
                bearerTokenInput.text = ""
            }
        }

        onRejected: {
            bearerTokenInput.text = ""
        }
    }
}
