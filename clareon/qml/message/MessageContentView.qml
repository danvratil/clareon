// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

/**
 * Renders the content of a single message as a sequence of typed blocks.
 *
 * `contentBlocksJson` is a JSON array produced by the Rust `parse_content_blocks`
 * function.  Each element is either:
 *   {"type":"text",  "content":"..."}
 *   {"type":"code",  "language":"...", "content":"..."}
 */
ColumnLayout {
    id: root

    property string contentBlocksJson: "[]"
    property string messageRole: "assistant"  // "user" | "assistant"

    spacing: Kirigami.Units.smallSpacing

    readonly property var parsedBlocks: {
        try {
            const b = JSON.parse(root.contentBlocksJson)
            return Array.isArray(b) ? b : [{ type: "text", content: root.contentBlocksJson }]
        } catch (_) {
            return [{ type: "text", content: root.contentBlocksJson }]
        }
    }

    Repeater {
        model: root.parsedBlocks

        delegate: Loader {
            id: blockLoader
            required property var   modelData
            required property int   index
            Layout.fillWidth: true

            sourceComponent: modelData.type === "code" ? codeBlockComp : textBlockComp

            // Bind properties once the component is ready and whenever modelData changes
            Binding { target: blockLoader.item; property: "content";     value: modelData.content  ?? ""; when: blockLoader.status === Loader.Ready }
            Binding { target: blockLoader.item; property: "language";    value: modelData.language ?? ""; when: blockLoader.status === Loader.Ready && modelData.type === "code" }
            Binding { target: blockLoader.item; property: "messageRole"; value: root.messageRole;         when: blockLoader.status === Loader.Ready && modelData.type === "text" }
        }
    }

    // ── Component definitions ─────────────────────────────────────────────

    Component {
        id: textBlockComp

        Item {
            property string messageRole: "assistant"
            property string content: ""

            // parent is the Loader; inherit its width so the TextEdit can wrap text
            width: parent.width
            implicitHeight: textEdit.implicitHeight

            TextEdit {
                id: textEdit
                anchors { left: parent.left; right: parent.right }
                readOnly: true
                selectByMouse: true
                textFormat: TextEdit.MarkdownText
                text: parent.content
                wrapMode: Text.Wrap
                color: parent.messageRole === "user"
                    ? Kirigami.Theme.highlightedTextColor
                    : Kirigami.Theme.textColor
            }
        }
    }

    Component {
        id: codeBlockComp
        CodeBlockComponent {}
    }
}
