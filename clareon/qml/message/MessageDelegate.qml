// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
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

    // ── Required properties (mapped from MessageListModel roles) ─────────────
    required property string conversationId
    required property int    messageId
    required property string role
    required property string textContent
    required property int    createdAt
    required property string messageState
    required property string contentBlocks          // JSON from ContentBlocks role
    required property bool   isGroupedWithPrevious  // from IsGroupedWithPrevious role

    // Error-related properties
    required property bool   isError
    required property string errorMessage
    required property string errorDetails
    required property string errorCategory
    required property bool   isRetryable
    required property int    retryAfterSecs
    required property string partialContent

    // Highlighting (set externally for search results)
    property bool highlighted: false

    // ── Layout ───────────────────────────────────────────────────────────────
    readonly property int avatarSize: Kirigami.Units.iconSizes.medium
    readonly property int cornerRadius: Kirigami.Units.cornerRadius

    height: outerRow.implicitHeight + Kirigami.Units.largeSpacing * 2

    // Highlight auto-dismiss
    Timer {
        id: highlightTimer
        interval: 3000
        onTriggered: root.highlighted = false
    }
    onHighlightedChanged: { if (highlighted) highlightTimer.restart() }

    // ── Outer row ─────────────────────────────────────────────────────────────
    RowLayout {
        id: outerRow
        anchors {
            left: parent.left
            right: parent.right
            top: parent.top
            margins: Kirigami.Units.largeSpacing
        }
        spacing: Kirigami.Units.largeSpacing

        // ── Left spacer: pushes user bubbles to the right ────────────────────
        // Use Layout.fillWidth (not visible) to control whether space is consumed.
        // Invisible items still participate in layout in Qt Quick Layouts.
        Item {
            Layout.fillWidth: root.role === "user" && !root.isError
        }

        // ── Avatar column: shown for assistant messages only ─────────────────
        // Layout.preferredWidth/maximumWidth explicitly control space allocation
        // so the column takes no space for user messages regardless of visibility.
        Item {
            id: avatarColumn
            readonly property bool shown: root.role === "assistant" && root.messageState !== "thinking" && !root.isError
            visible: shown
            Layout.preferredWidth: shown ? root.avatarSize : 0
            Layout.maximumWidth:   shown ? root.avatarSize : 0
            Layout.alignment: Qt.AlignBottom
            implicitHeight: bubble.implicitHeight

            Rectangle {
                id: avatar
                anchors.bottom: parent.bottom
                width: root.avatarSize
                height: root.avatarSize
                radius: width / 2
                color: Kirigami.Theme.highlightColor
                // Hide (but keep the space) for grouped messages
                opacity: root.isGroupedWithPrevious ? 0 : 1

                readonly property bool streaming: root.messageState === "streaming"

                // Expanding halo ring, shown only while streaming
                Rectangle {
                    id: avatarHalo
                    anchors.centerIn: parent
                    width: parent.width
                    height: parent.height
                    radius: width / 2
                    color: "transparent"
                    border.color: Kirigami.Theme.highlightColor
                    border.width: 2
                    visible: avatar.streaming
                    opacity: 0

                    ParallelAnimation {
                        running: avatar.streaming
                        loops: Animation.Infinite
                        NumberAnimation {
                            target: avatarHalo; property: "scale"
                            from: 1.0; to: 1.7; duration: 1200
                            easing.type: Easing.OutQuad
                        }
                        SequentialAnimation {
                            NumberAnimation {
                                target: avatarHalo; property: "opacity"
                                from: 0.0; to: 0.6; duration: 200
                            }
                            NumberAnimation {
                                target: avatarHalo; property: "opacity"
                                to: 0.0; duration: 1000
                                easing.type: Easing.InQuad
                            }
                        }
                    }
                }

                Kirigami.Icon {
                    anchors.centerIn: parent
                    width: Math.round(parent.width * 0.6)
                    height: width
                    source: "computer-symbolic"
                    color: Kirigami.Theme.highlightedTextColor
                    isMask: true
                }
            }
        }

        // ── Thinking indicator (assistant, thinking state) ───────────────────
        ThinkingIndicator {
            readonly property bool shown: root.role === "assistant" && root.messageState === "thinking" && !root.isError
            visible: shown
            Layout.preferredWidth: shown ? implicitWidth : 0
            Layout.maximumWidth:   shown ? implicitWidth : 0
        }

        // ── Error message ────────────────────────────────────────────────────
        Loader {
            id: errorLoader
            visible: root.isError
            Layout.fillWidth: root.isError
            Layout.preferredWidth: root.isError ? -1 : 0
            Layout.maximumWidth: root.isError ? -1 : 0
            active: root.isError
            sourceComponent: errorMessageComponent
        }

        // ── Message bubble ───────────────────────────────────────────────────
        Kirigami.ShadowedRectangle {
            id: bubble

            readonly property bool shown: root.messageState !== "thinking" && !root.isError
            visible: shown

            // Assistant: fills all remaining space after the avatar.
            // User: fixed preferred/maximum width; spacer (above) fills the rest.
            Layout.fillWidth: shown && root.role === "assistant"
            Layout.preferredWidth: shown
                ? (root.role === "user" ? root.width * 0.65 : -1)
                : 0
            Layout.maximumWidth: shown
                ? (root.role === "user"
                    ? root.width * 0.65
                    : root.width - root.avatarSize - Kirigami.Units.largeSpacing * 3)
                : 0

            color: root.role === "user"
                ? Kirigami.Theme.highlightColor
                : Kirigami.Theme.backgroundColor

            shadow {
                size: Kirigami.Units.smallSpacing
                color: Qt.rgba(
                    Kirigami.Theme.textColor.r,
                    Kirigami.Theme.textColor.g,
                    Kirigami.Theme.textColor.b,
                    0.10)
            }

            corners {
                topLeftRadius:     root.cornerRadius
                topRightRadius:    root.role === "user" ? 0 : root.cornerRadius
                bottomRightRadius: root.cornerRadius
                bottomLeftRadius:  root.role === "user" ? root.cornerRadius : 0
            }

            // Highlight border (search result flash)
            border {
                color: root.highlighted ? Kirigami.Theme.focusColor : "transparent"
                width: root.highlighted ? 2 : 0
            }
            Behavior on border.color { ColorAnimation { duration: 200 } }

            // ── Bubble interior ──────────────────────────────────────────────
            ColumnLayout {
                id: bubbleContent
                anchors {
                    left: parent.left
                    right: parent.right
                    top: parent.top
                    margins: Kirigami.Units.largeSpacing
                }
                spacing: Kirigami.Units.smallSpacing

                // Sender label (hidden for grouped assistant messages; never shown for user)
                Controls.Label {
                    visible: root.role === "assistant" && !root.isGroupedWithPrevious
                    text: "Assistant"
                    font.pointSize: Kirigami.Theme.smallFont.pointSize
                    font.capitalization: Font.SmallCaps
                    font.bold: true
                    color: Kirigami.Theme.highlightColor
                }

                // Content blocks
                MessageContentView {
                    Layout.fillWidth: true
                    contentBlocksJson: root.contentBlocks
                    messageRole: root.role
                }

                // ── Footer ───────────────────────────────────────────────────
                RowLayout {
                    Layout.fillWidth: true
                    spacing: Kirigami.Units.smallSpacing

                    Controls.Label {
                        text: Qt.formatTime(new Date(root.createdAt * 1000), Qt.locale().timeFormat(Locale.ShortFormat))
                        opacity: 0.6
                        font.pointSize: Kirigami.Theme.smallFont.pointSize
                        color: root.role === "user"
                            ? Kirigami.Theme.highlightedTextColor
                            : Kirigami.Theme.disabledTextColor
                    }

                    Item { Layout.fillWidth: true }

                    // Copy button (appears on hover)
                    Controls.ToolButton {
                        id: copyBtn
                        property bool copied: false
                        visible: bubbleHover.containsMouse || copied
                        icon.name: copied ? "dialog-ok" : "edit-copy"
                        flat: true
                        opacity: 0.7

                        onClicked: {
                            copyHelper.text = root.textContent
                            copyHelper.selectAll()
                            copyHelper.copy()
                            copied = true
                            copyBtnTimer.restart()
                        }
                        Timer {
                            id: copyBtnTimer
                            interval: 2000
                            onTriggered: copyBtn.copied = false
                        }
                    }
                }
            }

            implicitHeight: bubbleContent.implicitHeight + Kirigami.Units.largeSpacing * 2

            // Hover detection for copy button visibility
            HoverHandler { id: bubbleHover }
        }
    }

    // Hidden TextEdit for message-level copy
    TextEdit {
        id: copyHelper
        visible: false
    }

    // ── Error component ───────────────────────────────────────────────────────
    Component {
        id: errorMessageComponent

        Kirigami.InlineMessage {
            id: errorMsg
            // Kirigami.InlineMessage defaults to visible: false (it's designed
            // to be shown/hidden on demand). We want it shown unconditionally
            // when the delegate is instantiated for an error row.
            visible: true
            width: parent.width
            property bool detailsExpanded: false
            property int retryCountdown: 0
            Component.onCompleted: retryCountdown = root.retryAfterSecs

            type: {
                switch (root.errorCategory) {
                case "network":
                case "ratelimit":
                case "servererror":
                    return Kirigami.MessageType.Warning
                default:
                    return Kirigami.MessageType.Error
                }
            }

            text: {
                if (errorMsg.detailsExpanded && root.partialContent && root.partialContent.length > 0)
                    return root.errorMessage + "\n\n" + qsTr("Partial content received:\n") + root.partialContent
                if (errorMsg.detailsExpanded && root.errorDetails && root.errorDetails.length > 0)
                    return root.errorMessage + "\n\n" + qsTr("Details:\n") + root.errorDetails
                return root.errorMessage
            }

            showCloseButton: false

            actions: [
                Kirigami.Action {
                    text: errorMsg.retryCountdown > 0
                        ? qsTr("Retry in %1s").arg(errorMsg.retryCountdown)
                        : qsTr("Retry")
                    icon.name: "view-refresh"
                    visible: root.isRetryable
                    enabled: errorMsg.retryCountdown === 0
                    onTriggered: ServiceController.retryLastMessage(root.conversationId)
                },
                Kirigami.Action {
                    text: errorMsg.detailsExpanded ? qsTr("Hide Details") : qsTr("Show Details")
                    visible: root.errorDetails && root.errorDetails.length > 0
                    icon.name: "documentinfo"
                    onTriggered: errorMsg.detailsExpanded = !errorMsg.detailsExpanded
                }
            ]

            Timer {
                interval: 1000
                repeat: true
                running: root.isRetryable && errorMsg.retryCountdown > 0
                onTriggered: {
                    if (errorMsg.retryCountdown > 0) errorMsg.retryCountdown -= 1
                }
            }
        }
    }
}
