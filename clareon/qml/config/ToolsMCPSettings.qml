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
                checked: root.config.tools?.enabled || true
                onToggled: {
                    root.config.tools.enabled = checked
                }
            }

            Controls.CheckBox {
                id: autoExecuteCheckBox
                Kirigami.FormData.label: qsTr("Auto-execute:")
                text: qsTr("Automatically execute tools without approval")
                checked: root.config.tools?.autoExecute || true
                enabled: enableToolsCheckBox.checked
                onToggled: {
                    root.config.tools.autoExecute = checked
                }
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
                type: Kirigami.MessageType.Information
                text: qsTr("Servers can be configured and saved now. Live connections and tool discovery land in a follow-up.")
                visible: true
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

            // Server list from config.mcp.servers (id → object map)
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

                        Controls.CheckBox {
                            checked: serverEnabled
                            onToggled: root.setServerEnabled(serverId, checked)
                        }

                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: 0

                            Controls.Label {
                                text: displayName
                                font.bold: true
                                elide: Text.ElideRight
                                Layout.fillWidth: true
                            }

                            Controls.Label {
                                text: qsTr("%1 · %2").arg(transport).arg(summary)
                                font.pointSize: Kirigami.Theme.smallFont.pointSize
                                color: Kirigami.Theme.disabledTextColor
                                elide: Text.ElideRight
                                Layout.fillWidth: true
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
                    onClicked: addServerDialog.open()
                }

                Controls.Button {
                    text: qsTr("Import JSON…")
                    icon.name: "document-import"
                    enabled: enableMcpCheckBox.checked
                    onClicked: importDialog.open()
                }

                Item {
                    Layout.fillWidth: true
                }
            }
        }
    }

    // --- MCP helpers (config-only until runtime lands) ---

    /// Flatten config.mcp.servers map into a ListModel for the ListView.
    property ListModel serverListModel: ListModel {}

    function ensureMcp() {
        if (!root.config.mcp) {
            // Should not happen once codegen includes McpConfig; keep a soft guard.
            return
        }
        if (root.config.mcp.servers === undefined || root.config.mcp.servers === null) {
            root.config.mcp.servers = {}
        }
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
            serverListModel.append({
                serverId: id,
                displayName: s.name || id,
                transport: transport,
                serverEnabled: s.enabled !== false,
                summary: summary || qsTr("(incomplete)")
            })
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

    Component.onCompleted: refreshServerList()

    Kirigami.Dialog {
        id: addServerDialog
        title: qsTr("Add MCP Server")
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        preferredWidth: Kirigami.Units.gridUnit * 28

        property bool isRemote: transportCombo.currentIndex > 0

        onAccepted: {
            const id = serverIdField.text.trim()
            if (!id)
                return
            const transport = ["stdio", "http", "sse"][transportCombo.currentIndex] || "stdio"
            const entry = {
                enabled: true,
                name: displayNameField.text.trim() || id,
                transport: transport,
                command: transport === "stdio" ? commandField.text.trim() : "",
                args: transport === "stdio"
                      ? argsField.text.trim().split(/\s+/).filter(a => a.length > 0)
                      : [],
                env: {},
                cwd: transport === "stdio" ? (cwdField.text.trim() || null) : null,
                url: transport !== "stdio" ? urlField.text.trim() : "",
                headers: {},
                timeout_secs: null
            }
            // Prefer null/empty optional fields omitted as empty string for QVariantMap round-trip
            if (!entry.cwd)
                entry.cwd = ""
            root.upsertServer(id, entry)
        }

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
                    visible: !addServerDialog.isRemote
                    Accessible.name: qsTr("Command")
                }

                Controls.TextField {
                    id: argsField
                    Kirigami.FormData.label: qsTr("Arguments:")
                    placeholderText: qsTr("-y @modelcontextprotocol/server-filesystem /path")
                    visible: !addServerDialog.isRemote
                    Accessible.name: qsTr("Arguments")
                }

                Controls.TextField {
                    id: cwdField
                    Kirigami.FormData.label: qsTr("Working directory:")
                    placeholderText: qsTr("Optional")
                    visible: !addServerDialog.isRemote
                    Accessible.name: qsTr("Working directory")
                }

                Controls.TextField {
                    id: urlField
                    Kirigami.FormData.label: qsTr("URL:")
                    placeholderText: qsTr("https://example.com/mcp")
                    visible: addServerDialog.isRemote
                    Accessible.name: qsTr("URL")
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
