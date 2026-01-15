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

    signal conversationStarted(string conversationId)

    title: qsTr("Clareon")
    padding: 0

    // Store the pending message to send after conversation is created
    property string pendingMessage: ""

    Connections {
        target: ServiceController

        function onConversationCreated(conversationId) {
            // If we have a pending message, send it and navigate to the conversation
            if (root.pendingMessage.length > 0) {
                ServiceController.sendMessage(conversationId, root.pendingMessage)
                root.pendingMessage = ""
                root.conversationStarted(conversationId)
            }
        }
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        // Center content area
        Item {
            Layout.fillWidth: true
            Layout.fillHeight: true

            ColumnLayout {
                anchors.centerIn: parent
                width: Math.min(parent.width * 0.6, 800)
                spacing: Kirigami.Units.gridUnit * 2

                // Greeting text
                Kirigami.Heading {
                    Layout.alignment: Qt.AlignHCenter
                    level: 1
                    text: qsTr("Welcome to Clareon")
                    wrapMode: Text.WordWrap
                }

                Kirigami.Heading {
                    Layout.alignment: Qt.AlignHCenter
                    Layout.maximumWidth: parent.width
                    level: 3
                    text: qsTr("Your Claude assistant for Linux")
                    opacity: 0.7
                    wrapMode: Text.WordWrap
                }

                Item {
                    Layout.preferredHeight: Kirigami.Units.gridUnit
                }

                // Message input area
                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: Kirigami.Units.largeSpacing

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
                                placeholderText: qsTr("Message Claude...")
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
                            text: qsTr("Send")
                            icon.name: "document-send"
                            enabled: messageInput.text.trim().length > 0

                            onClicked: {
                                if (messageInput.text.trim().length === 0) {
                                    return
                                }

                                // Store the message to send after conversation is created
                                root.pendingMessage = messageInput.text

                                // Clear the input
                                messageInput.text = ""

                                // Create a new conversation
                                ServiceController.newConversation()
                            }
                        }
                    }

                    Kirigami.Separator {
                        Layout.fillWidth: true
                    }
                }
            }
        }
    }
}
