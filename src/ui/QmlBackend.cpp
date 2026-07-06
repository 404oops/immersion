#include "QmlBackend.h"

#include "AppStrings.h"

#include <QCryptographicHash>
#include <QDateTime>
#include <QDesktopServices>
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QJsonDocument>
#include <QJsonObject>
#include <QSaveFile>
#include <QSet>
#include <QUrl>

#include <algorithm>

#include "../core/BackupTemplate.h"
#include "../core/FileWatcherFactory.h"
#include "../core/IFileWatcher.h"
#include "../core/PathCleanup.h"
#include "../core/SnapshotService.h"
#include "../core/VersionId.h"
#include "../persistence/MetadataStore.h"
#include "../persistence/ObjectStore.h"
#include "../persistence/ProjectRegistry.h"

namespace {

QString formatHumanDateTime(const QDateTime& dateTime) {
    if (!dateTime.isValid()) {
        return {};
    }
    return dateTime.toLocalTime().toString(QStringLiteral("yyyy-MM-dd HH:mm:ss"));
}

QString nowHuman() {
    return formatHumanDateTime(QDateTime::currentDateTime());
}

QString humanizeTimestamp(const QString& rawTimestamp) {
    if (rawTimestamp.isEmpty()) {
        return {};
    }

    QDateTime parsed = QDateTime::fromString(rawTimestamp, Qt::ISODateWithMs);
    if (!parsed.isValid()) {
        parsed = QDateTime::fromString(rawTimestamp, Qt::ISODate);
    }
    if (!parsed.isValid()) {
        return rawTimestamp;
    }

    return formatHumanDateTime(parsed);
}

QDateTime effectiveLastOpenedForProject(const DiscoveredProject& project,
                                        const QHash<QString, QDateTime>& lastOpenedByRoot) {
    const QDateTime explicitLastOpened = lastOpenedByRoot.value(pathKey(project.rootPath));
    if (explicitLastOpened.isValid()) {
        return explicitLastOpened;
    }

    if (project.primaryProjectFile.isEmpty()) {
        return {};
    }

    const QString projectFilePath = QDir(project.rootPath).filePath(project.primaryProjectFile);
    const QFileInfo projectFileInfo(projectFilePath);
    if (!projectFileInfo.exists()) {
        return {};
    }

    return projectFileInfo.lastModified();
}

QString platformName() {
#if defined(Q_OS_MAC)
    return AppStrings::PlatformMacOS;
#elif defined(Q_OS_WIN)
    return AppStrings::PlatformWindows;
#else
    return AppStrings::PlatformLinux;
#endif
}

QString versionLabelFromId(const QString& versionId) {
    if (versionId.isEmpty()) {
        return {};
    }

    if (versionId.contains('.')) {
        return QStringLiteral(".%1").arg(versionId.section('.', -1));
    }

    return QStringLiteral("v%1").arg(versionId);
}

bool filesAreIdentical(const QString& leftPath, const QString& rightPath) {
    QFile leftFile(leftPath);
    QFile rightFile(rightPath);
    if (!leftFile.open(QIODevice::ReadOnly) || !rightFile.open(QIODevice::ReadOnly)) {
        return false;
    }

    if (leftFile.size() != rightFile.size()) {
        return false;
    }

    constexpr qint64 chunkSize = 256 * 1024;
    while (!leftFile.atEnd()) {
        const QByteArray leftChunk = leftFile.read(chunkSize);
        const QByteArray rightChunk = rightFile.read(chunkSize);
        if (leftChunk != rightChunk) {
            return false;
        }
    }

    return true;
}

QString sha256FileHex(const QString& filePath) {
    QFile file(filePath);
    if (!file.open(QIODevice::ReadOnly)) {
        return {};
    }

    QCryptographicHash hash(QCryptographicHash::Sha256);
    constexpr qint64 chunkSize = 256 * 1024;
    while (!file.atEnd()) {
        const QByteArray chunk = file.read(chunkSize);
        if (chunk.isEmpty() && file.error() != QFile::NoError) {
            return {};
        }
        hash.addData(chunk);
    }

    return QString::fromLatin1(hash.result().toHex());
}

QString resolveStagedPath(const QString& projectRoot, const QString& stagedPath) {
    if (stagedPath.isEmpty()) {
        return {};
    }

    const QFileInfo stagedInfo(stagedPath);
    if (stagedInfo.isAbsolute()) {
        return stagedInfo.absoluteFilePath();
    }

    return QDir(projectRoot).filePath(stagedPath);
}

bool writeLinesAtomically(const QString& filePath, const QList<QByteArray>& lines) {
    QSaveFile file(filePath);
    if (!file.open(QIODevice::WriteOnly | QIODevice::Truncate | QIODevice::Text)) {
        return false;
    }

    for (const QByteArray& line : lines) {
        if (file.write(line) < 0) {
            return false;
        }
    }

    return file.commit();
}

} // namespace

QmlBackend::QmlBackend(QObject* parent)
    : QObject(parent)
    , m_projectDiscovery(std::make_unique<ProjectDiscovery>())
    , m_projectRegistry(std::make_unique<ProjectRegistry>()) {
    m_statusMessage = AppStrings::StatusSelectProjectsFolder;

    const QString savedProjectsFolder = m_projectRegistry->loadProjectsFolder();
    if (!savedProjectsFolder.isEmpty()) {
        loadProjectsFromFolder(savedProjectsFolder);
    }

    runStartupSelfCheck();
}

QmlBackend::~QmlBackend() = default;

QString QmlBackend::statusMessage() const {
    return m_statusMessage;
}

QVariantList QmlBackend::projects() const {
    return m_projects;
}

bool QmlBackend::hasDiscoveredProjects() const {
    return !m_discoveredProjects.isEmpty();
}

QStringList QmlBackend::activity() const {
    return m_activity;
}

