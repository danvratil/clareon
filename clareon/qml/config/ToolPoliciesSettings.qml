// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as Controls
import org.kde.kirigami as Kirigami

Kirigami.ScrollablePage {
    id: root

    title: qsTr("Allow & Deny")

    property var config
    property bool isDirty: false
    property var allowModel: []
    property var denyModel: []

    Component.onCompleted: {
        allowModel = allowList()
        denyModel = denyList()
    }

    function toArray(list) {
        const arr = []
        if (!list)
            return arr
        if (Array.isArray(list))
            return list.slice()
        for (let i = 0; i < list.length; ++i)
            arr.push(String(list[i]))
        return arr
    }

    function allowList() {
        return root.toArray(root.config.tools?.alwaysAllow)
    }

    function denyList() {
        return root.toArray(root.config.tools?.alwaysDeny)
    }

    function setAllowList(arr) {
        root.config.tools.alwaysAllow = arr
        root.allowModel = arr
        root.isDirty = true
    }

    function setDenyList(arr) {
        root.config.tools.alwaysDeny = arr
        root.denyModel = arr
        root.isDirty = true
    }

    function describeRule(spec) {
        if (spec.startsWith("path:")) {
            const rest = spec.slice(5)
            const idx = rest.indexOf(":")
            if (idx > 0)
                return qsTr("%1 on %2 and anything under it").arg(rest.slice(0, idx)).arg(rest.slice(idx + 1))
        }
        if (spec.startsWith("tool:"))
            return qsTr("%1 (any arguments)").arg(spec.slice(5))
        if (spec.startsWith("exec:"))
            return qsTr("exec %1").arg(spec.slice(5))
        if (spec.startsWith("mcp_"))
            return qsTr("%1 (any arguments)").arg(spec)
        return spec
    }

    function ruleKind(spec) {
        if (spec.startsWith("path:"))
            return qsTr("Path")
        if (spec.startsWith("exec:"))
            return qsTr("Exec")
        return qsTr("Tool")
    }

    function addSpec(allow, spec) {
        if (!spec)
            return
        const list = allow ? root.allowList() : root.denyList()
        if (list.indexOf(spec) !== -1)
            return
        list.push(spec)
        if (allow)
            root.setAllowList(list)
        else
            root.setDenyList(list)
    }

    function removeAt(allow, index) {
        const list = allow ? root.allowList() : root.denyList()
        list.splice(index, 1)
        if (allow)
            root.setAllowList(list)
        else
            root.setDenyList(list)
    }

    function openAddDialog(allow) {
        addDialog.allow = allow
        addDialog.kindIndex = 0
        addDialog.pathToolIndex = 0
        addDialog.pathValue = ""
        addDialog.toolName = ""
        addDialog.execValue = ""
        addDialog.open()
    }

    ColumnLayout {
        width: root.width
        spacing: Kirigami.Units.largeSpacing

        Controls.Label {
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
            text: qsTr("These rules apply even when auto-execute is on. Deny always wins if both lists match. File tools are scoped to a path (and descendants), exec to a command prefix, and MCP tools to the tool name.")
            color: Kirigami.Theme.disabledTextColor
            font.pointSize: Kirigami.Theme.smallFont.pointSize
        }

        Kirigami.Heading {
            text: qsTr("Always allow")
            level: 3
        }

        RuleList {
            Layout.fillWidth: true
            model: root.allowModel
            emptyText: qsTr("No always-allow rules")
            emptyExplanation: qsTr("Add a rule here, or choose Always allow when a tool is requested.")
            onRemoveRequested: (index) => root.removeAt(true, index)
        }

        Controls.Button {
            text: qsTr("Add allow rule")
            icon.name: "list-add"
            onClicked: root.openAddDialog(true)
        }

        Kirigami.Heading {
            text: qsTr("Always deny")
            level: 3
        }

        RuleList {
            Layout.fillWidth: true
            model: root.denyModel
            emptyText: qsTr("No always-deny rules")
            emptyExplanation: qsTr("Denied tools are rejected automatically and never prompt.")
            onRemoveRequested: (index) => root.removeAt(false, index)
        }

        Controls.Button {
            text: qsTr("Add deny rule")
            icon.name: "list-add"
            onClicked: root.openAddDialog(false)
        }
    }

    component RuleList: Rectangle {
        id: box

        property var model: []
        property string emptyText
        property string emptyExplanation
        signal removeRequested(int index)

        implicitHeight: Math.max(list.contentHeight + Kirigami.Units.smallSpacing * 2, 120)
        Layout.maximumHeight: 280
        color: Kirigami.Theme.alternateBackgroundColor
        border.color: Kirigami.Theme.separatorColor
        border.width: 1
        radius: 4
        clip: true

        ListView {
            id: list
            anchors.fill: parent
            anchors.margins: Kirigami.Units.smallSpacing
            clip: true
            model: box.model
            spacing: Kirigami.Units.smallSpacing

            delegate: RowLayout {
                width: list.width
                spacing: Kirigami.Units.smallSpacing
                required property int index
                required property var modelData

                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 0
                    Controls.Label {
                        text: root.describeRule(String(modelData))
                        elide: Text.ElideMiddle
                        Layout.fillWidth: true
                    }
                    Controls.Label {
                        text: root.ruleKind(String(modelData)) + " · " + String(modelData)
                        font.pointSize: Kirigami.Theme.smallFont.pointSize
                        font.family: "monospace"
                        color: Kirigami.Theme.disabledTextColor
                        elide: Text.ElideMiddle
                        Layout.fillWidth: true
                    }
                }
                Controls.Button {
                    icon.name: "edit-delete"
                    flat: true
                    Accessible.name: qsTr("Remove rule")
                    onClicked: box.removeRequested(index)
                }
            }

            Kirigami.PlaceholderMessage {
                anchors.centerIn: parent
                width: parent.width - Kirigami.Units.gridUnit
                visible: list.count === 0
                text: box.emptyText
                explanation: box.emptyExplanation
            }
        }
    }

    Kirigami.Dialog {
        id: addDialog
        title: allow ? qsTr("Add allow rule") : qsTr("Add deny rule")
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        preferredWidth: Kirigami.Units.gridUnit * 28

        property bool allow: true
        property int kindIndex: 0
        property int pathToolIndex: 0
        property string pathValue: ""
        property string toolName: ""
        property string execValue: ""

        readonly property var pathTools: ["read_file", "write_file", "list_directory"]
        readonly property bool canAccept: {
            if (kindIndex === 0)
                return pathValue.trim().length > 0
            if (kindIndex === 1)
                return toolName.trim().length > 0
            return execValue.trim().length > 0
        }

        onOpened: {
            const ok = standardButton(Kirigami.Dialog.Ok)
            if (ok)
                ok.enabled = Qt.binding(() => addDialog.canAccept)
        }

        onAccepted: {
            let spec = ""
            if (kindIndex === 0) {
                spec = "path:" + pathTools[pathToolIndex] + ":" + pathValue.trim()
            } else if (kindIndex === 1) {
                const name = toolName.trim()
                spec = name.startsWith("tool:") ? name : ("tool:" + name)
            } else {
                spec = "exec:" + execValue.trim()
            }
            root.addSpec(allow, spec)
        }

        ColumnLayout {
            spacing: Kirigami.Units.smallSpacing

            Kirigami.FormLayout {
                Layout.fillWidth: true

                Controls.ComboBox {
                    id: kindCombo
                    Kirigami.FormData.label: qsTr("Rule type:")
                    model: [qsTr("File path"), qsTr("Whole tool (MCP)"), qsTr("Exec command")]
                    currentIndex: addDialog.kindIndex
                    onActivated: addDialog.kindIndex = currentIndex
                }

                Controls.ComboBox {
                    Kirigami.FormData.label: qsTr("Tool:")
                    visible: addDialog.kindIndex === 0
                    model: addDialog.pathTools
                    currentIndex: addDialog.pathToolIndex
                    onActivated: addDialog.pathToolIndex = currentIndex
                }

                Controls.TextField {
                    Kirigami.FormData.label: qsTr("Path:")
                    visible: addDialog.kindIndex === 0
                    placeholderText: qsTr("/home/you/project")
                    text: addDialog.pathValue
                    onTextChanged: addDialog.pathValue = text
                }

                Controls.TextField {
                    Kirigami.FormData.label: qsTr("Tool name:")
                    visible: addDialog.kindIndex === 1
                    placeholderText: qsTr("mcp_github_list_issues")
                    text: addDialog.toolName
                    onTextChanged: addDialog.toolName = text
                }

                Controls.TextField {
                    Kirigami.FormData.label: qsTr("Command prefix:")
                    visible: addDialog.kindIndex === 2
                    placeholderText: qsTr("git status")
                    text: addDialog.execValue
                    onTextChanged: addDialog.execValue = text
                }
            }

            Controls.Label {
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
                color: Kirigami.Theme.disabledTextColor
                font.pointSize: Kirigami.Theme.smallFont.pointSize
                text: {
                    if (addDialog.kindIndex === 0)
                        return qsTr("The path and any file or directory under it will match.")
                    if (addDialog.kindIndex === 1)
                        return qsTr("Matches this tool name for any arguments. Use the prefixed MCP name.")
                    return qsTr("Matches this command and any extra arguments after it.")
                }
            }
        }
    }
}
