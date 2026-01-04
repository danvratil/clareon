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

    implicitWidth: Kirigami.Units.gridUnit * 20

    property string currentConversationId: ""

    title: qsTr("Clareon")

    actions: [
        Kirigami.Action {
            text: qsTr("New Conversation")
            icon.name: "message-new"
            onTriggered: {
                console.log("Create new conversation")
            }
        },
        Kirigami.Action {
            text: qsTr("Settings")
            icon.name: "settings-configure"
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
        id: conversationDataProvider

        onDataChanged: {
            // Rebuild the list model from data provider
            conversationListModel.clear()
            for (var i = 0; i < conversationDataProvider.count; i++) {
                conversationListModel.append({
                    conversationId: conversationDataProvider.getId(i),
                    title: conversationDataProvider.getTitle(i),
                    updatedAt: conversationDataProvider.getUpdatedAt(i),
                    model: conversationDataProvider.getModel(i),
                    messageCount: 0  // Not available yet
                })
            }
        }
    }

    ListModel {
        id: conversationListModel
    }

    KItemModels.KSortFilterProxyModel {
        id: filteredModel
        sourceModel: conversationListModel
        filterRoleName: "title"
        filterRegularExpression: {
            if (searchField.text === "") return new RegExp()
            return new RegExp("%1".arg(searchField.text), "i")
        }
    }

    // Connect to ServiceController signals
    Connections {
        target: ServiceController

        function onConversationsChanged() {
            conversationDataProvider.refresh()
        }
    }

    function openConversation(conversationId) {
        const chatView = chatViewComponent.createObject(null, {
            conversationId: conversationId
        })

        if (chatView) {
            if (pageStack.depth > 2) {
                pageStack.replace(chatView)
            } else {
                pageStack.push(chatView)
            }

            pageRoot.currentConversationId = conversationId

        } else {
            console.error("Failed to create ChatView")
        }
    }

    Component {
        id: chatViewComponent
        ChatView {}
    }

    ListView {
        id: conversationListView
        anchors.fill: parent
        clip: true

        model: filteredModel

        delegate: ConversationItemDelegate {
            id: delegate
            width: conversationListView.width
            highlighted: delegate.conversationId === pageRoot.currentConversationId
            onClicked: pageRoot.openConversation(delegate.conversationId)
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
}