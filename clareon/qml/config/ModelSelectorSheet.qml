// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.kitemmodels as KItemModels
import cc.clareon.core 1.0
import cz.dvratil.clareon

// Use Dialog rather than OverlaySheet: OverlaySheet wraps content in its own
// ScrollView, which doubles up with GridView scrolling and lets page chrome
// (e.g. the "Provider" section header) show through the sheet.
Kirigami.Dialog {
    id: sheet

    property string provider
    property bool loading: true
    property string errorMessage: ""

    signal modelSelected(string modelId, int contextWindow, int maxOutputTokens)

    title: qsTr("Select Model")
    standardButtons: Kirigami.Dialog.NoButton
    showCloseButton: true
    padding: 0
    preferredWidth: Kirigami.Units.gridUnit * 42
    preferredHeight: Kirigami.Units.gridUnit * 34

    function parsePerToken(perTokenStr) {
        if (perTokenStr === "" || perTokenStr === undefined || perTokenStr === null)
            return NaN
        return parseFloat(perTokenStr)
    }

    function isFreePrice(perTokenStr) {
        const n = parsePerToken(perTokenStr)
        return !isNaN(n) && n <= 0
    }

    // True when both sides are free (or only one side is present and free).
    function isFreeModel(prompt, completion) {
        const hasPrompt = prompt !== "" && prompt !== undefined && prompt !== null
        const hasCompletion = completion !== "" && completion !== undefined && completion !== null
        if (!hasPrompt && !hasCompletion)
            return false
        if (hasPrompt && !isFreePrice(prompt))
            return false
        if (hasCompletion && !isFreePrice(completion))
            return false
        return (hasPrompt && isFreePrice(prompt)) || (hasCompletion && isFreePrice(completion))
    }

    // Format OpenRouter-style per-token price as "$X.XX/MTok" (empty if free/missing)
    function formatPerMillion(perTokenStr) {
        const n = parsePerToken(perTokenStr)
        if (isNaN(n) || n <= 0)
            return ""
        const perMillion = n * 1e6
        let amount
        if (perMillion >= 100)
            amount = perMillion.toFixed(0)
        else if (perMillion >= 1)
            amount = perMillion.toFixed(2)
        else if (perMillion >= 0.01)
            amount = perMillion.toFixed(3)
        else
            amount = Number(perMillion.toPrecision(2)).toString()
        return qsTr("$%1/MTok").arg(amount)
    }

    // "text+image->text" → "text+image → text"; otherwise show as-is
    function formatModality(inputMods, outputMods) {
        const combined = inputMods || ""
        if (combined.indexOf("->") >= 0)
            return combined.replace("->", " → ")
        if (inputMods && outputMods && inputMods !== outputMods)
            return inputMods + " → " + outputMods
        return inputMods || outputMods
    }

    function formatTokenCount(n) {
        if (n >= 1e6)
            return qsTr("%1M").arg((n / 1e6).toFixed(n % 1e6 === 0 ? 0 : 1))
        if (n >= 1000)
            return qsTr("%1K").arg(Math.round(n / 1000))
        return String(n)
    }

    function loadModels() {
        loading = true
        errorMessage = ""
        ServiceController.fetchAvailableModels(provider)
    }

    onOpened: loadModels()

    ModelListModel {
        id: modelListModel
    }

    KItemModels.KSortFilterProxyModel {
        id: filteredModels
        sourceModel: modelListModel
        filterRoleName: "searchable"
        filterRegularExpression: {
            if (searchField.text === "")
                return new RegExp()
            return new RegExp(searchField.text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'), "i")
        }
        sortRoleName: {
            switch (sortCombo.currentIndex) {
            case 1:
                return "modelContextWindow"
            case 2:
                return "modelPricingPrompt"
            default:
                return "modelName"
            }
        }
        sortOrder: sortCombo.currentIndex === 1 ? Qt.DescendingOrder : Qt.AscendingOrder
    }

    Connections {
        target: modelListModel
        function onModelsLoaded() {
            sheet.loading = false
            sheet.errorMessage = ""
        }
        function onModelsLoadFailed(error) {
            sheet.errorMessage = error
            sheet.loading = false
        }
    }

    ColumnLayout {
        spacing: 0

        RowLayout {
            Layout.fillWidth: true
            Layout.leftMargin: Kirigami.Units.largeSpacing
            Layout.rightMargin: Kirigami.Units.largeSpacing
            Layout.topMargin: Kirigami.Units.smallSpacing
            Layout.bottomMargin: Kirigami.Units.smallSpacing
            spacing: Kirigami.Units.mediumSpacing

            Controls.TextField {
                id: searchField
                Layout.fillWidth: true
                placeholderText: qsTr("Search models...")
                Accessible.name: qsTr("Search models")
            }

            Controls.Label {
                text: qsTr("Sort by:")
                Accessible.ignored: true
            }

            Controls.ComboBox {
                id: sortCombo
                model: [qsTr("Name"), qsTr("Context Window"), qsTr("Price")]
                Accessible.name: qsTr("Sort by")
            }
        }

        Kirigami.Separator {
            Layout.fillWidth: true
        }

        Item {
            Layout.fillWidth: true
            Layout.preferredHeight: Kirigami.Units.gridUnit * 26
            Layout.minimumHeight: Kirigami.Units.gridUnit * 18

            Kirigami.LoadingPlaceholder {
                anchors.centerIn: parent
                visible: sheet.loading
            }

            Kirigami.PlaceholderMessage {
                anchors.centerIn: parent
                width: parent.width - Kirigami.Units.gridUnit * 2
                visible: !sheet.loading && sheet.errorMessage !== ""
                text: sheet.errorMessage
                icon.name: "dialog-error"

                helpfulAction: Kirigami.Action {
                    text: qsTr("Retry")
                    icon.name: "view-refresh"
                    onTriggered: sheet.loadModels()
                }
            }

            Kirigami.PlaceholderMessage {
                anchors.centerIn: parent
                width: parent.width - Kirigami.Units.gridUnit * 2
                visible: !sheet.loading && sheet.errorMessage === "" && filteredModels.rowCount() === 0
                text: qsTr("No models found")
                icon.name: "edit-find"
            }

            GridView {
                id: modelGrid
                anchors.fill: parent
                visible: !sheet.loading && sheet.errorMessage === "" && filteredModels.rowCount() > 0

                cellWidth: Math.floor(width / 2)
                cellHeight: Kirigami.Units.gridUnit * 9

                clip: true
                model: filteredModels
                boundsBehavior: Flickable.StopAtBounds
                // Single scrollbar — Dialog does not wrap us in another ScrollView
                // when content fits preferredHeight.
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

                    readonly property bool freeModel: sheet.isFreeModel(modelPricingPrompt, modelPricingCompletion)
                    readonly property string promptPriceText: freeModel ? "" : sheet.formatPerMillion(modelPricingPrompt)
                    readonly property string completionPriceText: freeModel ? "" : sheet.formatPerMillion(modelPricingCompletion)
                    readonly property string modalityText: sheet.formatModality(modelInputModalities, modelOutputModalities)

                    Kirigami.AbstractCard {
                        id: card
                        anchors.fill: parent
                        anchors.margins: Kirigami.Units.smallSpacing
                        showClickFeedback: true

                        contentItem: ColumnLayout {
                            spacing: Kirigami.Units.smallSpacing

                            Controls.Label {
                                text: modelOwner.toUpperCase()
                                visible: modelOwner !== ""
                                font.pointSize: Kirigami.Theme.smallFont.pointSize
                                font.weight: Font.DemiBold
                                color: Kirigami.Theme.disabledTextColor
                                Layout.fillWidth: true
                                elide: Text.ElideRight
                            }

                            Controls.Label {
                                text: modelName || modelId
                                font.bold: true
                                Layout.fillWidth: true
                                elide: Text.ElideRight
                                wrapMode: Text.NoWrap
                            }

                            Controls.Label {
                                text: modelId
                                visible: modelName !== "" && modelName !== modelId
                                font.pointSize: Kirigami.Theme.smallFont.pointSize
                                color: Kirigami.Theme.disabledTextColor
                                Layout.fillWidth: true
                                elide: Text.ElideRight
                            }

                            Controls.Label {
                                text: modelDescription
                                visible: modelDescription !== ""
                                font.pointSize: Kirigami.Theme.smallFont.pointSize
                                color: Kirigami.Theme.disabledTextColor
                                Layout.fillWidth: true
                                Layout.maximumHeight: Kirigami.Units.gridUnit * 2.5
                                maximumLineCount: 2
                                elide: Text.ElideRight
                                wrapMode: Text.WordWrap
                            }

                            // OpenRouter hosts per-model detail pages at openrouter.ai/<modelId>
                            Kirigami.LinkButton {
                                text: qsTr("Show more")
                                visible: sheet.provider === "openrouter" && modelDescription !== ""
                                font.pointSize: Kirigami.Theme.smallFont.pointSize
                                Layout.alignment: Qt.AlignLeft
                                onClicked: Qt.openUrlExternally("https://openrouter.ai/" + modelId)
                            }

                            Item {
                                Layout.fillHeight: true
                                Layout.minimumHeight: 0
                            }

                            Flow {
                                Layout.fillWidth: true
                                spacing: Kirigami.Units.smallSpacing

                                readonly property color chipBg: Qt.alpha(Kirigami.Theme.textColor, 0.08)
                                readonly property color chipFg: Kirigami.Theme.textColor

                                Repeater {
                                    model: {
                                        const chips = []
                                        if (modalityText)
                                            chips.push(modalityText)
                                        if (freeModel)
                                            chips.push(qsTr("free"))
                                        else {
                                            if (promptPriceText)
                                                chips.push("↑ " + promptPriceText)
                                            if (completionPriceText)
                                                chips.push("↓ " + completionPriceText)
                                        }
                                        if (modelContextWindow > 0)
                                            chips.push(qsTr("%1 ctx").arg(sheet.formatTokenCount(modelContextWindow)))
                                        return chips
                                    }

                                    delegate: Rectangle {
                                        required property string modelData
                                        radius: 3
                                        color: parent.chipBg
                                        implicitHeight: chipLabel.implicitHeight + 4
                                        implicitWidth: chipLabel.implicitWidth + 10

                                        Controls.Label {
                                            id: chipLabel
                                            anchors.centerIn: parent
                                            text: modelData
                                            font.pointSize: Kirigami.Theme.smallFont.pointSize
                                            color: parent.parent.chipFg
                                        }
                                    }
                                }
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
}
