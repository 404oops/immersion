#include "SnapshotService.h"

#include <QFileInfo>

namespace {

QString resolveBranchBaseVersion(const QString& explicitBranchBase,
                                 const QString& latestStagedVersion) {
    QString branchBaseVersion = explicitBranchBase;
    if (branchBaseVersion.isEmpty()) {
        branchBaseVersion = latestStagedVersion;
    }

    // If we are at the latest version, continue normal top-level numbering.
    if (!branchBaseVersion.isEmpty() && !latestStagedVersion.isEmpty()
        && branchBaseVersion == latestStagedVersion) {
        branchBaseVersion.clear();
    }

    return branchBaseVersion;
}

} // namespace

SnapshotService::SnapshotService(QObject* parent)
    : QObject(parent) {}

bool SnapshotService::setProjectRoot(const QString& rootPath) {
    m_projectConfig.setRootPath(rootPath);

    if (!m_projectConfig.isReady()) {
        return false;
    }

    m_objectStore.setMusitRoot(m_projectConfig.musitPath());
    m_metadataStore.setMusitRoot(m_projectConfig.musitPath());

    const bool storageReady = m_objectStore.init();
    const bool metadataReady = m_metadataStore.init();

    return storageReady && metadataReady;
}

bool SnapshotService::snapshotFileNow(const QString& absolutePath, const QString& relativePath) {
    FileEvent event;
    event.type = FileEvent::Type::Modified;
    event.absolutePath = absolutePath;
    event.relativePath = relativePath;

    if (!m_projectConfig.shouldTrack(event.relativePath)) {
        emit snapshotSkipped(QString("Skipped baseline %1 (excluded)").arg(event.relativePath));
        return false;
    }

    const QFileInfo info(event.absolutePath);
    if (!info.exists() || !info.isFile()) {
        emit snapshotSkipped(QString("Skipped baseline %1 (missing or not a file)").arg(event.relativePath));
        return false;
    }

    const QString stagedPath = m_objectStore.stageFile(event.absolutePath, event.relativePath);
    if (stagedPath.isEmpty()) {
        emit snapshotError(QString("Failed to stage baseline %1").arg(event.relativePath));
        return false;
    }

    const QString objectHash = m_objectStore.storeFile(event.absolutePath);
    if (objectHash.isEmpty()) {
        emit snapshotError(QString("Failed to store baseline object for %1").arg(event.relativePath));
        return false;
    }

    const QString explicitBranchBase = m_branchBaseByPath.value(event.relativePath);
    const QString latestStagedVersion = m_metadataStore.latestStagedVersionForPath(event.relativePath);
    const QString branchBaseVersion = resolveBranchBaseVersion(explicitBranchBase, latestStagedVersion);
    if (explicitBranchBase == latestStagedVersion) {
        m_branchBaseByPath.remove(event.relativePath);
    }
    const bool appended = m_metadataStore.appendSnapshotEvent(event, objectHash, stagedPath, branchBaseVersion);
    if (!appended) {
        emit snapshotError(QString("Failed to append baseline metadata for %1").arg(event.relativePath));
        return false;
    }

    emit snapshotCreated(QString("Baseline snapshot %1 -> %2").arg(event.relativePath, objectHash.left(12)));
    return true;
}

bool SnapshotService::hasStagedVersionForPath(const QString& relativePath) const {
    if (relativePath.isEmpty()) {
        return false;
    }

    return !m_metadataStore.latestStagedVersionForPath(relativePath).isEmpty();
}

void SnapshotService::suppressNextEventsForPath(const QString& relativePath, int count) {
    if (relativePath.isEmpty() || count <= 0) {
        return;
    }

    m_suppressedEventsByPath[relativePath] += count;
}

void SnapshotService::setBranchBaseForPath(const QString& relativePath, const QString& versionId) {
    if (relativePath.isEmpty()) {
        return;
    }

    if (versionId.isEmpty()) {
        m_branchBaseByPath.remove(relativePath);
        return;
    }

    m_branchBaseByPath[relativePath] = versionId;
}

void SnapshotService::onFileEvent(const FileEvent& event) {
    if (event.type == FileEvent::Type::Created || event.type == FileEvent::Type::Modified) {
        const auto it = m_suppressedEventsByPath.constFind(event.relativePath);
        if (it != m_suppressedEventsByPath.constEnd() && it.value() > 0) {
            const int remaining = it.value() - 1;
            if (remaining > 0) {
                m_suppressedEventsByPath[event.relativePath] = remaining;
            } else {
                m_suppressedEventsByPath.remove(event.relativePath);
            }

            emit snapshotSkipped(QString("Suppressed self-triggered update: %1").arg(event.relativePath));
            return;
        }
    }

    if (!m_projectConfig.shouldTrack(event.relativePath)) {
        emit snapshotSkipped(QString("Skipped %1 (excluded)").arg(event.relativePath));
        return;
    }

    if (event.type == FileEvent::Type::Deleted) {
        const bool appended = m_metadataStore.appendSnapshotEvent(event, QString(), QString(), QString());
        if (appended) {
            emit snapshotCreated(QString("Recorded deletion: %1").arg(event.relativePath));
        } else {
            emit snapshotError(QString("Failed to record deletion: %1").arg(event.relativePath));
        }
        return;
    }

    const QFileInfo info(event.absolutePath);
    if (!info.exists() || !info.isFile()) {
        emit snapshotSkipped(QString("Skipped %1 (missing or not a file)").arg(event.relativePath));
        return;
    }

    const QString stagedPath = m_objectStore.stageFile(event.absolutePath, event.relativePath);
    if (stagedPath.isEmpty()) {
        emit snapshotError(QString("Failed to stage %1").arg(event.relativePath));
        return;
    }

    const QString objectHash = m_objectStore.storeFile(event.absolutePath);
    if (objectHash.isEmpty()) {
        emit snapshotError(QString("Failed to store object for %1").arg(event.relativePath));
        return;
    }

    const QString explicitBranchBase = m_branchBaseByPath.value(event.relativePath);
    const QString latestStagedVersion = m_metadataStore.latestStagedVersionForPath(event.relativePath);
    const QString branchBaseVersion = resolveBranchBaseVersion(explicitBranchBase, latestStagedVersion);
    if (explicitBranchBase == latestStagedVersion) {
        m_branchBaseByPath.remove(event.relativePath);
    }
    const bool appended = m_metadataStore.appendSnapshotEvent(event, objectHash, stagedPath, branchBaseVersion);
    if (!appended) {
        emit snapshotError(QString("Failed to append metadata for %1").arg(event.relativePath));
        return;
    }

    emit snapshotCreated(QString("Snapshot %1 -> %2").arg(event.relativePath, objectHash.left(12)));
}
