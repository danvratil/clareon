// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

#include "clareon-qt/qicon.h"

#include <cstdint>

namespace rust::clareon_qt {

QIcon qiconDefault() {
    return QIcon();
}

void qiconAddFile(QIcon& icon, const QString& filename) {
    icon.addFile(filename);
}

// Verify that QIcon has the same size as declared on Rust side
static_assert(sizeof(QIcon) == sizeof(::std::size_t),
              "QIcon size must match Rust declaration (one pointer)");

// Verify that QIcon can be relocated
static_assert(QTypeInfo<QIcon>::isRelocatable);

} // namespace rust::clareon_qt