QString QmlBackend::logLevel() const {
    return m_logLevel == LogLevel::Debug ? AppStrings::LogLevelDebug : AppStrings::LogLevelInfo;
}

void QmlBackend::setLogLevel(const QString& value) {
    const QString lowered = value.trimmed().toLower();
    const LogLevel nextLevel = lowered == AppStrings::LogLevelDebug.toLower() ? LogLevel::Debug : LogLevel::Info;
    if (nextLevel == m_logLevel) {
        return;
    }

    m_logLevel = nextLevel;
    emit logLevelChanged();
    rebuildVisibleActivity();
}

QString QmlBackend::searchText() const {
    return m_searchText;
}

void QmlBackend::setSearchText(const QString& value) {
    if (m_searchText == value) {
        return;
    }

    m_searchText = value;
    emit searchTextChanged();
    rebuildVisibleProjects();
}

QString QmlBackend::sortMode() const {
    return m_sortMode == SortMode::LastOpened ? AppStrings::SortLastOpened : AppStrings::SortName;
}

void QmlBackend::setSortMode(const QString& value) {
    const QString lowered = value.trimmed().toLower();
    const SortMode nextMode = lowered == AppStrings::SortLastOpened.toLower() ? SortMode::LastOpened : SortMode::Name;
    if (nextMode == m_sortMode) {
        return;
    }

    m_sortMode = nextMode;
    emit sortModeChanged();
    rebuildVisibleProjects();
}

int QmlBackend::selectedProjectIndex() const {
    return m_selectedProjectIndex;
}

void QmlBackend::setSelectedProjectIndex(int value) {
    // No early return on an unchanged index: the visible list can have been
    // refiltered/resorted, so the same index may point at a different project.
    applySelection(value);
    emit selectedProjectIndexChanged();
}

void QmlBackend::applySelection(int visibleIndex) {
    m_selectedProjectIndex = visibleIndex;
    if (visibleIndex >= 0 && visibleIndex < m_visibleProjectIndexes.size()) {
        const DiscoveredProject& project = m_discoveredProjects.at(m_visibleProjectIndexes.at(visibleIndex));
        m_projectRoot = project.rootPath;
        m_selectedProjectNote = m_projectNotes.value(project.rootPath);
        m_statusMessage = QString(AppStrings::StatusSelectedProjectFmt)
            .arg(project.name, ProjectDiscovery::kindToString(project.kind));
        emit statusMessageChanged();
    } else {
        m_projectRoot.clear();
        m_selectedProjectNote.clear();
    }

    emit selectedProjectNoteChanged();
    emit selectedProjectVersionGraphChanged();
}

QString QmlBackend::selectedProjectNote() const {
    return m_selectedProjectNote;
}

void QmlBackend::setSelectedProjectNote(const QString& value) {
    if (m_selectedProjectIndex < 0 || m_selectedProjectIndex >= m_visibleProjectIndexes.size()) {
        return;
    }

    const DiscoveredProject& project = m_discoveredProjects.at(m_visibleProjectIndexes.at(m_selectedProjectIndex));
    const QString trimmed = value.trimmed();
    if (m_projectNotes.value(project.rootPath) == trimmed) {
        return;
    }

    if (m_projectRegistry->saveProjectNote(project.rootPath, trimmed)) {
        m_projectNotes.insert(project.rootPath, trimmed);
        m_selectedProjectNote = trimmed;
        emit selectedProjectNoteChanged();
    }
}

void QmlBackend::loadProjectsFromFolder(const QString& folderPath) {
    if (folderPath.isEmpty()) {
        return;
    }
    QString cleanPath = folderPath;
    if (cleanPath.startsWith(QStringLiteral("file://"))) {
        cleanPath = QUrl(cleanPath).toLocalFile();
    }

    // A stale filter from a previous folder must not hide the new projects.
    if (!m_searchText.isEmpty()) {
        m_searchText.clear();
        emit searchTextChanged();
    }

    m_projectsFolderRoot = QDir::cleanPath(cleanPath);
    m_discoveredProjects = m_projectDiscovery->discoverAll(m_projectsFolderRoot);
    if (m_discoveredProjects.isEmpty()) {
        stopMonitoring();
        m_statusMessage = AppStrings::StatusNoSupportedProjectFiles;
        emit statusMessageChanged();
        appendActivity(QString(AppStrings::ActivityFirstDaySetupNoProjectsFmt)
            .arg(nowHuman(), cleanPath));
        rebuildVisibleProjects();
        return;
    }

    m_projectRegistry->saveProjectsFolder(cleanPath);
    m_projectNotes.clear();
    for (const DiscoveredProject& project : m_discoveredProjects) {
        m_projectRegistry->saveProject(project);
        m_projectNotes.insert(project.rootPath, m_projectRegistry->loadProjectNote(project.rootPath));
    }

    m_statusMessage = QString(AppStrings::StatusFirstTimeSetupFmt).arg(m_discoveredProjects.size());
    emit statusMessageChanged();

    appendActivity(QString(AppStrings::ActivityDiscoveredProjectsFmt)
        .arg(nowHuman())
        .arg(m_discoveredProjects.size())
        .arg(folderPath));

    rebuildVisibleProjects();
    startMonitoring();
}

