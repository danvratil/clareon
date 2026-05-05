// SPDX-FileContributor: Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! ArtifactListModel - Qt model for artifacts in a conversation

use std::pin::Pin;
use tracing::{debug, info};

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QHash, QHashPair_i32_QByteArray, QModelIndex, QString, QVariant};

use clareon_core::types::ConversationId;

use crate::model_helpers::{Subscription, get_item, make_role_names};
use crate::service::{ArtifactData, Command, Response};
use crate::service_controller::try_get_service_handle;

#[cxx_qt::bridge]
mod ffi {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;

        include!("cxx-qt-lib/qhash.h");
        type QHash_i32_QByteArray = cxx_qt_lib::QHash<cxx_qt_lib::QHashPair_i32_QByteArray>;

        include!("cxx-qt-lib/qlist.h");
        type QList_i32 = cxx_qt_lib::QList<i32>;

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
        #[qproperty(QString, conversation_id, READ, WRITE = set_conversation_id, NOTIFY)]
        type ArtifactListModel = super::ArtifactListModelRust;

        fn set_conversation_id(self: Pin<&mut ArtifactListModel>, id: QString);

        #[qinvokable]
        fn save_artifact(self: Pin<&mut ArtifactListModel>, artifact_id: i64, path: QString);

        #[cxx_override]
        fn row_count(self: &ArtifactListModel, parent: &QModelIndex) -> i32;

        #[cxx_override]
        fn data(self: &ArtifactListModel, index: &QModelIndex, role: i32) -> QVariant;

        #[cxx_override]
        fn role_names(self: &ArtifactListModel) -> QHash_i32_QByteArray;

        #[inherit]
        fn begin_insert_rows(
            self: Pin<&mut ArtifactListModel>,
            parent: &QModelIndex,
            first: i32,
            last: i32,
        );

        #[inherit]
        fn end_insert_rows(self: Pin<&mut ArtifactListModel>);

        #[inherit]
        fn begin_reset_model(self: Pin<&mut ArtifactListModel>);

        #[inherit]
        fn end_reset_model(self: Pin<&mut ArtifactListModel>);
    }

    impl cxx_qt::Threading for ArtifactListModel {}
}

/// Custom roles for the model
#[repr(i32)]
#[derive(Debug, Clone, Copy)]
enum ArtifactRole {
    ArtifactId = 0x1000 + 1, // Qt::UserRole + 1
    MessageId,
    Filename,
    MimeType,
    SizeBytes,
    ContentHash,
    CreatedAt,
    UpdatedAt,
}

impl From<ArtifactRole> for i32 {
    fn from(role: ArtifactRole) -> Self {
        role as i32
    }
}

#[derive(Debug, Clone)]
struct Artifact {
    id: i64,
    message_id: i64,
    filename: String,
    mime_type: String,
    size_bytes: i64,
    content_hash: String,
    created_at: i64,
    updated_at: i64,
}

impl From<ArtifactData> for Artifact {
    fn from(data: ArtifactData) -> Self {
        Self {
            id: data.id,
            message_id: data.message_id,
            filename: data.filename,
            mime_type: data.mime_type,
            size_bytes: data.size_bytes,
            content_hash: data.content_hash,
            created_at: data.created_at,
            updated_at: data.updated_at,
        }
    }
}

/// Rust implementation of ArtifactListModel
#[derive(Default)]
pub struct ArtifactListModelRust {
    conversation_id: QString,
    artifacts: Vec<Artifact>,
    subscription: Subscription,
}

#[allow(dead_code)]
impl ffi::ArtifactListModel {
    /// Set artifacts (replaces all existing artifacts)
    pub fn set_artifacts_internal(mut self: Pin<&mut Self>, artifacts: Vec<ArtifactData>) {
        self.as_mut().begin_reset_model();
        self.as_mut().rust_mut().artifacts = artifacts.into_iter().map(Artifact::from).collect();
        self.as_mut().end_reset_model();
    }

    /// Clear all artifacts
    pub fn clear_internal(mut self: Pin<&mut Self>) {
        if self.rust().artifacts.is_empty() {
            return;
        }
        self.as_mut().begin_reset_model();
        self.as_mut().rust_mut().artifacts.clear();
        self.as_mut().end_reset_model();
    }

