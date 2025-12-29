// SPDX-FileContributor: Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as Controls
import org.kde.kirigami as Kirigami
import cz.dvratil.clareon 1.0

Controls.ItemDelegate {
    id: delegate

    required property AppController appController
    required property string conversationId
    required property string title
    required property int updatedAt
    required property string model
    required property int messageCount

    highlighted: delegate.conversationId === appController.currentConversationId

    contentItem: ColumnLayout {
        spacing: Kirigami.Units.smallSpacing

        RowLayout {
            Layout.fillWidth: true
            spacing: Kirigami.Units.smallSpacing

            Controls.Label {
                Layout.fillWidth: true
                text: delegate.title
                font.bold: delegate.highlighted
                elide: Text.ElideRight
            }

            Controls.Label {
                text: Qt.formatDateTime(new Date(delegate.updatedAt * 1000), "MMM d")
                opacity: 0.7
                font.pointSize: Kirigami.Theme.smallFont.pointSize
            }
        }

        RowLayout {
            Layout.fillWidth: true
            spacing: Kirigami.Units.largeSpacing

            Controls.Label {
                text: delegate.model
                opacity: 0.6
                font.pointSize: Kirigami.Theme.smallFont.pointSize
                elide: Text.ElideRight
            }

            Controls.Label {
                text: delegate.messageCount + " messages"
                opacity: 0.6
                font.pointSize: Kirigami.Theme.smallFont.pointSize
            }
        }
    }
}
