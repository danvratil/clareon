// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as Controls
import org.kde.kirigami as Kirigami
import org.kde.kirigami.primitives as KP
import cz.dvratil.clareon // for ConfigManager

Kirigami.ApplicationWindow {
    id: root

    property var _config: ConfigManager.getConfig()
    property bool _previousAutoStart: _config.ui.autoStart || false

    title: qsTr("Clareon Settings")
    width: 900
    height: 700
    minimumWidth: 700
    minimumHeight: 500
    modality: Qt.ApplicationModal

    Kirigami.PagePool {
        id: pagePool
    }

    // Save all settings pages that have changes
    function saveSettings() {
        ConfigManager.saveConfig(root._config)

        // Apply autostart configuration if it changed
        if (root._config.ui.autoStart !== root._previousAutoStart) {
            ServiceController.setAutoStart(root._config.ui.autoStart)
            root._previousAutoStart = root._config.ui.autoStart
        }
    }

    globalDrawer: Kirigami.GlobalDrawer {
        interactiveResizeEnabled: true
        modal: false
        padding: 0

        actions: [
            Kirigami.PagePoolAction {
                id: generalAction
                text: qsTr("General")
                icon.name: "configure"
                page: Qt.resolvedUrl("GeneralSettings.qml")
                pagePool: pagePool
                initialProperties: {
                    return { config: root._config }
                }
            },
            Kirigami.PagePoolAction {
                text: qsTr("Accounts & Models")
                icon.name: "user-identity"
                page: Qt.resolvedUrl("AccountsModelsSettings.qml")
                pagePool: pagePool
                initialProperties: {
                    return { config: root._config }
                }
            },
            Kirigami.PagePoolAction {
                text: qsTr("Notifications")
                icon.name: "preferences-desktop-notification"
                page: Qt.resolvedUrl("NotificationsSettings.qml")
                pagePool: pagePool
                initialProperties: {
                    return { config: root._config }
                }
            },
            Kirigami.PagePoolAction {
                text: qsTr("Tools & MCP")
                icon.name: "tools"
                page: Qt.resolvedUrl("ToolsMCPSettings.qml")
                pagePool: pagePool
                initialProperties: {
                    return { config: root._config }
                }
            },
            Kirigami.PagePoolAction {
                text: qsTr("Advanced")
                icon.name: "preferences-system"
                page: Qt.resolvedUrl("AdvancedSettings.qml")
                pagePool: pagePool
                initialProperties: {
                    return { config: root._config }
                }
            }
        ]
    }

    Component.onCompleted: {
        // Load the initial page
        generalAction.trigger()
    }

    footer: Item {
        anchors {
            left: parent.left
            right: parent.right
        }
        height: buttonBox.implicitHeight + Kirigami.Units.largeSpacing

        Kirigami.Separator {
            id: separator
            anchors {
                top: parent.top
                left: parent.left
                right: parent.right
            }

            Kirigami.Theme.inherit: false
            Kirigami.Theme.colorSet: Kirigami.Theme.Header
        }

        Controls.DialogButtonBox {
            id: buttonBox

            anchors {
                top: separator.bottom
                right: parent.right
                left: parent.left
                margins: Kirigami.Units.smallSpacing
            }

            alignment: Qt.AlignRight
            standardButtons: Controls.DialogButtonBox.Ok | Controls.DialogButtonBox.Cancel | Controls.DialogButtonBox.Apply

            onAccepted: {
                root.saveSettings()
                root.close()
            }

            onRejected: {
                root.close()
            }

            onApplied: {
                root.saveSettings()
            }
        }
    }
}
