// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as Controls
import org.kde.kirigami as Kirigami
import org.kde.kitemmodels as KItemModels
import cc.clareon.core 1.0
import cz.dvratil.clareon 1.0


Kirigami.GlobalDrawer {
    id: drawer

    signal conversationSelected(string conversationId)

    property string currentConversationId: ""

    interactiveResizeEnabled: true
    modal: false
    padding: 0
    minimumSize: Kirigami.Units.gridUnit * 20

    header: Kirigami.AbstractApplicationHeader {
        contentItem: Kirigami.SearchField {
            id: searchField
            anchors {
                left: parent.left
                right: parent.right
            }
        }
    }

    topContent: [
        GlobalActionItem {
            action: newConversationAction
        },
        GlobalActionItem {
            action: settingsAction
        },
        GlobalActionItem {
            action: searchConversationsAction
        },
        Kirigami.Separator { 
            Layout.fillWidth: true
            Layout.topMargin: Kirigami.Units.smallSpacing
            Layout.leftMargin: Kirigami.Units.largeSpacing
            Layout.rightMargin: Kirigami.Units.largeSpacing
        }
    ]

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

        function onConversationCreated(id) {
            drawer.conversationSelected(id)
        }
    }

    ListView {
        id: conversationListView
        clip: true

        Layout.fillWidth: true
        Layout.fillHeight: true

        model: filteredModel

        delegate: ConversationItemDelegate {
            id: delegate
            width: conversationListView.width
            highlighted: delegate.conversationId === drawer.currentConversationId
            onClicked: drawer.conversationSelected(delegate.conversationId)
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