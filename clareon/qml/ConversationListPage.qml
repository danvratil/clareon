// SPDX-FileContributor: Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as Controls
import org.kde.kirigami as Kirigami
import org.kde.kitemmodels as KItemModels
import cz.dvratil.clareon 1.0

Kirigami.ScrollablePage {
    id: pageRoot

    required property variant appController

    implicitWidth: Kirigami.Units.gridUnit * 20

    title: qsTr("Clareon")

    actions: [
        Kirigami.Action {
            text: qsTr("Settings")
            icon.name: "settings-system"
            onTriggered: {
                console.log("Open settings")
            }
        }
    ]

    header: Controls.ToolBar {
        id: toolbar
        RowLayout {
            anchors.fill: parent
            Kirigami.SearchField {
                id: searchField

                Layout.alignment: Qt.AlignHCenter
                Layout.fillWidth: true
                Layout.maximumWidth: Kirigami.Units.gridUnit * 30
            }
        }
    }

    background: Rectangle {
        anchors.fill: parent
        Kirigami.Theme.colorSet: Kirigami.Theme.View
        color: Kirigami.Theme.backgroundColor
    }

    ConversationListModel {
        id: conversationModel
    }

    KItemModels.KSortFilterProxyModel {
        id: filteredModel
        sourceModel: conversationModel
        filterRoleName: "title"
        filterRegularExpression: {
            if (searchField.text === "") return new RegExp()
            return new RegExp("%1".arg(searchField.text), "i")
        }
    }

    function openConversation(conversationId) {
        const uri = "qrc:/qt/qml/cz/dvratil/clareon/qml/ChatView.qml"
        if (pageStack.depth > 2) {
            pageStack.replace(uri, {
                appController: pageRoot.appController,
                conversationId: conversationId,
            })
        } else {
            pageStack.push(uri, {
                appController: pageRoot.appController,
                conversationId: conversationId,
            })
        }
    }

    ListView {
        id: conversationListView
        anchors.fill: parent
        clip: true

        model: filteredModel

        delegate: ConversationItemDelegate {
            appController: pageRoot.appController

            onClicked: pageRoot.openConversation(model.conversationId)
        }

        // Empty state
        Kirigami.PlaceholderMessage {
            anchors.centerIn: parent
            width: parent.width - (Kirigami.Units.largeSpacing * 4)
            visible: conversationListView.count === 0
            icon.name: "view-conversation-balloon"
            text: "No conversations yet"
            explanation: "Start a conversation by clicking the + button"
        }
    }

    Component.onCompleted: {
        conversationModel.refreshConversations()
    }
}