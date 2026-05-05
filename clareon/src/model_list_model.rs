// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! ModelListModel - Qt model for the list of available LLM models

use std::pin::Pin;

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QHash, QHashPair_i32_QByteArray, QModelIndex, QString, QVariant};

use crate::model_helpers::{Subscription, get_item, make_role_names};
use crate::service::{ModelInfoData, Response};
use crate::service_controller::try_get_service_handle;

#[cxx_qt::bridge]
mod ffi {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;

        include!("cxx-qt-lib/qhash.h");
        type QHash_i32_QByteArray = cxx_qt_lib::QHash<cxx_qt_lib::QHashPair_i32_QByteArray>;

        include!("cxx-qt-lib/qvariant.h");
        type QVariant = cxx_qt_lib::QVariant;

        include!("cxx-qt-lib/qmodelindex.h");
        type QModelIndex = cxx_qt_lib::QModelIndex;

        include!("QtCore/QAbstractListModel");
        type QAbstractListModel;
    }

    #[auto_cxx_name]
    unsafe extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[base = QAbstractListModel]
        #[qproperty(bool, is_loading, READ, NOTIFY)]
        #[qproperty(QString, load_error, READ, NOTIFY)]
        type ModelListModel = super::ModelListModelRust;

        #[qsignal]
        fn models_loaded(self: Pin<&mut ModelListModel>);

        #[qsignal]
        fn models_load_failed(self: Pin<&mut ModelListModel>, error: QString);

        #[cxx_override]
        fn row_count(self: &ModelListModel, parent: &QModelIndex) -> i32;

        #[cxx_override]
        fn data(self: &ModelListModel, index: &QModelIndex, role: i32) -> QVariant;

        #[cxx_override]
        fn role_names(self: &ModelListModel) -> QHash_i32_QByteArray;

        #[inherit]
        fn begin_reset_model(self: Pin<&mut ModelListModel>);

        #[inherit]
        fn end_reset_model(self: Pin<&mut ModelListModel>);
    }

    impl cxx_qt::Threading for ModelListModel {}
    impl cxx_qt::Initialize for ModelListModel {}
}

#[repr(i32)]
#[derive(Debug, Clone, Copy)]
enum ModelRole {
    ModelId = 0x1000 + 1,
    ModelName,
    ModelContextWindow,
    ModelMaxOutputTokens,
    ModelDescription,
    ModelOwner,
    ModelPricingPrompt,
    ModelPricingCompletion,
    ModelInputModalities,
    ModelOutputModalities,
    /// Concatenation of name + id + owner + description for KSortFilterProxyModel text search.
    Searchable,
}

impl From<ModelRole> for i32 {
    fn from(role: ModelRole) -> Self {
        role as i32
    }
}

#[derive(Default)]
pub struct ModelListModelRust {
    models: Vec<ModelInfoData>,
    subscription: Subscription,
    is_loading: bool,
    load_error: QString,
}

impl cxx_qt::Initialize for ffi::ModelListModel {
    fn initialize(self: Pin<&mut Self>) {
        self.subscribe_to_events();
    }
}

impl ffi::ModelListModel {
    fn row_count(&self, _parent: &QModelIndex) -> i32 {
        self.rust().models.len() as i32
    }

    fn data(&self, index: &QModelIndex, role: i32) -> QVariant {
        let Some(model) = get_item(&self.rust().models, index) else {
            return QVariant::default();
        };

        if role == ModelRole::ModelId as i32 {
            QVariant::from(&QString::from(&model.id))
        } else if role == ModelRole::ModelName as i32 {
            QVariant::from(&QString::from(&model.name))
        } else if role == ModelRole::ModelContextWindow as i32 {
            QVariant::from(&(model.context_window as i32))
        } else if role == ModelRole::ModelMaxOutputTokens as i32 {
            QVariant::from(&(model.max_output_tokens as i32))
        } else if role == ModelRole::ModelDescription as i32 {
            QVariant::from(&QString::from(&model.description))
        } else if role == ModelRole::ModelOwner as i32 {
            QVariant::from(&QString::from(&model.owner))
        } else if role == ModelRole::ModelPricingPrompt as i32 {
            QVariant::from(&QString::from(&model.pricing_prompt))
        } else if role == ModelRole::ModelPricingCompletion as i32 {
            QVariant::from(&QString::from(&model.pricing_completion))
        } else if role == ModelRole::ModelInputModalities as i32 {
            QVariant::from(&QString::from(&model.input_modalities))
        } else if role == ModelRole::ModelOutputModalities as i32 {
            QVariant::from(&QString::from(&model.output_modalities))
        } else if role == ModelRole::Searchable as i32 {
            let s = format!(
                "{} {} {} {}",
                model.name, model.id, model.owner, model.description
            );
            QVariant::from(&QString::from(&s))
        } else {
            QVariant::default()
        }
    }

    fn role_names(&self) -> QHash<QHashPair_i32_QByteArray> {
        make_role_names(&[
            (ModelRole::ModelId.into(), "modelId"),
            (ModelRole::ModelName.into(), "modelName"),
            (ModelRole::ModelContextWindow.into(), "modelContextWindow"),
            (
                ModelRole::ModelMaxOutputTokens.into(),
                "modelMaxOutputTokens",
            ),
            (ModelRole::ModelDescription.into(), "modelDescription"),
            (ModelRole::ModelOwner.into(), "modelOwner"),
            (ModelRole::ModelPricingPrompt.into(), "modelPricingPrompt"),
            (
                ModelRole::ModelPricingCompletion.into(),
                "modelPricingCompletion",
            ),
            (
                ModelRole::ModelInputModalities.into(),
                "modelInputModalities",
            ),
            (
                ModelRole::ModelOutputModalities.into(),
                "modelOutputModalities",
            ),
            (ModelRole::Searchable.into(), "searchable"),
        ])
    }

    fn subscribe_to_events(self: Pin<&mut Self>) {
        let handle = match try_get_service_handle() {
            Some(h) => h,
            None => return,
        };

        let mut response_rx = handle.subscribe();
        let qt_thread = self.qt_thread();

        let task = crate::get_runtime().spawn(async move {
            while let Ok(response) = response_rx.recv().await {
                let is_relevant = matches!(
                    &response,
                    Response::ModelsLoaded { .. } | Response::ModelsLoadFailed { .. }
                );

                if !is_relevant {
                    continue;
                }

                let _ = qt_thread.queue(move |mut model| {
                    model.as_mut().handle_response(response);
                });
            }
        });

        self.rust().subscription.start(task);
    }

    fn handle_response(mut self: Pin<&mut Self>, response: Response) {
        match response {
            Response::ModelsLoaded { models } => {
                self.as_mut().begin_reset_model();
                self.as_mut().rust_mut().models = models;
                self.as_mut().rust_mut().is_loading = false;
                self.as_mut().rust_mut().load_error = QString::default();
                self.as_mut().end_reset_model();
                self.as_mut().is_loading_changed();
                self.as_mut().load_error_changed();
                self.as_mut().models_loaded();
            }
            Response::ModelsLoadFailed { error } => {
                let error_qs = QString::from(&error);
                self.as_mut().rust_mut().is_loading = false;
                self.as_mut().rust_mut().load_error = error_qs.clone();
                self.as_mut().is_loading_changed();
                self.as_mut().load_error_changed();
                self.as_mut().models_load_failed(error_qs);
            }
            _ => {}
        }
    }
}
