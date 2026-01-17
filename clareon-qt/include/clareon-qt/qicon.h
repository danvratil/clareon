// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include <QtGui/QIcon>

#include "rust/cxx.h"

namespace rust {

template<> struct IsRelocatable<QIcon> : ::std::true_type {};

} // namespace rust

namespace rust::clareon_qt {

QIcon qiconDefault();
void qiconAddFile(QIcon& icon, const QString& filename);

} // namespace rust::clareon_qt