void QmlBackend::openProject(int visibleIndex) {
    if (visibleIndex < 0 || visibleIndex >= m_visibleProjectIndexes.size()) {
        return;
    }

    setSelectedProjectIndex(visibleIndex);

    const DiscoveredProject& project = m_discoveredProjects.at(m_visibleProjectIndexes.at(visibleIndex));
    if (project.primaryProjectFile.isEmpty()) {
        appendActivity(QString(AppStrings::ActivityOpenFailedNoFileSelected)
            .arg(nowHuman()));
        return;
    }

    const QString projectFilePath = QDir(project.rootPath).filePath(project.primaryProjectFile);
    if (!QFileInfo::exists(projectFilePath)) {
        appendActivity(QString(AppStrings::ActivityOpenFailedMissingFileFmt)
            .arg(nowHuman(), projectFilePath));
        return;
    }

    const bool isBundle = BackupTemplates::kindIsBundle(project.kind);
    // Bundles (.logicx/.band) are directories; if the OS has no DAW
    // association, open the project folder instead of the bundle's parent.
    const QString fallbackPath = isBundle
        ? project.rootPath
        : QFileInfo(projectFilePath).absolutePath();

    bool opened = QDesktopServices::openUrl(QUrl::fromLocalFile(projectFilePath));
    if (!opened) {
        opened = QDesktopServices::openUrl(QUrl::fromLocalFile(fallbackPath));
        if (!opened) {
            appendActivity(QString(AppStrings::ActivityOpenFailedFmt)
                .arg(nowHuman(), projectFilePath));
            return;
        }

        m_lastOpenedAtByProjectRoot.insert(pathKey(project.rootPath), QDateTime::currentDateTime());
        if (m_sortMode == SortMode::LastOpened) {
            rebuildVisibleProjects();
        }

        appendActivity(QString(AppStrings::ActivityFileAssociationUnavailableOpenedFolderFmt)
            .arg(nowHuman(), fallbackPath));
        return;
    }

    m_lastOpenedAtByProjectRoot.insert(pathKey(project.rootPath), QDateTime::currentDateTime());
    appendActivity(QString(AppStrings::ActivityOpenedFmt)
        .arg(nowHuman(), projectFilePath));

    if (m_sortMode == SortMode::LastOpened) {
        rebuildVisibleProjects();
    }
}

void QmlBackend::manageProjectVersions(int visibleIndex) {
    if (visibleIndex < 0 || visibleIndex >= m_visibleProjectIndexes.size()) {
        return;
    }

    setSelectedProjectIndex(visibleIndex);
}

QVariantList QmlBackend::getProjectVersions(int visibleIndex) const {
    if (visibleIndex < 0 || visibleIndex >= m_visibleProjectIndexes.size()) {
        return {};
    }

    const DiscoveredProject& project = m_discoveredProjects.at(m_visibleProjectIndexes.at(visibleIndex));
    const QString logPath = QDir(project.rootPath).filePath(AppStrings::MusitVersionLogRelativePath);

    QVariantList versions;
    QFile logFile(logPath);
    if (!logFile.exists() || !logFile.open(QIODevice::ReadOnly | QIODevice::Text)) {
        return versions;
    }

    // One entry per version. Bundle saves write several log lines (one per
    // internal file) sharing a version id; they are folded into one entry
    // whose "files" list holds every file of that save.
    QHash<QString, int> versionIndexById;
    int legacyCount = 0;
    while (!logFile.atEnd()) {
        const QByteArray line = logFile.readLine().trimmed();
        if (line.isEmpty()) {
            continue;
        }

        const QJsonDocument doc = QJsonDocument::fromJson(line);
        if (!doc.isObject()) {
            continue;
        }

        const QJsonObject obj = doc.object();
        const QString stagedPath = obj.value("staged").toString();
        if (!artifactEquals(MetadataStore::artifactOfLogLine(obj), project.primaryProjectFile)
            || stagedPath.isEmpty()) {
            continue;
        }

        QString versionId = obj.value("version").toString();
        if (versionId.isEmpty()) {
            ++legacyCount;
            versionId = QString::number(legacyCount);
        }

        QVariantMap fileEntry;
        fileEntry.insert(QStringLiteral("path"), obj.value("path").toString());
        fileEntry.insert(QStringLiteral("stagedPath"), stagedPath);
        fileEntry.insert(QStringLiteral("objectHash"), obj.value("object").toString());

        const auto indexIt = versionIndexById.constFind(versionId);
        if (indexIt != versionIndexById.constEnd()) {
            QVariantMap version = versions.at(indexIt.value()).toMap();
            QVariantList files = version.value(QStringLiteral("files")).toList();
            files.append(fileEntry);
            version.insert(QStringLiteral("files"), files);
            if (version.value(QStringLiteral("note")).toString().isEmpty()) {
                version.insert(QStringLiteral("note"), obj.value("note").toString());
            }
            versions[indexIt.value()] = version;
            continue;
        }

        QVariantMap version;
        version.insert(QStringLiteral("id"), versionId);
        version.insert(QStringLiteral("label"), versionLabelFromId(versionId));
        version.insert(QStringLiteral("fullLabel"), QStringLiteral("v%1").arg(versionId));
        version.insert(QStringLiteral("timestamp"), humanizeTimestamp(obj.value("ts").toString()));
        version.insert(QStringLiteral("note"), obj.value("note").toString());
        version.insert(QStringLiteral("parent"), obj.value("parent").toString());
        version.insert(QStringLiteral("isCurrent"), false);
        version.insert(QStringLiteral("files"), QVariantList {fileEntry});
        versionIndexById.insert(versionId, versions.size());
        versions.append(version);
    }
    logFile.close();

    // A version is "current" when every file it captured matches what is on
    // disk right now (by object hash, or by content against the staged copy).
    QString currentVersionId;
    QHash<QString, QString> diskHashByPath;
    for (const QVariant& value : versions) {
        const QVariantMap version = value.toMap();
        const QString candidateVersionId = version.value(QStringLiteral("id")).toString();
        const QVariantList files = version.value(QStringLiteral("files")).toList();

        bool allMatch = !files.isEmpty();
        for (const QVariant& fileValue : files) {
            const QVariantMap fileEntry = fileValue.toMap();
            const QString diskPath = QDir(project.rootPath).filePath(
                fileEntry.value(QStringLiteral("path")).toString());
            if (!QFileInfo::exists(diskPath)) {
                allMatch = false;
                break;
            }

            auto hashIt = diskHashByPath.constFind(diskPath);
            if (hashIt == diskHashByPath.constEnd()) {
                hashIt = diskHashByPath.insert(diskPath, sha256FileHex(diskPath));
            }

            bool matches = false;
            const QString objectHash = fileEntry.value(QStringLiteral("objectHash")).toString();
            if (!hashIt->isEmpty() && !objectHash.isEmpty()) {
                matches = objectHash.compare(hashIt.value(), Qt::CaseInsensitive) == 0;
            }

            if (!matches) {
                const QString stagedPath = resolveStagedPath(
                    project.rootPath, fileEntry.value(QStringLiteral("stagedPath")).toString());
                if (!stagedPath.isEmpty() && QFileInfo::exists(stagedPath)) {
                    matches = filesAreIdentical(diskPath, stagedPath);
                }
            }

            if (!matches) {
                allMatch = false;
                break;
            }
        }

        if (allMatch
            && (currentVersionId.isEmpty() || VersionId::lessThan(currentVersionId, candidateVersionId))) {
            currentVersionId = candidateVersionId;
        }
    }

    // If the on-disk file matches no snapshot, no version is marked current.
    // Claiming the latest one is current would mislead the user into
    // believing their working state is already versioned.
    if (!currentVersionId.isEmpty()) {
        for (int i = 0; i < versions.size(); ++i) {
            QVariantMap version = versions.at(i).toMap();
            version.insert(QStringLiteral("isCurrent"),
                           version.value(QStringLiteral("id")).toString() == currentVersionId);
            versions[i] = version;
        }
    }

    return versions;
}

