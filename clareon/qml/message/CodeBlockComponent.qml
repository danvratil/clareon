// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

/**
 * Displays a fenced code block with a language label and a copy-to-clipboard button.
 * Syntax highlighting is intentionally omitted here; it will be added later
 * using KSyntaxHighlighting.
 */
ColumnLayout {
    id: root

    property string language: ""
    property string content: ""

    spacing: 0

    // ── Header bar ───────────────────────────────────────────────────────────
    Rectangle {
        Layout.fillWidth: true
        implicitHeight: headerRow.implicitHeight + Kirigami.Units.smallSpacing * 2
        color: Qt.darker(Kirigami.Theme.alternateBackgroundColor, 1.06)
        radius: Kirigami.Units.cornerRadius
        // Only round the top corners; bottom is covered by the code body rectangle
        Rectangle {
            anchors.bottom: parent.bottom
            width: parent.width
            height: parent.radius
            color: parent.color
        }

        RowLayout {
            id: headerRow
            anchors {
                left: parent.left
                right: parent.right
                verticalCenter: parent.verticalCenter
                leftMargin: Kirigami.Units.largeSpacing
                rightMargin: Kirigami.Units.smallSpacing
            }

            Controls.Label {
                text: root.language.length > 0 ? root.language : "text"
                font.family: Kirigami.Theme.fixedWidthFont.family
                font.pointSize: Kirigami.Theme.smallFont.pointSize
                color: Kirigami.Theme.disabledTextColor
            }

            Item { Layout.fillWidth: true }

            Controls.ToolButton {
                id: copyButton
                property bool copied: false

                text: copied ? qsTr("Copied!") : qsTr("Copy")
                icon.name: copied ? "dialog-ok" : "edit-copy"
                display: Controls.AbstractButton.TextBesideIcon
                font.pointSize: Kirigami.Theme.smallFont.pointSize
                flat: true

                onClicked: {
                    // Use a hidden TextEdit to write to the system clipboard
                    clipboardHelper.text = root.content
                    clipboardHelper.selectAll()
                    clipboardHelper.copy()
                    copied = true
                    copyResetTimer.restart()
                }

                Timer {
                    id: copyResetTimer
                    interval: 2000
                    onTriggered: copyButton.copied = false
                }
            }
        }
    }

    // ── Code body ────────────────────────────────────────────────────────────
    Rectangle {
        Layout.fillWidth: true
        implicitHeight: codeScroll.implicitHeight
        color: Kirigami.Theme.alternateBackgroundColor
        radius: Kirigami.Units.cornerRadius
        // Only round the bottom corners
        Rectangle {
            anchors.top: parent.top
            width: parent.width
            height: parent.radius
            color: parent.color
        }

        Controls.ScrollView {
            id: codeScroll
            anchors.fill: parent
            // Show horizontal scrollbar only when content overflows
            Controls.ScrollBar.horizontal.policy: Controls.ScrollBar.AsNeeded
            Controls.ScrollBar.vertical.policy: Controls.ScrollBar.AlwaysOff
            implicitHeight: codeEdit.implicitHeight + Kirigami.Units.largeSpacing * 2

            TextEdit {
                id: codeEdit
                readOnly: true
                selectByMouse: true
                text: root.content
                font.family: Kirigami.Theme.fixedWidthFont.family
                font.pointSize: Kirigami.Theme.defaultFont.pointSize
                color: Kirigami.Theme.textColor
                wrapMode: Text.NoWrap
                padding: Kirigami.Units.largeSpacing
            }
        }
    }

    // Hidden TextEdit used only for clipboard access
    TextEdit {
        id: clipboardHelper
        visible: false
    }
}
