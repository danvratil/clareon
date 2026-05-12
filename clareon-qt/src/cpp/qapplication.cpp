// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

#include "clareon-qt/qapplication.h"

namespace rust::clareon_qt {

void qapplicationSetWindowIcon(QApplication& app, const QIcon& icon) {
    app.setWindowIcon(icon);
}

void qapplicationSetDesktopFileName(QApplication& app, const QString& desktopFileName) {
    app.setDesktopFileName(desktopFileName);
}

void qapplicationSetStyle(QApplication& app, const QString& style) {
    app.setStyle(style);
}

} // namespace rust::clareon_qt