QVariantList QmlBackend::selectedProjectVersionGraph() const {
    if (m_selectedProjectIndex < 0 || m_selectedProjectIndex >= m_visibleProjectIndexes.size()) {
        return {};
    }

    const QVariantList versions = getProjectVersions(m_selectedProjectIndex);
    if (versions.isEmpty()) {
        return {};
    }

    QHash<QString, QVariantMap> nodeById;
    QHash<QString, QStringList> childrenByParent;

    QHash<QString, QString> parentOf;
    for (const QVariant& value : versions) {
        QVariantMap node = value.toMap();
        const QString id = node.value(QStringLiteral("id")).toString();
        if (id.isEmpty()) {
            continue;
        }
        nodeById.insert(id, node);
        parentOf.insert(id, node.value(QStringLiteral("parent")).toString());
    }

    for (auto it = parentOf.constBegin(); it != parentOf.constEnd(); ++it) {
        QString parentId = it.value();
        // Treat nodes whose parent no longer exists (e.g. deleted by older
        // app versions) as roots instead of silently dropping their subtree.
        if (!parentId.isEmpty() && !nodeById.contains(parentId)) {
            parentId.clear();
        }
        nodeById[it.key()].insert(QStringLiteral("parentId"), parentId);
        childrenByParent[parentId].append(it.key());
    }

    QVariantList graph;
    int row = 0;

    std::function<void(const QString&, int)> appendChildren = [&](const QString& parentId, int depth) {
        QStringList children = childrenByParent.value(parentId);
        std::sort(children.begin(), children.end(), VersionId::lessThan);
        for (const QString& childId : children) {
            QVariantMap node = nodeById.value(childId);
            node.insert(QStringLiteral("depth"), depth);
            node.insert(QStringLiteral("row"), row);
            node.insert(QStringLiteral("x"), 80 + depth * 160);
            node.insert(QStringLiteral("y"), 60 + row * 84);
            graph.append(node);
            ++row;
            appendChildren(childId, depth + 1);
        }
    };

    appendChildren({}, 0);
    return graph;
}

