// SPDX-FileCopyrightText: 2026 Daniel Vratil <me@dvratil.cz>
// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

#include "qml.hpp"
#include "clareon/src/artifact_list_model.cxxqt.h"
#include "clareon/src/conversation_list_model.cxxqt.h"
#include "clareon/src/message_list_model.cxxqt.h"
#include "clareon/src/model_list_model.cxxqt.h"
#include "clareon/src/search_result_model.cxxqt.h"
#include "clareon/config_generated.h"

#include <qqml.h>
#include <QtQml/qqmldebug.h>

#include <QStringList>

bool enableQmlDebugger(int port)
{
#if QT_CONFIG(qml_debug)
    // Programmatic enable (avoids needing QT_QML_DEBUG compile define and
    // avoids clap consuming the -qmljsdebugger CLI flag before Qt sees it).
    QQmlDebuggingEnabler::enableDebugging(true);

    // Services expected by QMLMCP: DebugMessages, QmlDebugger, QmlInspector
    QStringList services;
    services << QStringLiteral("DebugMessages")
             << QStringLiteral("QmlDebugger")
             << QStringLiteral("QmlInspector");
    QQmlDebuggingEnabler::setServices(services);

    return QQmlDebuggingEnabler::startTcpDebugServer(
        port, QQmlDebuggingEnabler::DoNotWaitForClient);
#else
    Q_UNUSED(port);
    return false;
#endif
}

void registerClareonQmlTypes() {
    qmlRegisterType<ArtifactListModel>("cc.clareon.core", 1, 0, "ArtifactListModel");
    qmlRegisterType<ConversationListModel>("cc.clareon.core", 1, 0, "ConversationListModel");
    qmlRegisterType<MessageListModel>("cc.clareon.core", 1, 0, "MessageListModel");
    qmlRegisterType<ModelListModel>("cc.clareon.core", 1, 0, "ModelListModel");
    qmlRegisterType<SearchResultModel>("cc.clareon.core", 1, 0, "SearchResultModel");
    qmlRegisterUncreatableType<ConfigCpp>("cc.clareon.core", 1, 0, "ConfigCpp",
                                      QStringLiteral("ConfigCpp is not creatable from QML"));
    qmlRegisterUncreatableType<ProvidersConfigCpp>("cc.clareon.core", 1, 0, "ProvidersConfigCpp",
                                      QStringLiteral("ProvidersConfigCpp is not creatable from QML"));
    qmlRegisterUncreatableType<AnthropicConfigCpp>("cc.clareon.core", 1, 0, "AnthropicConfigCpp",
                                      QStringLiteral("AnthropicConfigCpp is not creatable from QML"));
    qmlRegisterUncreatableType<BedrockConfigCpp>("cc.clareon.core", 1, 0, "BedrockConfigCpp",
                                      QStringLiteral("BedrockConfigCpp is not creatable from QML"));
    qmlRegisterUncreatableType<OpenAiBackendConfigCpp>("cc.clareon.core", 1, 0, "OpenAiBackendConfigCpp",
                                      QStringLiteral("OpenAiBackendConfigCpp is not creatable from QML"));
    qmlRegisterUncreatableType<ToolsConfigCpp>("cc.clareon.core", 1, 0, "ToolsConfigCpp",
                                      QStringLiteral("ToolsConfigCpp is not creatable from QML"));
    qmlRegisterUncreatableType<McpConfigCpp>("cc.clareon.core", 1, 0, "McpConfigCpp",
                                      QStringLiteral("McpConfigCpp is not creatable from QML"));
    qmlRegisterUncreatableType<McpServerConfigCpp>("cc.clareon.core", 1, 0, "McpServerConfigCpp",
                                      QStringLiteral("McpServerConfigCpp is not creatable from QML"));
    qmlRegisterUncreatableType<SystemPromptConfigCpp>("cc.clareon.core", 1, 0, "SystemPromptConfigCpp",
                                      QStringLiteral("SystemPromptConfigCpp is not creatable from QML"));
}
