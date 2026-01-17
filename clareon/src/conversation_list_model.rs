// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! ConversationListModel - Qt wrapper for conversation list

use std::pin::Pin;

use cxx_qt_lib::QString;

#[cxx_qt::bridge]
mod ffi {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    #[auto_cxx_name]
    unsafe extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(i32, count)]
        type ConversationListModel = super::ConversationListModelRust;

        #[qinvokable]
        fn refresh(self: Pin<&mut ConversationListModel>);

        #[qinvokable]
        fn get_id(self: &ConversationListModel, index: i32) -> QString;

        #[qinvokable]
        fn get_title(self: &ConversationListModel, index: i32) -> QString;

        #[qinvokable]
        fn get_updated_at(self: &ConversationListModel, index: i32) -> i64;

        #[qinvokable]
        fn get_model(self: &ConversationListModel, index: i32) -> QString;

        #[qsignal]
        fn data_changed(self: Pin<&mut ConversationListModel>);
    }

    impl cxx_qt::Threading for ConversationListModel {}
}

/// Rust implementation of ConversationListModel
#[derive(Default)]
pub struct ConversationListModelRust {
    count: i32,
}

impl ffi::ConversationListModel {
    /// Refresh the model (re-reads from cache)
    pub fn refresh(mut self: Pin<&mut Self>) {
        let count = crate::qt::conversations_cache().lock().unwrap().len() as i32;
        self.as_mut().set_count(count);
        self.as_mut().data_changed();
    }

    pub fn get_id(&self, index: i32) -> QString {
        if index < 0 {
            return QString::default();
        }
        let cache = crate::qt::conversations_cache();
        let conversations = cache.lock().unwrap();
        conversations
            .get(index as usize)
            .map(|c| QString::from(&c.id.to_string()))
            .unwrap_or_default()
    }

    pub fn get_title(&self, index: i32) -> QString {
        if index < 0 {
            return QString::default();
        }
        let cache = crate::qt::conversations_cache();
        let conversations = cache.lock().unwrap();
        conversations
            .get(index as usize)
            .and_then(|c| c.title.as_ref())
            .map(QString::from)
            .unwrap_or_default()
    }

    pub fn get_updated_at(&self, index: i32) -> i64 {
        if index < 0 {
            return 0;
        }
        let cache = crate::qt::conversations_cache();
        let conversations = cache.lock().unwrap();
        conversations
            .get(index as usize)
            .map(|c| c.updated_at)
            .unwrap_or(0)
    }

    pub fn get_model(&self, index: i32) -> QString {
        if index < 0 {
            return QString::default();
        }
        let cache = crate::qt::conversations_cache();
        let conversations = cache.lock().unwrap();
        conversations
            .get(index as usize)
            .map(|c| QString::from(&c.model))
            .unwrap_or_default()
    }
}

impl cxx_qt::Initialize for ffi::ConversationListModel {
    fn initialize(self: Pin<&mut Self>) {
        // Load initial data
        self.refresh();
    }
}