bool QmlBackend::restoreVersionById(const QString& versionId) {
    if (m_selectedProjectIndex < 0 || m_selectedProjectIndex >= m_visibleProjectIndexes.size() || versionId.isEmpty()) {
        return false;
    }

    const DiscoveredProject& project = m_discoveredProjects.at(m_visibleProjectIndexes.at(m_selectedProjectIndex));
    SnapshotService* snapshotService = snapshotServiceForRoot(project.rootPath);
    const QString artifactPath = QDir(project.rootPath).filePath(project.primaryProjectFile);

    // If the on-disk state matches no snapshot (e.g. the watcher has not
    // seen the latest save yet), snapshot it now so the restore cannot
    // silently destroy the user's most recent work.
    {
        const QVariantList preRestoreVersions = getProjectVersions(m_selectedProjectIndex);
        bool anyCurrent = false;
        for (const QVariant& value : preRestoreVersions) {
            if (value.toMap().value(QStringLiteral("isCurrent")).toBool()) {
                anyCurrent = true;
                break;
            }
        }
        if (!anyCurrent && snapshotService && QFileInfo::exists(artifactPath)) {
            snapshotService->snapshotPathNow(artifactPath, project.primaryProjectFile);
        }
    }

    const QVariantList versions = getProjectVersions(m_selectedProjectIndex);

    QVariantMap selectedVersion;
    for (const QVariant& value : versions) {
        const QVariantMap candidate = value.toMap();
        if (candidate.value(QStringLiteral("id")).toString() == versionId) {
            selectedVersion = candidate;
            break;
        }
    }

    if (selectedVersion.isEmpty()) {
        return false;
    }

    // A version may span several files (bundle internals saved together).
    // Materialize every file into a temp next to its destination first, so
    // nothing is touched unless the whole version is available; recent
    // versions come from staged copies, compacted ones are decompressed
    // from the object store.
    const QVariantList files = selectedVersion.value(QStringLiteral("files")).toList();
    const ObjectStore objectStore(QDir(project.rootPath).filePath(QStringLiteral(".musit")));

    struct PendingRestore {
        QString destinationPath;
        QString tempPath;
        QString relativePath;
    };
    QList<PendingRestore> pending;

    const auto cleanupTemps = [&pending]() {
        for (const PendingRestore& item : pending) {
            QFile::remove(item.tempPath);
        }
    };

    for (const QVariant& fileValue : files) {
        const QVariantMap fileEntry = fileValue.toMap();
        const QString relativePath = fileEntry.value(QStringLiteral("path")).toString();
        const QString destinationPath = QDir(project.rootPath).filePath(relativePath);

        if (!QDir().mkpath(QFileInfo(destinationPath).dir().absolutePath())) {
            cleanupTemps();
            appendActivity(QString(AppStrings::ActivityRestoreFailedCreateFolderFmt)
                .arg(nowHuman(), destinationPath));
            return false;
        }

        const QString tempPath = destinationPath + QStringLiteral(".musit-restore.tmp");
        QFile::remove(tempPath);

        const QString stagedPath = resolveStagedPath(
            project.rootPath, fileEntry.value(QStringLiteral("stagedPath")).toString());
        const QString objectHash = fileEntry.value(QStringLiteral("objectHash")).toString();

        bool materialized = false;
        if (!stagedPath.isEmpty() && QFileInfo::exists(stagedPath)) {
            materialized = QFile::copy(stagedPath, tempPath);
        }
        if (!materialized && !objectHash.isEmpty()) {
            materialized = objectStore.extractObject(objectHash, tempPath);
        }

        if (!materialized) {
            QFile::remove(tempPath);
            cleanupTemps();
            appendActivity(QString(AppStrings::ActivityRestoreFailedSnapshotMissingFmt)
                .arg(nowHuman(), project.name, versionId));
            return false;
        }

        pending.append(PendingRestore {destinationPath, tempPath, relativePath});
    }

    if (pending.isEmpty()) {
        return false;
    }

    // All temps are ready; swap them in.
    for (const PendingRestore& item : pending) {
        if (QFileInfo::exists(item.destinationPath) && !QFile::remove(item.destinationPath)) {
            cleanupTemps();
            appendActivity(QString(AppStrings::ActivityRestoreFailedReplaceFmt)
                .arg(nowHuman(), item.destinationPath));
            return false;
        }

        if (!QFile::rename(item.tempPath, item.destinationPath)) {
            // The complete data is still in the temp file; try a plain copy
            // as a last resort before giving up.
            if (!QFile::copy(item.tempPath, item.destinationPath)) {
                appendActivity(QString(AppStrings::ActivityRestoreFailedCopyFmt)
                    .arg(nowHuman(), item.tempPath, item.destinationPath));
                return false;
            }
            QFile::remove(item.tempPath);
        }
    }

    if (snapshotService) {
        for (const PendingRestore& item : pending) {
            snapshotService->suppressNextEventsForPath(item.relativePath, 1);
        }
        snapshotService->setBranchBaseForArtifact(project.primaryProjectFile, versionId);
    }

    m_statusMessage = QString(AppStrings::StatusRestoredToVersionFmt).arg(project.name, versionId);
    emit statusMessageChanged();
    appendActivity(QString(AppStrings::ActivityRestoredToVersionFmt)
        .arg(nowHuman(), project.name, versionId));

    openProject(m_selectedProjectIndex);
    emit selectedProjectVersionGraphChanged();
    return true;
}

bool QmlBackend::saveVersionNote(const QString& versionId, const QString& note) {
    if (m_selectedProjectIndex < 0 || m_selectedProjectIndex >= m_visibleProjectIndexes.size() || versionId.isEmpty()) {
        return false;
    }

    const DiscoveredProject& project = m_discoveredProjects.at(m_visibleProjectIndexes.at(m_selectedProjectIndex));
    const QString logPath = QDir(project.rootPath).filePath(AppStrings::MusitVersionLogRelativePath);

    QFile logFile(logPath);
    if (!logFile.exists() || !logFile.open(QIODevice::ReadOnly | QIODevice::Text)) {
        return false;
    }

    QList<QByteArray> lines;
    while (!logFile.atEnd()) {
        lines.append(logFile.readLine());
    }
    logFile.close();

    // The note lives on the first log line of the version (a bundle version
    // spans several lines; getProjectVersions reads the first non-empty note).
    bool updated = false;
    int count = 0;
    for (QByteArray& rawLine : lines) {
        const QByteArray trimmed = rawLine.trimmed();
        const QJsonDocument doc = QJsonDocument::fromJson(trimmed);
        if (!doc.isObject()) {
            continue;
        }

        QJsonObject obj = doc.object();
        const QString stagedPath = obj.value("staged").toString();
        if (!artifactEquals(MetadataStore::artifactOfLogLine(obj), project.primaryProjectFile)
            || stagedPath.isEmpty()) {
            continue;
        }

        ++count;
        const QString lineVersion = obj.value("version").toString(QString::number(count));
        if (lineVersion != versionId) {
            continue;
        }

        const QString trimmedNote = note.trimmed();
        if (trimmedNote.isEmpty()) {
            obj.remove("note");
        } else {
            obj.insert("note", trimmedNote);
        }
        
        if (!obj.contains("version")) {
            obj.insert("version", lineVersion);
        }

        rawLine = QJsonDocument(obj).toJson(QJsonDocument::Compact) + '\n';
        updated = true;
        break;
    }

    if (!updated) {
        return false;
    }

    if (!writeLinesAtomically(logPath, lines)) {
        return false;
    }

    emit selectedProjectVersionGraphChanged();
    return true;
}