    fn set_conversation_id(mut self: Pin<&mut Self>, id: QString) {
        if id == *self.conversation_id() {
            return;
        }

        self.as_mut().unsubscribe_from_events();
        self.as_mut().clear_internal();
        if id.is_empty() {
            return;
        }

        self.as_mut().rust_mut().conversation_id = id;
        self.as_mut().conversation_id_changed();

        self.as_mut().subscribe_to_events();

        if let Some(handle) = try_get_service_handle() {
            let conv_id = ConversationId::from(self.conversation_id().to_string());
            info!(
                conversation_id = self.conversation_id().to_string(),
                "Loading artifacts for conversation in ArtifactListModel"
            );
            let _ = handle.send(Command::LoadArtifacts { conv_id });
        }
    }

    fn save_artifact(self: Pin<&mut Self>, artifact_id: i64, path: QString) {
        if let Some(handle) = try_get_service_handle() {
            info!(
                artifact_id = artifact_id,
                path = path.to_string(),
                "Saving artifact"
            );
            let _ = handle.send(Command::SaveArtifact {
                artifact_id,
                path: path.to_string(),
            });
        }
    }

    fn row_count(&self, _parent: &QModelIndex) -> i32 {
        self.rust().artifacts.len() as i32
    }

    fn data(&self, index: &QModelIndex, role: i32) -> QVariant {
        let Some(artifact) = get_item(&self.rust().artifacts, index) else {
            return QVariant::default();
        };

        if role == ArtifactRole::ArtifactId as i32 {
            QVariant::from(&artifact.id)
        } else if role == ArtifactRole::MessageId as i32 {
            QVariant::from(&artifact.message_id)
        } else if role == ArtifactRole::Filename as i32 {
            QVariant::from(&QString::from(&artifact.filename))
        } else if role == ArtifactRole::MimeType as i32 {
            QVariant::from(&QString::from(&artifact.mime_type))
        } else if role == ArtifactRole::SizeBytes as i32 {
            QVariant::from(&artifact.size_bytes)
        } else if role == ArtifactRole::ContentHash as i32 {
            QVariant::from(&QString::from(&artifact.content_hash))
        } else if role == ArtifactRole::CreatedAt as i32 {
            QVariant::from(&artifact.created_at)
        } else if role == ArtifactRole::UpdatedAt as i32 {
            QVariant::from(&artifact.updated_at)
        } else {
            QVariant::default()
        }
    }

    fn role_names(&self) -> QHash<QHashPair_i32_QByteArray> {
        make_role_names(&[
            (ArtifactRole::ArtifactId.into(), "artifactId"),
            (ArtifactRole::MessageId.into(), "messageId"),
            (ArtifactRole::Filename.into(), "filename"),
            (ArtifactRole::MimeType.into(), "mimeType"),
            (ArtifactRole::SizeBytes.into(), "sizeBytes"),
            (ArtifactRole::ContentHash.into(), "contentHash"),
            (ArtifactRole::CreatedAt.into(), "createdAt"),
            (ArtifactRole::UpdatedAt.into(), "updatedAt"),
        ])
    }

    /// Subscribe to broadcast events for the current conversation
    fn subscribe_to_events(self: Pin<&mut Self>) {
        let conv_id_str = self.conversation_id().to_string();
        debug!(
            conversation_id = conv_id_str,
            "Subscribing to conversation events for ArtifactListModel"
        );
        if conv_id_str.is_empty() {
            return;
        }

        let conv_id = ConversationId::from(conv_id_str);
        let qt_thread = self.qt_thread();

        // Get service handle and subscribe
        let handle = match crate::service_controller::try_get_service_handle() {
            Some(h) => h,
            None => return,
        };

        let mut response_rx = handle.subscribe();

        let task = crate::get_runtime().spawn(async move {
            while let Ok(response) = response_rx.recv().await {
                let is_relevant = match &response {
                    Response::ArtifactsLoaded { conv_id: id, .. } => *id == conv_id,
                    _ => false,
                };

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

    /// Unsubscribe from broadcast events
    fn unsubscribe_from_events(self: Pin<&mut Self>) {
        debug!(
            conversation_id = self.conversation_id().to_string(),
            "Unsubscribing from conversation events for ArtifactListModel"
        );
        self.rust().subscription.cancel();
    }

    /// Handle a filtered response
    fn handle_response(mut self: Pin<&mut Self>, response: crate::service::Response) {
        match response {
            Response::ArtifactsLoaded { artifacts, .. } => {
                self.as_mut().set_artifacts_internal(artifacts);
            }
            _ => {
                // Ignore other response types
            }
        }
    }
}
