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

    required property string conversationId

    signal messageSent(string text)

    Kirigami.Separator {
        Layout.fillWidth: true
    }

    RowLayout {
        Layout.fillWidth: true
        Layout.margins: Kirigami.Units.largeSpacing
        spacing: Kirigami.Units.largeSpacing

        Controls.ScrollView {
            Layout.fillWidth: true
            Layout.maximumHeight: 150
            Layout.minimumHeight: 60

            Controls.TextArea {
                id: messageInput
                placeholderText: "Message Claude..."
                wrapMode: TextEdit.Wrap
                selectByMouse: true
                focus: true

                Keys.onReturnPressed: (event) => {
                    if (event.modifiers & Qt.ControlModifier || event.modifiers & Qt.ShiftModifier) {
                        // Ctrl+Enter or Shift+Enter: insert newline
                        event.accepted = false
                    } else {
                        // Enter: send message
                        event.accepted = true
                        sendButton.clicked()
                    }
                }

                Keys.onEscapePressed: {
                    messageInput.text = ""
                }
            }
        }

        Controls.Button {
            id: sendButton
            text: "Send"
            icon.name: "document-send"
            enabled: messageInput.text.trim().length > 0

            onClicked: {
                if (messageInput.text.trim().length === 0) {
                    return
                }

                // Store the message text before clearing
                const messageText = messageInput.text

                // Clear the input first
                messageInput.text = ""

                // Call ServiceController to send the message
                ServiceController.sendMessage(root.conversationId, messageText)

                // Emit signal
                root.messageSent(messageText)
            }
        }
    }
}
