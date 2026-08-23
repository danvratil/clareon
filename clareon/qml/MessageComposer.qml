// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import QtQuick.Dialogs
import org.kde.kirigami as Kirigami
import cz.dvratil.clareon 1.0

 ColumnLayout {
    id: root
    spacing: 0

    required property string conversationId
    property bool streaming: false

    signal messageSent(string text)

    // List to store attached file paths
    property var attachedFiles: []

    // Function to add a file
    function addFile(filePath) {
        // Convert file:// URL to local path
        let path = filePath.toString().replace(/^file:\/\//, '')

        // Check if file is already attached
        if (attachedFiles.indexOf(path) === -1) {
            attachedFiles = attachedFiles.concat([path])
        }
    }

    // Function to remove a file by index
    function removeFile(index) {
        let newFiles = []
        for (let i = 0; i < attachedFiles.length; i++) {
            if (i !== index) {
                newFiles.push(attachedFiles[i])
            }
        }
        attachedFiles = newFiles
    }

    // Function to clear all files
    function clearFiles() {
        attachedFiles = []
    }

    // Function to send the message
    function sendMessage() {
        if (root.streaming) {
            return
        }

        const messageText = messageInput.text.trim()
        const hasText = messageText.length > 0
        const hasFiles = attachedFiles.length > 0

        if (!hasText && !hasFiles) {
            return
        }

        // Clear the input first
        messageInput.text = ""

        // Convert JavaScript array to QStringList
        let filePathsList = []
        for (let i = 0; i < attachedFiles.length; i++) {
            filePathsList.push(attachedFiles[i])
        }

        // Send the message
        if (hasFiles) {
            ServiceController.sendMessageWithFiles(root.conversationId, messageText, filePathsList)
        } else {
            ServiceController.sendMessage(root.conversationId, messageText)
        }

        // Clear attached files
        clearFiles()

        // Emit signal
        root.messageSent(messageText)
    }

    Kirigami.Separator {
        Layout.fillWidth: true
    }

    // Attached files display
    Flow {
        id: attachedFilesFlow
        Layout.fillWidth: true
        Layout.margins: attachedFiles.length > 0 ? Kirigami.Units.largeSpacing : 0
        Layout.bottomMargin: 0
        spacing: Kirigami.Units.smallSpacing
        visible: attachedFiles.length > 0

        Repeater {
            model: root.attachedFiles
            delegate: Controls.Button {
                required property int index
                required property string modelData

                text: modelData.split('/').pop()
                icon.name: "text-x-generic"
                flat: true

                Controls.ToolTip.visible: hovered
                Controls.ToolTip.text: modelData

                // Remove button
                Controls.ToolButton {
                    anchors.right: parent.right
                    anchors.top: parent.top
                    anchors.margins: 2
                    icon.name: "edit-delete-remove"
                    onClicked: root.removeFile(index)
                    width: 20
                    height: 20
                }
            }
        }
    }

    RowLayout {
        Layout.fillWidth: true
        Layout.margins: Kirigami.Units.largeSpacing
        spacing: Kirigami.Units.largeSpacing

        Controls.ToolButton {
            id: uploadButton
            icon.name: "document-open"
            Controls.ToolTip.visible: hovered
            Controls.ToolTip.text: "Attach file"

            onClicked: fileDialog.open()
        }

        Controls.ScrollView {
            Layout.fillWidth: true
            Layout.maximumHeight: 150
            Layout.minimumHeight: 60

            Controls.TextArea {
                id: messageInput
                placeholderText: "Message Clareon..."
                wrapMode: TextEdit.Wrap
                selectByMouse: true
                focus: true

                Keys.onReturnPressed: (event) => {
                    if (event.modifiers & Qt.ControlModifier || event.modifiers & Qt.ShiftModifier) {
                        // Ctrl+Enter or Shift+Enter: insert newline
                        event.accepted = false
                    } else {
                        // Enter: send message
                        event.accepted = true
                        root.sendMessage()
                    }
                }

                Keys.onEscapePressed: {
                    messageInput.text = ""
                    root.clearFiles()
                }

                // Drag & Drop support
                DropArea {
                    anchors.fill: parent

                    onDropped: (drop) => {
                        if (drop.hasUrls) {
                            for (let i = 0; i < drop.urls.length; i++) {
                                let url = drop.urls[i].toString()
                                root.addFile(url)
                            }
                            drop.accept()
                        }
                    }

                    onEntered: (drag) => {
                        if (drag.hasUrls) {
                            drag.accept()
                        }
                    }
                }
            }
        }

        Controls.Button {
            id: sendButton
            text: root.streaming ? qsTr("Stop") : qsTr("Send")
            icon.name: root.streaming ? "process-stop" : "document-send"
            enabled: root.streaming || messageInput.text.trim().length > 0 || attachedFiles.length > 0

            onClicked: {
                if (root.streaming) {
                    ServiceController.stopGeneration(root.conversationId)
                } else {
                    root.sendMessage()
                }
            }
        }
    }

    // File dialog for selecting files
    FileDialog {
        id: fileDialog
        title: "Select files"
        nameFilters: ["All files (*)"]
        fileMode: FileDialog.OpenFiles

        onAccepted: {
            for (let i = 0; i < selectedFiles.length; i++) {
                root.addFile(selectedFiles[i])
            }
        }
    }
}
