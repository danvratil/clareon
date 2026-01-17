// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use core::pin::Pin;

#[cxx::bridge]
mod ffi {
    unsafe extern "C++" {
        include!("cxx-qt-lib-extras/gui/qapplication.h");
        type QApplication = cxx_qt_lib_extras::QApplication;

        include!("clareon-qt/qicon.h");
        type QIcon = crate::qicon::QIcon;
    }

    #[namespace = "rust::clareon_qt"]
    unsafe extern "C++" {
        include!("clareon-qt/qapplication.h");

        /// Sets the default window icon
        #[rust_name = "qapplication_set_window_icon"]
        fn qapplicationSetWindowIcon(app: Pin<&mut QApplication>, icon: &QIcon);
    }
}

/// Extension trait for QApplication to add methods not in cxx-qt-lib-extras
pub trait QApplicationExt {
    /// Sets the default window icon for the application
    fn set_window_icon(self: Pin<&mut Self>, icon: &crate::QIcon);
}

impl QApplicationExt for cxx_qt_lib_extras::QApplication {
    fn set_window_icon(self: Pin<&mut Self>, icon: &crate::QIcon) {
        ffi::qapplication_set_window_icon(self, icon);
    }
}
