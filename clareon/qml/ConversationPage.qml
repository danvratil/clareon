// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import cz.dvratil.clareon 1.0
import cc.clareon.core 1.0
import "message"
import "artifacts"

Kirigami.Page {
    id: root

    required property string conversationId

    // Optional property to highlight a specific message (used for search results)
    property int highlightMessageId: -1

    title: qsTr("Conversation")
    padding: 0

    // Keyboard shortcut to toggle artifacts drawer
    Shortcut {
        sequence: "Ctrl+O"
        onActivated: artifactDrawer.open()
    }

    actions: [
        Kirigami.Action {
            text: qsTr("Artifacts")
            icon.name: "folder-documents"
            checkable: true
            checked: artifactDrawer.drawerOpen
            onTriggered: {
                artifactDrawer.open()
            }
        },
        Kirigami.Action {
            text: qsTr("Conversation settings")
            icon.name: "settings-configure"
            onTriggered: {
                console.log("Open conversation settings for", root.conversationId)
            }
        },
        Kirigami.Action {
            text: qsTr("Delete conversation")
            icon.name: "edit-delete"
            onTriggered: {
                deleteConfirmDialog.open()
            }
        }
    ]

    // Confirmation dialog for deleting conversations
    Kirigami.PromptDialog {
        id: deleteConfirmDialog
        title: qsTr("Delete Conversation")
        subtitle: qsTr("Are you sure you want to delete this conversation? This action cannot be undone.")
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel

        onAccepted: {
            ServiceController.deleteConversation(root.conversationId)
            // Navigate back to the conversation list
            if (pageStack.depth > 1) {
                pageStack.pop()
            }
        }
    }

    MessageListModel {
        id: messageListModel
        conversationId: root.conversationId

        // When messages are loaded, scroll to highlighted message if set
        onRowsInserted: {
            if (root.highlightMessageId >= 0) {
                // Delay scrolling slightly to ensure delegates are rendered
                Qt.callLater(function() {
                    root.scrollToMessage(root.highlightMessageId)
                })
            }
        }
    }

    ArtifactDrawer {
        id: artifactDrawer
        conversationId: root.conversationId

        onPreviewRequested: (id, name, mimeType) => {
            previewSheet.loadArtifact(id, name, mimeType)
        }
    }

    ArtifactPreview {
        id: previewSheet

        onDownloadRequested: (id, filepath) => {
            artifactDrawer.saveArtifact(id, filepath)
        }
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

            Controls.ScrollBar.vertical.policy: Controls.ScrollBar.AsNeeded

            // I tried listening to various Model signals (rowsInserted, dataChanged, etc.), but
            // it seems that when those signals are triggered, the contentHeight is not yet updated
            // (probably because the delegate is not yet instantiated or updated), so in the end
            // scrolling to bottom when contentHeight changes seems to be the only reliable way.
            // FIXME: This makes it impossible for the user to scroll up while a response is being
            // streamed in, as contentHeight keeps changing. We need to make sure this does't
            // trigger when the user scrolls up manually.
            onContentHeightChanged: {
                scrollToBottom()
            }

            Column {
                id: messagesView
                width: messageView.availableWidth
                spacing: 0

                anchors.fill: parent

                // The Repeater is a bit unfortunate choice here, but ListView has serious issues with
                // dynamic height items and scrolling (and scrollbars), since ListView can only guesstimate
                // the heights of delegates that are currently not rendered. This leads to a poor UX.
                // The Repeater instantiates all items, so the scrolling always works correctly. The drawback
                // is that for very long (or large) conversations, this will impact memory usage and initial
                // load times - we will likely need to revisit this in the future.
                Repeater {
                    id: messageRepeater
                    model: messageListModel

                    MessageDelegate {
                        width: parent.width
                        conversationId: root.conversationId
                        // Role names from MessageListModel are automatically set as properties
                        highlighted: root.highlightMessageId === messageId
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

            // Scroll to bottom helper
            function scrollToBottom() {
                if (contentHeight > height) {
                    contentItem.contentY = contentHeight - height
                }
            }

            // Scroll to specific message helper
            function scrollToMessageItem(item) {
                if (item) {
                    // Calculate position to center the message in the view
                    var itemY = item.mapToItem(messageView.contentItem, 0, 0).y
                    var targetY = itemY - (messageView.height / 2) + (item.height / 2)

                    // Clamp to valid range
                    targetY = Math.max(0, Math.min(targetY, contentHeight - messageView.height))

                    contentItem.contentY = targetY
                }
            }
        }

        // Helper function to scroll to a specific message by ID
        function scrollToMessage(messageId) {
            // Find the message delegate with the given ID
            for (var i = 0; i < messageRepeater.count; i++) {
                var item = messageRepeater.itemAt(i)
                if (item && item.messageId === messageId) {
                    messageView.scrollToMessageItem(item)
                    return
                }
            }
        }

        // Message composer
        MessageComposer {
            Layout.fillWidth: true
            conversationId: root.conversationId
        }

        // Token usage display
        TokenUsageDisplay {
            Layout.fillWidth: true
            totalInputTokens: messageListModel.totalInputTokens
            totalOutputTokens: messageListModel.totalOutputTokens
        }
    }
}
