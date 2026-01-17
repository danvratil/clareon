// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as Controls
import org.kde.kirigami as Kirigami
import Qt.labs.platform as Platform
import cz.dvratil.clareon 1.0

Kirigami.ApplicationWindow {
    id: root
    title: "Clareon"
    width: 1200
    height: 800
    minimumWidth: 800
    minimumHeight: 600

    property string currentConversationId: ""

    // System tray icon
    Platform.SystemTrayIcon {
        id: systemTray
        visible: true
        icon.name: ":/clareon-256.png"
        tooltip: "Clareon Assistant"

        onActivated: function(reason) {
            if (reason === Platform.SystemTrayIcon.Trigger) {
                root.show()
                root.raise()
                root.requestActivate()
            }
        }

        menu: Platform.Menu {
            Platform.MenuItem {
                text: qsTr("Show Clareon")
                onTriggered: {
                    root.show()
                    root.raise()
                    root.requestActivate()
                }
            }
            Platform.MenuSeparator {}
            Platform.MenuItem {
                text: qsTr("New Conversation")
                onTriggered: {
                    root.show()
                    root.raise()
                    root.requestActivate()
                    root.openTitlePage()
                }
            }
            Platform.MenuSeparator {}
            Platform.MenuItem {
                text: qsTr("Quit")
                onTriggered: Qt.quit()
            }
        }
    }

    // Override close event to hide instead of quit
    onClosing: function(close) {
        close.accepted = false
        root.hide()
    }

    Kirigami.Action {
        id: newConversationAction
        text: qsTr("New Conversation")
        shortcut: "Ctrl+N"
        icon.name: "message-new"
        onTriggered: {
            openTitlePage()
        }
    }

    Kirigami.Action {
        id: settingsAction
        text: qsTr("Settings")
        shortcut: "Ctrl+,"
        icon.name: "settings-configure"
        onTriggered: {
            openConfiguration()
        }
    }

    Kirigami.Action {
        id: searchConversationsAction
        text: qsTr("Search Conversations")
        shortcut: "Ctrl+F"
        icon.name: "edit-find"
        onTriggered: {
            openSearchPage()
        }
    }

    // Listen for conversation deletion events
    Connections {
        target: ServiceController

        function onConversationDeleted(conversationId) {
            // If the currently open conversation was deleted, navigate to title page
            if (root.currentConversationId === conversationId) {
                root.openTitlePage()
            }
        }
    }

    globalDrawer: Drawer {
        id: drawer

        onConversationSelected: {
            root.openConversation(conversationId)
        }
    }

    pageStack {
        columnView.columnResizeMode: Kirigami.ColumnView.Dynamic
    }

    Component {
        id: titlePage
        TitlePage {}
    }

    Component {
        id: searchResultsPage
        SearchResultsPage {}
    }

    Component {
        id: converstationPage
        ConversationPage {}
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

    function openConversation(conversationId) {
        const page = converstationPage.createObject(null, {
            conversationId: conversationId,
        })

        if (page) {
            pageStack.clear()
            pageStack.replace(page)
            root.currentConversationId = conversationId
            globalDrawer.currentConversationId = conversationId
        } else {
            console.error("Failed to create ")
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
            searchPage.onSelectedConversationIdChanged.connect(function() {
                globalDrawer.currentConversationId = searchPage.selectedConversationId
            })
            pageStack.replace(searchPage)
        } else {
            console.error("Failed to create SearchResultsPage")
        }
    }

    function openTitlePage() {
        const page = titlePage.createObject(null)
        if (page) {
            page.conversationStarted.connect(function(conversationId) {
                openConversation(conversationId)
            })
            pageStack.clear()
            pageStack.replace(page)
            root.currentConversationId = ""
            globalDrawer.currentConversationId = ""
        } else {
            console.error("Failed to create TitlePage")
        }
    }

    Component.onCompleted: {
        // Show the title page on startup
        openTitlePage()
    }
}
