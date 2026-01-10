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

    // ServiceController singleton is automatically available
    Component.onCompleted: {
        console.log("Clareon initialized")
    }

    // Keyboard shortcuts
    Shortcut {
        sequence: "Ctrl+N"
        onActivated: ServiceController.newConversation()
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

    Shortcut {
        sequence: "Ctrl+F"
        onActivated: openSearchPage()
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
                ListElement { shortcut: "Ctrl+F"; description: "Search conversations" }
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
        initialPage: conversationListPage
        columnView.columnResizeMode: pageStack.wideMode ? Kirigami.ColumnView.DynamicColumns : Kirigami.ColumnView.SingleColumn
    }

    Component {
        id: conversationListPage
        ConversationListPage {}
    }

    Component {
        id: searchResultsPage
        SearchResultsPage {}
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

    function openSearchPage() {
        const searchPage = searchResultsPage.createObject(null)
        if (searchPage) {
            pageStack.push(searchPage)
        } else {
            console.error("Failed to create SearchResultsPage")
        }
    }
}
