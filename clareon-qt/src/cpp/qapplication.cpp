// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

#include "clareon-qt/qapplication.h"

namespace rust::clareon_qt {

void qapplicationSetWindowIcon(QApplication& app, const QIcon& icon) {
    app.setWindowIcon(icon);
}

} // namespace rust::clareon_qt