// SPDX-FileCopyrightText: 2025 Daniel Vratil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

#include "logging.hpp"
#include "clareon/src/logging.cxx.h"
#include <QtLogging>

void installMessageHandler() {
    qInstallMessageHandler(tracingMessageHandler);
}