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

    // Signal connections for AppController
    Connections {
        target: appController

        function onConversationChanged() {
            messageModel.loadMessages(appController.currentConversationId)
        }

        function onMessagesLoaded() {
            // Messages loaded, scroll handled by ChatView
        }

        function onMessageSent() {
            messageModel.loadMessages(appController.currentConversationId)
        }

        function onError(message) {
            // TODO: Show error dialog instead of just logging
            console.error("Error:", message)
        }
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

    Shortcut {
        sequence: "Ctrl+,"
        onActivated: openConfiguration()
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
                ListElement { shortcut: "Ctrl+,"; description: "Open settings" }
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

    pageStack {
        initialPage: ConversationListPage {
            appController: root.appController
        }
        columnView.columnResizeMode: pageStack.wideMode ? Kirigami.ColumnView.DynamicColumns : Kirigami.ColumnView.SingleColumn
    }

    // Configuration window loader
    Loader {
        id: configWindowLoader
        active: false
        source: "qrc:/qt/qml/cz/dvratil/clareon/qml/config/ConfigurationPage.qml"

        onLoaded: {
            item.show()
            item.raise()
            item.requestActivate()
        }
    }

    function openConfiguration() {
        if (configWindowLoader.item) {
            // Window already exists, just show it
            configWindowLoader.item.show()
            configWindowLoader.item.raise()
            configWindowLoader.item.requestActivate()
        } else {
            // Create the window
            configWindowLoader.active = true
        }
    }
}
