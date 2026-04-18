// SPDX-FileContributor: Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import QtQuick.Dialogs
import org.kde.kirigami as Kirigami
import cz.dvratil.clareon

Kirigami.OverlaySheet {
    id: root

    property int artifactId: -1
    property string filename: ""
    property string mimeType: ""
    property string content: ""

    signal downloadRequested(int artifactId, string filepath)

    title: filename
    width: Math.min(parent.width * 0.7, Kirigami.Units.gridUnit * 60)

    Connections {
        target: ServiceController

        function onArtifactLoaded(artifactId, filename, mimeType, content) {
            if (artifactId === root.artifactId) {
                root.content = content
            }
        }
    }

    FileDialog {
        id: saveDialog
        fileMode: FileDialog.SaveFile
        currentFolder: "file://" + StandardPaths.writableLocation(StandardPaths.DocumentsLocation)
        currentFile: root.filename

        onAccepted: {
            if (root.artifactId !== -1) {
                root.downloadRequested(root.artifactId, selectedFile.toString().replace("file://", ""))
            }
        }
    }

    header: RowLayout {
        Kirigami.Heading {
            Layout.fillWidth: true
            text: root.filename
            elide: Text.ElideMiddle
        }

        QQC2.ToolButton {
            icon.name: "document-save"
            text: "Save"
            onClicked: {
                saveDialog.open()
            }
        }
    }

    ColumnLayout {
        spacing: Kirigami.Units.largeSpacing

        // Metadata section
        Kirigami.FormLayout {
            Layout.fillWidth: true

            QQC2.Label {
                Layout.fillWidth: true

                Kirigami.FormData.label: "Type:"
                text: root.mimeType
            }

            QQC2.Label {
                Layout.fillWidth: true

                Kirigami.FormData.label: "Size:"
                text: formatFileSize(root.content.length)
            }
        }

        Kirigami.Separator {
            Layout.fillWidth: true
        }

        // Content preview
        QQC2.ScrollView {
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.preferredHeight: Kirigami.Units.gridUnit * 30
            visible: root.content !== "" && isPreviewSupported()

            contentItem: Loader {
                id: previewLoader

                sourceComponent: {
                    if (root.mimeType.includes("html")) {
                        return htmlPreview
                    } else {
                        return textPreview
                    }
                }
            }
        }

        // Loading indicator
        QQC2.BusyIndicator {
            Layout.alignment: Qt.AlignHCenter
            running: root.content === "" && root.artifactId !== -1
            visible: running
        }

        // Not supported message
        Kirigami.PlaceholderMessage {
            Layout.fillWidth: true
            Layout.fillHeight: true
            visible: root.content !== "" && !isPreviewSupported()

            icon.name: "dialog-information"
            text: "Preview not supported"
            explanation: "Preview is not yet supported for " + root.mimeType + " files.\nUse the Save button to download the file."
        }
    }

    // Text preview component
    Component {
        id: textPreview

        QQC2.TextArea {
            readOnly: true
            text: root.content
            wrapMode: Text.Wrap
            font.family: "monospace"
            selectByMouse: true
        }
    }

    // HTML preview component
    Component {
        id: htmlPreview

        QQC2.Label {
            text: root.content
            textFormat: Text.RichText
            wrapMode: Text.Wrap
        }
    }

    function isPreviewSupported() {
        return root.mimeType.startsWith("text/") ||
               root.mimeType.includes("html") ||
               root.mimeType.includes("json") ||
               root.mimeType.includes("xml") ||
               root.mimeType.includes("javascript") ||
               root.mimeType.includes("markdown")
    }

    function formatFileSize(bytes) {
        if (bytes < 1024) return bytes + " B"
        if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + " KB"
        if (bytes < 1024 * 1024 * 1024) return (bytes / (1024 * 1024)).toFixed(1) + " MB"
        return (bytes / (1024 * 1024 * 1024)).toFixed(1) + " GB"
    }

    // Function to load artifact content
    function loadArtifact(id, name, mime) {
        artifactId = id
        filename = name
        mimeType = mime
        content = "" // Clear previous content
        root.open()

        // Request artifact content from backend
        ServiceController.loadArtifact(id)
    }
}
