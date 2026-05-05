// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Shared utilities for QAbstractListModel implementations.

use std::sync::{Arc, Mutex};

use cxx_qt_lib::{QByteArray, QHash, QHashPair_i32_QByteArray, QModelIndex};
use tokio::task::JoinHandle;

/// Drop-safe async task handle. Cancels the background task when dropped.
pub struct Subscription(Arc<Mutex<Option<JoinHandle<()>>>>);

impl Subscription {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(None)))
    }

    pub fn start(&self, task: JoinHandle<()>) {
        *self.0.lock().expect("subscription lock poisoned") = Some(task);
    }

    pub fn cancel(&self) {
        if let Some(task) = self.0.lock().expect("subscription lock poisoned").take() {
            task.abort();
        }
    }

    /// Returns a clone of the inner Arc for storing in model Rust structs that
    /// need to pass it into an async closure (where `self` can't be moved).
    pub fn inner(&self) -> Arc<Mutex<Option<JoinHandle<()>>>> {
        self.0.clone()
    }
}

impl Default for Subscription {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        self.cancel();
    }
}

/// Build a `QHash<i32, QByteArray>` role-name map from a slice of `(role_int, name)` pairs.
pub fn make_role_names(pairs: &[(i32, &str)]) -> QHash<QHashPair_i32_QByteArray> {
    let mut roles = QHash::default();
    for &(id, name) in pairs {
        roles.insert(id, QByteArray::from(name));
    }
    roles
}

/// Bounds-checked item access for `data()` implementations.
///
/// Returns `None` when the row is out of range, which callers should map to
/// `QVariant::default()`.
pub fn get_item<'a, T>(items: &'a [T], index: &QModelIndex) -> Option<&'a T> {
    let row = index.row();
    if row < 0 {
        return None;
    }
    items.get(row as usize)
}
