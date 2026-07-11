#include "SnapshotService.h"

#include "BackupTemplate.h"
#include "PathCleanup.h"

#include <QDir>
#include <QDirIterator>
#include <QFile>
#include <QFileInfo>
#include <QJsonDocument>
#include <QJsonObject>
#include <QSaveFile>

#include <algorithm>

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

void SnapshotService::setUncompressedRecentVersions(const int keepCount) {
    m_uncompressedRecentVersions = std::clamp(
        keepCount,
        BackupTemplates::kMinUncompressedRecentVersions,
        BackupTemplates::kMaxUncompressedRecentVersions);
}

int SnapshotService::uncompressedRecentVersions() const {
    return m_uncompressedRecentVersions;
}

void SnapshotService::compactAllStagedCopies() {
    for (const QString& relativePath : m_metadataStore.snapshotPaths()) {
        compactStagedCopies(relativePath);
    }
}

bool SnapshotService::setProjectRoot(const QString& rootPath) {
    m_projectConfig.setRootPath(rootPath);

    if (!m_projectConfig.isReady()) {
        return false;
    }

    m_objectStore.setMusitRoot(m_projectConfig.musitPath());
    m_metadataStore.setMusitRoot(m_projectConfig.musitPath());

    const bool storageReady = m_objectStore.init();
    const bool metadataReady = m_metadataStore.init();
    if (!storageReady || !metadataReady) {
        return false;
    }

    loadBranchState();
    return true;
}

QString SnapshotService::branchStateFilePath() const {
    if (!m_projectConfig.isReady()) {
        return {};
    }
    return QDir(m_projectConfig.musitPath()).filePath("versions/branch-state.json");
}

void SnapshotService::loadBranchState() {
    m_branchBaseByArtifact.clear();

    QFile file(branchStateFilePath());
    if (!file.exists() || !file.open(QIODevice::ReadOnly)) {
        return;
    }

    const QJsonDocument doc = QJsonDocument::fromJson(file.readAll());
    if (!doc.isObject()) {
        return;
    }

    const QJsonObject obj = doc.object().value("branch_base_by_path").toObject();
    for (auto it = obj.constBegin(); it != obj.constEnd(); ++it) {
        const QString versionId = it.value().toString();
        if (!versionId.isEmpty()) {
            m_branchBaseByArtifact.insert(it.key(), versionId);
        }
    }
}

void SnapshotService::saveBranchState() const {
    const QString filePath = branchStateFilePath();
    if (filePath.isEmpty()) {
        return;
    }

    QJsonObject byArtifact;
    for (auto it = m_branchBaseByArtifact.constBegin(); it != m_branchBaseByArtifact.constEnd(); ++it) {
        byArtifact.insert(it.key(), it.value());
    }

    QJsonObject root;
    root.insert("branch_base_by_path", byArtifact);

    QSaveFile file(filePath);
    if (!file.open(QIODevice::WriteOnly | QIODevice::Truncate)) {
        return;
    }
    file.write(QJsonDocument(root).toJson(QJsonDocument::Compact));
    file.commit();
}

bool SnapshotService::createSnapshot(const FileEvent& event, bool isBaseline) {
    const QString label = isBaseline ? QStringLiteral("baseline ") : QString();

    if (!m_projectConfig.shouldTrack(event.relativePath)) {
        emit snapshotSkipped(QString("Skipped %1%2 (excluded)").arg(label, event.relativePath));
        return false;
    }

    const QFileInfo info(event.absolutePath);
    if (!info.exists() || !info.isFile()) {
        emit snapshotSkipped(QString("Skipped %1%2 (missing or not a file)").arg(label, event.relativePath));
        return false;
    }

    const QString stagedPath = m_objectStore.stageFile(event.absolutePath, event.relativePath);
    if (stagedPath.isEmpty()) {
        emit snapshotError(QString("Failed to stage %1%2").arg(label, event.relativePath));
        return false;
    }
    const auto discardStagedCopy = [this, &stagedPath]() {
        if (!QFile::remove(stagedPath)) {
            return;
        }
        const QString stagingRoot = QDir(m_projectConfig.musitPath()).filePath("staging");
        removeEmptyParentDirs(QFileInfo(stagedPath).dir().absolutePath(), stagingRoot);
    };

    const QString objectHash = m_objectStore.storeFile(event.absolutePath);
    if (objectHash.isEmpty()) {
        discardStagedCopy();
        emit snapshotError(QString("Failed to store %1object for %2").arg(label, event.relativePath));
        return false;
    }

    // Store the staged path relative to the project root so version history
    // survives moving or renaming the project folder.
    const QString relativeStagedPath =
        QDir(m_projectConfig.rootPath()).relativeFilePath(stagedPath);

    // Files of the same artifact stabilizing in the same watcher scan (or
    // the same synthetic batch) share one version id.
    const QString artifact = BackupTemplates::artifactForPath(event.relativePath);
    QString versionId;
    QString parentVersion;

    const auto groupIt = m_activeGroupByArtifact.constFind(artifact);
    if (event.scanSequence != 0
        && groupIt != m_activeGroupByArtifact.constEnd()
        && groupIt->scanSequence == event.scanSequence) {
        versionId = groupIt->versionId;
        parentVersion = groupIt->parentVersion;
    } else {
        const QString explicitBranchBase = m_branchBaseByArtifact.value(artifact);
        const QString latestStagedVersion = m_metadataStore.latestStagedVersionForArtifact(artifact);
        const QString branchBaseVersion = resolveBranchBaseVersion(explicitBranchBase, latestStagedVersion);
        if (!explicitBranchBase.isEmpty() && explicitBranchBase == latestStagedVersion) {
            m_branchBaseByArtifact.remove(artifact);
            saveBranchState();
        }

        versionId = m_metadataStore.nextVersionIdForArtifact(artifact, branchBaseVersion);
        parentVersion = branchBaseVersion;
        m_activeGroupByArtifact.insert(artifact, ActiveGroup {event.scanSequence, versionId, parentVersion});

        if (!isBaseline) {
            emit saveRecorded(versionId, event.relativePath);
        }
    }

    const bool appended = m_metadataStore.appendSnapshotEvent(
        event, artifact, objectHash, relativeStagedPath, versionId, parentVersion);
    if (!appended) {
        discardStagedCopy();
        emit snapshotError(QString("Failed to append %1metadata for %2").arg(label, event.relativePath));
        return false;
    }

    compactStagedCopies(event.relativePath);

    emit snapshotCreated(QString("%1 %2 -> %3 (v%4)")
        .arg(isBaseline ? QStringLiteral("Baseline snapshot") : QStringLiteral("Snapshot"),
             event.relativePath,
             objectHash.left(12),
             versionId));
    return true;
}

