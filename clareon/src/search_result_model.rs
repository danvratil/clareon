// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! SearchResultModel - Qt wrapper for search results

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
        type SearchResultModel = super::SearchResultModelRust;

        #[qinvokable]
        fn refresh(self: Pin<&mut SearchResultModel>);

        #[qinvokable]
        fn clear(self: Pin<&mut SearchResultModel>);

        #[qinvokable]
        fn get_conversation_id(self: &SearchResultModel, index: i32) -> QString;

        #[qinvokable]
        fn get_conversation_title(self: &SearchResultModel, index: i32) -> QString;

        #[qinvokable]
        fn get_message_id(self: &SearchResultModel, index: i32) -> i64;

        #[qinvokable]
        fn get_role(self: &SearchResultModel, index: i32) -> QString;

        #[qinvokable]
        fn get_snippet(self: &SearchResultModel, index: i32) -> QString;

        #[qinvokable]
        fn get_created_at(self: &SearchResultModel, index: i32) -> i64;

        #[qsignal]
        fn data_changed(self: Pin<&mut SearchResultModel>);
    }

    impl cxx_qt::Threading for SearchResultModel {}
}

/// Rust implementation of SearchResultModel
#[derive(Default)]
pub struct SearchResultModelRust {
    count: i32,
}

impl ffi::SearchResultModel {
    /// Refresh the model (re-reads from cache)
    pub fn refresh(mut self: Pin<&mut Self>) {
        let count = crate::qt::search_results_cache().lock().unwrap().len() as i32;
        self.as_mut().set_count(count);
        self.as_mut().data_changed();
    }

    /// Clear the search results
    pub fn clear(mut self: Pin<&mut Self>) {
        crate::qt::search_results_cache().lock().unwrap().clear();
        self.as_mut().set_count(0);
        self.as_mut().data_changed();
    }

    pub fn get_conversation_id(&self, index: i32) -> QString {
        if index < 0 {
            return QString::default();
        }
        let cache = crate::qt::search_results_cache();
        let results = cache.lock().unwrap();
        results
            .get(index as usize)
            .map(|r| QString::from(&r.conversation_id.to_string()))
            .unwrap_or_default()
    }

    pub fn get_conversation_title(&self, index: i32) -> QString {
        if index < 0 {
            return QString::default();
        }
        let cache = crate::qt::search_results_cache();
        let results = cache.lock().unwrap();
        results
            .get(index as usize)
            .and_then(|r| r.conversation_title.as_ref())
            .map(QString::from)
            .unwrap_or_default()
    }

    pub fn get_message_id(&self, index: i32) -> i64 {
        if index < 0 {
            return 0;
        }
        let cache = crate::qt::search_results_cache();
        let results = cache.lock().unwrap();
        results
            .get(index as usize)
            .map(|r| r.message_id)
            .unwrap_or(0)
    }

    pub fn get_role(&self, index: i32) -> QString {
        if index < 0 {
            return QString::default();
        }
        let cache = crate::qt::search_results_cache();
        let results = cache.lock().unwrap();
        results
            .get(index as usize)
            .map(|r| QString::from(&r.role))
            .unwrap_or_default()
    }

    pub fn get_snippet(&self, index: i32) -> QString {
        if index < 0 {
            return QString::default();
        }
        let cache = crate::qt::search_results_cache();
        let results = cache.lock().unwrap();
        results
            .get(index as usize)
            .map(|r| QString::from(&r.snippet))
            .unwrap_or_default()
    }

    pub fn get_created_at(&self, index: i32) -> i64 {
        if index < 0 {
            return 0;
        }
        let cache = crate::qt::search_results_cache();
        let results = cache.lock().unwrap();
        results
            .get(index as usize)
            .map(|r| r.created_at)
            .unwrap_or(0)
    }
}

impl cxx_qt::Initialize for ffi::SearchResultModel {
    fn initialize(self: Pin<&mut Self>) {
        // Start with empty results
        self.clear();
    }
}
