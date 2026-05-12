// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

#include "clareon-qt/kiconthemes.h"

#include <KIconTheme>

namespace rust::clareon_qt {

void kiconthemeInitTheme() {
    KIconTheme::initTheme();
}

} // namespace rust::clareon_qt
