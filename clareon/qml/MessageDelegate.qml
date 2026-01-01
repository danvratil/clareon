// SPDX-FileContributor: Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

Item {
    id: root

    // Properties map to MessageListModel roles
    required property int messageId
    required property string role
    required property string textContent
    required property int createdAt

    width: ListView.view.width
    height: messageLayout.implicitHeight + Kirigami.Units.largeSpacing * 2

    RowLayout {
        id: messageLayout
        anchors.fill: parent
        anchors.margins: Kirigami.Units.largeSpacing
        spacing: Kirigami.Units.largeSpacing

        // Spacer for user messages (right-align)
        Item {
            Layout.fillWidth: true
            Layout.preferredWidth: parent.width * 0.2
            visible: root.role === "user"
        }

        // Message bubble
        Kirigami.Card {
            Layout.fillWidth: true
            /// Assistant messages can use full width, user messages are limited to 75%, right-aligned
            Layout.maximumWidth: root.role === "user" ? root.width * 0.75 : root.width

            background: Rectangle {
                color: root.role === "user"
                    ? Kirigami.Theme.highlightColor
                    : Kirigami.Theme.backgroundColor
                border.color: root.role === "user"
                    ? Qt.darker(Kirigami.Theme.highlightColor, 1.1)
                    : Kirigami.Theme.alternateBackgroundColor
                border.width: 1
                radius: Kirigami.Units.cornerRadius
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
