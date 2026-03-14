// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import cz.dvratil.clareon

Kirigami.OverlaySheet {
    id: sheet

    property string provider
    property bool loading: true
    property string errorMessage: ""
    property var allModels: []
    property var filteredModels: []

    signal modelSelected(string modelId, int contextWindow, int maxOutputTokens)

    title: qsTr("Select Model")

    readonly property bool hasContextWindowData: {
        for (let i = 0; i < allModels.length; i++) {
            if (allModels[i].contextWindow > 0) return true
        }
        return false
    }

    readonly property bool hasPricingData: {
        for (let i = 0; i < allModels.length; i++) {
            if (allModels[i].pricingPrompt !== "") return true
        }
        return false
    }

    function loadModels() {
        loading = true
        errorMessage = ""
        ServiceController.fetchAvailableModels(provider)
    }

    function applyFilter() {
        let result = allModels.slice()
        let query = searchField.text.toLowerCase()
        if (query.length > 0) {
            result = result.filter(m =>
                m.name.toLowerCase().includes(query) ||
                m.id.toLowerCase().includes(query) ||
                m.owner.toLowerCase().includes(query) ||
                m.description.toLowerCase().includes(query)
            )
        }
        switch (sortCombo.currentIndex) {
            case 0: result.sort((a, b) => a.name.localeCompare(b.name)); break
            case 1: result.sort((a, b) => b.contextWindow - a.contextWindow); break
            case 2: result.sort((a, b) => a.pricingPrompt.localeCompare(b.pricingPrompt)); break
        }
        filteredModels = result
    }

    onOpened: loadModels()

    Connections {
        target: ServiceController
        function onModelsLoaded() {
            let models = []
            let count = ServiceController.getModelCount()
            for (let i = 0; i < count; i++) {
                models.push({
                    id: ServiceController.getModelId(i),
                    name: ServiceController.getModelName(i),
                    contextWindow: ServiceController.getModelContextWindow(i),
                    maxOutputTokens: ServiceController.getModelMaxOutputTokens(i),
                    description: ServiceController.getModelDescription(i),
                    owner: ServiceController.getModelOwner(i),
                    pricingPrompt: ServiceController.getModelPricingPrompt(i),
                    pricingCompletion: ServiceController.getModelPricingCompletion(i),
                    inputModalities: ServiceController.getModelInputModalities(i),
                    outputModalities: ServiceController.getModelOutputModalities(i),
                })
            }
            allModels = models
            applyFilter()
            loading = false
        }
        function onModelsLoadFailed(error) {
            errorMessage = error
            loading = false
        }
    }

    header: RowLayout {
        spacing: Kirigami.Units.mediumSpacing

        Controls.TextField {
            id: searchField
            Layout.fillWidth: true
            placeholderText: qsTr("Search models...")
            onTextChanged: applyFilter()
        }

        Controls.ComboBox {
            id: sortCombo
            model: {
                let options = [qsTr("Name")]
                if (sheet.hasContextWindowData) {
                    options.push(qsTr("Context Window"))
                }
                if (sheet.hasPricingData) {
                    options.push(qsTr("Price"))
                }
                return options
            }
            onCurrentIndexChanged: applyFilter()
        }
    }

    ColumnLayout {
        spacing: Kirigami.Units.largeSpacing
        Layout.preferredWidth: Kirigami.Units.gridUnit * 40

        // Loading state
        Kirigami.LoadingPlaceholder {
            Layout.fillWidth: true
            Layout.alignment: Qt.AlignHCenter
            visible: sheet.loading
        }

        // Error state
        Kirigami.PlaceholderMessage {
            Layout.fillWidth: true
            visible: !sheet.loading && sheet.errorMessage !== ""
            text: sheet.errorMessage
            icon.name: "dialog-error"

            helpfulAction: Kirigami.Action {
                text: qsTr("Retry")
                icon.name: "view-refresh"
                onTriggered: sheet.loadModels()
            }
        }

        // Empty state
        Kirigami.PlaceholderMessage {
            Layout.fillWidth: true
            visible: !sheet.loading && sheet.errorMessage === "" && sheet.filteredModels.length === 0
            text: qsTr("No models found")
            icon.name: "edit-find"
        }

        // Model grid
        GridLayout {
            columns: 2
            columnSpacing: Kirigami.Units.largeSpacing
            rowSpacing: Kirigami.Units.largeSpacing
            Layout.fillWidth: true
            visible: !sheet.loading && sheet.errorMessage === "" && sheet.filteredModels.length > 0

            Repeater {
                model: sheet.filteredModels

                delegate: Rectangle {
                    id: card

                    required property var modelData
                    required property int index

                    Layout.fillWidth: true
                    Layout.preferredHeight: cardContent.implicitHeight + 2 * Kirigami.Units.largeSpacing

                    radius: Kirigami.Units.cornerRadius
                    color: cardMouseArea.containsMouse ? Kirigami.Theme.highlightColor : Kirigami.Theme.backgroundColor
                    border.color: cardMouseArea.containsMouse ? Kirigami.Theme.highlightColor : Kirigami.Theme.separatorColor
                    border.width: 1

                    Kirigami.Theme.colorSet: Kirigami.Theme.View
                    Kirigami.Theme.inherit: false

                    ColumnLayout {
                        id: cardContent
                        anchors.fill: parent
                        anchors.margins: Kirigami.Units.largeSpacing
                        spacing: Kirigami.Units.smallSpacing

                        // Owner
                        Controls.Label {
                            text: card.modelData.owner.toUpperCase()
                            visible: card.modelData.owner !== ""
                            font.pointSize: Kirigami.Theme.smallFont.pointSize
                            font.weight: Font.Medium
                            color: Kirigami.Theme.disabledTextColor
                            Layout.fillWidth: true
                        }

                        // Model name
                        Controls.Label {
                            text: card.modelData.name || card.modelData.id
                            font.bold: true
                            font.pointSize: Kirigami.Theme.defaultFont.pointSize * 1.1
                            Layout.fillWidth: true
                            elide: Text.ElideRight
                        }

                        // Model ID
                        Controls.Label {
                            text: card.modelData.id
                            visible: card.modelData.name !== "" && card.modelData.name !== card.modelData.id
                            font.pointSize: Kirigami.Theme.smallFont.pointSize
                            color: Kirigami.Theme.disabledTextColor
                            Layout.fillWidth: true
                            elide: Text.ElideRight
                        }

                        // Description
                        Controls.Label {
                            text: card.modelData.description
                            visible: card.modelData.description !== ""
                            font.pointSize: Kirigami.Theme.smallFont.pointSize
                            color: Kirigami.Theme.disabledTextColor
                            Layout.fillWidth: true
                            maximumLineCount: 2
                            elide: Text.ElideRight
                            wrapMode: Text.WordWrap
                        }

                        // Chips
                        Flow {
                            Layout.fillWidth: true
                            spacing: Kirigami.Units.smallSpacing
                            visible: inputModalitiesRepeater.count > 0
                                || card.modelData.pricingPrompt !== ""
                                || card.modelData.pricingCompletion !== ""
                                || card.modelData.contextWindow > 0
                                || card.modelData.maxOutputTokens > 0

                            Repeater {
                                id: inputModalitiesRepeater
                                model: card.modelData.inputModalities || []
                                delegate: Kirigami.Chip {
                                    text: modelData
                                    closable: false
                                    checkable: false
                                }
                            }

                            Kirigami.Chip {
                                text: qsTr("Prompt: %1").arg(card.modelData.pricingPrompt)
                                visible: card.modelData.pricingPrompt !== ""
                                closable: false
                                checkable: false
                            }

                            Kirigami.Chip {
                                text: qsTr("Completion: %1").arg(card.modelData.pricingCompletion)
                                visible: card.modelData.pricingCompletion !== ""
                                closable: false
                                checkable: false
                            }

                            Kirigami.Chip {
                                text: qsTr("Context: %1").arg(card.modelData.contextWindow.toLocaleString())
                                visible: card.modelData.contextWindow > 0
                                closable: false
                                checkable: false
                            }

                            Kirigami.Chip {
                                text: qsTr("Max output: %1").arg(card.modelData.maxOutputTokens.toLocaleString())
                                visible: card.modelData.maxOutputTokens > 0
                                closable: false
                                checkable: false
                            }
                        }
                    }

                    MouseArea {
                        id: cardMouseArea
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: {
                            sheet.modelSelected(card.modelData.id, card.modelData.contextWindow, card.modelData.maxOutputTokens)
                            sheet.close()
                        }
                    }
                }
            }
        }
    }
}