void SnapshotService::compactStagedCopies(const QString& relativePath) {
    // Only the newest versions keep a fast uncompressed staged copy; older
    // versions live on solely as compressed objects and are decompressed on
    // demand when restored.
    const QStringList oldStagedPaths = m_metadataStore.stagedPathsBeyondNewest(
        relativePath, m_uncompressedRecentVersions);
    if (oldStagedPaths.isEmpty()) {
        return;
    }

    const QDir rootDir(m_projectConfig.rootPath());
    const QString stagingRoot = QDir(m_projectConfig.musitPath()).filePath("staging");

    for (const QString& stagedPath : oldStagedPaths) {
        const QString absoluteStagedPath = QFileInfo(stagedPath).isAbsolute()
            ? stagedPath
            : rootDir.filePath(stagedPath);

        if (QFileInfo::exists(absoluteStagedPath) && QFile::remove(absoluteStagedPath)) {
            removeEmptyParentDirs(QFileInfo(absoluteStagedPath).dir().absolutePath(), stagingRoot);
        }
    }
}

bool SnapshotService::snapshotPathNow(const QString& absolutePath, const QString& relativePath) {
    const QFileInfo info(absolutePath);

    if (!info.isDir()) {
        FileEvent event;
        event.type = FileEvent::Type::Modified;
        event.absolutePath = absolutePath;
        event.relativePath = relativePath;
        event.scanSequence = --m_syntheticScanSequence;
        return createSnapshot(event, true);
    }

    // Directory bundle: snapshot every tracked file inside it as one
    // grouped version (negative sequences never collide with watcher ones).
    const qint64 batchSequence = --m_syntheticScanSequence;
    const QDir bundleDir(absolutePath);
    bool anySucceeded = false;

    QDirIterator it(absolutePath, QDir::Files | QDir::Hidden, QDirIterator::Subdirectories);
    while (it.hasNext()) {
        const QString fileAbsolutePath = it.next();
        const QString fileRelativePath =
            QDir(relativePath).filePath(bundleDir.relativeFilePath(fileAbsolutePath));

        if (!m_projectConfig.shouldTrack(fileRelativePath)) {
            continue;
        }

        FileEvent event;
        event.type = FileEvent::Type::Modified;
        event.absolutePath = fileAbsolutePath;
        event.relativePath = fileRelativePath;
        event.scanSequence = batchSequence;
        anySucceeded = createSnapshot(event, true) || anySucceeded;
    }

    return anySucceeded;
}

bool SnapshotService::hasVersionForArtifact(const QString& artifact) const {
    if (artifact.isEmpty()) {
        return false;
    }

    return !m_metadataStore.latestStagedVersionForArtifact(artifact).isEmpty();
}

void SnapshotService::suppressNextEventsForPath(const QString& relativePath, int count) {
    if (relativePath.isEmpty() || count <= 0) {
        return;
    }

    m_suppressedEventsByPath[relativePath] += count;
}

void SnapshotService::setBranchBaseForArtifact(const QString& artifact, const QString& versionId) {
    if (artifact.isEmpty()) {
        return;
    }

    if (versionId.isEmpty()) {
        m_branchBaseByArtifact.remove(artifact);
    } else {
        m_branchBaseByArtifact[artifact] = versionId;
    }
    saveBranchState();
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
        const QString artifact = BackupTemplates::artifactForPath(event.relativePath);
        const bool appended = m_metadataStore.appendSnapshotEvent(event, artifact, QString(), QString());
        if (appended) {
            emit snapshotCreated(QString("Recorded deletion: %1").arg(event.relativePath));
        } else {
            emit snapshotError(QString("Failed to record deletion: %1").arg(event.relativePath));
        }
        return;
    }

    createSnapshot(event, false);
}