bool QmlBackend::deleteVersionById(const QString& versionId) {
    if (m_selectedProjectIndex < 0 || m_selectedProjectIndex >= m_visibleProjectIndexes.size() || versionId.isEmpty()) {
        return false;
    }

    const DiscoveredProject& project = m_discoveredProjects.at(m_visibleProjectIndexes.at(m_selectedProjectIndex));
    const QString logPath = QDir(project.rootPath).filePath(AppStrings::MusitVersionLogRelativePath);

    QFile logFile(logPath);
    if (!logFile.exists() || !logFile.open(QIODevice::ReadOnly | QIODevice::Text)) {
        return false;
    }

    // First pass: find every line of the version (a bundle version spans one
    // line per internal file), plus its parent and object hashes.
    QList<QJsonObject> parsedLines;
    QList<bool> lineIsParsed;
    QList<QByteArray> originalLines;

    QStringList deletedStagedPaths;
    QSet<QString> deletedObjectHashes;
    QSet<int> deletedLineIndexes;
    QString deletedParentId;
    int count = 0;
    while (!logFile.atEnd()) {
        const QByteArray rawLine = logFile.readLine();
        originalLines.append(rawLine);
        const QJsonDocument doc = QJsonDocument::fromJson(rawLine.trimmed());
        const bool parsed = doc.isObject();
        lineIsParsed.append(parsed);
        parsedLines.append(parsed ? doc.object() : QJsonObject());

        if (!parsed) {
            continue;
        }

        const QJsonObject obj = doc.object();
        const QString objStagedPath = obj.value("staged").toString();
        if (artifactEquals(MetadataStore::artifactOfLogLine(obj), project.primaryProjectFile)
            && !objStagedPath.isEmpty()) {
            ++count;
            const QString lineVersion = obj.value("version").toString(QString::number(count));
            if (lineVersion == versionId) {
                deletedStagedPaths.append(resolveStagedPath(project.rootPath, objStagedPath));
                if (deletedParentId.isEmpty()) {
                    deletedParentId = obj.value("parent").toString();
                }
                const QString objectHash = obj.value("object").toString();
                if (!objectHash.isEmpty()) {
                    deletedObjectHashes.insert(objectHash.toLower());
                }
                deletedLineIndexes.insert(originalLines.size() - 1);
            }
        }
    }
    logFile.close();

    if (deletedLineIndexes.isEmpty()) {
        return false;
    }

    // Second pass: drop the deleted lines and reparent children of the
    // deleted version onto its own parent so their subtree stays reachable
    // in the version graph.
    QList<QByteArray> keptLines;
    QSet<QString> stillReferencedHashes;
    for (int i = 0; i < originalLines.size(); ++i) {
        if (deletedLineIndexes.contains(i)) {
            continue;
        }

        if (!lineIsParsed.at(i)) {
            keptLines.append(originalLines.at(i));
            continue;
        }

        QJsonObject obj = parsedLines.at(i);
        bool rewritten = false;
        if (artifactEquals(MetadataStore::artifactOfLogLine(obj), project.primaryProjectFile)
            && obj.value("parent").toString() == versionId) {
            if (deletedParentId.isEmpty()) {
                obj.remove("parent");
            } else {
                obj.insert("parent", deletedParentId);
            }
            rewritten = true;
        }

        const QString objectHash = obj.value("object").toString().toLower();
        if (deletedObjectHashes.contains(objectHash)) {
            stillReferencedHashes.insert(objectHash);
        }

        keptLines.append(rewritten
            ? QJsonDocument(obj).toJson(QJsonDocument::Compact) + '\n'
            : originalLines.at(i));
    }

    if (!writeLinesAtomically(logPath, keptLines)) {
        return false;
    }

    const QString stagedRoot = QDir(project.rootPath).filePath(AppStrings::MusitStagingRelativePath);
    for (const QString& stagedPath : deletedStagedPaths) {
        if (stagedPath.isEmpty()) {
            continue;
        }
        if (QFileInfo::exists(stagedPath)) {
            (void)QFile::remove(stagedPath);
        }
        removeEmptyParentDirs(QFileInfo(stagedPath).dir().absolutePath(), stagedRoot);
    }

    // Remove compressed objects no other version references (objects are
    // content-addressed and can be shared between versions).
    const ObjectStore objectStore(QDir(project.rootPath).filePath(QStringLiteral(".musit")));
    for (const QString& objectHash : deletedObjectHashes) {
        if (stillReferencedHashes.contains(objectHash)) {
            continue;
        }
        const QString objectPath = objectStore.objectPathForHash(objectHash);
        if (!objectPath.isEmpty() && QFileInfo::exists(objectPath)) {
            (void)QFile::remove(objectPath);
        }
    }

    appendActivity(QString(AppStrings::ActivityDeletedStagedVariationFmt)
        .arg(nowHuman(), versionId, project.name));
    emit selectedProjectVersionGraphChanged();
    return true;
}

void QmlBackend::appendActivityWithLevel(const QString& line, LogLevel level) {
    m_activityAll.append(line);
    m_activityLevels.append(level);
    rebuildVisibleActivity();
}

void QmlBackend::appendActivity(const QString& line) {
    appendActivityWithLevel(line, LogLevel::Info);
}

void QmlBackend::appendDebugActivity(const QString& line) {
    appendActivityWithLevel(line, LogLevel::Debug);
}

void QmlBackend::rebuildVisibleActivity() {
    QStringList filtered;
    filtered.reserve(m_activityAll.size());

    for (int i = 0; i < m_activityAll.size(); ++i) {
        const LogLevel level = i < m_activityLevels.size() ? m_activityLevels.at(i) : LogLevel::Info;
        if (m_logLevel == LogLevel::Info && level == LogLevel::Debug) {
            continue;
        }
        filtered.append(m_activityAll.at(i));
    }

    m_activity = filtered;
    emit activityChanged();
}

