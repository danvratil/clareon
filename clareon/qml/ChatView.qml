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
    padding: 0

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
            id: messageView
            Layout.fillWidth: true
            Layout.fillHeight: true

            contentWidth: availableWidth

            ColumnLayout {
                id: messagesView
                width: messageView.availableWidth
                spacing: 0

                // The Repeater is a bit unfortunate choice here, but ListView has serious issues with
                // dynamic height items and scrolling (and scrollbars), since ListView can only guesstimate
                // the heights of delegates that are currently not rendered. This leads to a poor UX.
                // The Repeater instantiates all items, so the scrolling always works correctly. The drawback
                // is that for very long (or large) conversations, this will impact memory usage and initial
                // load times - we will likely need to revisit this in the future.
                Repeater {
                    model: messageListModel

                    MessageDelegate {
                        Layout.fillWidth: true
                        // Role maps to delegate's properties automatically
                    }
                }

                // Empty state
                Kirigami.PlaceholderMessage {
                    Layout.fillWidth: true
                    Layout.alignment: Qt.AlignVCenter
                    visible: messagesView.count === 0
                    icon.name: "view-conversation-balloon"
                    text: "No messages yet"
                    explanation: "Start a conversation by typing a message below"
                }
            }

            Controls.ScrollBar.vertical: Controls.ScrollBar { }

            // Scroll to bottom helper
            function scrollToBottom() {
                if (contentHeight > height) {
                    contentItem.contentY = contentHeight - height
                }
            }

            // Auto-scroll when messages change
            Connections {
                target: messageListModel
                function onCountChanged() {
                    Qt.callLater(() => messageView.scrollToBottom())
                }
            }

            Component.onCompleted: {
                Qt.callLater(() => scrollToBottom())
            }
        }

        // Message composer
        MessageComposer {
            Layout.fillWidth: true
            conversationId: root.conversationId
        }
    }
}
