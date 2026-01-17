// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as Controls
import org.kde.kirigami as Kirigami
import cz.dvratil.clareon 1.0
import cc.clareon.core 1.0

Kirigami.Page {
    id: pageRoot

    title: qsTr("Search")

    property string selectedConversationId: ""
    property int selectedMessageId: -1

    actions: [
        Kirigami.Action {
            text: qsTr("Close Search")
            icon.name: "window-close"
            onTriggered: {
                // pop just the search page, keep the conversation page (if any) opened
                pageStack.removePage(pageRoot)
            }
        }
    ]

    // Search debounce timer
    Timer {
        id: searchDebounceTimer
        interval: 300
        onTriggered: {
            if (searchField.text.length >= 2) {
                ServiceController.search(searchField.text)
            } else {
                searchResultModel.clear()
            }
        }
    }

    SearchResultModel {
        id: searchResultModel

        onDataChanged: {
            // Rebuild the list model from data provider
            searchResultListModel.clear()
            for (var i = 0; i < searchResultModel.count; i++) {
                searchResultListModel.append({
                    conversationId: searchResultModel.getConversationId(i),
                    conversationTitle: searchResultModel.getConversationTitle(i),
                    messageId: searchResultModel.getMessageId(i),
                    role: searchResultModel.getRole(i),
                    snippet: searchResultModel.getSnippet(i),
                    createdAt: searchResultModel.getCreatedAt(i)
                })
            }
        }
    }

    ListModel {
        id: searchResultListModel
    }

    // Connect to ServiceController signals
    Connections {
        target: ServiceController

        function onSearchResultsReady() {
            searchResultModel.refresh()
        }
    }

    Component {
        id: conversationPageLoader
        ConversationPage {}
    }


    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        // Search field header
        Controls.ToolBar {
            Layout.fillWidth: true

            RowLayout {
                anchors.fill: parent

                Kirigami.SearchField {
                    id: searchField
                    Layout.fillWidth: true
                    Layout.margins: Kirigami.Units.smallSpacing

                    placeholderText: qsTr("Search conversations...")
                    autoAccept: false

                    onTextChanged: {
                        searchDebounceTimer.restart()
                    }

                    Component.onCompleted: {
                        forceActiveFocus()
                    }
                }
            }
        }

        // Left pane: Search results
        Kirigami.ScrollablePage {
            Controls.SplitView.preferredWidth: parent.width * 0.3
            Controls.SplitView.minimumWidth: Kirigami.Units.gridUnit * 15
            Controls.SplitView.maximumWidth: parent.width * 0.5

            title: qsTr("Results")
            padding: 0

            background: Rectangle {
                Kirigami.Theme.colorSet: Kirigami.Theme.View
                color: Kirigami.Theme.backgroundColor
            }

            ListView {
                id: searchResultsListView
                anchors.fill: parent
                clip: true

                model: searchResultListModel

                delegate: SearchResultDelegate {
                    id: delegate
                    width: searchResultsListView.width
                    highlighted: delegate.conversationId === pageRoot.selectedConversationId &&
                                delegate.messageId === pageRoot.selectedMessageId

                    onClicked: {
                        pageRoot.selectedConversationId = delegate.conversationId
                        pageRoot.selectedMessageId = delegate.messageId

                        pageStack.push(conversationPageLoader, {
                            conversationId: delegate.conversationId,
                            highlightMessageId: delegate.messageId
                        })
                    }
                }

                // Empty state - no results yet
                Kirigami.PlaceholderMessage {
                    anchors.centerIn: parent
                    width: parent.width - (Kirigami.Units.largeSpacing * 4)
                    visible: searchResultsListView.count === 0 && searchField.text.length === 0
                    icon.name: "search"
                    text: qsTr("Enter a search query")
                    explanation: qsTr("Type at least 2 characters to search")
                }

                // Empty state - no results found
                Kirigami.PlaceholderMessage {
                    anchors.centerIn: parent
                    width: parent.width - (Kirigami.Units.largeSpacing * 4)
                    visible: searchResultsListView.count === 0 && searchField.text.length >= 2
                    icon.name: "edit-none"
                    text: qsTr("No results found")
                    explanation: qsTr("Try a different search query")
                }
            }
        }
    }
}
