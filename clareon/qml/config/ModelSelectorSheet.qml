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

Kirigami.OverlaySheet {
    id: sheet

    property string provider
    property bool loading: true
    property string errorMessage: ""

    signal modelSelected(string modelId, int contextWindow, int maxOutputTokens)

    title: qsTr("Select Model")

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
            if (searchField.text === "") return new RegExp()
            return new RegExp(searchField.text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'), "i")
        }
        sortRoleName: {
            switch (sortCombo.currentIndex) {
                case 1: return "modelContextWindow"
                case 2: return "modelPricingPrompt"
                default: return "modelName"
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

    header: RowLayout {
        spacing: Kirigami.Units.mediumSpacing

        Controls.TextField {
            id: searchField
            Layout.fillWidth: true
            placeholderText: qsTr("Search models...")
        }

        Controls.ComboBox {
            id: sortCombo
            model: [qsTr("Name"), qsTr("Context Window"), qsTr("Price")]
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
            visible: !sheet.loading && sheet.errorMessage === "" && filteredModels.rowCount() === 0
            text: qsTr("No models found")
            icon.name: "edit-find"
        }

        // Virtualized model grid - only instantiates visible delegates
        GridView {
            id: modelGrid
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.preferredHeight: Kirigami.Units.gridUnit * 30
            visible: !sheet.loading && sheet.errorMessage === "" && filteredModels.rowCount() > 0

            cellWidth: modelGrid.width / 2
            cellHeight: Kirigami.Units.gridUnit * 12

            clip: true
            model: filteredModels

            Controls.ScrollBar.vertical: Controls.ScrollBar {
                policy: Controls.ScrollBar.Never
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

                            Controls.Label {
                                visible: modelInputModalities !== ""
                                text: qsTr("Input: %1").arg(modelInputModalities)
                                font.pointSize: Kirigami.Theme.smallFont.pointSize
                            }

                            Controls.Label {
                                visible: modelPricingPrompt !== ""
                                text: qsTr("Price: ")
                                font.pointSize: Kirigami.Theme.smallFont.pointSize
                            }
                            Controls.Label {
                                visible: modelContextWindow > 0
                                text: qsTr("Context: %1 tok").arg(modelContextWindow.toLocaleString())
                                font.pointSize: Kirigami.Theme.smallFont.pointSize
                            }
                            Controls.Label {
                                visible: modelMaxOutputTokens > 0
                                text: qsTr("Max Output: %1 tok").arg(modelMaxOutputTokens.toLocaleString())
                                font.pointSize: Kirigami.Theme.smallFont.pointSize
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
