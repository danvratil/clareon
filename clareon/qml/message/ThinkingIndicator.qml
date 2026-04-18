// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

/**
 * Animated three-dot indicator shown while the assistant is thinking.
 * Replaces the plain BusyIndicator.
 */
Row {
    id: root

    spacing: Kirigami.Units.smallSpacing * 1.5

    readonly property int dotSize: Math.round(Kirigami.Units.gridUnit * 0.55)

    // Left padding to align with bubble content
    leftPadding: Kirigami.Units.largeSpacing
    topPadding: Kirigami.Units.largeSpacing
    bottomPadding: Kirigami.Units.largeSpacing

    Repeater {
        model: 3

        Rectangle {
            required property int index

            width: root.dotSize
            height: width
            radius: width / 2
            color: Kirigami.Theme.disabledTextColor
            anchors.verticalCenter: parent.verticalCenter

            SequentialAnimation on scale {
                loops: Animation.Infinite
                running: true
                PauseAnimation  { duration: index * 160 }
                NumberAnimation { to: 1.6; duration: 220; easing.type: Easing.OutQuad }
                NumberAnimation { to: 1.0; duration: 220; easing.type: Easing.InQuad }
                PauseAnimation  { duration: (2 - index) * 160 }
            }
        }
    }
}
