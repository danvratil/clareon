// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

/**
 * Displays token usage statistics for a conversation
 */
Controls.Control {
    id: root

    required property int totalInputTokens
    required property int totalOutputTokens

    readonly property int totalTokens: totalInputTokens + totalOutputTokens

    padding: Kirigami.Units.mediumSpacing
    background: Rectangle {
        color: Kirigami.Theme.backgroundColor
        border.color: Kirigami.Theme.disabledTextColor
        border.width: 1
        opacity: 0.5
    }

    contentItem: RowLayout {
        spacing: Kirigami.Units.largeSpacing

        Kirigami.Icon {
            source: "office-chart-bar"
            Layout.preferredWidth: Kirigami.Units.iconSizes.small
            Layout.preferredHeight: Kirigami.Units.iconSizes.small
            color: Kirigami.Theme.disabledTextColor
        }

        Controls.Label {
            text: qsTr("Tokens:")
            font.pointSize: Kirigami.Theme.smallFont.pointSize
            color: Kirigami.Theme.disabledTextColor
        }

        Controls.Label {
            text: qsTr("Input: %1").arg(root.totalInputTokens.toLocaleString())
            font.pointSize: Kirigami.Theme.smallFont.pointSize
            color: Kirigami.Theme.disabledTextColor

            Controls.ToolTip.visible: hoverHandler.hovered
            Controls.ToolTip.text: qsTr("Total number of tokens sent to the model (prompts)")
            Controls.ToolTip.delay: Kirigami.Units.toolTipDelay

            HoverHandler {
                id: hoverHandler
            }
        }

        Controls.Label {
            text: "•"
            font.pointSize: Kirigami.Theme.smallFont.pointSize
            color: Kirigami.Theme.disabledTextColor
        }

        Controls.Label {
            text: qsTr("Output: %1").arg(root.totalOutputTokens.toLocaleString())
            font.pointSize: Kirigami.Theme.smallFont.pointSize
            color: Kirigami.Theme.disabledTextColor

            Controls.ToolTip.visible: outputHoverHandler.hovered
            Controls.ToolTip.text: qsTr("Total number of tokens received from the model (responses)")
            Controls.ToolTip.delay: Kirigami.Units.toolTipDelay

            HoverHandler {
                id: outputHoverHandler
            }
        }

        Controls.Label {
            text: "•"
            font.pointSize: Kirigami.Theme.smallFont.pointSize
            color: Kirigami.Theme.disabledTextColor
        }

        Controls.Label {
            text: qsTr("Total: %1").arg(root.totalTokens.toLocaleString())
            font.pointSize: Kirigami.Theme.smallFont.pointSize
            color: Kirigami.Theme.disabledTextColor
            font.bold: true

            Controls.ToolTip.visible: totalHoverHandler.hovered
            Controls.ToolTip.text: qsTr("Combined total of input and output tokens")
            Controls.ToolTip.delay: Kirigami.Units.toolTipDelay

            HoverHandler {
                id: totalHoverHandler
            }
        }

        Item {
            Layout.fillWidth: true
        }
    }

    visible: root.totalTokens > 0
}
