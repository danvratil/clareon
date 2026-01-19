// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as Controls
import org.kde.kirigami as Kirigami
import cz.dvratil.clareon 1.0

// Quick input popup window
Controls.ApplicationWindow {
    id: popup

    // Window properties
    flags: Qt.Dialog | Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint
    width: 600
    height: 80
    
    // Center on screen
    x: (Screen.width - width) / 2
    y: (Screen.height - height) / 3  // Slightly above center, like KRunner

    // Make it semi-transparent
    color: "transparent"

    signal promptSubmitted(string prompt)

    // Background with rounded corners and shadow effect
    Kirigami.ShadowedRectangle {
        id: background
        anchors.fill: parent
        radius: Kirigami.Units.cornerRadius
        color: Kirigami.Theme.backgroundColor
        border.color: Kirigami.Theme.focusColor
        border.width: 2

        RowLayout {
            anchors.fill: parent
            anchors.margins: Kirigami.Units.largeSpacing
            spacing: Kirigami.Units.mediumSpacing

            // Clareon icon
            Kirigami.Icon {
                source: ":/clareon.svg"
                Layout.preferredWidth: Kirigami.Units.iconSizes.large
                Layout.preferredHeight: Kirigami.Units.iconSizes.large
                Layout.alignment: Qt.AlignVCenter
            }

            // Text input
            Controls.TextField {
                id: inputField
                Layout.fillWidth: true
                Layout.alignment: Qt.AlignVCenter

                placeholderText: qsTr("Ask Clareon...")
                font.pointSize: Kirigami.Theme.defaultFont.pointSize * 1.2

                background: Item {} // Transparent background

                Keys.onPressed: function(event) {
                    if (event.key === Qt.Key_Escape) {
                        popup.hide()
                        event.accepted = true
                    } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                        if (inputField.text.trim().length > 0) {
                            popup.promptSubmitted(inputField.text)
                            inputField.text = ""
                            popup.hide()
                        }
                        event.accepted = true
                    }
                }

                onActiveFocusChanged: {
                    if (!activeFocus) {
                        // Hide popup when focus is lost
                        Qt.callLater(function() {
                            if (!popup.activeFocusItem) {
                                popup.hide()
                            }
                        })
                    }
                }
            }
        }
    }

    // Show and focus on the input field
    function showAndFocus() {
        popup.show()
        popup.raise()
        popup.requestActivate()
        inputField.forceActiveFocus()
    }

    // Override show to ensure focus
    onVisibilityChanged: {
        if (visible) {
            inputField.forceActiveFocus()
        } else {
            inputField.text = ""  // Clear input when hidden
        }
    }
}
