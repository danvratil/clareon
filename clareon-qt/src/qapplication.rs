// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use core::pin::Pin;

use cxx_qt_lib::QString;

#[cxx::bridge]
mod ffi {
    unsafe extern "C++" {
        include!("cxx-qt-lib-extras/gui/qapplication.h");
        type QApplication = cxx_qt_lib_extras::QApplication;
        include!("cxx-qt-lib/core/qstring.h");
        type QString = cxx_qt_lib::QString;

        include!("clareon-qt/qicon.h");
        type QIcon = crate::qicon::QIcon;
    }

    #[namespace = "rust::clareon_qt"]
    unsafe extern "C++" {
        include!("clareon-qt/qapplication.h");

        /// Sets the default window icon
        #[rust_name = "qapplication_set_window_icon"]
        fn qapplicationSetWindowIcon(app: Pin<&mut QApplication>, icon: &QIcon);

        /// Sets the desktop file name for the application
        #[rust_name = "qapplication_set_desktop_file_name"]
        fn qapplicationSetDesktopFileName(app: Pin<&mut QApplication>, desktopFileName: &QString);

        /// Sets the application style to use
        #[rust_name = "qapplication_set_style"]
        fn qapplicationSetStyle(app: Pin<&mut QApplication>, style: &QString);
    }
}

/// Extension trait for QApplication to add methods not in cxx-qt-lib-extras
pub trait QApplicationExt {
    /// Sets the default window icon for the application
    fn set_window_icon(self: Pin<&mut Self>, icon: &crate::QIcon);

    /// Sets the desktop file name for the application
    fn set_desktop_file_name(self: Pin<&mut Self>, desktop_file_name: &QString);

    /// Sets the application style to use
    fn set_style(self: Pin<&mut Self>, style: &QString);
}

impl QApplicationExt for cxx_qt_lib_extras::QApplication {
    fn set_window_icon(self: Pin<&mut Self>, icon: &crate::QIcon) {
        ffi::qapplication_set_window_icon(self, icon);
    }

    fn set_desktop_file_name(self: Pin<&mut Self>, desktop_file_name: &QString) {
        ffi::qapplication_set_desktop_file_name(self, desktop_file_name);
    }

    fn set_style(self: Pin<&mut Self>, style: &QString) {
        ffi::qapplication_set_style(self, style);
    }
}
