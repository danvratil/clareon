// SPDX-FileContributor: Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import cz.dvratil.clareon 1.0

ColumnLayout {
    id: root
    spacing: 0

    required property AppController appController
    required property ConversationListModel conversationModel

    // Search bar at the top
    Kirigami.SearchField {
        id: searchField
        Layout.fillWidth: true
        Layout.margins: Kirigami.Units.smallSpacing
        placeholderText: "Search conversations..."

        onTextChanged: {
            if (text.length > 0) {
                appController.search(text)
            } else {
                appController.viewMode = "chat"
            }
        }

        onAccepted: {
            if (text.length > 0) {
                appController.search(text)
            }
        }
    }

    Kirigami.Separator {
        Layout.fillWidth: true
    }

    // Conversation list
    ListView {
        id: conversationList
        Layout.fillWidth: true
        Layout.fillHeight: true

        model: conversationModel

        delegate: Controls.ItemDelegate {
            id: conversationDelegate

            required property string conversationId
            required property string title
            required property int updatedAt
            required property string model
            required property int messageCount

            highlighted: conversationDelegate.conversationId === appController.currentConversationId

            contentItem: ColumnLayout {
                spacing: Kirigami.Units.smallSpacing

                RowLayout {
                    Layout.fillWidth: true
                    spacing: Kirigami.Units.smallSpacing

                    Controls.Label {
                        Layout.fillWidth: true
                        text: conversationDelegate.title
                        font.bold: conversationDelegate.highlighted
                        elide: Text.ElideRight
                    }

                    Controls.Label {
                        text: Qt.formatDateTime(new Date(conversationDelegate.updatedAt * 1000), "MMM d")
                        opacity: 0.7
                        font.pointSize: Kirigami.Theme.smallFont.pointSize
                    }
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: Kirigami.Units.largeSpacing

                    Controls.Label {
                        text: conversationDelegate.model
                        opacity: 0.6
                        font.pointSize: Kirigami.Theme.smallFont.pointSize
                        elide: Text.ElideRight
                    }

                    Controls.Label {
                        text: conversationDelegate.messageCount + " messages"
                        opacity: 0.6
                        font.pointSize: Kirigami.Theme.smallFont.pointSize
                    }
                }
            }

            onClicked: {
                appController.selectConversation(conversationDelegate.conversationId)
            }
        }

        // Empty state
        Kirigami.PlaceholderMessage {
            anchors.centerIn: parent
            width: parent.width - (Kirigami.Units.largeSpacing * 4)
            visible: conversationList.count === 0
            text: "No conversations"
            explanation: "Start a new conversation to get started"
        }
    }
}
