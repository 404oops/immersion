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
#include <QStandardPaths>
#include <QUrl>

#include <algorithm>

#include "../core/FileWatcherFactory.h"
#include "../core/IFileWatcher.h"
#include "../core/SnapshotService.h"
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
    const QDateTime explicitLastOpened = lastOpenedByRoot.value(project.rootPath);
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

void removeEmptyParentDirs(const QString& startDirPath, const QString& stopDirPath) {
    const QString stop = QDir(stopDirPath).absolutePath();
    QString current = QDir(startDirPath).absolutePath();
    while (!current.isEmpty() && current.startsWith(stop)) {
        QDir dir(current);
        if (!dir.exists()) {
            break;
        }

        const QStringList entries = dir.entryList(QDir::NoDotAndDotDot | QDir::AllEntries);
        if (!entries.isEmpty()) {
            break;
        }

        const QString parent = QFileInfo(current).dir().absolutePath();
        QDir().rmdir(current);

        if (current == stop) {
            break;
        }
        current = parent;
    }
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
    if (m_selectedProjectIndex == value) {
        return;
    }

    m_selectedProjectIndex = value;
    if (value >= 0 && value < m_visibleProjectIndexes.size()) {
        const DiscoveredProject& project = m_discoveredProjects.at(m_visibleProjectIndexes.at(value));
        m_projectRoot = project.rootPath;
        m_selectedProjectNote = m_projectNotes.value(project.rootPath);
        m_statusMessage = QString(AppStrings::StatusSelectedProjectFmt)
            .arg(project.name, ProjectDiscovery::kindToString(project.kind));
        emit statusMessageChanged();
        emit selectedProjectNoteChanged();
        emit selectedProjectVersionGraphChanged();
    }

    emit selectedProjectIndexChanged();
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

    bool opened = QDesktopServices::openUrl(QUrl::fromLocalFile(projectFilePath));
    if (!opened) {
        const QString folderPath = QFileInfo(projectFilePath).absolutePath();
        opened = QDesktopServices::openUrl(QUrl::fromLocalFile(folderPath));
        if (!opened) {
            appendActivity(QString(AppStrings::ActivityOpenFailedFmt)
                .arg(nowHuman(), projectFilePath));
            return;
        }

        m_lastOpenedAtByProjectRoot.insert(project.rootPath, QDateTime::currentDateTime());
        if (m_sortMode == SortMode::LastOpened) {
            rebuildVisibleProjects();
        }

        appendActivity(QString(AppStrings::ActivityFileAssociationUnavailableOpenedFolderFmt)
            .arg(nowHuman(), folderPath));
        return;
    }

    m_lastOpenedAtByProjectRoot.insert(project.rootPath, QDateTime::currentDateTime());
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

    int count = 0;
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
        const QString relPath = obj.value("path").toString();
        const QString stagedPath = obj.value("staged").toString();
        if (relPath != project.primaryProjectFile || stagedPath.isEmpty()) {
            continue;
        }

        ++count;
        const QString versionId = obj.value("version").toString(QString::number(count));
        QVariantMap version;
        version.insert(QStringLiteral("id"), versionId);
        version.insert(QStringLiteral("label"), versionLabelFromId(versionId));
        version.insert(QStringLiteral("fullLabel"), QStringLiteral("v%1").arg(versionId));
        version.insert(QStringLiteral("timestamp"), humanizeTimestamp(obj.value("ts").toString()));
        version.insert(QStringLiteral("note"), obj.value("note").toString());
        version.insert(QStringLiteral("stagedPath"), stagedPath);
        version.insert(QStringLiteral("objectHash"), obj.value("object").toString());
        version.insert(QStringLiteral("parent"), obj.value("parent").toString());
        version.insert(QStringLiteral("isCurrent"), false);
        versions.append(version);
    }
    logFile.close();

    const QString projectFilePath = QDir(project.rootPath).filePath(project.primaryProjectFile);
    QString currentVersionId;
    if (QFileInfo::exists(projectFilePath)) {
        const QString projectFileHash = sha256FileHex(projectFilePath);
        for (const QVariant& value : versions) {
            const QVariantMap version = value.toMap();
            const QString candidateVersionId = version.value(QStringLiteral("id")).toString();

            bool matchesCurrent = false;
            const QString objectHash = version.value(QStringLiteral("objectHash")).toString();
            if (!projectFileHash.isEmpty() && !objectHash.isEmpty()) {
                matchesCurrent = objectHash.compare(projectFileHash, Qt::CaseInsensitive) == 0;
            }

            if (!matchesCurrent) {
                const QString stagedPath = resolveStagedPath(
                    project.rootPath,
                    version.value(QStringLiteral("stagedPath")).toString());
                if (!stagedPath.isEmpty() && QFileInfo::exists(stagedPath)) {
                    matchesCurrent = filesAreIdentical(projectFilePath, stagedPath);
                }
            }

            if (matchesCurrent
                && (currentVersionId.isEmpty() || versionLessThan(currentVersionId, candidateVersionId))) {
                currentVersionId = candidateVersionId;
            }
        }
    }

    if (currentVersionId.isEmpty()) {
        for (const QVariant& value : versions) {
            const QString candidateVersionId = value.toMap().value(QStringLiteral("id")).toString();
            if (candidateVersionId.isEmpty()) {
                continue;
            }
            if (currentVersionId.isEmpty() || versionLessThan(currentVersionId, candidateVersionId)) {
                currentVersionId = candidateVersionId;
            }
        }
    }

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

    for (const QVariant& value : versions) {
        QVariantMap node = value.toMap();
        const QString id = node.value(QStringLiteral("id")).toString();
        const QString parentId = node.value(QStringLiteral("parent")).toString();
        if (id.isEmpty()) {
            continue;
        }
        node.insert(QStringLiteral("parentId"), parentId);
        nodeById.insert(id, node);
        childrenByParent[parentId].append(id);
    }

    QVariantList graph;
    int row = 0;

    std::function<void(const QString&, int)> appendChildren = [&](const QString& parentId, int depth) {
        QStringList children = childrenByParent.value(parentId);
        std::sort(children.begin(), children.end(), versionLessThan);
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

    const QString stagedPath = selectedVersion.value(QStringLiteral("stagedPath")).toString();
    if (stagedPath.isEmpty() || !QFileInfo::exists(stagedPath)) {
        appendActivity(QString(AppStrings::ActivityRestoreFailedSnapshotMissingFmt)
            .arg(nowHuman(), project.name, versionId));
        return false;
    }

    const QString destinationPath = QDir(project.rootPath).filePath(project.primaryProjectFile);
    const QFileInfo destinationInfo(destinationPath);
    if (!QDir().mkpath(destinationInfo.dir().absolutePath())) {
        appendActivity(QString(AppStrings::ActivityRestoreFailedCreateFolderFmt)
            .arg(nowHuman(), destinationPath));
        return false;
    }

    if (QFileInfo::exists(destinationPath) && !QFile::remove(destinationPath)) {
        appendActivity(QString(AppStrings::ActivityRestoreFailedReplaceFmt)
            .arg(nowHuman(), destinationPath));
        return false;
    }

    if (!QFile::copy(stagedPath, destinationPath)) {
        appendActivity(QString(AppStrings::ActivityRestoreFailedCopyFmt)
            .arg(nowHuman(), stagedPath, destinationPath));
        return false;
    }

    SnapshotService* snapshotService = snapshotServiceForRoot(project.rootPath);
    if (snapshotService) {
        snapshotService->suppressNextEventsForPath(project.primaryProjectFile, 1);
        snapshotService->setBranchBaseForPath(project.primaryProjectFile, versionId);
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

    bool updated = false;
    int count = 0;
    for (QByteArray& rawLine : lines) {
        const QByteArray trimmed = rawLine.trimmed();
        const QJsonDocument doc = QJsonDocument::fromJson(trimmed);
        if (!doc.isObject()) {
            continue;
        }

        QJsonObject obj = doc.object();
        const QString relPath = obj.value("path").toString();
        const QString stagedPath = obj.value("staged").toString();
        if (relPath != project.primaryProjectFile || stagedPath.isEmpty()) {
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

    if (!logFile.open(QIODevice::WriteOnly | QIODevice::Truncate | QIODevice::Text)) {
        return false;
    }

    for (const QByteArray& rawLine : lines) {
        if (logFile.write(rawLine) < 0) {
            logFile.close();
            return false;
        }
    }
    logFile.close();

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

    QList<QByteArray> keptLines;
    QString stagedPath;
    bool removedMetadata = false;
    int count = 0;
    while (!logFile.atEnd()) {
        const QByteArray rawLine = logFile.readLine();
        const QByteArray trimmed = rawLine.trimmed();
        const QJsonDocument doc = QJsonDocument::fromJson(trimmed);
        if (!doc.isObject()) {
            keptLines.append(rawLine);
            continue;
        }

        const QJsonObject obj = doc.object();
        const QString relPath = obj.value("path").toString();
        const QString objStagedPath = obj.value("staged").toString();
        if (relPath == project.primaryProjectFile && !objStagedPath.isEmpty()) {
            ++count;
            const QString lineVersion = obj.value("version").toString(QString::number(count));
            if (lineVersion == versionId) {
                stagedPath = resolveStagedPath(project.rootPath, objStagedPath);
                removedMetadata = true;
                continue;
            }
        }

        keptLines.append(rawLine);
    }
    logFile.close();

    if (!removedMetadata) {
        return false;
    }

    if (!logFile.open(QIODevice::WriteOnly | QIODevice::Truncate | QIODevice::Text)) {
        return false;
    }

    for (const QByteArray& rawLine : keptLines) {
        if (logFile.write(rawLine) < 0) {
            logFile.close();
            return false;
        }
    }
    logFile.close();

    if (!stagedPath.isEmpty()) {
        if (QFileInfo::exists(stagedPath)) {
            (void)QFile::remove(stagedPath);
        }

        const QString stagedRoot = QDir(project.rootPath).filePath(AppStrings::MusitStagingRelativePath);
        const QString stagedParentDir = QFileInfo(stagedPath).dir().absolutePath();
        removeEmptyParentDirs(stagedParentDir, stagedRoot);
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

    const QString appDataDir = QStandardPaths::writableLocation(QStandardPaths::AppDataLocation);
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
            const QString musitRoot = QDir(savedFolder).filePath(QStringLiteral(".musit"));
            const QString stagingDir = QDir(musitRoot).filePath(QStringLiteral("staging"));
            const QString versionsDir = QDir(musitRoot).filePath(QStringLiteral("versions"));

            if (!QDir().mkpath(stagingDir) || !QDir().mkpath(versionsDir)) {
                issues.append(QString(AppStrings::DialogCouldNotInitializeMusitStorageFmt).arg(savedFolder));
            } else {
                const QString probePath = QDir(musitRoot).filePath(QStringLiteral(".write_probe"));
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

                    if (projectRoot == m_projectRoot) {
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
        m_snapshotServiceByRoot.insert(QDir::cleanPath(project.rootPath), rawService);
        m_snapshotServices.emplace_back(std::move(snapshotService));

        const QString primaryFile = project.primaryProjectFile;
        if (!primaryFile.isEmpty() && !rawService->hasStagedVersionForPath(primaryFile)) {
            const QString absolutePrimaryPath = QDir(project.rootPath).filePath(primaryFile);
            const bool seeded = rawService->snapshotFileNow(absolutePrimaryPath, primaryFile);
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
        m_snapshotServices.clear();
        return;
    }

    connect(m_fileWatcher.get(), &IFileWatcher::fileEvent,
            this, [this](const FileEvent& event) {
                const QString normalizedAbsolutePath = QDir::cleanPath(event.absolutePath);

                QString matchedRoot;
                int matchedLength = -1;
                for (auto it = m_snapshotServiceByRoot.constBegin(); it != m_snapshotServiceByRoot.constEnd(); ++it) {
                    const QString rootPath = it.key();
                    const QString rootPrefix = rootPath + QDir::separator();
                    const bool inRoot = normalizedAbsolutePath == rootPath
                        || normalizedAbsolutePath.startsWith(rootPrefix);
                    if (!inRoot) {
                        continue;
                    }

                    if (rootPath.size() > matchedLength) {
                        matchedRoot = rootPath;
                        matchedLength = rootPath.size();
                    }
                }

                if (matchedRoot.isEmpty()) {
                    return;
                }

                auto it = m_snapshotServiceByRoot.find(matchedRoot);
                if (it == m_snapshotServiceByRoot.end() || !it.value()) {
                    return;
                }

                FileEvent translatedEvent = event;
                translatedEvent.relativePath = QDir(matchedRoot).relativeFilePath(normalizedAbsolutePath);
                it.value()->onFileEvent(translatedEvent);
            });

    if (!m_fileWatcher->startWatching(m_projectsFolderRoot)) {
        m_statusMessage = AppStrings::StatusFailedStartWatcher;
        emit statusMessageChanged();
        m_fileWatcher.reset();
        m_snapshotServiceByRoot.clear();
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
    m_snapshotServices.clear();
    appendActivity(QStringLiteral("[%1] monitoring stopped")
        .arg(QDateTime::currentDateTime().toString(Qt::ISODate)));
}

SnapshotService* QmlBackend::snapshotServiceForRoot(const QString& projectRoot) {
    const QString normalizedRoot = QDir::cleanPath(projectRoot);
    auto it = m_snapshotServiceByRoot.find(normalizedRoot);
    if (it == m_snapshotServiceByRoot.end()) {
        return nullptr;
    }
    return it.value();
}
