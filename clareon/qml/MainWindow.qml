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
    property var config: ConfigManager.getConfig()

    Connections {
        target: ConfigManager
        function onConfigChanged() {
            root.config = ConfigManager.getConfig()
        }
    }

    // Cache of ConversationPage instances keyed by conversationId. Keeping a
    // page around across navigation preserves its MessageListModel — in
    // particular, an in-flight streaming placeholder — so quickly switching
    // between active conversations doesn't tear down state mid-stream.
    property var conversationPages: ({})

    // Off-screen Item that owns cached ConversationPage instances when they
    // are not currently displayed in pageStack. Parenting cached pages here
    // (instead of leaving them owned by pageStack) keeps them alive across
    // pageStack.clear()/replace() calls.
    //
    // Sized to match pageStack so that cached pages layout their (potentially
    // large) Repeater-based message lists against the correct dimensions and
    // don't have to relayout when reparented back into pageStack on switch.
    Item {
        id: pageCacheHolder
        visible: false
        width: pageStack.width
        height: pageStack.height
    }

    // Override close event to hide instead of quit (if minimize to tray is enabled)
    onClosing: function(close) {
        if (config.ui.minimizeToTray) {
            close.accepted = false
            root.hide()
        }
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

    // Track if we're waiting for a quick conversation to be created
    property string pendingQuickConversationId: ""

    // Listen for conversation deletion events
    Connections {
        target: ServiceController

        function onConversationDeleted(conversationId) {
            const wasCurrent = root.currentConversationId === conversationId
            const cached = root.conversationPages[conversationId]

            // Navigate away first so the page being destroyed isn't the
            // currently-displayed one in pageStack.
            if (wasCurrent) {
                root.openTitlePage()
            }

            if (cached) {
                delete root.conversationPages[conversationId]
                cached.destroy()
            }
        }

        function onConversationCreated(conversationId) {
            // If we're waiting for a quick conversation, open it
            if (root.pendingQuickConversationId !== "") {
                root.pendingQuickConversationId = ""
                root.openConversation(conversationId)
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
        source: Qt.resolvedUrl("config/ConfigurationPage.qml")

        onLoaded: {
            item.show()
            item.raise()
            item.requestActivate()
        }
    }

    // If the page currently shown in pageStack is one of our cached
    // ConversationPages, reparent it back to pageCacheHolder so the upcoming
    // pageStack mutation can't destroy it.
    function _detachActiveCachedPage() {
        if (pageStack.depth === 0) {
            return
        }
        const top = pageStack.currentItem
        if (top && top.conversationId !== undefined
                && root.conversationPages[top.conversationId] === top) {
            top.parent = pageCacheHolder
        }
    }

    function openConversation(conversationId) {
        let page = root.conversationPages[conversationId]
        if (!page) {
            page = converstationPage.createObject(pageCacheHolder, {
                conversationId: conversationId,
            })
            if (!page) {
                console.error("Failed to create ConversationPage")
                return
            }
            root.conversationPages[conversationId] = page
        }

        if (pageStack.depth > 0 && pageStack.currentItem === page) {
            return
        }

        _detachActiveCachedPage()
        pageStack.clear()
        pageStack.replace(page)
        root.currentConversationId = conversationId
        globalDrawer.currentConversationId = conversationId
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
            _detachActiveCachedPage()
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
            _detachActiveCachedPage()
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
