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

    // ListModel for GridView - populated from filteredModels JS array
    ListModel {
        id: gridModel
    }

    // Sync filteredModels array to ListModel whenever it changes
    onFilteredModelsChanged: {
        gridModel.clear()
        for (let i = 0; i < filteredModels.length; i++) {
            let m = filteredModels[i]
            gridModel.append({
                modelId: m.id,
                modelName: m.name,
                modelContextWindow: m.contextWindow,
                modelMaxOutputTokens: m.maxOutputTokens,
                modelDescription: m.description,
                modelOwner: m.owner,
                modelPricingPrompt: m.pricingPrompt,
                modelPricingCompletion: m.pricingCompletion,
                modelInputModalities: m.inputModalities,
                modelOutputModalities: m.outputModalities,
            })
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

        // Virtualized model grid - only instantiates visible delegates
        GridView {
            id: modelGrid
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.preferredHeight: Kirigami.Units.gridUnit * 30
            visible: !sheet.loading && sheet.errorMessage === "" && sheet.filteredModels.length > 0

            cellWidth: modelGrid.width / 2
            cellHeight: Kirigami.Units.gridUnit * 12

            clip: true
            model: gridModel

            Controls.ScrollBar.vertical: Controls.ScrollBar {
                policy: Controls.ScrollBar.AsNeeded
            }

            delegate: Item {
                width: modelGrid.cellWidth
                height: modelGrid.cellHeight

                required property int index
                required property string modelId
                required property string modelName
                required property int modelContextWindow
                required property int modelMaxOutputTokens
                required property string modelDescription
                required property string modelOwner
                required property string modelPricingPrompt
                required property string modelPricingCompletion
                required property string modelInputModalities
                required property string modelOutputModalities

                Kirigami.AbstractCard {
                    id: card
                    anchors.fill: parent
                    anchors.margins: Kirigami.Units.smallSpacing
                    showClickFeedback: true

                    contentItem: ColumnLayout {
                        spacing: Kirigami.Units.smallSpacing

                        // Owner
                        Controls.Label {
                            text: modelOwner.toUpperCase()
                            visible: modelOwner !== ""
                            font.pointSize: Kirigami.Theme.smallFont.pointSize
                            font.weight: Font.Medium
                            color: Kirigami.Theme.disabledTextColor
                            Layout.fillWidth: true
                        }

                        // Model name
                        Controls.Label {
                            text: modelName || modelId
                            font.bold: true
                            font.pointSize: Kirigami.Theme.defaultFont.pointSize * 1.1
                            Layout.fillWidth: true
                            elide: Text.ElideRight
                        }

                        // Model ID
                        Controls.Label {
                            text: modelId
                            visible: modelName !== "" && modelName !== modelId
                            font.pointSize: Kirigami.Theme.smallFont.pointSize
                            color: Kirigami.Theme.disabledTextColor
                            Layout.fillWidth: true
                            elide: Text.ElideRight
                        }

                        // Description
                        Controls.Label {
                            text: modelDescription
                            visible: modelDescription !== ""
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
                            visible: modelInputModalities !== ""
                                || modelPricingPrompt !== ""
                                || modelPricingCompletion !== ""
                                || modelContextWindow > 0
                                || modelMaxOutputTokens > 0

                            Repeater {
                                model: modelInputModalities !== "" ? modelInputModalities.split(",") : []
                                delegate: Kirigami.Chip {
                                    text: modelData
                                    closable: false
                                    checkable: false
                                }
                            }

                            Kirigami.Chip {
                                text: qsTr("Prompt: %1").arg(modelPricingPrompt)
                                visible: modelPricingPrompt !== ""
                                closable: false
                                checkable: false
                            }

                            Kirigami.Chip {
                                text: qsTr("Completion: %1").arg(modelPricingCompletion)
                                visible: modelPricingCompletion !== ""
                                closable: false
                                checkable: false
                            }

                            Kirigami.Chip {
                                text: qsTr("Context: %1").arg(modelContextWindow.toLocaleString())
                                visible: modelContextWindow > 0
                                closable: false
                                checkable: false
                            }

                            Kirigami.Chip {
                                text: qsTr("Max output: %1").arg(modelMaxOutputTokens.toLocaleString())
                                visible: modelMaxOutputTokens > 0
                                closable: false
                                checkable: false
                            }
                        }

                        // Spacer to push content to top
                        Item {
                            Layout.fillHeight: true
                        }
                    }

                    onClicked: {
                        sheet.modelSelected(modelId, modelContextWindow, modelMaxOutputTokens)
                        sheet.close()
                    }
                }
            }
        }
    }
}
