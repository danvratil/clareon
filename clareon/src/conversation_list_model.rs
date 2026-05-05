// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! ConversationListModel - Qt model for the list of conversations

use std::pin::Pin;
use tracing::debug;

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QHash, QHashPair_i32_QByteArray, QModelIndex, QString, QVariant};

use clareon_core::types::ConversationSummary;

use crate::model_helpers::{Subscription, get_item, make_role_names};
use crate::service::{Command, Response};
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
        type ConversationListModel = super::ConversationListModelRust;

        #[cxx_override]
        fn row_count(self: &ConversationListModel, parent: &QModelIndex) -> i32;

        #[cxx_override]
        fn data(self: &ConversationListModel, index: &QModelIndex, role: i32) -> QVariant;

        #[cxx_override]
        fn role_names(self: &ConversationListModel) -> QHash_i32_QByteArray;

        #[inherit]
        fn begin_insert_rows(
            self: Pin<&mut ConversationListModel>,
            parent: &QModelIndex,
            first: i32,
            last: i32,
        );

        #[inherit]
        fn end_insert_rows(self: Pin<&mut ConversationListModel>);

        #[inherit]
        fn begin_remove_rows(
            self: Pin<&mut ConversationListModel>,
            parent: &QModelIndex,
            first: i32,
            last: i32,
        );

        #[inherit]
        fn end_remove_rows(self: Pin<&mut ConversationListModel>);

        #[inherit]
        fn begin_reset_model(self: Pin<&mut ConversationListModel>);

        #[inherit]
        fn end_reset_model(self: Pin<&mut ConversationListModel>);
    }

    impl cxx_qt::Threading for ConversationListModel {}
    impl cxx_qt::Initialize for ConversationListModel {}
}

#[repr(i32)]
#[derive(Debug, Clone, Copy)]
enum ConversationRole {
    ConversationId = 0x1000 + 1,
    Title,
    UpdatedAt,
    Model,
    MessageCount,
}

impl From<ConversationRole> for i32 {
    fn from(role: ConversationRole) -> Self {
        role as i32
    }
}

#[derive(Default)]
pub struct ConversationListModelRust {
    conversations: Vec<ConversationSummary>,
    subscription: Subscription,
}

impl cxx_qt::Initialize for ffi::ConversationListModel {
    fn initialize(self: Pin<&mut Self>) {
        self.subscribe_to_events();

        if let Some(handle) = try_get_service_handle() {
            let _ = handle.send(Command::RefreshConversations);
        }
    }
}

impl ffi::ConversationListModel {
    fn row_count(&self, _parent: &QModelIndex) -> i32 {
        self.rust().conversations.len() as i32
    }

    fn data(&self, index: &QModelIndex, role: i32) -> QVariant {
        let Some(conv) = get_item(&self.rust().conversations, index) else {
            return QVariant::default();
        };

        if role == ConversationRole::ConversationId as i32 {
            QVariant::from(&QString::from(conv.id.as_ref()))
        } else if role == ConversationRole::Title as i32 {
            QVariant::from(&QString::from(conv.display_title()))
        } else if role == ConversationRole::UpdatedAt as i32 {
            QVariant::from(&conv.updated_at)
        } else if role == ConversationRole::Model as i32 {
            QVariant::from(&QString::from(&conv.model))
        } else if role == ConversationRole::MessageCount as i32 {
            QVariant::from(&(conv.message_count as i32))
        } else {
            QVariant::default()
        }
    }

    fn role_names(&self) -> QHash<QHashPair_i32_QByteArray> {
        make_role_names(&[
            (ConversationRole::ConversationId.into(), "conversationId"),
            (ConversationRole::Title.into(), "title"),
            (ConversationRole::UpdatedAt.into(), "updatedAt"),
            (ConversationRole::Model.into(), "model"),
            (ConversationRole::MessageCount.into(), "messageCount"),
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
                    Response::ConversationsRefreshed { .. }
                        | Response::ConversationCreated { .. }
                        | Response::ConversationDeleted { .. }
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
            Response::ConversationsRefreshed { conversations } => {
                debug!(
                    "ConversationListModel: refreshed {} conversations",
                    conversations.len()
                );
                self.as_mut().begin_reset_model();
                self.as_mut().rust_mut().conversations = conversations;
                self.as_mut().end_reset_model();
            }
            Response::ConversationCreated { conversation } => {
                debug!(
                    "ConversationListModel: conversation created {}",
                    conversation.id
                );
                self.as_mut()
                    .begin_insert_rows(&QModelIndex::default(), 0, 0);
                self.as_mut()
                    .rust_mut()
                    .conversations
                    .insert(0, conversation);
                self.as_mut().end_insert_rows();
            }
            Response::ConversationDeleted { id } => {
                debug!("ConversationListModel: conversation deleted {}", id);
                if let Some(pos) = self.rust().conversations.iter().position(|c| c.id == id) {
                    let row = pos as i32;
                    self.as_mut()
                        .begin_remove_rows(&QModelIndex::default(), row, row);
                    self.as_mut().rust_mut().conversations.remove(pos);
                    self.as_mut().end_remove_rows();
                }
            }
            _ => {}
        }
    }
}
