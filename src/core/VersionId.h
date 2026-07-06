#pragma once

#include <QList>
#include <QString>
#include <QStringList>

#include <algorithm>

// Shared helpers for comparing dotted version ids like "2", "2.1", "2.1.3".
namespace VersionId {

inline QList<int> key(const QString& versionId) {
    QList<int> key;
    const QStringList segments = versionId.split('.', Qt::SkipEmptyParts);
    for (const QString& segment : segments) {
        bool ok = false;
        const int value = segment.toInt(&ok);
        key.append(ok ? value : 0);
    }
    return key;
}

inline bool lessThan(const QString& left, const QString& right) {
    const QList<int> leftKey = key(left);
    const QList<int> rightKey = key(right);
    const int maxSize = std::max(leftKey.size(), rightKey.size());
    for (int i = 0; i < maxSize; ++i) {
        const int lv = i < leftKey.size() ? leftKey.at(i) : -1;
        const int rv = i < rightKey.size() ? rightKey.at(i) : -1;
        if (lv == rv) {
            continue;
        }
        return lv < rv;
    }
    return left < right;
}

} // namespace VersionId
