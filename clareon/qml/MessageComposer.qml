// SPDX-FileContributor: Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

ColumnLayout {
    id: root
    spacing: 0

    required property var appController
    required property var messageModel

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
            enabled: !root.appController.isWaiting && messageInput.text.trim().length > 0

            onClicked: {
                if (messageInput.text.trim().length === 0) {
                    return
                }

                // Add user message to the model
                root.messageModel.appendMessage("user", messageInput.text)

                // Call the app controller to send the message
                root.appController.sendMessage(messageInput.text)

                // Simulate assistant response after a delay
                // In a real app, this would come from the backend
                Qt.callLater(() => {
                    root.messageModel.appendMessage(
                        "assistant",
                        "This is a mock response. In the final version, this will be a real response from Claude."
                    )
                })

                // Clear the input
                messageInput.text = ""

                // Emit signal
                root.messageSent(messageInput.text)
            }
        }
    }

    // Status bar
    RowLayout {
        Layout.fillWidth: true
        Layout.margins: Kirigami.Units.smallSpacing
        spacing: Kirigami.Units.smallSpacing
        visible: root.appController.statusMessage.length > 0

        Controls.Label {
            Layout.fillWidth: true
            text: root.appController.statusMessage
            font.pointSize: Kirigami.Theme.smallFont.pointSize
            opacity: 0.7
            elide: Text.ElideRight
        }

        Controls.BusyIndicator {
            Layout.preferredWidth: Kirigami.Units.iconSizes.small
            Layout.preferredHeight: Kirigami.Units.iconSizes.small
            running: root.appController.isWaiting
            visible: root.appController.isWaiting
        }
    }
}
