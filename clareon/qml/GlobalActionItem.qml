// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as Controls
import org.kde.kirigami as Kirigami
import org.kde.kirigami.delegates as KirigamiDelegates

Controls.ItemDelegate {
    id: item

    Layout.fillWidth: true

    contentItem: RowLayout {
        KirigamiDelegates.IconTitleSubtitle {
            Layout.fillWidth: true

            title: item.text
            icon: icon.fromControlsIcon(item.icon)

            selected: item.highlighted || item.pressed
        }
    }
}