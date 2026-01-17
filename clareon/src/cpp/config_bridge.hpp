// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include "config_generated.h"
#include "rust/cxx.h"

#include <QJsonDocument>
#include <QJsonObject>
#include <QString>
#include <QVariantMap>
#include <memory>

namespace config_bridge {

/// Create ConfigCpp from JSON string
/// Returns an empty ConfigCpp on error
inline std::unique_ptr<ConfigCpp> createConfigFromJson(rust::Str jsonString) {
    QString qstr = QString::fromUtf8(jsonString.data(), jsonString.size());
    QJsonDocument doc = QJsonDocument::fromJson(qstr.toUtf8());
    if (doc.isNull() || !doc.isObject()) {
        return std::make_unique<ConfigCpp>();  // Return default-constructed on error
    }

    QVariantMap map = doc.object().toVariantMap();
    return std::make_unique<ConfigCpp>(map);
}

/// Convert ConfigCpp to JSON string
inline rust::String configToJson(const ConfigCpp& config) {
    QVariantMap map = config.toVariantMap();
    QJsonObject obj = QJsonObject::fromVariantMap(map);
    QJsonDocument doc(obj);
    QByteArray json = doc.toJson(QJsonDocument::Compact);
    return rust::String(json.constData(), json.size());
}

} // namespace config_bridge
