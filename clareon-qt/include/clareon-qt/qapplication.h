// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include <QApplication>

#include "clareon-qt/qicon.h"

namespace rust::clareon_qt {

void qapplicationSetWindowIcon(QApplication& app, const QIcon& icon);
void qapplicationSetDesktopFileName(QApplication& app, const QString& desktopFileName);

} // namespace rust::clareon_qt
