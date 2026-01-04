// SPDX-FileCopyrightText: 2026 Daniel Vratil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

#include "message_list_model.hpp"
#include "clareon/src/message_list_model.cxxqt.h"

#include <qqml.h>

void qmlRegisterMessageList() {
    qmlRegisterType<MessageListModel>("cc.clareon.core", 1, 0, "MessageListModel");
}