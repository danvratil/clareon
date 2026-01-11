// SPDX-FileContributor: Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as Controls
import org.kde.kirigami as Kirigami
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
                pageStack.pop()
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

        // Split view for results and conversation
        Controls.SplitView {
            Layout.fillWidth: true
            Layout.fillHeight: true
            orientation: Qt.Horizontal

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
                            chatViewLoader.openConversationAndScrollToMessage(
                                delegate.conversationId,
                                delegate.messageId
                            )
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

            // Right pane: Conversation view
            Rectangle {
                Controls.SplitView.fillWidth: true
                color: Kirigami.Theme.backgroundColor

                Loader {
                    id: chatViewLoader
                    anchors.fill: parent
                    sourceComponent: chatViewComponent

                    function openConversationAndScrollToMessage(conversationId, messageId) {
                        if (item) {
                            item.conversationId = conversationId
                            item.highlightMessageId = messageId
                            // Load messages and scroll to message
                            ServiceController.loadMessages(conversationId)
                            // Scrolling will happen in ChatView after messages are loaded
                        }
                    }
                }

                Component {
                    id: chatViewComponent

                    ChatView {
                        id: chatView
                        // Will be set by loader
                    }
                }

                // Empty state - no conversation selected
                Kirigami.PlaceholderMessage {
                    anchors.centerIn: parent
                    width: parent.width - (Kirigami.Units.largeSpacing * 4)
                    visible: pageRoot.selectedConversationId === ""
                    icon.name: "view-conversation-balloon"
                    text: qsTr("No conversation selected")
                    explanation: qsTr("Click a search result to view the conversation")
                }
            }
        }
    }
}
