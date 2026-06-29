// SPDX-FileCopyrightText: 2026 Daniel Vratil <me@dvratil.cz>
// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

void registerClareonQmlTypes();

/// Enable the QML/JS debug server so tools like QMLMCP can attach.
/// Must be called before creating any QQmlEngine / QQmlApplicationEngine.
/// Returns true if the TCP debug server started successfully.
bool enableQmlDebugger(int port);