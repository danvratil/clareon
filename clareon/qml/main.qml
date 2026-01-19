// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import Qt.labs.platform as Platform
import cz.dvratil.clareon 1.0

Item {
    id: app

    property var config: ConfigManager.getConfig()

    MainWindow {
        id: mainWindow
        visible: false

        function open() {
            mainWindow.show()
            mainWindow.raise()
            mainWindow.requestActivate()
        }
    }

    QuickInputPopup {
        id: quickInput
        visible: false

        onPromptSubmitted: function(prompt) {
            // Mark that we're waiting for the conversation to be created
            // Create conversation with the prompt
            ServiceController.newQuickConversation(prompt)
            // Show and activate main window
            mainWindow.open()
            mainWindow.pendingQuickConversationId = "pending"
        }

        function open() {
            quickInput.show()
            quickInput.raise()
            quickInput.requestActivate()
        }
    }

    Component.onCompleted: {
        ServiceController.onMainWindowRequested.connect(function() {
            mainWindow.open()
        })

        ServiceController.onQuickInputRequested.connect(function() {
            quickInput.open()
        })

        // Show window on startup if not configured to start minimized
        if (!config.ui.startMinimized) {
            mainWindow.open()
        }
    }

    // System tray icon - only visible if minimize to tray is enabled
    Platform.SystemTrayIcon {
        id: systemTray
        visible: config.ui.minimizeToTray || config.ui.startMinimized
        icon.name: ":/clareon-256.png"
        tooltip: "Clareon Assistant"

        onActivated: function(reason) {
            mainWindow.open()
        }

        menu: Platform.Menu {
            Platform.MenuItem {
                text: qsTr("&Show Clareon")
                icon.name: "window-restore"
                onTriggered: mainWindow.open()
            }
            Platform.MenuItem {
                text: qsTr("Quick &Input...")
                // FIXME: This needs to be synced with the shortcut as currently configured
                // by user in KGlobalAccel/XDG Global Shortcuts
                shortcut: "Meta+C"
                icon.name: "quickopen"
                onTriggered: quickInput.open()
            }
            Platform.MenuSeparator {}
            Platform.MenuItem {
                text: qsTr("&New Conversation")
                icon.name: "message-new"
                onTriggered: {
                    mainWindow.open()
                    mainWindow.openTitlePage()
                }
            }
            Platform.MenuSeparator {}
            Platform.MenuItem {
                text: qsTr("&Quit")
                icon.name: "application-exit"
                onTriggered: Qt.quit()
            }
        }
    }
}