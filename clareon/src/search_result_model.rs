// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! SearchResultModel - Qt model for full-text search results

use std::pin::Pin;

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QHash, QHashPair_i32_QByteArray, QModelIndex, QString, QVariant};

use clareon_core::types::SearchResult;

use crate::model_helpers::{Subscription, get_item, make_role_names};
use crate::service::Response;
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
        type SearchResultModel = super::SearchResultModelRust;

        #[qinvokable]
        fn clear(self: Pin<&mut SearchResultModel>);

        #[cxx_override]
        fn row_count(self: &SearchResultModel, parent: &QModelIndex) -> i32;

        #[cxx_override]
        fn data(self: &SearchResultModel, index: &QModelIndex, role: i32) -> QVariant;

        #[cxx_override]
        fn role_names(self: &SearchResultModel) -> QHash_i32_QByteArray;

        #[inherit]
        fn begin_reset_model(self: Pin<&mut SearchResultModel>);

        #[inherit]
        fn end_reset_model(self: Pin<&mut SearchResultModel>);
    }

    impl cxx_qt::Threading for SearchResultModel {}
    impl cxx_qt::Initialize for SearchResultModel {}
}

#[repr(i32)]
#[derive(Debug, Clone, Copy)]
enum SearchRole {
    ConversationId = 0x1000 + 1,
    ConversationTitle,
    MessageId,
    Role,
    Snippet,
    CreatedAt,
}

impl From<SearchRole> for i32 {
    fn from(role: SearchRole) -> Self {
        role as i32
    }
}

#[derive(Default)]
pub struct SearchResultModelRust {
    results: Vec<SearchResult>,
    subscription: Subscription,
}

impl cxx_qt::Initialize for ffi::SearchResultModel {
    fn initialize(self: Pin<&mut Self>) {
        self.subscribe_to_events();
    }
}

impl ffi::SearchResultModel {
    pub fn clear(mut self: Pin<&mut Self>) {
        if self.rust().results.is_empty() {
            return;
        }
        self.as_mut().begin_reset_model();
        self.as_mut().rust_mut().results.clear();
        self.as_mut().end_reset_model();
    }

    fn row_count(&self, _parent: &QModelIndex) -> i32 {
        self.rust().results.len() as i32
    }

    fn data(&self, index: &QModelIndex, role: i32) -> QVariant {
        let Some(result) = get_item(&self.rust().results, index) else {
            return QVariant::default();
        };

        if role == SearchRole::ConversationId as i32 {
            QVariant::from(&QString::from(result.conversation_id.as_ref()))
        } else if role == SearchRole::ConversationTitle as i32 {
            QVariant::from(&QString::from(
                result.conversation_title.as_deref().unwrap_or(""),
            ))
        } else if role == SearchRole::MessageId as i32 {
            QVariant::from(&result.message_id)
        } else if role == SearchRole::Role as i32 {
            QVariant::from(&QString::from(&result.role))
        } else if role == SearchRole::Snippet as i32 {
            QVariant::from(&QString::from(&result.snippet))
        } else if role == SearchRole::CreatedAt as i32 {
            QVariant::from(&result.created_at)
        } else {
            QVariant::default()
        }
    }

    fn role_names(&self) -> QHash<QHashPair_i32_QByteArray> {
        make_role_names(&[
            (SearchRole::ConversationId.into(), "conversationId"),
            (SearchRole::ConversationTitle.into(), "conversationTitle"),
            (SearchRole::MessageId.into(), "messageId"),
            (SearchRole::Role.into(), "role"),
            (SearchRole::Snippet.into(), "snippet"),
            (SearchRole::CreatedAt.into(), "createdAt"),
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
                if !matches!(&response, Response::SearchResults { .. }) {
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
        if let Response::SearchResults { results } = response {
            self.as_mut().begin_reset_model();
            self.as_mut().rust_mut().results = results;
            self.as_mut().end_reset_model();
        }
    }
}