void QmlBackend::runStartupSelfCheck() {
    QStringList issues;
    QStringList checks;

    // Probe the same directory the registry actually writes to.
    const QString appDataDir = ProjectRegistry::appConfigDirectory();
    if (appDataDir.isEmpty()) {
        issues.append(AppStrings::DialogCouldNotResolveAppDataDir);
    } else {
        checks.append(QString(AppStrings::DialogConfigDirectoryFmt).arg(appDataDir));

        if (!QDir().mkpath(appDataDir)) {
            issues.append(QString(AppStrings::DialogCouldNotCreateConfigDirFmt).arg(appDataDir));
        } else {
            const QString probePath = QDir(appDataDir).filePath(QStringLiteral(".musit_write_probe"));
            QFile probe(probePath);
            const bool opened = probe.open(QIODevice::WriteOnly | QIODevice::Truncate);
            if (!opened) {
                issues.append(QString(AppStrings::DialogConfigDirectoryNotWritableFmt).arg(appDataDir));
            } else {
                probe.write("ok");
                probe.close();
                QFile::remove(probePath);
            }
        }
    }

    const QString savedFolder = m_projectRegistry->loadProjectsFolder();
    if (!savedFolder.isEmpty()) {
        checks.append(QString(AppStrings::DialogSavedProjectsFolderFmt).arg(savedFolder));

        const QFileInfo folderInfo(savedFolder);
        if (!folderInfo.exists() || !folderInfo.isDir()) {
            issues.append(QString(AppStrings::DialogSavedProjectsFolderMissingFmt).arg(savedFolder));
        } else {
            // Version storage lives under each project's .musit folder, not
            // at the top level of the folder the user picked. Verify the
            // projects folder itself is writable so per-project storage can
            // be created on first snapshot.
            const QString probePath = QDir(savedFolder).filePath(QStringLiteral(".musit_write_probe"));
            QFile probe(probePath);
            const bool opened = probe.open(QIODevice::WriteOnly | QIODevice::Truncate);
            if (!opened) {
                issues.append(QString(AppStrings::DialogMusitStorageNotWritableFmt).arg(savedFolder));
            } else {
                probe.write("ok");
                probe.close();
                QFile::remove(probePath);
            }
        }
    }

    if (issues.isEmpty()) {
        appendDebugActivity(QString(AppStrings::ActivityStartupSelfCheckPassedFmt)
            .arg(nowHuman(), platformName()));
        return;
    }

    appendDebugActivity(QString(AppStrings::ActivityStartupSelfCheckFoundIssuesFmt)
        .arg(nowHuman())
        .arg(issues.size())
        .arg(platformName()));

}

void QmlBackend::rebuildVisibleProjects() {
    const QString query = m_searchText.trimmed().toLower();

    QList<int> visible;
    visible.reserve(m_discoveredProjects.size());
    for (int i = 0; i < m_discoveredProjects.size(); ++i) {
        const DiscoveredProject& project = m_discoveredProjects.at(i);
        if (!query.isEmpty()) {
            const QString haystack = QStringLiteral("%1 %2 %3 %4")
                .arg(project.name, project.primaryProjectFile, project.rootPath, ProjectDiscovery::kindToString(project.kind))
                .toLower();
            if (!haystack.contains(query)) {
                continue;
            }
        }
        visible.append(i);
    }

    std::sort(visible.begin(), visible.end(), [this](int left, int right) {
        const DiscoveredProject& l = m_discoveredProjects.at(left);
        const DiscoveredProject& r = m_discoveredProjects.at(right);

        if (m_sortMode == SortMode::LastOpened) {
            const QDateTime lOpened = effectiveLastOpenedForProject(l, m_lastOpenedAtByProjectRoot);
            const QDateTime rOpened = effectiveLastOpenedForProject(r, m_lastOpenedAtByProjectRoot);
            if (lOpened.isValid() != rOpened.isValid()) {
                return lOpened.isValid();
            }
            if (lOpened.isValid() && rOpened.isValid() && lOpened != rOpened) {
                return lOpened > rOpened;
            }
        }

        const int byName = QString::compare(l.name, r.name, Qt::CaseInsensitive);
        if (byName != 0) {
            return byName < 0;
        }

        return QString::compare(l.rootPath, r.rootPath, Qt::CaseInsensitive) < 0;
    });

    m_visibleProjectIndexes = visible;

    // The selected index points into the visible list, so remap it whenever
    // that list changes; otherwise operations act on the wrong project.
    if (!m_projectRoot.isEmpty()) {
        int newIndex = -1;
        for (int i = 0; i < m_visibleProjectIndexes.size(); ++i) {
            if (pathEquals(m_discoveredProjects.at(m_visibleProjectIndexes.at(i)).rootPath, m_projectRoot)) {
                newIndex = i;
                break;
            }
        }
        if (newIndex != m_selectedProjectIndex) {
            m_selectedProjectIndex = newIndex;
            emit selectedProjectIndexChanged();
        }
    } else if (m_selectedProjectIndex != -1) {
        m_selectedProjectIndex = -1;
        emit selectedProjectIndexChanged();
    }

    QVariantList items;
    items.reserve(m_visibleProjectIndexes.size());
    for (const int index : m_visibleProjectIndexes) {
        const DiscoveredProject& project = m_discoveredProjects.at(index);
        const QDateTime effectiveLastOpened = effectiveLastOpenedForProject(project, m_lastOpenedAtByProjectRoot);
        const QFileInfo rootInfo(project.rootPath);
        const QString parentPath = rootInfo.dir().absolutePath();
        QVariantMap item;
        item.insert(QStringLiteral("name"), project.name);
        item.insert(QStringLiteral("type"), ProjectDiscovery::kindToString(project.kind));
        item.insert(QStringLiteral("path"), parentPath.isEmpty() ? project.rootPath : parentPath);
        item.insert(QStringLiteral("file"), project.primaryProjectFile);
        item.insert(QStringLiteral("lastOpened"), formatHumanDateTime(effectiveLastOpened));
        items.append(item);
    }

    m_projects = items;
    emit projectsChanged();
}

