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

    title: qsTr("Tools & MCP")

    property var config
    property bool isDirty: false

    ColumnLayout {
        width: root.width
        spacing: Kirigami.Units.largeSpacing

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
                text: qsTr("Allow the assistant to use tools")
                checked: root.config.tools?.enabled ?? true
                onToggled: {
                    root.config.tools.enabled = checked
                }
            }

            Controls.CheckBox {
                id: autoExecuteCheckBox
                Kirigami.FormData.label: qsTr("Auto-execute:")
                text: qsTr("Automatically execute tools without approval")
                checked: root.config.tools?.autoExecute ?? true
                enabled: enableToolsCheckBox.checked
                onToggled: {
                    root.config.tools.autoExecute = checked
                }
            }

            Controls.Label {
                text: qsTr("When disabled, you'll be prompted to allow or deny each tool use. Manage remembered rules in Settings → Allow & Deny.")
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
                value: root.config.tools?.defaultTimeout || 30
                stepSize: 5
                enabled: enableToolsCheckBox.checked
                onValueChanged: {
                    if (value !== root.config.tools?.defaultTimeout) {
                        root.config.tools.defaultTimeout = value
                    }
                }
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
                model: ["strict", "basic", "none"]
                displayText: {
                    const modeNames = {
                        "strict": qsTr("Strict (Recommended)"),
                        "basic": qsTr("Basic"),
                        "none": qsTr("None (dangerous)")
                    }
                    return modeNames[currentValue] || currentValue
                }
                currentIndex: {
                    let mode = root.config.tools?.sandboxMode || "strict"
                    return model.indexOf(mode) >= 0 ? model.indexOf(mode) : 0
                }
                enabled: enableToolsCheckBox.checked
                onActivated: {
                    root.config.tools.sandboxMode = model[currentIndex]
                }
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
                value: root.config.tools?.maxWorkspaceSizeMb || 500
                stepSize: 50
                enabled: enableToolsCheckBox.checked
                onValueChanged: {
                    if (value !== root.config.tools?.maxWorkspaceSizeMb) {
                        root.config.tools.maxWorkspaceSizeMb = value
                    }
                }
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
                value: root.config.tools?.maxUploadSizeMb || 100
                stepSize: 10
                enabled: enableToolsCheckBox.checked
                onValueChanged: {
                    if (value !== root.config.tools?.maxUploadSizeMb) {
                        root.config.tools.maxUploadSizeMb = value
                    }
                }
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
                value: root.config.tools?.workspaceRetentionDays || 30
                stepSize: 1
                enabled: enableToolsCheckBox.checked
                onValueChanged: {
                    if (value !== root.config.tools?.workspaceRetentionDays) {
                        root.config.tools.workspaceRetentionDays = value
                    }
                }
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
            }

            Kirigami.Heading {
                text: qsTr("MCP Servers")
                level: 3
            }

            Controls.Label {
                text: qsTr("Model Context Protocol (MCP) servers provide additional tools and resources to the assistant. Enabling a server runs that process or connects to that URL with your user privileges.")
                font.pointSize: Kirigami.Theme.smallFont.pointSize
                color: Kirigami.Theme.disabledTextColor
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
            }

            Kirigami.InlineMessage {
                Layout.fillWidth: true
                type: Kirigami.MessageType.Warning
                text: qsTr("MCP servers run with your user privileges. Only enable servers you trust.")
                visible: enableMcpCheckBox.checked
            }

            Kirigami.InlineMessage {
                id: oauthStatusMessage
                Layout.fillWidth: true
                visible: false
                showCloseButton: true
                type: Kirigami.MessageType.Information
            }

            Controls.CheckBox {
                id: enableMcpCheckBox
                text: qsTr("Enable MCP servers")
                checked: root.config.mcp?.enabled ?? true
                onToggled: {
                    ensureMcp()
                    root.config.mcp.enabled = checked
                    root.isDirty = true
                }
            }

            // Server list from config.mcp.servers (id → object map), merged with live status
            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: Math.max(serverListView.contentHeight + Kirigami.Units.smallSpacing * 2, 120)
                Layout.maximumHeight: 280
                color: Kirigami.Theme.alternateBackgroundColor
                border.color: Kirigami.Theme.separatorColor
                border.width: 1
                radius: 4
                clip: true

                ListView {
                    id: serverListView
                    anchors.fill: parent
                    anchors.margins: Kirigami.Units.smallSpacing
                    clip: true
                    model: root.serverListModel
                    spacing: Kirigami.Units.smallSpacing

                    delegate: RowLayout {
                        width: serverListView.width
                        spacing: Kirigami.Units.smallSpacing

                        required property string serverId
                        required property string displayName
                        required property string transport
                        required property bool serverEnabled
                        required property string summary
                        required property string status
                        required property string statusError
                        required property int toolCount
                        required property int resourceCount
                        required property int promptCount

                        Controls.CheckBox {
                            checked: serverEnabled
                            onToggled: root.setServerEnabled(serverId, checked)
                        }

                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: 0

                            RowLayout {
                                Layout.fillWidth: true
                                Controls.Label {
                                    text: displayName
                                    font.bold: true
                                    elide: Text.ElideRight
                                    Layout.fillWidth: true
                                }
                                Rectangle {
                                    radius: 3
                                    color: {
                                        switch (status) {
                                        case "connected": return Kirigami.Theme.positiveTextColor
                                        case "failed": return Kirigami.Theme.negativeTextColor
                                        case "connecting": return Kirigami.Theme.neutralTextColor
                                        default: return Kirigami.Theme.disabledTextColor
                                        }
                                    }
                                    implicitHeight: statusLabel.implicitHeight + 2
                                    implicitWidth: statusLabel.implicitWidth + 8
                                    Controls.Label {
                                        id: statusLabel
                                        anchors.centerIn: parent
                                        text: status || qsTr("unknown")
                                        font.pointSize: Kirigami.Theme.smallFont.pointSize
                                        color: Kirigami.Theme.backgroundColor
                                    }
                                }
                            }

                            Controls.Label {
                                text: {
                                    let parts = [transport, summary]
                                    if (toolCount > 0 || resourceCount > 0 || promptCount > 0)
                                        parts.push(qsTr("%1 tools · %2 res · %3 prompts")
                                            .arg(toolCount).arg(resourceCount).arg(promptCount))
                                    if (statusError)
                                        parts.push(statusError)
                                    return parts.filter(Boolean).join(" · ")
                                }
                                font.pointSize: Kirigami.Theme.smallFont.pointSize
                                color: status === "failed"
                                       ? Kirigami.Theme.negativeTextColor
                                       : Kirigami.Theme.disabledTextColor
                                elide: Text.ElideRight
                                Layout.fillWidth: true
                            }
                        }

                        Controls.Button {
                            icon.name: "document-edit"
                            flat: true
                            Accessible.name: qsTr("Edit server")
                            onClicked: root.openEditServer(serverId)
                        }

                        Controls.Button {
                            visible: {
                                const live = root.liveStatus[serverId] || {}
                                return !!live.oauth_enabled
                            }
                            text: {
                                const live = root.liveStatus[serverId] || {}
                                return live.oauth_logged_in ? qsTr("Log out") : qsTr("Log in")
                            }
                            icon.name: "network-wireless"
                            flat: true
                            Accessible.name: qsTr("OAuth login")
                            onClicked: {
                                const live = root.liveStatus[serverId] || {}
                                if (live.oauth_logged_in) {
                                    ServiceController.logoutMcpOauth(serverId)
                                } else {
                                    // Immediate feedback — discovery can take several seconds.
                                    oauthStatusMessage.text = qsTr("Starting OAuth login for “%1”…").arg(serverId)
                                    oauthStatusMessage.type = Kirigami.MessageType.Information
                                    oauthStatusMessage.visible = true
                                    ServiceController.startMcpOauthLogin(serverId)
                                }
                            }
                        }

                        Controls.Button {
                            icon.name: "edit-delete"
                            flat: true
                            Accessible.name: qsTr("Remove server")
                            onClicked: root.removeServer(serverId)
                        }
                    }

                    Kirigami.PlaceholderMessage {
                        anchors.centerIn: parent
                        width: parent.width - Kirigami.Units.gridUnit
                        visible: serverListView.count === 0
                        text: qsTr("No MCP servers configured")
                        explanation: qsTr("Add a stdio or remote server, or import a Claude Desktop config snippet.")
                    }
                }
            }

            RowLayout {
                Layout.fillWidth: true

                Controls.Button {
                    text: qsTr("Add Server")
                    icon.name: "list-add"
                    enabled: enableMcpCheckBox.checked
                    onClicked: root.openAddServer()
                }

                Controls.Button {
                    text: qsTr("Import JSON…")
                    icon.name: "document-import"
                    enabled: enableMcpCheckBox.checked
                    onClicked: importDialog.open()
                }

                Controls.Button {
                    text: qsTr("Reconnect")
                    icon.name: "view-refresh"
                    enabled: enableMcpCheckBox.checked
                    onClicked: ServiceController.restartMcpServers()
                }

                Item {
                    Layout.fillWidth: true
                }
            }

            // Resources & prompts browsers
            Kirigami.Heading {
                text: qsTr("Resources")
                level: 4
                visible: enableMcpCheckBox.checked
            }

            RowLayout {
                Layout.fillWidth: true
                visible: enableMcpCheckBox.checked

                Controls.Button {
                    text: qsTr("Refresh resources")
                    icon.name: "view-refresh"
                    onClicked: ServiceController.listMcpResources("")
                }
                Item { Layout.fillWidth: true }
            }

            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: 100
                visible: enableMcpCheckBox.checked
                color: Kirigami.Theme.alternateBackgroundColor
                border.color: Kirigami.Theme.separatorColor
                border.width: 1
                radius: 4
                clip: true

                ListView {
                    id: resourceListView
                    anchors.fill: parent
                    anchors.margins: Kirigami.Units.smallSpacing
                    model: root.resourceListModel
                    clip: true
                    delegate: Controls.ItemDelegate {
                        width: resourceListView.width
                        required property string serverId
                        required property string uri
                        required property string resourceName
                        text: qsTr("%1 · %2").arg(serverId).arg(resourceName || uri)
                        onClicked: ServiceController.readMcpResource(serverId, uri)
                    }
                    Kirigami.PlaceholderMessage {
                        anchors.centerIn: parent
                        visible: resourceListView.count === 0
                        text: qsTr("No resources")
                        explanation: qsTr("Connect a server that advertises resources, then refresh.")
                    }
                }
            }

            Controls.TextArea {
                id: resourcePreview
                Layout.fillWidth: true
                Layout.preferredHeight: 80
                visible: enableMcpCheckBox.checked && text.length > 0
                readOnly: true
                wrapMode: TextEdit.Wrap
                placeholderText: qsTr("Resource preview")
            }

            Kirigami.Heading {
                text: qsTr("Prompts")
                level: 4
                visible: enableMcpCheckBox.checked
            }

            RowLayout {
                Layout.fillWidth: true
                visible: enableMcpCheckBox.checked
                Controls.Button {
                    text: qsTr("Refresh prompts")
                    icon.name: "view-refresh"
                    onClicked: ServiceController.listMcpPrompts("")
                }
                Item { Layout.fillWidth: true }
            }

            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: 100
                visible: enableMcpCheckBox.checked
                color: Kirigami.Theme.alternateBackgroundColor
                border.color: Kirigami.Theme.separatorColor
                border.width: 1
                radius: 4
                clip: true

                ListView {
                    id: promptListView
                    anchors.fill: parent
                    anchors.margins: Kirigami.Units.smallSpacing
                    model: root.promptListModel
                    clip: true
                    delegate: Controls.ItemDelegate {
                        width: promptListView.width
                        required property string serverId
                        required property string promptName
                        required property string promptDescription
                        text: qsTr("%1 · %2").arg(serverId).arg(promptName)
                        onClicked: {
                            root.selectedPromptServer = serverId
                            root.selectedPromptName = promptName
                            ServiceController.getMcpPrompt(serverId, promptName, "{}")
                        }
                    }
                    Kirigami.PlaceholderMessage {
                        anchors.centerIn: parent
                        visible: promptListView.count === 0
                        text: qsTr("No prompts")
                    }
                }
            }

            Controls.TextArea {
                id: promptPreview
                Layout.fillWidth: true
                Layout.preferredHeight: 80
                visible: enableMcpCheckBox.checked && text.length > 0
                readOnly: true
                wrapMode: TextEdit.Wrap
            }

            Controls.Button {
                text: qsTr("Inject prompt into current conversation")
                icon.name: "document-import"
                visible: enableMcpCheckBox.checked && root.selectedPromptName !== ""
                enabled: root.currentConversationId !== ""
                onClicked: {
                    ServiceController.injectMcpPrompt(
                        root.currentConversationId,
                        root.selectedPromptServer,
                        root.selectedPromptName,
                        "{}")
                }
            }
        }
    }

    // --- MCP helpers ---

    property ListModel serverListModel: ListModel {}
    property ListModel resourceListModel: ListModel {}
    property ListModel promptListModel: ListModel {}
    /// Live status map: id → status object from service
    property var liveStatus: ({})
    property string selectedPromptServer: ""
    property string selectedPromptName: ""
    /// Optional: set by parent when a conversation is active
    property string currentConversationId: ""

    function ensureMcp() {
        if (!root.config.mcp)
            return
        if (root.config.mcp.servers === undefined || root.config.mcp.servers === null)
            root.config.mcp.servers = {}
    }

    function refreshServerList() {
        serverListModel.clear()
        ensureMcp()
        const servers = root.config.mcp.servers || {}
        const ids = Object.keys(servers).sort()
        for (let i = 0; i < ids.length; i++) {
            const id = ids[i]
            const s = servers[id] || {}
            const transport = (s.transport || "stdio").toString()
            let summary = ""
            if (transport === "stdio") {
                const cmd = s.command || ""
                const args = s.args || []
                summary = ([cmd].concat(args)).filter(Boolean).join(" ")
            } else {
                summary = s.url || ""
            }
            const live = root.liveStatus[id] || {}
            serverListModel.append({
                serverId: id,
                displayName: s.name || id,
                transport: transport,
                serverEnabled: s.enabled !== false,
                summary: summary || qsTr("(incomplete)"),
                status: live.status || "disconnected",
                statusError: live.error || "",
                toolCount: live.tool_count || 0,
                resourceCount: live.resource_count || 0,
                promptCount: live.prompt_count || 0
            })
        }
    }

    function applyLiveStatus(jsonStr) {
        try {
            const arr = JSON.parse(jsonStr)
            const map = {}
            for (let i = 0; i < arr.length; i++)
                map[arr[i].id] = arr[i]
            root.liveStatus = map
            refreshServerList()
        } catch (e) {
            console.warn("Failed to parse MCP status:", e)
        }
    }

    function applyResources(jsonStr) {
        resourceListModel.clear()
        try {
            const arr = JSON.parse(jsonStr)
            for (let i = 0; i < arr.length; i++) {
                const r = arr[i]
                resourceListModel.append({
                    serverId: r.server_id || "",
                    uri: r.uri || "",
                    resourceName: r.name || r.uri || ""
                })
            }
        } catch (e) {
            console.warn("Failed to parse MCP resources:", e)
        }
    }

    function applyPrompts(jsonStr) {
        promptListModel.clear()
        try {
            const arr = JSON.parse(jsonStr)
            for (let i = 0; i < arr.length; i++) {
                const p = arr[i]
                promptListModel.append({
                    serverId: p.server_id || "",
                    promptName: p.name || "",
                    promptDescription: p.description || ""
                })
            }
        } catch (e) {
            console.warn("Failed to parse MCP prompts:", e)
        }
    }

    function setServerEnabled(id, enabled) {
        ensureMcp()
        const servers = Object.assign({}, root.config.mcp.servers || {})
        if (!servers[id])
            return
        const entry = Object.assign({}, servers[id])
        entry.enabled = enabled
        servers[id] = entry
        root.config.mcp.servers = servers
        root.isDirty = true
        refreshServerList()
    }

    function removeServer(id) {
        ensureMcp()
        const servers = Object.assign({}, root.config.mcp.servers || {})
        delete servers[id]
        root.config.mcp.servers = servers
        root.isDirty = true
        refreshServerList()
    }

    function upsertServer(id, entry) {
        ensureMcp()
        const servers = Object.assign({}, root.config.mcp.servers || {})
        servers[id] = entry
        root.config.mcp.servers = servers
        root.config.mcp.enabled = true
        enableMcpCheckBox.checked = true
        root.isDirty = true
        refreshServerList()
    }

    /// Parse "Header-Name: value" lines into a map
    function parseHeadersText(text) {
        const headers = {}
        const lines = (text || "").split(/\r?\n/)
        for (let i = 0; i < lines.length; i++) {
            const line = lines[i].trim()
            if (!line)
                continue
            const idx = line.indexOf(":")
            if (idx <= 0)
                continue
            const key = line.slice(0, idx).trim()
            const val = line.slice(idx + 1).trim()
            if (key)
                headers[key] = val
        }
        return headers
    }

    function headersToText(headers) {
        if (!headers || typeof headers !== "object")
            return ""
        const keys = Object.keys(headers).sort()
        return keys.map(k => k + ": " + headers[k]).join("\n")
    }

    function envToText(env) {
        if (!env || typeof env !== "object")
            return ""
        const keys = Object.keys(env).sort()
        return keys.map(k => k + "=" + env[k]).join("\n")
    }

    function parseEnvText(text) {
        const env = {}
        const lines = (text || "").split(/\r?\n/)
        for (let i = 0; i < lines.length; i++) {
            const line = lines[i].trim()
            if (!line)
                continue
            const idx = line.indexOf("=")
            if (idx <= 0)
                continue
            env[line.slice(0, idx).trim()] = line.slice(idx + 1)
        }
        return env
    }

    property string editingServerId: ""
    property bool editingExisting: false

    function openAddServer() {
        editingExisting = false
        editingServerId = ""
        serverIdField.text = ""
        serverIdField.enabled = true
        displayNameField.text = ""
        transportCombo.currentIndex = 0
        commandField.text = ""
        argsField.text = ""
        cwdField.text = ""
        urlField.text = ""
        headersField.text = ""
        bearerTokenField.text = ""
        envField.text = ""
        oauthCheckBox.checked = false
        oauthClientIdField.text = ""
        oauthClientSecretField.text = ""
        oauthScopesField.text = ""
        serverDialog.title = qsTr("Add MCP Server")
        serverDialog.open()
    }

    function openEditServer(id) {
        ensureMcp()
        const s = (root.config.mcp.servers || {})[id]
        if (!s)
            return
        editingExisting = true
        editingServerId = id
        serverIdField.text = id
        serverIdField.enabled = false
        displayNameField.text = s.name || ""
        const transport = (s.transport || "stdio").toString()
        transportCombo.currentIndex = transport === "http" ? 1 : (transport === "sse" ? 2 : 0)
        commandField.text = s.command || ""
        const args = s.args || []
        argsField.text = Array.isArray(args) ? args.join(" ") : String(args)
        cwdField.text = s.cwd || ""
        urlField.text = s.url || ""
        headersField.text = headersToText(s.headers)
        bearerTokenField.text = s.bearer_token || s.bearerToken || ""
        envField.text = envToText(s.env)
        oauthCheckBox.checked = !!(s.oauth)
        oauthClientIdField.text = s.oauth_client_id || s.oauthClientId || ""
        oauthClientSecretField.text = s.oauth_client_secret || s.oauthClientSecret || ""
        const scopes = s.oauth_scopes || s.oauthScopes || []
        oauthScopesField.text = Array.isArray(scopes) ? scopes.join(" ") : String(scopes)
        serverDialog.title = qsTr("Edit MCP Server")
        serverDialog.open()
    }

    function saveServerDialog() {
        const id = serverIdField.text.trim()
        if (!id)
            return
        const transport = ["stdio", "http", "sse"][transportCombo.currentIndex] || "stdio"
        const isRemote = transport !== "stdio"
        const scopesRaw = oauthScopesField.text.trim()
        const entry = {
            enabled: true,
            name: displayNameField.text.trim() || id,
            transport: transport,
            command: !isRemote ? commandField.text.trim() : "",
            args: !isRemote
                  ? argsField.text.trim().split(/\s+/).filter(a => a.length > 0)
                  : [],
            env: parseEnvText(envField.text),
            cwd: !isRemote ? (cwdField.text.trim() || "") : "",
            url: isRemote ? urlField.text.trim() : "",
            headers: isRemote ? parseHeadersText(headersField.text) : {},
            bearer_token: isRemote ? (bearerTokenField.text.trim() || "") : "",
            oauth: isRemote && oauthCheckBox.checked,
            oauth_client_id: isRemote ? (oauthClientIdField.text.trim() || "") : "",
            oauth_client_secret: isRemote ? (oauthClientSecretField.text.trim() || "") : "",
            oauth_scopes: isRemote && scopesRaw
                          ? scopesRaw.split(/\s+/).filter(s => s.length > 0)
                          : [],
            timeout_secs: null
        }
        // Preserve enabled flag when editing
        if (editingExisting) {
            const prev = (root.config.mcp.servers || {})[id]
            if (prev && prev.enabled === false)
                entry.enabled = false
        }
        root.upsertServer(id, entry)
    }

    Component.onCompleted: {
        refreshServerList()
        ServiceController.refreshMcpServers()
        ServiceController.listMcpResources("")
        ServiceController.listMcpPrompts("")
    }

    Connections {
        target: ServiceController
        function onMcpServersUpdated(json) { root.applyLiveStatus(json) }
        function onMcpResourcesUpdated(json) { root.applyResources(json) }
        function onMcpResourceRead(serverId, uri, text) {
            resourcePreview.text = text
        }
        function onMcpPromptsUpdated(json) { root.applyPrompts(json) }
        function onMcpPromptResolved(json) {
            try {
                const r = JSON.parse(json)
                promptPreview.text = r.text || ""
            } catch (e) {
                promptPreview.text = json
            }
        }
        function onMcpOauthStatus(serverId, message) {
            oauthStatusMessage.text = message
            oauthStatusMessage.type = Kirigami.MessageType.Information
            oauthStatusMessage.visible = true
        }
        function onMcpOauthUrl(serverId, url) {
            console.log("MCP OAuth URL for", serverId, ":", url)
            // Prefer Qt's portal-aware opener (works better under Flatpak / portals).
            const ok = Qt.openUrlExternally(url)
            if (!ok) {
                console.warn("Qt.openUrlExternally failed for", url)
            }
            oauthAuthUrlDialog.authUrl = url
            oauthAuthUrlDialog.serverId = serverId
            oauthAuthUrlDialog.open()
            oauthStatusMessage.text = qsTr("Browser should open for “%1”. If not, use the link dialog.").arg(serverId)
            oauthStatusMessage.type = Kirigami.MessageType.Information
            oauthStatusMessage.visible = true
        }
        function onMcpOauthFinished(serverId, success, message) {
            oauthStatusMessage.text = message
            oauthStatusMessage.type = success
                ? Kirigami.MessageType.Positive
                : Kirigami.MessageType.Error
            oauthStatusMessage.visible = true
            if (success)
                oauthAuthUrlDialog.close()
            ServiceController.refreshMcpServers()
        }
    }

    Kirigami.Dialog {
        id: oauthAuthUrlDialog
        title: qsTr("OAuth Login")
        standardButtons: Kirigami.Dialog.Close
        preferredWidth: Kirigami.Units.gridUnit * 36
        property string authUrl: ""
        property string serverId: ""

        ColumnLayout {
            spacing: Kirigami.Units.smallSpacing

            Controls.Label {
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
                text: qsTr("Complete sign-in in your browser. If no window opened, copy or open the URL below.")
            }

            Controls.TextField {
                id: oauthUrlField
                Layout.fillWidth: true
                readOnly: true
                text: oauthAuthUrlDialog.authUrl
                Accessible.name: qsTr("Authorization URL")
            }

            RowLayout {
                Layout.fillWidth: true
                Controls.Button {
                    text: qsTr("Open in browser")
                    icon.name: "internet-web-browser"
                    onClicked: Qt.openUrlExternally(oauthAuthUrlDialog.authUrl)
                }
                Controls.Button {
                    text: qsTr("Copy URL")
                    icon.name: "edit-copy"
                    onClicked: {
                        oauthUrlField.selectAll()
                        oauthUrlField.copy()
                    }
                }
                Item { Layout.fillWidth: true }
            }
        }
    }

    Kirigami.Dialog {
        id: serverDialog
        title: qsTr("Add MCP Server")
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        preferredWidth: Kirigami.Units.gridUnit * 32
        preferredHeight: Kirigami.Units.gridUnit * 36

        property bool isRemote: transportCombo.currentIndex > 0

        onAccepted: root.saveServerDialog()

        ColumnLayout {
            spacing: Kirigami.Units.smallSpacing

            Kirigami.FormLayout {
                Layout.fillWidth: true

                Controls.TextField {
                    id: serverIdField
                    Kirigami.FormData.label: qsTr("Server id:")
                    placeholderText: qsTr("filesystem")
                    Accessible.name: qsTr("Server id")
                }

                Controls.TextField {
                    id: displayNameField
                    Kirigami.FormData.label: qsTr("Display name:")
                    placeholderText: qsTr("Optional")
                    Accessible.name: qsTr("Display name")
                }

                Controls.ComboBox {
                    id: transportCombo
                    Kirigami.FormData.label: qsTr("Transport:")
                    model: [qsTr("stdio (local process)"), qsTr("HTTP (streamable)"), qsTr("SSE")]
                    Accessible.name: qsTr("Transport")
                }

                Controls.TextField {
                    id: commandField
                    Kirigami.FormData.label: qsTr("Command:")
                    placeholderText: qsTr("npx")
                    visible: !serverDialog.isRemote
                    Accessible.name: qsTr("Command")
                }

                Controls.TextField {
                    id: argsField
                    Kirigami.FormData.label: qsTr("Arguments:")
                    placeholderText: qsTr("-y @modelcontextprotocol/server-filesystem /path")
                    visible: !serverDialog.isRemote
                    Accessible.name: qsTr("Arguments")
                }

                Controls.TextField {
                    id: cwdField
                    Kirigami.FormData.label: qsTr("Working directory:")
                    placeholderText: qsTr("Optional")
                    visible: !serverDialog.isRemote
                    Accessible.name: qsTr("Working directory")
                }

                Controls.TextArea {
                    id: envField
                    Kirigami.FormData.label: qsTr("Environment:")
                    placeholderText: qsTr("KEY=value (one per line)")
                    visible: !serverDialog.isRemote
                    Layout.preferredHeight: Kirigami.Units.gridUnit * 4
                    Layout.fillWidth: true
                    Accessible.name: qsTr("Environment variables")
                }

                Controls.TextField {
                    id: urlField
                    Kirigami.FormData.label: qsTr("URL:")
                    placeholderText: qsTr("https://example.com/mcp")
                    visible: serverDialog.isRemote
                    Accessible.name: qsTr("URL")
                }

                Controls.TextArea {
                    id: headersField
                    Kirigami.FormData.label: qsTr("HTTP headers:")
                    placeholderText: qsTr("Header-Name: value\nX-Api-Key: secret")
                    visible: serverDialog.isRemote
                    Layout.preferredHeight: Kirigami.Units.gridUnit * 5
                    Layout.fillWidth: true
                    Accessible.name: qsTr("HTTP headers")
                }

                Controls.TextField {
                    id: bearerTokenField
                    Kirigami.FormData.label: qsTr("Bearer token:")
                    placeholderText: qsTr("Optional static token (without “Bearer ” prefix)")
                    visible: serverDialog.isRemote && !oauthCheckBox.checked
                    echoMode: TextInput.Password
                    Accessible.name: qsTr("Bearer token")
                }

                Controls.CheckBox {
                    id: oauthCheckBox
                    Kirigami.FormData.label: qsTr("OAuth:")
                    text: qsTr("Use browser OAuth login")
                    visible: serverDialog.isRemote
                }

                Controls.Label {
                    visible: serverDialog.isRemote && oauthCheckBox.checked
                    text: {
                        const u = (urlField.text || "").toLowerCase()
                        if (u.indexOf("githubcopilot.com") >= 0 || u.indexOf("api.github.com") >= 0)
                            return qsTr("GitHub remote MCP does not support Dynamic Client Registration. Prefer a Personal Access Token: turn OAuth off and set Bearer token. Or create your own GitHub App/OAuth App with callback http://127.0.0.1:38471/callback and paste the Client ID below.")
                        return qsTr("Many hosts (including GitHub) do not support automatic client registration. Register a native OAuth client with redirect URI:\nhttp://127.0.0.1:38471/callback\nthen paste the Client ID below. Save, then Log in.")
                    }
                    font.pointSize: Kirigami.Theme.smallFont.pointSize
                    color: Kirigami.Theme.disabledTextColor
                    wrapMode: Text.WordWrap
                    Layout.fillWidth: true
                    Layout.maximumWidth: Kirigami.Units.gridUnit * 28
                }

                Controls.TextField {
                    id: oauthClientIdField
                    Kirigami.FormData.label: qsTr("OAuth client id:")
                    placeholderText: qsTr("Required unless the server supports dynamic registration")
                    visible: serverDialog.isRemote && oauthCheckBox.checked
                    Accessible.name: qsTr("OAuth client id")
                }

                Controls.TextField {
                    id: oauthClientSecretField
                    Kirigami.FormData.label: qsTr("OAuth client secret:")
                    placeholderText: qsTr("Only if the provider issued a secret (public clients leave empty)")
                    visible: serverDialog.isRemote && oauthCheckBox.checked
                    echoMode: TextInput.Password
                    Accessible.name: qsTr("OAuth client secret")
                }

                Controls.TextField {
                    id: oauthScopesField
                    Kirigami.FormData.label: qsTr("OAuth scopes:")
                    placeholderText: qsTr("space-separated; empty = server default")
                    visible: serverDialog.isRemote && oauthCheckBox.checked
                    Accessible.name: qsTr("OAuth scopes")
                }

                Controls.TextField {
                    id: oauthRedirectUriField
                    Kirigami.FormData.label: qsTr("Redirect URI:")
                    text: "http://127.0.0.1:38471/callback"
                    readOnly: true
                    visible: serverDialog.isRemote && oauthCheckBox.checked
                    Accessible.name: qsTr("OAuth redirect URI")
                }
            }
        }
    }

    Kirigami.Dialog {
        id: importDialog
        title: qsTr("Import MCP Servers")
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        preferredWidth: Kirigami.Units.gridUnit * 32
        preferredHeight: Kirigami.Units.gridUnit * 22

        onAccepted: {
            // Client-side parse of Claude Desktop–style JSON; merge into config.mcp.servers
            try {
                const text = importTextArea.text.trim()
                if (!text)
                    return
                const data = JSON.parse(text)
                let map = null
                if (data.mcpServers && typeof data.mcpServers === "object")
                    map = data.mcpServers
                else if (data.servers && typeof data.servers === "object")
                    map = data.servers
                else if (typeof data === "object")
                    map = data
                if (!map)
                    return

                root.ensureMcp()
                const servers = Object.assign({}, root.config.mcp.servers || {})
                const ids = Object.keys(map)
                for (let i = 0; i < ids.length; i++) {
                    const id = ids[i]
                    const src = map[id] || {}
                    if (typeof src !== "object")
                        continue
                    let transport = (src.transport || "").toString().toLowerCase()
                    if (!transport) {
                        if (src.command)
                            transport = "stdio"
                        else if (src.url)
                            transport = "http"
                        else
                            transport = "stdio"
                    }
                    if (transport === "streamablehttp" || transport === "streamable_http")
                        transport = "http"

                    servers[id] = {
                        enabled: src.enabled !== false,
                        name: src.name || id,
                        transport: transport,
                        command: src.command || "",
                        args: Array.isArray(src.args) ? src.args : [],
                        env: src.env && typeof src.env === "object" ? src.env : {},
                        cwd: src.cwd || "",
                        url: src.url || "",
                        headers: src.headers && typeof src.headers === "object" ? src.headers : {},
                        bearer_token: src.bearer_token || src.bearerToken || "",
                        oauth: !!(src.oauth),
                        oauth_client_id: src.oauth_client_id || src.oauthClientId || "",
                        oauth_client_secret: src.oauth_client_secret || src.oauthClientSecret || "",
                        oauth_scopes: Array.isArray(src.oauth_scopes)
                            ? src.oauth_scopes
                            : (Array.isArray(src.oauthScopes) ? src.oauthScopes : []),
                        timeout_secs: src.timeout_secs || src.timeout || null
                    }
                }
                root.config.mcp.servers = servers
                root.config.mcp.enabled = true
                enableMcpCheckBox.checked = true
                root.isDirty = true
                root.refreshServerList()
            } catch (e) {
                console.warn("MCP import failed:", e)
            }
        }

        ColumnLayout {
            anchors.fill: parent
            spacing: Kirigami.Units.smallSpacing

            Controls.Label {
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
                text: qsTr("Paste a Claude Desktop or Cursor MCP config snippet (mcpServers map). Existing server ids are overwritten.")
            }

            Controls.TextArea {
                id: importTextArea
                Layout.fillWidth: true
                Layout.fillHeight: true
                placeholderText: qsTr("{\n  \"mcpServers\": {\n    \"filesystem\": {\n      \"command\": \"npx\",\n      \"args\": [\"-y\", \"@modelcontextprotocol/server-filesystem\", \"/tmp\"]\n    }\n  }\n}")
                wrapMode: TextEdit.Wrap
                Accessible.name: qsTr("MCP import JSON")
            }
        }
    }
}
