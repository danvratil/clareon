// SPDX-FileContributor: Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import cz.dvratil.clareon 1.0
import cc.clareon.core 1.0

Kirigami.Page {
    id: root

    required property string conversationId

    title: qsTr("Conversation")
    padding: 0

    MessageListModel {
        id: messageListModel
        conversationId: root.conversationId
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
                    model: messageListModel

                    MessageDelegate {
                        width: parent.width
                        // Role names from MessageListModel are automatically set as properties
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
        }

        // Message composer
        MessageComposer {
            Layout.fillWidth: true
            conversationId: root.conversationId
        }
    }
}
