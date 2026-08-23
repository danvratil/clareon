// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as Controls
import org.kde.kirigami as Kirigami
import cz.dvratil.clareon 1.0

Rectangle {
    id: root

    required property string pendingApprovalJson
    required property string conversationId

    readonly property var tools: {
        try {
            const parsed = JSON.parse(root.pendingApprovalJson || "[]")
            return Array.isArray(parsed) ? parsed : []
        } catch (e) {
            return []
        }
    }

    visible: tools.length > 0
    color: Kirigami.Theme.alternateBackgroundColor
    border.color: Kirigami.Theme.neutralTextColor
    border.width: 1
    radius: Kirigami.Units.cornerRadius

    implicitHeight: content.implicitHeight + Kirigami.Units.largeSpacing * 2

    function formatInput(input) {
        if (input === undefined || input === null)
            return qsTr("(no arguments)")
        if (typeof input === "string") {
            const trimmed = input.trim()
            if (!trimmed.length)
                return qsTr("(no arguments)")
            try {
                return JSON.stringify(JSON.parse(trimmed), null, 2)
            } catch (e) {
                return input
            }
        }
        if (typeof input === "object" && Object.keys(input).length === 0)
            return qsTr("(no arguments)")
        try {
            return JSON.stringify(input, null, 2)
        } catch (e) {
            return String(input)
        }
    }

    ColumnLayout {
        id: content
        anchors.fill: parent
        anchors.margins: Kirigami.Units.largeSpacing
        spacing: Kirigami.Units.smallSpacing

        Controls.Label {
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
            font.bold: true
            text: tools.length === 1
                  ? qsTr("The assistant wants to use a tool")
                  : qsTr("The assistant wants to use %1 tools").arg(tools.length)
        }

        Repeater {
            model: root.tools

            delegate: ColumnLayout {
                required property var modelData
                Layout.fillWidth: true
                spacing: Kirigami.Units.smallSpacing / 2

                Controls.Label {
                    Layout.fillWidth: true
                    text: modelData.name || qsTr("Unknown tool")
                    font.bold: true
                    elide: Text.ElideRight
                }

                Controls.ScrollView {
                    Layout.fillWidth: true
                    Layout.maximumHeight: Kirigami.Units.gridUnit * 10
                    contentWidth: availableWidth

                    Controls.TextArea {
                        readOnly: true
                        wrapMode: TextEdit.Wrap
                        selectByMouse: true
                        font.family: "monospace"
                        font.pointSize: Kirigami.Theme.smallFont.pointSize
                        text: root.formatInput(modelData.input)
                    }
                }

                Controls.Label {
                    Layout.fillWidth: true
                    visible: !!(modelData.always_label)
                    wrapMode: Text.WordWrap
                    font.pointSize: Kirigami.Theme.smallFont.pointSize
                    color: Kirigami.Theme.disabledTextColor
                    text: qsTr("Always allow/deny will remember: %1").arg(modelData.always_label)
                }
            }
        }

        Flow {
            Layout.fillWidth: true
            spacing: Kirigami.Units.smallSpacing

            Controls.Button {
                text: qsTr("Allow")
                icon.name: "dialog-ok"
                onClicked: ServiceController.resolveToolApproval(root.conversationId, "allow")
            }
            Controls.Button {
                text: qsTr("Always allow")
                icon.name: "flag-green"
                onClicked: ServiceController.resolveToolApproval(root.conversationId, "always")
            }
            Controls.Button {
                text: qsTr("Always deny")
                icon.name: "flag-red"
                onClicked: ServiceController.resolveToolApproval(root.conversationId, "always_deny")
            }
            Controls.Button {
                text: qsTr("Deny")
                icon.name: "dialog-cancel"
                onClicked: ServiceController.resolveToolApproval(root.conversationId, "deny")
            }
        }
    }
}
