// SPDX-FileContributor: Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as Controls
import org.kde.kirigami as Kirigami

Kirigami.ApplicationWindow {
    id: root

    title: qsTr("Clareon Settings")
    width: 900
    height: 700
    minimumWidth: 700
    minimumHeight: 500
    modality: Qt.ApplicationModal

    // Create a two-column layout with side navigation
    RowLayout {
        anchors.fill: parent
        spacing: 0

        ColumnLayout {
            Layout.fillHeight: true
            Layout.fillWidth: true
            Layout.preferredWidth: 250
            spacing: 0

            // Navigation list
            ListView {
                id: categoryList
                Layout.fillWidth: true
                Layout.fillHeight: true

                clip: true
                currentIndex: 0

                model: ListModel {
                    ListElement {
                        name: qsTr("General")
                        icon: "configure"
                        page: "GeneralSettings.qml"
                    }
                    ListElement {
                        name: qsTr("Accounts & Models")
                        icon: "user-identity"
                        page: "AccountsModelsSettings.qml"
                    }
                    ListElement {
                        name: qsTr("Notifications")
                        icon: "preferences-desktop-notification"
                        page: "NotificationsSettings.qml"
                    }
                    ListElement {
                        name: qsTr("Tools & MCP")
                        icon: "tools"
                        page: "ToolsMCPSettings.qml"
                    }
                    ListElement {
                        name: qsTr("Advanced")
                        icon: "preferences-system"
                        page: "AdvancedSettings.qml"
                    }
                }

                delegate: Controls.ItemDelegate {
                    width: ListView.view.width
                    highlighted: ListView.isCurrentItem

                    contentItem: RowLayout {
                        spacing: Kirigami.Units.largeSpacing

                        Kirigami.Icon {
                            source: model.icon
                            width: Kirigami.Units.iconSizes.smallMedium
                            height: Kirigami.Units.iconSizes.smallMedium
                        }

                        Controls.Label {
                            text: model.name
                            Layout.fillWidth: true
                        }
                    }

                    onClicked: {
                        categoryList.currentIndex = index
                        settingsLoader.loadPage(model.page)
                    }
                }
            }
        }

        // Main content area
        Rectangle {
            Layout.fillWidth: true
            Layout.fillHeight: true

            Loader {
                id: settingsLoader
                anchors.fill: parent
                anchors.margins: Kirigami.Units.largeSpacing

                // Load the first page by default
                Component.onCompleted: {
                    loadPage("GeneralSettings.qml")
                }

                function loadPage(page) {
                    const path = "qrc:/qt/qml/cz/dvratil/clareon/qml/config/" + page
                    setSource(path)
                }
            }
        }
    }

    // Keyboard shortcuts
    Shortcut {
        sequence: "Ctrl+W"
        onActivated: root.close()
    }

    Shortcut {
        sequence: "Esc"
        onActivated: root.close()
    }
}
