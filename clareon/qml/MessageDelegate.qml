// SPDX-FileContributor: Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQml
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import cz.dvratil.clareon 1.0

Item {
    id: root

    // Conversation context
    required property string conversationId

    // Properties map to MessageListModel roles
    required property int messageId
    required property string role
    required property string textContent
    required property int createdAt
    required property string messageState

    // Error-related properties
    required property bool isError
    required property string errorMessage
    required property string errorDetails
    required property string errorCategory
    required property bool isRetryable
    required property int retryAfterSecs
    required property string partialContent
    // Highlighting property (set externally)
    property bool highlighted: false

    height: messageLayout.implicitHeight + Kirigami.Units.largeSpacing * 2

    // Highlight animation timer
    Timer {
        id: highlightTimer
        interval: 3000
        onTriggered: root.highlighted = false
    }

    // Watch for highlight changes and start timer
    onHighlightedChanged: {
        if (highlighted) {
            highlightTimer.restart()
        }
    }

    RowLayout {
        id: messageLayout
        anchors.fill: parent
        anchors.margins: Kirigami.Units.largeSpacing
        spacing: Kirigami.Units.largeSpacing

        Controls.BusyIndicator {
            visible: root.messageState == "thinking" && root.role == "assistant" && !root.isError
            running: root.messageState == "thinking"
        }

        Component {
            id: errorMessageComponent

            // Error message display
            Kirigami.InlineMessage {
                id: errorMessage
                Layout.fillWidth: true
                Layout.maximumWidth: root.width * 0.9

                visible: root.isError

                property bool detailsExpanded: false

                type: {
                    switch (root.errorCategory) {
                        case "network":
                        case "ratelimit":
                        case "servererror":
                            return Kirigami.MessageType.Warning
                        case "authentication":
                        case "clienterror":
                        case "contextlimit":
                            return Kirigami.MessageType.Error
                        default:
                            return Kirigami.MessageType.Error
                    }
                }

                text: if (errorMessage.detailsExpanded && root.partialContent && root.partialContent.length > 0) {
                        return root.errorMessage + "\n\n" + qsTr("Partial content received:\n") + root.partialContent
                    } else if (errorMessage.detailsExpanded && root.errorDetails && root.errorDetails.length > 0) {
                        return root.errorMessage + "\n\n" + qsTr("Details:\n") + root.errorDetails
                    } else {
                        return root.errorMessage
                    }
                showCloseButton: false

                actions: [
                    Kirigami.Action {
                        text: root.retryAfterSecs > 0
                            ? qsTr("Retry in %1s").arg(root.retryAfterSecs)
                            : qsTr("Retry")
                        icon.name: "view-refresh"
                        visible: root.isRetryable
                        enabled: root.retryAfterSecs === 0
                        onTriggered: {
                            ServiceController.retryLastMessage(root.conversationId)
                        }
                    },
                    Kirigami.Action {
                        text: errorMessage.detailsExpanded ? qsTr("Hide Details") : qsTr("Show Details")
                        visible: root.errorDetails && root.errorDetails.length > 0
                        icon.name: "documentinfo"
                        onTriggered: {
                            errorMessage.detailsExpanded = !errorMessage.detailsExpanded
                        }
                    }
                ]

                Timer {
                    interval: 1000
                    repeat: true
                    running: root.isRetryable && root.retryAfterSecs > 0
                    onTriggered: {
                        if (root.retryAfterSecs > 0) {
                            root.retryAfterSecs -= 1
                        }
                        if (root.retryAfterSecs === 0) {
                            stop()
                        }
                    }
                }
            }
        }

        // Load error message if present - we don't want to instantiate the InlineMessage for every single message,
        // since it's only used in (hopefully) rare error cases.
        Loader {
            Layout.fillWidth: true
            id: errorLoader
            active: root.isError
            sourceComponent: errorMessageComponent
        }

        // Message bubble
        Kirigami.Card {
            Layout.fillWidth: root.role === "assistant"
            Layout.alignment: root.role === "user" ? Qt.AlignRight : Qt.AlignLeft
            /// Assistant messages can use full width, user messages are limited to 75%, right-aligned
            Layout.maximumWidth: root.role === "user" ? root.width * 0.75 : root.width

            visible: root.messageState !== "thinking" && !root.isError

            background: Rectangle {
                color: root.role === "user"
                    ? Kirigami.Theme.highlightColor
                    : Kirigami.Theme.backgroundColor
                border.color: root.highlighted
                    ? Kirigami.Theme.focusColor
                    : (root.role === "user"
                        ? Qt.darker(Kirigami.Theme.highlightColor, 1.1)
                        : Kirigami.Theme.alternateBackgroundColor)
                border.width: root.highlighted ? 3 : 1
                radius: Kirigami.Units.cornerRadius

                // Highlight animation
                Behavior on border.color {
                    ColorAnimation { duration: 200 }
                }
                Behavior on border.width {
                    NumberAnimation { duration: 200 }
                }
            }

            contentItem: TextEdit {
                Layout.fillWidth: true
                readOnly: true
                textFormat: TextEdit.MarkdownText
                text: root.textContent
                wrapMode: Text.Wrap
                color: root.role === "user"
                    ? Kirigami.Theme.highlightedTextColor
                    : Kirigami.Theme.textColor
            }

            footer: RowLayout {
                Layout.fillWidth: true
                spacing: Kirigami.Units.smallSpacing

                Controls.Label {
                    text: root.role === "user" ? "You" : "Claude"
                    opacity: 0.7
                    font.pointSize: Kirigami.Theme.smallFont.pointSize
                    font.bold: true
                    color: root.role === "user"
                        ? Kirigami.Theme.highlightedTextColor
                        : Kirigami.Theme.textColor
                }

                Controls.Label {
                    text: "•"
                    opacity: 0.5
                    font.pointSize: Kirigami.Theme.smallFont.pointSize
                    color: root.role === "user"
                        ? Kirigami.Theme.highlightedTextColor
                        : Kirigami.Theme.textColor
                }

                Controls.Label {
                    text: Qt.formatDateTime(new Date(root.createdAt * 1000), "h:mm AP")
                    opacity: 0.7
                    font.pointSize: Kirigami.Theme.smallFont.pointSize
                    color: root.role === "user"
                        ? Kirigami.Theme.highlightedTextColor
                        : Kirigami.Theme.textColor
                }

                Item {
                    Layout.fillWidth: true
                }
            }
        }
    }
}
