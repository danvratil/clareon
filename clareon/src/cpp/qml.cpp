// SPDX-FileCopyrightText: 2026 Daniel Vratil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

#include "qml.hpp"
#include "clareon/src/message_list_model.cxxqt.h"
#include "clareon/src/search_result_model.cxxqt.h"

#include <qqml.h>

void registerClareonQmlTypes() {
    qmlRegisterType<MessageListModel>("cc.clareon.core", 1, 0, "MessageListModel");
    qmlRegisterType<SearchResultModel>("cc.clareon.core", 1, 0, "SearchResultModel");
}
