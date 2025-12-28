import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import cz.dvratil.clareon 1.0

Kirigami.Page {
    id: root

    required property AppController appController
    required property MessageListModel messageModel

    title: appController.conversationTitle

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        // Messages view
        Controls.ScrollView {
            Layout.fillWidth: true
            Layout.fillHeight: true

            ListView {
                id: messagesView
                clip: true

                model: root.messageModel

                // Scroll to bottom when new messages arrive
                onCountChanged: {
                    Qt.callLater(() => {
                        messagesView.positionViewAtEnd()
                    })
                }

                Component.onCompleted: {
                    messagesView.positionViewAtEnd()
                }

                delegate: MessageDelegate {
                    // Role maps to delegate's properties automatically
                }

                // Empty state
                Kirigami.PlaceholderMessage {
                    anchors.centerIn: parent
                    width: parent.width - (Kirigami.Units.largeSpacing * 4)
                    visible: messagesView.count === 0
                    icon.name: "view-conversation-balloon"
                    text: "No messages yet"
                    explanation: "Start a conversation by typing a message below"
                }
            }
        }

        // Message composer
        MessageComposer {
            Layout.fillWidth: true
            appController: root.appController
            messageModel: root.messageModel
        }
    }
}
