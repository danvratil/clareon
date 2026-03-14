// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as Controls
import org.kde.kirigami as Kirigami

Controls.ItemDelegate {
    id: delegate

    required property string conversationId
    required property string conversationTitle
    required property int messageId
    required property string role
    required property string snippet
    required property int createdAt

    contentItem: ColumnLayout {
        spacing: Kirigami.Units.smallSpacing

        // Conversation title and timestamp
        RowLayout {
            Layout.fillWidth: true
            spacing: Kirigami.Units.smallSpacing

            Controls.Label {
                Layout.fillWidth: true
                text: if (delegate.conversationTitle != "") {
                    delegate.conversationTitle
                } else {
                    qsTr("Unnamed Conversation")
                }
                font.bold: true
                elide: Text.ElideRight
            }

            Controls.Label {
                text: Qt.formatDateTime(new Date(delegate.createdAt * 1000), "MMM d, h:mm AP")
                opacity: 0.7
                font.pointSize: Kirigami.Theme.smallFont.pointSize
            }
        }

        // Role indicator
        Controls.Label {
            text: delegate.role === "user" ? qsTr("You") : qsTr("Assistant")
            opacity: 0.6
            font.pointSize: Kirigami.Theme.smallFont.pointSize
            font.bold: true
            color: delegate.role === "user"
                ? Kirigami.Theme.highlightColor
                : Kirigami.Theme.textColor
        }

        // Snippet with highlighting
        Text {
            Layout.fillWidth: true
            text: delegate.snippet
            textFormat: Text.StyledText
            wrapMode: Text.WordWrap
            maximumLineCount: 3
            elide: Text.ElideRight
            color: Kirigami.Theme.textColor

            // Style for <mark> tags (highlight matches)
            font.pointSize: Kirigami.Theme.defaultFont.pointSize
        }
    }

    // Visual separator
    Rectangle {
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        height: 1
        color: Kirigami.Theme.alternateBackgroundColor
    }
}
