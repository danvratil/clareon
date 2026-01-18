// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import Qt.labs.platform as Platform
import cz.dvratil.clareon 1.0

Item {
    id: app

    MainWindow {
        id: mainWindow
        visible: false

        function open() {
            mainWindow.show()
            mainWindow.raise()
            mainWindow.requestActivate()
        }
    }

    Component.onCompleted: {
        ServiceController.onMainWindowRequested.connect(function() {
            mainWindow.open()
        })
    }

    // System tray icon
    Platform.SystemTrayIcon {
        id: systemTray
        visible: true
        icon.name: ":/clareon-256.png"
        tooltip: "Clareon Assistant"

        onActivated: function(reason) {
            mainWindow.open()
        }

        menu: Platform.Menu {
            Platform.MenuItem {
                text: qsTr("Show Clareon")
                onTriggered: mainWindow.open()
            }
            Platform.MenuSeparator {}
            Platform.MenuItem {
                text: qsTr("New Conversation")
                onTriggered: {
                    mainWindow.open()
                    mainWindow.openTitlePage()
                }
            }
            Platform.MenuSeparator {}
            Platform.MenuItem {
                text: qsTr("Quit")
                onTriggered: Qt.quit()
            }
        }
    }
}