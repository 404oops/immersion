#include "MetadataStore.h"

#include <algorithm>
#include <QDateTime>
#include <QDir>
#include <QFile>
#include <QJsonDocument>
#include <QJsonObject>

namespace {

int topLevelVersion(const QString& versionId) {
    const QString firstSegment = versionId.section('.', 0, 0);
    bool ok = false;
    const int value = firstSegment.toInt(&ok);
    return ok ? value : -1;
}

int directChildVersion(const QString& versionId, const QString& branchBaseVersion) {
    if (branchBaseVersion.isEmpty()) {
        return -1;
    }

    const QString prefix = branchBaseVersion + ".";
    if (!versionId.startsWith(prefix)) {
        return -1;
    }

    const QString remainder = versionId.mid(prefix.size());
    if (remainder.contains('.')) {
        return -1;
    }

    bool ok = false;
    const int child = remainder.toInt(&ok);
    return ok ? child : -1;
}

QList<int> versionKey(const QString& versionId) {
    QList<int> key;
    const QStringList segments = versionId.split('.', Qt::SkipEmptyParts);
    for (const QString& segment : segments) {
        bool ok = false;
        const int value = segment.toInt(&ok);
        key.append(ok ? value : 0);
    }
    return key;
}

bool versionLessThan(const QString& left, const QString& right) {
    const QList<int> leftKey = versionKey(left);
    const QList<int> rightKey = versionKey(right);
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

} // namespace

MetadataStore::MetadataStore(QString musitRoot)
    : m_musitRoot(std::move(musitRoot)) {}

void MetadataStore::setMusitRoot(const QString& musitRoot) {
    m_musitRoot = musitRoot;
}

bool MetadataStore::init() {
    if (m_musitRoot.isEmpty()) {
        return false;
    }

    const QString versionsDir = QDir(m_musitRoot).filePath("versions");
    return QDir().mkpath(versionsDir);
}

QString MetadataStore::nextVersionIdForPath(const QString& relativePath, const QString& branchBaseVersion) const {
    const QString logPath = QDir(m_musitRoot).filePath("versions/log.jsonl");
    QFile logFile(logPath);

    int runningOrdinal = 0;
    int maxTopLevel = 0;
    int maxChildForBase = 0;

    if (logFile.exists() && logFile.open(QIODevice::ReadOnly | QIODevice::Text)) {
        while (!logFile.atEnd()) {
            const QByteArray rawLine = logFile.readLine().trimmed();
            if (rawLine.isEmpty()) {
                continue;
            }

            const QJsonDocument doc = QJsonDocument::fromJson(rawLine);
            if (!doc.isObject()) {
                continue;
            }

            const QJsonObject obj = doc.object();
            if (obj.value("path").toString() != relativePath) {
                continue;
            }

            const QString stagedPath = obj.value("staged").toString();
            if (stagedPath.isEmpty()) {
                continue;
            }

            ++runningOrdinal;
            QString versionId = obj.value("version").toString();
            if (versionId.isEmpty()) {
                versionId = QString::number(runningOrdinal);
            }

            const int topLevel = topLevelVersion(versionId);
            if (topLevel > maxTopLevel) {
                maxTopLevel = topLevel;
            }

            const int child = directChildVersion(versionId, branchBaseVersion);
            if (child > maxChildForBase) {
                maxChildForBase = child;
            }
        }
    }

    if (!branchBaseVersion.isEmpty()) {
        return QString("%1.%2").arg(branchBaseVersion).arg(maxChildForBase + 1);
    }

    return QString::number(maxTopLevel + 1);
}

bool MetadataStore::appendSnapshotEvent(
    const FileEvent& event,
    const QString& objectHash,
    const QString& stagedPath,
    const QString& branchBaseVersion) {
    if (m_musitRoot.isEmpty()) {
        return false;
    }

    const QString logPath = QDir(m_musitRoot).filePath("versions/log.jsonl");

    QFile logFile(logPath);
    if (!logFile.open(QIODevice::WriteOnly | QIODevice::Append | QIODevice::Text)) {
        return false;
    }

    QString typeValue = "modified";
    switch (event.type) {
    case FileEvent::Type::Created:
        typeValue = "created";
        break;
    case FileEvent::Type::Modified:
        typeValue = "modified";
        break;
    case FileEvent::Type::Deleted:
        typeValue = "deleted";
        break;
    case FileEvent::Type::Renamed:
        typeValue = "renamed";
        break;
    }

    QJsonObject line;
    line.insert("ts", QDateTime::currentDateTimeUtc().toString(Qt::ISODateWithMs));
    line.insert("type", typeValue);
    line.insert("path", event.relativePath);
    line.insert("object", objectHash);
    line.insert("staged", stagedPath);

    if (!stagedPath.isEmpty()) {
        const QString versionId = nextVersionIdForPath(event.relativePath, branchBaseVersion);
        if (!versionId.isEmpty()) {
            line.insert("version", versionId);
        }
        if (!branchBaseVersion.isEmpty()) {
            line.insert("parent", branchBaseVersion);
        }
    }

    const QByteArray encoded = QJsonDocument(line).toJson(QJsonDocument::Compact) + '\n';
    const qint64 written = logFile.write(encoded);

    return written == encoded.size();
}

QString MetadataStore::latestStagedVersionForPath(const QString& relativePath) const {
    if (m_musitRoot.isEmpty() || relativePath.isEmpty()) {
        return {};
    }

    const QString logPath = QDir(m_musitRoot).filePath("versions/log.jsonl");
    QFile logFile(logPath);
    if (!logFile.exists() || !logFile.open(QIODevice::ReadOnly | QIODevice::Text)) {
        return {};
    }

    int runningOrdinal = 0;
    QString latestVersionId;
    while (!logFile.atEnd()) {
        const QByteArray rawLine = logFile.readLine().trimmed();
        if (rawLine.isEmpty()) {
            continue;
        }

        const QJsonDocument doc = QJsonDocument::fromJson(rawLine);
        if (!doc.isObject()) {
            continue;
        }

        const QJsonObject obj = doc.object();
        if (obj.value("path").toString() != relativePath) {
            continue;
        }

        const QString stagedPath = obj.value("staged").toString();
        if (stagedPath.isEmpty()) {
            continue;
        }

        ++runningOrdinal;
        const QString candidateVersion = obj.value("version").toString(QString::number(runningOrdinal));
        if (latestVersionId.isEmpty() || versionLessThan(latestVersionId, candidateVersion)) {
            latestVersionId = candidateVersion;
        }
    }

    return latestVersionId;
}
