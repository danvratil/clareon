// SPDX-FileContributor: Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

Kirigami.AbstractCard {
    id: root

    required property int artifactId
    required property string filename
    required property string mimeType
    required property int sizeBytes
    required property var createdAt

    signal downloadRequested(int artifactId, string filename)
    signal previewRequested(int artifactId, string filename, string mimeType)

    contentItem: RowLayout {
        spacing: Kirigami.Units.smallSpacing

        // Icon based on mime type
        Kirigami.Icon {
            Layout.preferredWidth: Kirigami.Units.iconSizes.medium
            Layout.preferredHeight: Kirigami.Units.iconSizes.medium
            Layout.alignment: Qt.AlignVCenter

            source: {
                if (mimeType.startsWith("image/")) return "image-x-generic"
                if (mimeType.startsWith("text/")) return "text-x-generic"
                if (mimeType.includes("html")) return "text-html"
                if (mimeType.includes("pdf")) return "application-pdf"
                if (mimeType.includes("zip") || mimeType.includes("tar")) return "package-x-generic"
                return "application-x-executable"
            }
        }

        ColumnLayout {
            Layout.fillWidth: true
            spacing: Kirigami.Units.smallSpacing

            QQC2.Label {
                Layout.fillWidth: true
                text: filename
                font.bold: true
                elide: Text.ElideMiddle
            }

            RowLayout {
                spacing: Kirigami.Units.largeSpacing

                QQC2.Label {
                    text: formatFileSize(sizeBytes)
                    font.pointSize: Kirigami.Theme.smallFont.pointSize
                    color: Kirigami.Theme.disabledTextColor
                }

                QQC2.Label {
                    text: formatMimeType(mimeType)
                    font.pointSize: Kirigami.Theme.smallFont.pointSize
                    color: Kirigami.Theme.disabledTextColor
                }
            }
        }

        // Preview button (only for supported types)
        QQC2.ToolButton {
            visible: canPreview()
            icon.name: "document-preview"
            QQC2.ToolTip.text: "Preview"
            QQC2.ToolTip.visible: hovered
            QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay

            onClicked: root.previewRequested(artifactId, filename, mimeType)
        }

        // Save button
        QQC2.ToolButton {
            icon.name: "document-save"
            QQC2.ToolTip.text: "Save"
            QQC2.ToolTip.visible: hovered
            QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay

            onClicked: root.downloadRequested(artifactId, filename)
        }
    }

    function formatFileSize(bytes) {
        if (bytes < 1024) return bytes + " B"
        if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + " KB"
        if (bytes < 1024 * 1024 * 1024) return (bytes / (1024 * 1024)).toFixed(1) + " MB"
        return (bytes / (1024 * 1024 * 1024)).toFixed(1) + " GB"
    }

    function formatMimeType(mime) {
        // Extract the subtype for display
        const parts = mime.split("/")
        if (parts.length === 2) {
            return parts[1].toUpperCase()
        }
        return mime
    }

    function canPreview() {
        return mimeType.startsWith("text/") ||
               mimeType.includes("html") ||
               mimeType.includes("markdown") ||
               mimeType.startsWith("image/") ||
               mimeType.includes("javascript") ||
               mimeType.includes("json") ||
               mimeType.includes("xml")
    }
}