void QmlBackend::startMonitoring() {
    if (m_projectsFolderRoot.isEmpty() || m_discoveredProjects.isEmpty()) {
        return;
    }

    if (m_fileWatcher) {
        m_fileWatcher->stopWatching();
        m_fileWatcher.reset();
    }

    m_snapshotServiceByRoot.clear();
    m_canonicalRootByKey.clear();
    m_snapshotServices.clear();
    for (const DiscoveredProject& project : m_discoveredProjects) {
        auto snapshotService = std::make_unique<SnapshotService>();
        if (!snapshotService->setProjectRoot(project.rootPath)) {
            appendDebugActivity(QStringLiteral("[%1] failed to initialize versioning for %2")
                .arg(nowHuman(), project.rootPath));
            continue;
        }

        connect(snapshotService.get(), &SnapshotService::snapshotCreated, this,
                [this, projectRoot = project.rootPath](const QString& message) {
                    appendActivity(QString("[%1] %2").arg(nowHuman(), message));

                    if (pathEquals(projectRoot, m_projectRoot)) {
                        emit selectedProjectVersionGraphChanged();
                    }
                });

        connect(snapshotService.get(), &SnapshotService::snapshotSkipped, this,
                [this](const QString& reason) {
                    appendDebugActivity(QString("[%1] %2").arg(nowHuman(), reason));
                });

        connect(snapshotService.get(), &SnapshotService::snapshotError, this,
                [this](const QString& error) {
                    appendActivity(QString("[%1] %2").arg(nowHuman(), error));
                });

        SnapshotService* rawService = snapshotService.get();
        const QString rootKey = pathKey(project.rootPath);
        m_snapshotServiceByRoot.insert(rootKey, rawService);
        m_canonicalRootByKey.insert(rootKey, project.rootPath);
        m_snapshotServices.emplace_back(std::move(snapshotService));

        const QString primaryFile = project.primaryProjectFile;
        if (!primaryFile.isEmpty() && !rawService->hasVersionForArtifact(primaryFile)) {
            const QString absolutePrimaryPath = QDir(project.rootPath).filePath(primaryFile);
            const bool seeded = rawService->snapshotPathNow(absolutePrimaryPath, primaryFile);
            if (seeded) {
                appendDebugActivity(QStringLiteral("[%1] seeded initial version v1 for %2")
                    .arg(nowHuman(), absolutePrimaryPath));
            }
        }
    }

    if (m_snapshotServiceByRoot.isEmpty()) {
        m_statusMessage = AppStrings::StatusFailedInitializeVersioning;
        emit statusMessageChanged();
        return;
    }

    m_fileWatcher = createFileWatcher();
    if (!m_fileWatcher) {
        m_statusMessage = AppStrings::StatusFailedCreateWatcher;
        emit statusMessageChanged();
        m_snapshotServiceByRoot.clear();
        m_canonicalRootByKey.clear();
        m_snapshotServices.clear();
        return;
    }

    connect(m_fileWatcher.get(), &IFileWatcher::fileEvent,
            this, [this](const FileEvent& event) {
                const QString normalizedAbsolutePath = normalizeAbsolutePath(event.absolutePath);

                QString matchedRootKey;
                int matchedLength = -1;
                for (auto it = m_snapshotServiceByRoot.constBegin(); it != m_snapshotServiceByRoot.constEnd(); ++it) {
                    const QString rootKey = it.key();
                    const QString canonicalRoot = m_canonicalRootByKey.value(rootKey);
                    if (canonicalRoot.isEmpty() || !pathIsUnderRoot(normalizedAbsolutePath, canonicalRoot)) {
                        continue;
                    }

                    if (canonicalRoot.size() > matchedLength) {
                        matchedRootKey = rootKey;
                        matchedLength = canonicalRoot.size();
                    }
                }

                if (matchedRootKey.isEmpty()) {
                    return;
                }

                auto it = m_snapshotServiceByRoot.find(matchedRootKey);
                if (it == m_snapshotServiceByRoot.end() || !it.value()) {
                    return;
                }

                const QString canonicalRoot = m_canonicalRootByKey.value(matchedRootKey);
                if (canonicalRoot.isEmpty()) {
                    return;
                }

                FileEvent translatedEvent = event;
                translatedEvent.relativePath = QDir(canonicalRoot).relativeFilePath(normalizedAbsolutePath);
                it.value()->onFileEvent(translatedEvent);
            });

    if (!m_fileWatcher->startWatching(m_projectsFolderRoot)) {
        m_statusMessage = AppStrings::StatusFailedStartWatcher;
        emit statusMessageChanged();
        m_fileWatcher.reset();
        m_snapshotServiceByRoot.clear();
        m_canonicalRootByKey.clear();
        m_snapshotServices.clear();
        return;
    }

    m_statusMessage = QString(AppStrings::StatusMonitoringFmt).arg(m_projectsFolderRoot);
    emit statusMessageChanged();

    appendActivity(QStringLiteral("[%1] monitoring started for %2")
        .arg(QDateTime::currentDateTime().toString(Qt::ISODate), m_projectsFolderRoot));
}

void QmlBackend::stopMonitoring() {
    if (m_fileWatcher) {
        m_fileWatcher->stopWatching();
        m_fileWatcher.reset();
    }

    m_snapshotServiceByRoot.clear();
    m_canonicalRootByKey.clear();
    m_snapshotServices.clear();
    appendActivity(QStringLiteral("[%1] monitoring stopped")
        .arg(QDateTime::currentDateTime().toString(Qt::ISODate)));
}

SnapshotService* QmlBackend::snapshotServiceForRoot(const QString& projectRoot) {
    auto it = m_snapshotServiceByRoot.find(pathKey(projectRoot));
    if (it == m_snapshotServiceByRoot.end()) {
        return nullptr;
    }
    return it.value();
}
