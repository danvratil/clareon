// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use cxx::{ExternType, type_id};
use std::mem::MaybeUninit;

/// QIcon provides scalable icons in different modes and states.
///
/// Qt Documentation: [QIcon](https://doc.qt.io/qt/qicon.html)
#[repr(C)]
pub struct QIcon {
    /// QIcon uses a d-pointer, so it's just one pointer (8 bytes on 64-bit)
    _space: MaybeUninit<usize>,
}

#[cxx::bridge]
mod ffi {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;

        include!("clareon-qt/qicon.h");
        type QIcon = super::QIcon;
    }

    #[namespace = "rust::clareon_qt"]
    unsafe extern "C++" {
        /// Creates a new QIcon
        #[rust_name = "qicon_default"]
        fn qiconDefault() -> QIcon;

        /// Adds a pixmap from file to this icon
        #[rust_name = "qicon_add_file"]
        fn qiconAddFile(icon: &mut QIcon, filename: &QString);
    }
}

impl QIcon {
    /// Creates a new empty QIcon
    pub fn new() -> Self {
        ffi::qicon_default()
    }

    /// Adds a pixmap from the given file to this icon
    pub fn add_file(&mut self, filename: &cxx_qt_lib::QString) {
        ffi::qicon_add_file(self, filename);
    }
}

impl Default for QIcon {
    fn default() -> Self {
        Self::new()
    }
}

// Safety:
//
// QIcon is safe to pass across FFI boundary using its copy constructor
// (similar to QString which also uses d-pointer/implicit sharing)
// Static checks on the C++ side ensure the size matches.
unsafe impl ExternType for QIcon {
    type Id = type_id!("QIcon");
    type Kind = cxx::kind::Trivial;
}
