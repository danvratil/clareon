// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

#[cxx::bridge]
mod ffi {
    #[namespace = "rust::clareon_qt"]
    unsafe extern "C++" {
        include!("clareon-qt/kiconthemes.h");

        /// Sets the default window icon
        #[rust_name = "kicontheme_init_theme"]
        fn kiconthemeInitTheme();
    }
}

pub struct KIconTheme;

impl KIconTheme {
    /// Initializes the KIconTheme integration
    pub fn init_theme() {
        ffi::kicontheme_init_theme();
    }
}
