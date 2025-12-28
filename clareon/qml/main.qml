// SPDX-FileContributor: Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as Controls
import org.kde.kirigami as Kirigami
import cz.dvratil.clareon 1.0

Kirigami.ApplicationWindow {
    id: root
    title: "Clareon"
    width: 1200
    height: 800
    minimumWidth: 800
    minimumHeight: 600

    // Create the models and controller
    AppController {
        id: appController
    }

    ConversationListModel {
        id: conversationModel
    }

    MessageListModel {
        id: messageModel
    }

    // Global drawer with conversation list
    globalDrawer: Kirigami.GlobalDrawer {
        id: drawer
        title: "Conversations"
        titleIcon: "view-conversation-balloon"

        modal: false
        handleVisible: true
        width: 320

        actions: [
            Kirigami.Action {
                text: "New Conversation"
                icon.name: "list-add"
                onTriggered: {
                    appController.newConversation()
                }
            },
            Kirigami.Action {
                text: "Settings"
                icon.name: "configure"
                onTriggered: {
                    // TODO: Open settings dialog
                }
            }
        ]

        // Drawer content
        ConversationDrawer {
            Layout.fillWidth: true
            appController: appController
            conversationModel: conversationModel
        }
    }

    // Main chat view
    pageStack.initialPage: ChatView {
        appController: appController
        messageModel: messageModel
    }

    // Keyboard shortcuts
    Shortcut {
        sequence: "Ctrl+N"
        onActivated: appController.newConversation()
    }

    Shortcut {
        sequence: "Ctrl+O"
        onActivated: drawer.drawerOpen = !drawer.drawerOpen
    }

    Shortcut {
        sequence: "Ctrl+Q"
        onActivated: Qt.quit()
    }

    Shortcut {
        sequence: "Ctrl+W"
        onActivated: Qt.quit()
    }

    // Help dialog
    Controls.Dialog {
        id: helpDialog
        title: "Keyboard Shortcuts"
        modal: true
        standardButtons: Controls.Dialog.Close

        anchors.centerIn: parent
        width: 400

        contentItem: ListView {
            implicitHeight: contentHeight
            model: ListModel {
                ListElement { shortcut: "Ctrl+N"; description: "New conversation" }
                ListElement { shortcut: "Ctrl+O"; description: "Toggle conversation drawer" }
                ListElement { shortcut: "Enter"; description: "Send message" }
                ListElement { shortcut: "Shift+Enter"; description: "New line in message" }
                ListElement { shortcut: "Esc"; description: "Clear message input" }
                ListElement { shortcut: "Ctrl+Q"; description: "Quit application" }
            }

            delegate: Controls.ItemDelegate {
                width: ListView.view.width
                contentItem: RowLayout {
                    Controls.Label {
                        text: model.shortcut
                        font.family: "monospace"
                        Layout.preferredWidth: 120
                    }
                    Controls.Label {
                        text: model.description
                        Layout.fillWidth: true
                    }
                }
            }
        }
    }

    Shortcut {
        sequence: "F1"
        onActivated: helpDialog.open()
    }

    Shortcut {
        sequence: "Ctrl+H"
        onActivated: helpDialog.open()
    }
}
