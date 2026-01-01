// SPDX-FileContributor: Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import cz.dvratil.clareon 1.0

Kirigami.Page {
    id: root

    required property string conversationId

    title: qsTr("Conversation")

    MessageListModel {
        id: messageDataProvider
        conversationId: root.conversationId

        onDataChanged: {
            // Rebuild the message list from data provider
            messageListModel.clear()
            for (var i = 0; i < messageDataProvider.count; i++) {
                messageListModel.append({
                    messageId: messageDataProvider.getId(i),
                    role: messageDataProvider.getRole(i),
                    textContent: messageDataProvider.getText(i),
                    createdAt: messageDataProvider.getCreatedAt(i)
                })
            }
        }
    }

    ListModel {
        id: messageListModel
    }

    // Connect to ServiceController signals
    Connections {
        target: ServiceController

        function onMessagesLoaded(conversationId) {
            if (conversationId === root.conversationId) {
                messageDataProvider.refresh()
            }
        }

        function onMessagesChanged(conversationId) {
            if (conversationId === root.conversationId) {
                messageDataProvider.refresh()
            }
        }

        function onStreamingComplete(conversationId) {
            if (conversationId === root.conversationId) {
                messageDataProvider.refresh()
            }
        }
    }

    // Set which conversation this model is displaying
    Component.onCompleted: {
        ServiceController.loadMessages(root.conversationId)
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        // Messages view
        Controls.ScrollView {
            Layout.fillWidth: true
            Layout.fillHeight: true

            ListView {
                id: messagesView
                clip: true

                model: messageListModel

                // Scroll to bottom when new messages arrive
                onCountChanged: {
                    Qt.callLater(() => {
                        messagesView.positionViewAtEnd()
                    })
                }

                Component.onCompleted: {
                    messagesView.positionViewAtEnd()
                }

                delegate: MessageDelegate {
                    // Role maps to delegate's properties automatically
                }

                // Empty state
                Kirigami.PlaceholderMessage {
                    anchors.centerIn: parent
                    width: parent.width - (Kirigami.Units.largeSpacing * 4)
                    visible: messagesView.count === 0
                    icon.name: "view-conversation-balloon"
                    text: "No messages yet"
                    explanation: "Start a conversation by typing a message below"
                }
            }
        }

        // Message composer
        MessageComposer {
            Layout.fillWidth: true
            conversationId: root.conversationId
        }
    }
}
