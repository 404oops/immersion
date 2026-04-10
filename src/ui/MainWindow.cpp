#include "MainWindow.h"

#include "AppStrings.h"

#include <QDateTime>
#include <QAbstractItemView>
#include <QAction>
#include <QApplication>
#include <QCloseEvent>
#include <QCoreApplication>
#include <QDesktopServices>
#include <QFile>
#include <QFileDialog>
#include <QFileInfo>
#include <QFileIconProvider>
#include <QHBoxLayout>
#include <QDialog>
#include <QGridLayout>
#include <QJsonDocument>
#include <QJsonObject>
#include <QJsonArray>
#include <QInputDialog>
#include <QLabel>
#include <QLineEdit>
#include <QListWidget>
#include <QListWidgetItem>
#include <QMenu>
#include <QCursor>
#include <QProcess>
#include <QPushButton>
#include <QComboBox>
#include <QRegularExpression>
#include <QSet>
#include <QSizePolicy>
#include <QSplitter>
#include <QStringList>
#include <QStyle>
#include <QSystemTrayIcon>
#include <QUrl>
#include <QVBoxLayout>
#include <QWidget>
#include <QIcon>
#include <QDir>

#include "../core/FileWatcherFactory.h"
#include "../core/IFileWatcher.h"
#include "../core/ProjectDiscovery.h"
#include "../core/SnapshotService.h"
#include "../persistence/ProjectRegistry.h"

namespace {

struct VirtualProjectFileEntry {
    QString display;
    QString selectedRealFile;
    bool isVersionGroup {false};
    int minVersion {-1};
    int maxVersion {-1};
};

struct VersionEntry {
    int lineIndex {-1};
    QString versionId;
    QString parentVersionId;
    QString relativePath;
    QString stagedPath;
    QString timestamp;
    QString note;
};

int extractVersionNumberFromDisplayName(const QString& fileName) {
    static const QRegularExpression versionRegex(
        "(?:^|[ _-])v(\\d+)(?:$|[ _.-])",
        QRegularExpression::CaseInsensitiveOption);

    const QString stem = QFileInfo(fileName).completeBaseName();
    const QRegularExpressionMatch match = versionRegex.match(stem);
    if (!match.hasMatch()) {
        return -1;
    }

    bool ok = false;
    const int version = match.captured(1).toInt(&ok);
    return ok ? version : -1;
}

QString baseNameWithoutVersion(const QString& fileName) {
    QString stem = QFileInfo(fileName).completeBaseName();
    static const QRegularExpression versionRegex(
        "(?:^|[ _-])v\\d+(?:$|[ _.-])",
        QRegularExpression::CaseInsensitiveOption);
    stem.replace(versionRegex, " ");
    stem = stem.simplified();
    return stem;
}

QString normalizedGroupKey(const QString& fileName) {
    QString base = baseNameWithoutVersion(fileName);
    if (base.isEmpty()) {
        base = QFileInfo(fileName).completeBaseName();
    }

    return QString("%1|%2")
        .arg(base.toLower(), QFileInfo(fileName).suffix().toLower());
}

QString formatTimestampHumanReadable(const QString& rawTimestamp) {
    QDateTime parsed = QDateTime::fromString(rawTimestamp, Qt::ISODateWithMs);
    if (!parsed.isValid()) {
        parsed = QDateTime::fromString(rawTimestamp, Qt::ISODate);
    }
    if (!parsed.isValid()) {
        return rawTimestamp;
    }
    return parsed.toLocalTime().toString("yyyy-MM-dd hh:mm:ss");
}

QIcon nativeAppIconForFile(const QString& absoluteFilePath) {
    static QHash<QString, QIcon> iconCache;
    const QString cacheKey = absoluteFilePath.isEmpty() ? QStringLiteral("<empty>") : absoluteFilePath;

    const auto cached = iconCache.constFind(cacheKey);
    if (cached != iconCache.constEnd()) {
        return cached.value();
    }

    QIcon icon;
    if (!absoluteFilePath.isEmpty()) {
        QFileIconProvider provider;
        icon = provider.icon(QFileInfo(absoluteFilePath));
    }

    iconCache.insert(cacheKey, icon);
    return icon;
}

QList<VirtualProjectFileEntry> buildVirtualProjectFileEntries(
    const QStringList& projectFiles,
    const QString& currentPrimaryFile) {
    QHash<QString, QStringList> filesByGroup;
    for (const QString& fileName : projectFiles) {
        filesByGroup[normalizedGroupKey(fileName)].append(fileName);
    }

    QList<VirtualProjectFileEntry> entries;
    entries.reserve(filesByGroup.size());

    for (auto it = filesByGroup.constBegin(); it != filesByGroup.constEnd(); ++it) {
        const QStringList files = it.value();
        QString selected = files.first();
        QString representative = files.first();
        int selectedVersion = extractVersionNumberFromDisplayName(selected);
        int minVersion = selectedVersion >= 0 ? selectedVersion : -1;
        int maxVersion = selectedVersion;
        bool hasVersion = selectedVersion >= 0;

        for (const QString& fileName : files) {
            const int version = extractVersionNumberFromDisplayName(fileName);
            if (version >= 0) {
                hasVersion = true;
                if (minVersion < 0 || version < minVersion) {
                    minVersion = version;
                }
                if (maxVersion < 0 || version > maxVersion) {
                    maxVersion = version;
                }
            }

            if (fileName == currentPrimaryFile) {
                selected = fileName;
                representative = fileName;
                selectedVersion = version;
                continue;
            }

            if (selected != currentPrimaryFile) {
                const bool betterVersion = version > selectedVersion;
                const bool sameVersionNameTieBreak = version == selectedVersion && fileName < selected;
                if (betterVersion || sameVersionNameTieBreak) {
                    selected = fileName;
                    representative = fileName;
                    selectedVersion = version;
                }
            }
        }

        VirtualProjectFileEntry entry;
        entry.selectedRealFile = selected;
        entry.isVersionGroup = hasVersion;
        entry.minVersion = minVersion;
        entry.maxVersion = maxVersion;

        const QString base = baseNameWithoutVersion(representative);
        const QString extension = QFileInfo(representative).suffix();

        if (hasVersion && !base.isEmpty()) {
            if (minVersion >= 0 && maxVersion >= 0 && minVersion != maxVersion) {
                entry.display = QString("%1 (v%2-v%3).%4")
                    .arg(base)
                    .arg(minVersion)
                    .arg(maxVersion)
                    .arg(extension);
            } else if (maxVersion >= 0) {
                entry.display = QString("%1 (v%2).%3")
                    .arg(base)
                    .arg(maxVersion)
                    .arg(extension);
            }
        }

        if (entry.display.isEmpty()) {
            entry.display = representative;
        }

        entries.append(entry);
    }

    std::sort(entries.begin(), entries.end(), [](const VirtualProjectFileEntry& left, const VirtualProjectFileEntry& right) {
        return left.display.toLower() < right.display.toLower();
    });

    return entries;
}

QList<VersionEntry> loadVersionEntriesForPath(
    const QString& logPath,
    const QString& targetRelativePath,
    QList<QByteArray>* allLogLines = nullptr,
    bool excludeLatest = false) {
    QList<VersionEntry> matchingEntries;

    QFile logFile(logPath);
    if (!logFile.exists() || !logFile.open(QIODevice::ReadOnly | QIODevice::Text)) {
        return matchingEntries;
    }

    int lineIndex = 0;
    int runningOrdinal = 0;
    while (!logFile.atEnd()) {
        const QByteArray rawLine = logFile.readLine();
        const QByteArray trimmed = rawLine.trimmed();

        if (allLogLines) {
            allLogLines->append(rawLine);
        }

        if (trimmed.isEmpty()) {
            ++lineIndex;
            continue;
        }

        const QJsonDocument doc = QJsonDocument::fromJson(trimmed);
        if (!doc.isObject()) {
            ++lineIndex;
            continue;
        }

        const QJsonObject obj = doc.object();
        const QString relPath = obj.value("path").toString();
        const QString stagedPath = obj.value("staged").toString();
        if (relPath != targetRelativePath || stagedPath.isEmpty()) {
            ++lineIndex;
            continue;
        }

        ++runningOrdinal;

        VersionEntry entry;
        entry.lineIndex = lineIndex;
        entry.versionId = obj.value("version").toString();
        if (entry.versionId.isEmpty()) {
            entry.versionId = QString::number(runningOrdinal);
        }
        entry.parentVersionId = obj.value("parent").toString();
        entry.relativePath = relPath;
        entry.stagedPath = stagedPath;
        entry.timestamp = obj.value("ts").toString();
        entry.note = obj.value("note").toString();
        matchingEntries.append(entry);

        ++lineIndex;
    }

    if (excludeLatest && matchingEntries.size() > 1) {
        matchingEntries.removeLast();
    }

    return matchingEntries;
}

bool updateVersionNoteAtLine(const QString& logPath, int lineIndex, const QString& note) {
    QFile logFile(logPath);
    if (!logFile.exists() || !logFile.open(QIODevice::ReadOnly | QIODevice::Text)) {
        return false;
    }

    QList<QByteArray> allLines;
    while (!logFile.atEnd()) {
        allLines.append(logFile.readLine());
    }
    logFile.close();

    if (lineIndex < 0 || lineIndex >= allLines.size()) {
        return false;
    }

    const QByteArray trimmed = allLines.at(lineIndex).trimmed();
    const QJsonDocument doc = QJsonDocument::fromJson(trimmed);
    if (!doc.isObject()) {
        return false;
    }

    QJsonObject obj = doc.object();
    if (note.trimmed().isEmpty()) {
        obj.remove("note");
    } else {
        obj.insert("note", note.trimmed());
    }

    allLines[lineIndex] = QJsonDocument(obj).toJson(QJsonDocument::Compact) + '\n';

    QByteArray rewritten;
    for (const QByteArray& line : allLines) {
        rewritten.append(line);
    }

    if (!logFile.open(QIODevice::WriteOnly | QIODevice::Truncate | QIODevice::Text)) {
        return false;
    }

    if (logFile.write(rewritten) < 0) {
        logFile.close();
        return false;
    }

    logFile.close();
    return true;
}

QList<qint64> pidsUsingFilePath(const QString& absoluteFilePath) {
#if defined(Q_OS_MAC)
    QProcess lsof;
    lsof.start("/usr/sbin/lsof", {"-t", absoluteFilePath});
    if (!lsof.waitForFinished(1500)) {
        return {};
    }

    QList<qint64> pids;
    const QString output = QString::fromUtf8(lsof.readAllStandardOutput());
    const QStringList lines = output.split('\n', Qt::SkipEmptyParts);
    for (const QString& line : lines) {
        bool ok = false;
        const qint64 pid = line.trimmed().toLongLong(&ok);
        if (ok && pid > 0) {
            pids.append(pid);
        }
    }

    return pids;
#else
    Q_UNUSED(absoluteFilePath);
    return {};
#endif
}

bool killProcesses(const QList<qint64>& pids) {
    const qint64 musitPid = QCoreApplication::applicationPid();
    bool allKilled = true;
    for (const qint64 pid : pids) {
        if (pid == musitPid) {
            continue;
        }

        QProcess killer;
#if defined(Q_OS_WIN)
        killer.start("taskkill", {"/PID", QString::number(pid), "/T", "/F"});
#else
        killer.start("/bin/kill", {"-9", QString::number(pid)});
#endif
        if (!killer.waitForFinished(1500) || killer.exitCode() != 0) {
            allKilled = false;
        }
    }
    return allKilled;
}

QString runAppleScript(const QString& script) {
#if defined(Q_OS_MAC)
    QProcess process;
    process.start("/usr/bin/osascript", {"-e", script});
    if (!process.waitForFinished(1500) || process.exitCode() != 0) {
        return {};
    }
    return QString::fromUtf8(process.readAllStandardOutput()).trimmed();
#else
    Q_UNUSED(script);
    return {};
#endif
}

QString frontmostAppBundleId() {
    return runAppleScript(
        "tell application \"System Events\" to get bundle identifier of first process whose frontmost is true");
}

bool quitAppByBundleId(const QString& bundleId) {
    if (bundleId.isEmpty()) {
        return false;
    }

    const QString script = QString("tell application id \"%1\" to quit").arg(bundleId);
    runAppleScript(script);
    return true;
}

QList<qint64> pidsForBundleId(const QString& bundleId) {
    if (bundleId.isEmpty()) {
        return {};
    }

    const QString script = QString(
        "tell application \"System Events\" to get unix id of (every process whose bundle identifier is \"%1\")")
            .arg(bundleId);
    const QString result = runAppleScript(script);
    if (result.isEmpty()) {
        return {};
    }

    QList<qint64> pids;
    const QStringList parts = result.split(',', Qt::SkipEmptyParts);
    for (const QString& part : parts) {
        bool ok = false;
        const qint64 pid = part.trimmed().toLongLong(&ok);
        if (ok && pid > 0) {
            pids.append(pid);
        }
    }

    return pids;
}

void forceKillAppByBundleId(const QString& bundleId) {
    const QList<qint64> pids = pidsForBundleId(bundleId);
    if (!pids.isEmpty()) {
        killProcesses(pids);
    }
}

bool isLikelyDawBundleId(const QString& bundleId) {
    const QString lower = bundleId.toLower();
    return lower.contains("bitwig")
        || lower.contains("ableton")
        || lower.contains("logic")
        || lower.contains("steinberg")
        || lower.contains("reaper")
        || lower.contains("ardour")
        || lower.contains("renoise")
        || lower.contains("protools")
        || lower.contains("studioone")
        || lower.contains("cakewalk")
        || lower.contains("reason")
        || lower.contains("garageband")
        || lower.contains("image-line")
        || lower.contains("fl");
}

void forceKillKnownDawProcessesCli() {
#if defined(Q_OS_MAC)
    const QStringList patterns {
        "BitwigStudio|Bitwig Audio Engine|BitwigPluginHost",
        "Ableton Live",
        "Logic Pro",
        "REAPER",
        "Cubase",
        "Pro Tools",
        "Studio One",
        "Renoise",
        "Ardour",
        "GarageBand",
        "FL Studio",
        "Reason"
    };

    for (const QString& pattern : patterns) {
        QProcess killer;
        killer.start("/usr/bin/pkill", {"-9", "-f", pattern});
        killer.waitForFinished(1200);
    }
#elif defined(Q_OS_WIN)
    const QString script =
        "$patterns=@('Bitwig*','Ableton*','Logic*','REAPER*','Cubase*','ProTools*','Studio One*','Renoise*','Ardour*','FL*','Reason*','GarageBand*');"
        "Get-Process -ErrorAction SilentlyContinue | "
        "Where-Object { $n=$_.ProcessName; $patterns | ForEach-Object { if ($n -like $_) { $true; return } } } | "
        "Stop-Process -Force -ErrorAction SilentlyContinue;";

    QProcess killer;
    killer.start("powershell", {"-NoProfile", "-Command", script});
    killer.waitForFinished(2000);
#endif
}

QString guessedBundleIdForProjectKind(ProjectKind kind) {
    switch (kind) {
    case ProjectKind::Bitwig:
        return QStringLiteral("com.bitwig.BitwigStudio");
    case ProjectKind::Ableton:
        return QStringLiteral("com.ableton.live");
    case ProjectKind::Logic:
        return QStringLiteral("com.apple.logic10");
    case ProjectKind::GarageBand:
        return QStringLiteral("com.apple.garageband10");
    case ProjectKind::Reaper:
        return QStringLiteral("com.cockos.reaper");
    case ProjectKind::ProTools:
        return QStringLiteral("com.avid.ProTools");
    default:
        return {};
    }
}

} // namespace

MainWindow::MainWindow() {
    auto* central = new QWidget(this);
    auto* layout = new QVBoxLayout(central);

    m_statusLabel = new QLabel(AppStrings::StatusSelectProjectsFolder, this);
    m_statusLabel->setWordWrap(true);
    m_projectSearchEdit = new QLineEdit(this);
    m_projectSearchEdit->setPlaceholderText(AppStrings::LabelSearchPlaceholder);
    m_projectSortCombo = new QComboBox(this);
    m_projectSortCombo->addItem(AppStrings::SortName, static_cast<int>(ProjectSortMode::Name));
    m_projectSortCombo->addItem(AppStrings::SortLastOpened, static_cast<int>(ProjectSortMode::LastOpened));
    m_projectList = new QListWidget(this);
    m_activityLog = new QListWidget(this);
    m_logLevelCombo = new QComboBox(this);
    m_chooseButton = new QPushButton(AppStrings::LabelChooseProjectFolder, this);

    m_projectDiscovery = std::make_unique<ProjectDiscovery>();
    m_projectRegistry = std::make_unique<ProjectRegistry>();
    setupTray();

    auto* buttonRow = new QHBoxLayout();
    buttonRow->addWidget(m_chooseButton);

    auto* discoveredLabel = new QLabel(AppStrings::LabelDiscoveredProjects, this);
    auto* activityLabel = new QLabel(AppStrings::LabelActivity, this);

    auto* projectHeaderLayout = new QHBoxLayout();
    projectHeaderLayout->setContentsMargins(0, 0, 0, 0);
    projectHeaderLayout->setSpacing(8);
    projectHeaderLayout->addWidget(discoveredLabel);
    projectHeaderLayout->addStretch(1);
    projectHeaderLayout->addWidget(new QLabel(AppStrings::LabelSort, this));
    projectHeaderLayout->addWidget(m_projectSortCombo);

    m_logLevelCombo->addItem(AppStrings::LogLevelInfo, static_cast<int>(ActivityLogLevel::Info));
    m_logLevelCombo->addItem(AppStrings::LogLevelDebug, static_cast<int>(ActivityLogLevel::Debug));
    m_logLevelCombo->setCurrentIndex(0);

    auto* activityHeaderLayout = new QHBoxLayout();
    activityHeaderLayout->setContentsMargins(0, 0, 0, 0);
    activityHeaderLayout->addWidget(activityLabel);
    activityHeaderLayout->addStretch(1);
    activityHeaderLayout->addWidget(new QLabel(AppStrings::LabelLogLevel, this));
    activityHeaderLayout->addWidget(m_logLevelCombo);

    m_projectList->setSelectionMode(QAbstractItemView::SingleSelection);
    m_activityLog->setWordWrap(true);
    m_activityLog->setTextElideMode(Qt::ElideNone);
    m_activityLog->setVerticalScrollMode(QAbstractItemView::ScrollPerPixel);

    auto* projectPane = new QWidget(this);
    auto* projectPaneLayout = new QVBoxLayout(projectPane);
    projectPaneLayout->setContentsMargins(0, 0, 0, 0);
    projectPaneLayout->setSpacing(4);
    projectPaneLayout->addLayout(projectHeaderLayout);
    projectPaneLayout->addWidget(m_projectSearchEdit);
    projectPaneLayout->addWidget(m_projectList, 1);

    auto* activityPane = new QWidget(this);
    auto* activityPaneLayout = new QVBoxLayout(activityPane);
    activityPaneLayout->setContentsMargins(0, 0, 0, 0);
    activityPaneLayout->setSpacing(4);
    activityPaneLayout->addLayout(activityHeaderLayout);
    activityPaneLayout->addWidget(m_activityLog, 1);

    auto* splitter = new QSplitter(Qt::Vertical, this);
    splitter->addWidget(projectPane);
    splitter->addWidget(activityPane);
    splitter->setStretchFactor(0, 4);
    splitter->setStretchFactor(1, 2);
    splitter->setChildrenCollapsible(false);

    layout->addWidget(m_statusLabel);
    layout->addLayout(buttonRow);
    layout->addWidget(splitter, 1);
    m_activityLog->scrollToBottom();

    setCentralWidget(central);
    setWindowTitle(AppStrings::AppName);
    resize(860, 540);

    connect(m_chooseButton, &QPushButton::clicked, this, &MainWindow::chooseProjectFolder);
    connect(m_projectList, &QListWidget::currentRowChanged, this, &MainWindow::onDiscoveredProjectSelected);
    connect(m_logLevelCombo, &QComboBox::currentIndexChanged, this, &MainWindow::onLogLevelChanged);
    connect(m_projectSearchEdit, &QLineEdit::textChanged, this, &MainWindow::onProjectSearchChanged);
    connect(m_projectSortCombo, &QComboBox::currentIndexChanged, this, &MainWindow::onProjectSortChanged);

    const QString savedProjectsFolder = m_projectRegistry->loadProjectsFolder();
    if (!savedProjectsFolder.isEmpty()) {
        loadProjectsFromFolder(savedProjectsFolder, false);
    }
}

MainWindow::~MainWindow() {
    if (m_fileWatcher) {
        m_fileWatcher->stopWatching();
    }
}

void MainWindow::chooseProjectFolder() {
    const QString folder = QFileDialog::getExistingDirectory(this, AppStrings::DialogSelectProjectsFolder);
    if (folder.isEmpty()) {
        return;
    }

    loadProjectsFromFolder(folder, true);
}

bool MainWindow::loadProjectsFromFolder(const QString& folder, bool persistFolder) {
    const QString previousRoot = m_projectRoot;

    m_discoveredProjects = m_projectDiscovery->discoverAll(folder);
    if (m_discoveredProjects.isEmpty()) {
        setStatus(AppStrings::StatusNoSupportedProjects);
        m_activityLog->addItem(QString(AppStrings::ActivityNoProjectFilesFmt)
            .arg(QDateTime::currentDateTime().toString(Qt::ISODate), folder));
        return false;
    }

    if (persistFolder && !m_projectRegistry->saveProjectsFolder(folder)) {
        m_activityLog->addItem(QString("[%1] failed to persist projects folder %2")
            .arg(QDateTime::currentDateTime().toString(Qt::ISODateWithMs), folder));
    }

    bool savedAll = true;
    for (const DiscoveredProject& project : m_discoveredProjects) {
        if (!m_projectRegistry->saveProject(project)) {
            savedAll = false;
        }
    }

    m_registryPath = m_projectRegistry->dataFilePath();
    populateProjectList();
    refreshTrayProjectMenu();
    refreshTrayVersionMenu();

    m_activityLog->addItem(QString(AppStrings::ActivityDiscoveredFmt)
        .arg(QDateTime::currentDateTime().toString(Qt::ISODateWithMs),
            QString::number(m_discoveredProjects.size()),
            folder));
    m_activityLog->addItem(QString(AppStrings::ActivityRegistryStatusFmt)
        .arg(QDateTime::currentDateTime().toString(Qt::ISODateWithMs),
            savedAll ? AppStrings::RegistrySaved : AppStrings::RegistryPartiallySaved,
            m_registryPath));
    m_activityLog->scrollToBottom();

    setStatus(QString(AppStrings::StatusFirstTimeSetupFmt)
        .arg(m_discoveredProjects.size()));

    const DiscoveredProject& first = m_discoveredProjects.first();
    m_activityLog->addItem(QString(AppStrings::ActivityDefaultSelectionFmt)
        .arg(QDateTime::currentDateTime().toString(Qt::ISODateWithMs),
            first.name,
            first.primaryProjectFile));

    if (m_fileWatcher && previousRoot != m_projectRoot) {
        stopMonitoring();
    }

    if (!m_fileWatcher) {
        startMonitoring();
    }

    return true;
}

void MainWindow::startMonitoring() {
    if (m_projectRoot.isEmpty()) {
        setStatus(AppStrings::StatusNoProjectSelected);
        return;
    }

    if (m_fileWatcher) {
        setStatus(QString(AppStrings::StatusMonitoringActiveFmt).arg(m_projectRoot));
        return;
    }

    m_snapshotService = std::make_unique<SnapshotService>();
    if (!m_snapshotService->setProjectRoot(m_projectRoot)) {
        setStatus(AppStrings::StatusFailedInitStorage);
        return;
    }

    m_fileWatcher = createFileWatcher();
    if (!m_fileWatcher) {
        setStatus(AppStrings::StatusFailedCreateWatcher);
        return;
    }

    connect(m_fileWatcher.get(), &IFileWatcher::fileEvent, m_snapshotService.get(), &SnapshotService::onFileEvent);
    connect(m_snapshotService.get(), &SnapshotService::snapshotCreated, this, &MainWindow::onSnapshotMessage);
    connect(m_snapshotService.get(), &SnapshotService::snapshotSkipped, this, &MainWindow::onSnapshotMessage);
    connect(m_snapshotService.get(), &SnapshotService::snapshotError, this, &MainWindow::onSnapshotMessage);

    if (!m_fileWatcher->startWatching(m_projectRoot)) {
        setStatus(AppStrings::StatusFailedStartWatcher);
        return;
    }

    setStatus(QString(AppStrings::StatusMonitoringFmt).arg(m_projectRoot));
    m_activityLog->addItem(QString(AppStrings::ActivityMonitoringStartedFmt)
        .arg(QDateTime::currentDateTime().toString(Qt::ISODate)));
    refreshTrayVersionMenu();
}

void MainWindow::stopMonitoring() {
    if (m_fileWatcher) {
        m_fileWatcher->stopWatching();
    }

    m_fileWatcher.reset();
    m_snapshotService.reset();

    setStatus(AppStrings::StatusMonitoringStopped);
    m_activityLog->addItem(QString(AppStrings::ActivityMonitoringStoppedFmt)
        .arg(QDateTime::currentDateTime().toString(Qt::ISODate)));
    refreshTrayVersionMenu();
}

void MainWindow::showRestoreVersionMenuForRow(int row, QWidget* anchorWidget) {
    if (row < 0 || row >= m_discoveredProjects.size()) {
        setStatus(AppStrings::StatusNoProjectSelectedForRestore);
        return;
    }

    const DiscoveredProject& project = m_discoveredProjects.at(row);
    if (project.primaryProjectFile.isEmpty()) {
        setStatus(AppStrings::StatusNoProjectFileSelectedForRestore);
        return;
    }

    const QString targetRelativePath = project.primaryProjectFile;
    const QString logPath = QDir(project.rootPath).filePath(AppStrings::MusitVersionLogRelativePath);
    QFile logFile(logPath);
    if (!logFile.exists() || !logFile.open(QIODevice::ReadOnly | QIODevice::Text)) {
        setStatus(AppStrings::StatusNoVersionHistoryForProject);
        return;
    }

    const QList<VersionEntry> matchingEntries = loadVersionEntriesForPath(logPath, targetRelativePath, nullptr, true);

    if (matchingEntries.isEmpty()) {
        setStatus(AppStrings::StatusNoRestorableVersionsForFile);
        return;
    }

    auto* menu = new QMenu(this);
    for (const VersionEntry& entry : matchingEntries) {
        QString versionLabel = QString("v%1").arg(entry.versionId);
        if (!entry.parentVersionId.isEmpty()) {
            versionLabel += QString(" (from v%1)").arg(entry.parentVersionId);
        }

        QAction* action = menu->addAction(QString("%1  %2").arg(versionLabel, entry.relativePath));
        connect(action, &QAction::triggered, this, [this, entry]() {
            const QString relPath = entry.relativePath;
            const QString stagedPath = entry.stagedPath;
            if (restoreFromVersionAction(relPath, stagedPath, entry.versionId)) {
                m_activityLog->addItem(QString(AppStrings::ActivityRestoredFmt)
                    .arg(QDateTime::currentDateTime().toString(Qt::ISODateWithMs), relPath, stagedPath));
                m_activityLog->scrollToBottom();
                setStatus(QString(AppStrings::StatusRestoredFmt).arg(relPath));
            } else {
                m_activityLog->addItem(QString(AppStrings::ActivityRestoreFailedFmt)
                    .arg(QDateTime::currentDateTime().toString(Qt::ISODateWithMs), relPath));
                m_activityLog->scrollToBottom();
                setStatus(QString(AppStrings::StatusRestoreFailedFmt).arg(relPath));
            }
        });
    }

    const QSize menuSize = menu->sizeHint();
    const QPoint cursorPos = anchorWidget ? anchorWidget->mapToGlobal(anchorWidget->rect().center()) : QCursor::pos();
    const QPoint popupPos(cursorPos.x(), cursorPos.y() - menuSize.height());
    menu->popup(popupPos);
}

void MainWindow::showManageVersionsDialogForRow(int row, QWidget* anchorWidget) {
    if (row < 0 || row >= m_discoveredProjects.size()) {
        setStatus(AppStrings::StatusNoProjectSelectedForManage);
        return;
    }

    const DiscoveredProject& project = m_discoveredProjects.at(row);
    if (project.primaryProjectFile.isEmpty()) {
        setStatus(AppStrings::StatusNoProjectFileSelectedForManage);
        return;
    }

    const QString targetRelativePath = project.primaryProjectFile;
    const QString logPath = QDir(project.rootPath).filePath(AppStrings::MusitVersionLogRelativePath);
    QList<QByteArray> allLogLines;
    const QList<VersionEntry> entries = loadVersionEntriesForPath(logPath, targetRelativePath, &allLogLines, true);
    if (entries.isEmpty()) {
        setStatus(AppStrings::StatusNoVersionsToManage);
        return;
    }

    auto* dialog = new QDialog(this, Qt::Popup);
    dialog->setWindowTitle(AppStrings::DialogManageTitle);
    auto* layout = new QVBoxLayout(dialog);
    layout->setContentsMargins(10, 10, 10, 10);

    auto* title = new QLabel(QString(AppStrings::DialogManageVersionsForFmt).arg(targetRelativePath), dialog);
    title->setWordWrap(true);
    layout->addWidget(title);

    auto* list = new QListWidget(dialog);
    list->setSelectionMode(QAbstractItemView::SingleSelection);
    for (const VersionEntry& entry : entries) {
        const QString timestampLabel = formatTimestampHumanReadable(entry.timestamp);
        QString itemText = QString("v%1  %2").arg(entry.versionId).arg(timestampLabel);
        if (!entry.note.trimmed().isEmpty()) {
            itemText += QString("  |  note: %1").arg(entry.note);
        }
        auto* item = new QListWidgetItem(itemText, list);
        item->setFlags(item->flags() | Qt::ItemIsUserCheckable);
        item->setCheckState(Qt::Unchecked);
        item->setData(Qt::UserRole, entry.lineIndex);
        item->setData(Qt::UserRole + 1, entry.stagedPath);
        item->setData(Qt::UserRole + 2, entry.versionId);
        item->setData(Qt::UserRole + 3, timestampLabel);
        item->setData(Qt::UserRole + 4, entry.note);
    }
    layout->addWidget(list);

    auto* buttons = new QHBoxLayout();
    auto* cancelButton = new QPushButton(AppStrings::ButtonCancel, dialog);
    auto* noteButton = new QPushButton(AppStrings::ButtonAddEditNote, dialog);
    auto* deleteButton = new QPushButton(AppStrings::ButtonDeleteSelectedVersions, dialog);
    buttons->addWidget(cancelButton);
    buttons->addWidget(noteButton);
    buttons->addWidget(deleteButton);
    layout->addLayout(buttons);

    connect(cancelButton, &QPushButton::clicked, dialog, &QDialog::reject);
    connect(noteButton, &QPushButton::clicked, this, [this, list, logPath, targetRelativePath]() {
        QListWidgetItem* item = list->currentItem();
        if (!item) {
            setStatus(AppStrings::StatusSelectVersionRowForNote);
            return;
        }

        const int lineIndex = item->data(Qt::UserRole).toInt();
        const QString currentNote = item->data(Qt::UserRole + 4).toString();
        bool ok = false;
        const QString newNote = QInputDialog::getText(
            this,
            AppStrings::DialogVersionNoteTitle,
            AppStrings::DialogVersionNotePrompt,
            QLineEdit::Normal,
            currentNote,
            &ok);

        if (!ok) {
            return;
        }

        if (!updateVersionNoteAtLine(logPath, lineIndex, newNote)) {
            setStatus(AppStrings::StatusFailedSaveVersionNote);
            return;
        }

        const QString versionId = item->data(Qt::UserRole + 2).toString();
        const QString timestampLabel = item->data(Qt::UserRole + 3).toString();
        const QString trimmedNote = newNote.trimmed();
        QString label = QString("v%1  %2").arg(versionId).arg(timestampLabel);
        if (!trimmedNote.isEmpty()) {
            label += QString("  |  note: %1").arg(trimmedNote);
        }

        item->setText(label);
        item->setData(Qt::UserRole + 4, trimmedNote);

        appendActivityLog(QString("[%1] updated version note for %2 (v%3)")
            .arg(QDateTime::currentDateTime().toString(Qt::ISODateWithMs))
            .arg(targetRelativePath)
            .arg(versionId),
            ActivityLogLevel::Info);
    });
    connect(deleteButton, &QPushButton::clicked, this, [this, dialog, list, allLogLines, entries, logPath, targetRelativePath]() {
        QList<QByteArray> currentLogLines = allLogLines;
        QFile currentLogFile(logPath);
        if (currentLogFile.exists() && currentLogFile.open(QIODevice::ReadOnly | QIODevice::Text)) {
            currentLogLines.clear();
            while (!currentLogFile.atEnd()) {
                currentLogLines.append(currentLogFile.readLine());
            }
            currentLogFile.close();
        }

        QSet<int> selectedLineIndexes;
        QStringList stagedPathsToDelete;
        QStringList deletedVersionLabels;
        QHash<int, QString> versionIdByLineIndex;
        for (const VersionEntry& entry : entries) {
            versionIdByLineIndex.insert(entry.lineIndex, entry.versionId);
        }

        for (int i = 0; i < list->count(); ++i) {
            QListWidgetItem* item = list->item(i);
            if (!item || item->checkState() != Qt::Checked) {
                continue;
            }

            const int lineIndex = item->data(Qt::UserRole).toInt();
            selectedLineIndexes.insert(lineIndex);
            const QString stagedPath = item->data(Qt::UserRole + 1).toString();
            if (!stagedPath.isEmpty()) {
                stagedPathsToDelete.append(stagedPath);
            }

            const QString versionId = item->data(Qt::UserRole + 2).toString();
            const QString timestampLabel = item->data(Qt::UserRole + 3).toString();
            deletedVersionLabels.append(QString("v%1 (%2)").arg(versionId).arg(timestampLabel));
        }

        if (selectedLineIndexes.isEmpty()) {
            setStatus(AppStrings::StatusNoVersionsSelectedForDeletion);
            dialog->reject();
            return;
        }

        QByteArray rewritten;
        for (int i = 0; i < currentLogLines.size(); ++i) {
            if (!selectedLineIndexes.contains(i)) {
                const QByteArray trimmed = currentLogLines.at(i).trimmed();
                const QJsonDocument doc = QJsonDocument::fromJson(trimmed);
                if (!doc.isObject()) {
                    rewritten.append(currentLogLines.at(i));
                    continue;
                }

                QJsonObject obj = doc.object();
                if (obj.value("path").toString() != targetRelativePath || obj.value("staged").toString().isEmpty()) {
                    rewritten.append(currentLogLines.at(i));
                    continue;
                }

                if (!obj.contains("version") || obj.value("version").toString().isEmpty()) {
                    const QString existingVersionId = versionIdByLineIndex.value(i);
                    if (!existingVersionId.isEmpty()) {
                        obj.insert("version", existingVersionId);
                    }
                }

                rewritten.append(QJsonDocument(obj).toJson(QJsonDocument::Compact));
                rewritten.append('\n');
            }
        }

        QFile logFile(logPath);
        if (!logFile.open(QIODevice::WriteOnly | QIODevice::Truncate | QIODevice::Text)) {
            setStatus(AppStrings::StatusFailedRewriteVersionLog);
            dialog->reject();
            return;
        }

        if (logFile.write(rewritten) < 0) {
            logFile.close();
            setStatus(AppStrings::StatusFailedPersistDeletedVersions);
            dialog->reject();
            return;
        }
        logFile.close();

        int removedStageCopies = 0;
        for (const QString& stagedPath : stagedPathsToDelete) {
            if (QFile::remove(stagedPath)) {
                ++removedStageCopies;
            }
        }

        m_activityLog->addItem(QString("[%1] deleted %2 versions for %3: %4 (removed %5 staged files)")
            .arg(QDateTime::currentDateTime().toString(Qt::ISODateWithMs))
            .arg(selectedLineIndexes.size())
            .arg(targetRelativePath)
            .arg(deletedVersionLabels.join(", "))
            .arg(removedStageCopies));
        m_activityLog->scrollToBottom();
        setStatus(QString(AppStrings::StatusDeletedVersionsFmt).arg(selectedLineIndexes.size()).arg(targetRelativePath));

        refreshTrayVersionMenu();
        dialog->accept();
    });

    dialog->adjustSize();
    const QPoint anchorPos = anchorWidget ? anchorWidget->mapToGlobal(anchorWidget->rect().center()) : QCursor::pos();
    dialog->move(anchorPos.x(), anchorPos.y() - dialog->height());
    dialog->show();
}

void MainWindow::onSnapshotMessage(const QString& message) {
    const QString lower = message.toLower();
    const bool isDebug = lower.contains("skipped")
        || lower.contains("suppressed")
        || lower.contains("excluded")
        || lower.contains("ignored");
    const ActivityLogLevel level = isDebug ? ActivityLogLevel::Debug : ActivityLogLevel::Info;
    appendActivityLog(QString("[%1] %2")
        .arg(QDateTime::currentDateTime().toString(Qt::ISODateWithMs), message),
        level);

    // Notify on real file-save snapshots (not baseline/deletion/suppression chatter).
    if (m_trayIcon && message.startsWith("Snapshot ")) {
        QString relativePath = message.mid(QString("Snapshot ").size());
        const int arrowIndex = relativePath.indexOf(" -> ");
        if (arrowIndex > 0) {
            relativePath = relativePath.left(arrowIndex);
        }

        m_trayIcon->showMessage(
            AppStrings::AppName,
            QString(AppStrings::TraySaveDetectedAndVersionedFmt).arg(relativePath),
            QSystemTrayIcon::Information,
            2400);
    }
}

void MainWindow::onLogLevelChanged(int index) {
    if (index < 0 || !m_logLevelCombo) {
        return;
    }

    const int dataValue = m_logLevelCombo->itemData(index).toInt();
    m_minLogLevel = static_cast<ActivityLogLevel>(dataValue);
    appendActivityLog(QString("[%1] log level set to %2")
        .arg(QDateTime::currentDateTime().toString(Qt::ISODateWithMs), m_logLevelCombo->currentText()),
        ActivityLogLevel::Info);
}

void MainWindow::appendActivityLog(const QString& line, ActivityLogLevel level) {
    if (!m_activityLog) {
        return;
    }

    if (static_cast<int>(level) < static_cast<int>(m_minLogLevel)) {
        return;
    }

    m_activityLog->addItem(line);
    m_activityLog->scrollToBottom();
}

void MainWindow::setStatus(const QString& status) {
    m_statusLabel->setText(status);
}

void MainWindow::populateProjectList() {
    refreshProjectListFilterAndSort();
}

void MainWindow::refreshProjectListFilterAndSort() {
    const QString selectedRoot = m_projectRoot;
    const QString query = m_projectSearchEdit ? m_projectSearchEdit->text().trimmed().toLower() : QString();

    QList<int> candidateIndexes;
    candidateIndexes.reserve(m_discoveredProjects.size());
    for (int i = 0; i < m_discoveredProjects.size(); ++i) {
        const DiscoveredProject& project = m_discoveredProjects.at(i);
        if (!query.isEmpty()) {
            const QString haystack = QString("%1 %2 %3 %4")
                .arg(project.name, project.primaryProjectFile, project.rootPath, ProjectDiscovery::kindToString(project.kind))
                .toLower();
            if (!haystack.contains(query)) {
                continue;
            }
        }
        candidateIndexes.append(i);
    }

    std::sort(candidateIndexes.begin(), candidateIndexes.end(), [this](int left, int right) {
        const DiscoveredProject& l = m_discoveredProjects.at(left);
        const DiscoveredProject& r = m_discoveredProjects.at(right);

        if (m_projectSortMode == ProjectSortMode::LastOpened) {
            const QDateTime lOpened = m_lastOpenedAtByProjectRoot.value(l.rootPath);
            const QDateTime rOpened = m_lastOpenedAtByProjectRoot.value(r.rootPath);
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

    m_visibleProjectIndexes = candidateIndexes;
    m_projectList->clear();
    for (const int projectIndex : m_visibleProjectIndexes) {
        auto* item = new QListWidgetItem(m_projectList);
        item->setSizeHint(QSize(100, 81));
        m_projectList->setItemWidget(item, createProjectRowWidget(projectIndex));
    }

    int selectedRow = -1;
    if (!selectedRoot.isEmpty()) {
        for (int row = 0; row < m_visibleProjectIndexes.size(); ++row) {
            const int projectIndex = m_visibleProjectIndexes.at(row);
            if (m_discoveredProjects.at(projectIndex).rootPath == selectedRoot) {
                selectedRow = row;
                break;
            }
        }
    }

    if (selectedRow < 0 && !m_visibleProjectIndexes.isEmpty()) {
        selectedRow = 0;
    }

    if (selectedRow >= 0) {
        m_projectList->setCurrentRow(selectedRow);
        setActiveProjectByRow(m_visibleProjectIndexes.at(selectedRow));
    } else {
        setActiveProjectByRow(-1);
    }
}

int MainWindow::visibleRowForProjectIndex(int projectIndex) const {
    for (int row = 0; row < m_visibleProjectIndexes.size(); ++row) {
        if (m_visibleProjectIndexes.at(row) == projectIndex) {
            return row;
        }
    }
    return -1;
}

QWidget* MainWindow::createProjectRowWidget(int index) {
    const DiscoveredProject& project = m_discoveredProjects.at(index);
    const QList<VirtualProjectFileEntry> virtualFiles = buildVirtualProjectFileEntries(
        project.projectFiles,
        project.primaryProjectFile);
    const QString primaryProjectPath = QDir(project.rootPath).filePath(project.primaryProjectFile);
    QIcon projectIcon = nativeAppIconForFile(primaryProjectPath);
    if (projectIcon.isNull()) {
        projectIcon = iconForProjectKind(project.kind);
    }

    auto* row = new QWidget();
    auto* rowLayout = new QHBoxLayout(row);
    rowLayout->setContentsMargins(7, 4, 7, 9);
    rowLayout->setSpacing(7);

    auto* iconLabel = new QLabel(row);
    iconLabel->setFixedSize(62, 44);
    iconLabel->setAlignment(Qt::AlignCenter);
    iconLabel->setPixmap(projectIcon.pixmap(36, 36));
    rowLayout->addWidget(iconLabel, 0, Qt::AlignLeft | Qt::AlignVCenter);

    auto* detailLayout = new QVBoxLayout();

    auto* nameLabel = new QLabel(project.name, row);
    QFont boldFont = nameLabel->font();
    boldFont.setBold(true);
    nameLabel->setFont(boldFont);
    detailLayout->addWidget(nameLabel);

    bool canOpen = false;
    if (virtualFiles.size() <= 1) {
        const QString fileName = virtualFiles.isEmpty() ? "No project file" : virtualFiles.first().display;
        auto* fileLabel = new QLabel(QString("Project file: %1").arg(fileName), row);
        detailLayout->addWidget(fileLabel);
        canOpen = !virtualFiles.isEmpty();
    } else {
        auto* filePicker = new QComboBox(row);
        QHash<QString, QString> selectedRealByDisplay;
        for (const VirtualProjectFileEntry& entry : virtualFiles) {
            filePicker->addItem(entry.display);
            selectedRealByDisplay.insert(entry.display, entry.selectedRealFile);
        }

        int defaultIndex = -1;
        for (int i = 0; i < virtualFiles.size(); ++i) {
            if (virtualFiles.at(i).selectedRealFile == project.primaryProjectFile) {
                defaultIndex = i;
                break;
            }
        }
        if (defaultIndex >= 0) {
            filePicker->setCurrentIndex(defaultIndex);
        }

        QObject::connect(filePicker, &QComboBox::currentTextChanged, row, [this, index, selectedRealByDisplay](const QString& value) {
            if (index < 0 || index >= m_discoveredProjects.size()) {
                return;
            }

            const QString selectedRealFile = selectedRealByDisplay.value(value);
            if (selectedRealFile.isEmpty()) {
                return;
            }

            m_discoveredProjects[index].primaryProjectFile = selectedRealFile;
            const int currentVisibleRow = m_projectList ? m_projectList->currentRow() : -1;
            if (currentVisibleRow >= 0
                && currentVisibleRow < m_visibleProjectIndexes.size()
                && m_visibleProjectIndexes.at(currentVisibleRow) == index) {
                setActiveProjectByRow(index);
            }
        });

        detailLayout->addWidget(filePicker);
        canOpen = true;
    }

    auto* metaLabel = new QLabel(QString("Type: %1  |  Path: %2")
            .arg(ProjectDiscovery::kindToString(project.kind), project.rootPath), row);
    metaLabel->setStyleSheet("color: #b8b8b8;");
    detailLayout->addWidget(metaLabel);

    rowLayout->addLayout(detailLayout, 1);

    auto* actionLayout = new QGridLayout();
    actionLayout->setContentsMargins(0, 0, 0, 0);
    actionLayout->setHorizontalSpacing(7);
    actionLayout->setVerticalSpacing(5);
    actionLayout->setColumnStretch(0, 5);
    actionLayout->setColumnStretch(1, 4);

    auto* openButton = new QPushButton("Open", row);
    openButton->setEnabled(canOpen);
    openButton->setMinimumWidth(62);
    openButton->setMinimumHeight(47);
    openButton->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Expanding);
    QObject::connect(openButton, &QPushButton::clicked, row, [this, index]() {
        openProjectFromRow(index);
    });
    actionLayout->addWidget(openButton, 0, 0, 2, 1);

    auto* manageVersionsButton = new QPushButton("Manage", row);
    manageVersionsButton->setEnabled(canOpen);
    manageVersionsButton->setMinimumWidth(62);
    manageVersionsButton->setMinimumHeight(47);
    manageVersionsButton->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Expanding);
    QObject::connect(manageVersionsButton, &QPushButton::clicked, row, [this, index, manageVersionsButton]() {
        const int visibleRow = visibleRowForProjectIndex(index);
        m_projectList->setCurrentRow(visibleRow);
        setActiveProjectByRow(index);
        showManageVersionsDialogForRow(index, manageVersionsButton);
    });
    actionLayout->addWidget(manageVersionsButton, 0, 1, 2, 1);

    rowLayout->addLayout(actionLayout, 0);

    return row;
}

void MainWindow::setActiveProjectByRow(int row) {
    if (row < 0 || row >= m_discoveredProjects.size()) {
        m_projectRoot.clear();
        return;
    }

    const DiscoveredProject& selected = m_discoveredProjects.at(row);
    m_projectRoot = selected.rootPath;
    m_activeTypeCounts = selected.typeCounts;
    m_totalProjectFileCount = selected.totalProjectFiles;

    setStatus(QString("Selected project: %1 (%2) in %3")
        .arg(selected.name, ProjectDiscovery::kindToString(selected.kind), selected.rootPath));
}

QIcon MainWindow::iconForProjectKind(ProjectKind kind) const {
    switch (kind) {
    case ProjectKind::Bitwig:
        return QIcon::fromTheme("applications-multimedia", style()->standardIcon(QStyle::SP_MediaPlay));
    case ProjectKind::FLStudio:
        return QIcon::fromTheme("audio-x-generic", style()->standardIcon(QStyle::SP_MediaVolume));
    case ProjectKind::Ableton:
        return QIcon::fromTheme("folder-music", style()->standardIcon(QStyle::SP_DirIcon));
    case ProjectKind::Logic:
        return QIcon::fromTheme("multimedia-player", style()->standardIcon(QStyle::SP_FileDialogDetailedView));
    case ProjectKind::Cubase:
        return QIcon::fromTheme("applications-audio", style()->standardIcon(QStyle::SP_MediaPlay));
    case ProjectKind::Reaper:
        return QIcon::fromTheme("audio-x-generic", style()->standardIcon(QStyle::SP_MediaSeekForward));
    case ProjectKind::Ardour:
        return QIcon::fromTheme("multimedia-volume-control", style()->standardIcon(QStyle::SP_MediaVolume));
    case ProjectKind::Renoise:
        return QIcon::fromTheme("media-playback-start", style()->standardIcon(QStyle::SP_MediaPlay));
    case ProjectKind::ProTools:
        return QIcon::fromTheme("applications-multimedia", style()->standardIcon(QStyle::SP_FileDialogInfoView));
    case ProjectKind::StudioOne:
        return QIcon::fromTheme("folder-music", style()->standardIcon(QStyle::SP_DirIcon));
    case ProjectKind::Cakewalk:
        return QIcon::fromTheme("audio-card", style()->standardIcon(QStyle::SP_MediaVolume));
    case ProjectKind::Reason:
        return QIcon::fromTheme("media-optical", style()->standardIcon(QStyle::SP_DriveCDIcon));
    case ProjectKind::GarageBand:
        return QIcon::fromTheme("folder-music", style()->standardIcon(QStyle::SP_FileDialogListView));
    case ProjectKind::Unknown:
    default:
        return style()->standardIcon(QStyle::SP_FileIcon);
    }
}

void MainWindow::onDiscoveredProjectSelected(int row) {
    const QString previousRoot = m_projectRoot;
    if (row < 0 || row >= m_visibleProjectIndexes.size()) {
        setActiveProjectByRow(-1);
        return;
    }

    const int projectIndex = m_visibleProjectIndexes.at(row);
    setActiveProjectByRow(projectIndex);

    if (m_fileWatcher && previousRoot != m_projectRoot) {
        stopMonitoring();
        startMonitoring();
    }

    const DiscoveredProject& selected = m_discoveredProjects.at(projectIndex);
    m_activityLog->addItem(QString("[%1] selected project=%2 file=%3 counts=%4")
        .arg(QDateTime::currentDateTime().toString(Qt::ISODateWithMs),
            selected.name,
            selected.primaryProjectFile,
            summarizeTypeCounts()));
    m_activityLog->scrollToBottom();
    refreshTrayVersionMenu();
}

void MainWindow::setupTray() {
    m_trayIcon = new QSystemTrayIcon(style()->standardIcon(QStyle::SP_DriveFDIcon), this);
    m_trayMenu = new QMenu(this);

    auto* showAction = m_trayMenu->addAction(AppStrings::TrayShowApp);
    connect(showAction, &QAction::triggered, this, [this]() {
        showNormal();
        raise();
        activateWindow();
    });

    m_trayProjectMenu = m_trayMenu->addMenu(AppStrings::TraySelectProject);
    m_trayVersionMenu = m_trayMenu->addMenu(AppStrings::TrayPastVersions);

    m_trayMenu->addSeparator();
    auto* quitAction = m_trayMenu->addAction(AppStrings::TrayQuit);
    connect(quitAction, &QAction::triggered, qApp, &QApplication::quit);

    m_trayIcon->setContextMenu(m_trayMenu);
    connect(m_trayIcon, &QSystemTrayIcon::activated, this, [this](QSystemTrayIcon::ActivationReason reason) {
        if (reason == QSystemTrayIcon::Trigger) {
            if (isVisible()) {
                hide();
            } else {
                showNormal();
                raise();
                activateWindow();
            }
        }
    });

    m_trayIcon->show();
}

void MainWindow::refreshTrayProjectMenu() {
    if (!m_trayProjectMenu) {
        return;
    }

    m_trayProjectMenu->clear();
    if (m_discoveredProjects.isEmpty()) {
        auto* noneAction = m_trayProjectMenu->addAction(AppStrings::TrayNoProjectsDiscovered);
        noneAction->setEnabled(false);
        return;
    }

    for (int i = 0; i < m_discoveredProjects.size(); ++i) {
        const DiscoveredProject& project = m_discoveredProjects.at(i);
        const QString primaryProjectPath = QDir(project.rootPath).filePath(project.primaryProjectFile);
        QIcon projectIcon = nativeAppIconForFile(primaryProjectPath);
        if (projectIcon.isNull()) {
            projectIcon = iconForProjectKind(project.kind);
        }

        QAction* action = m_trayProjectMenu->addAction(projectIcon, QString("%1 (%2)")
                .arg(project.name, ProjectDiscovery::kindToString(project.kind)));
        connect(action, &QAction::triggered, this, [this, i]() {
            const int visibleRow = visibleRowForProjectIndex(i);
            m_projectList->setCurrentRow(visibleRow);
            setActiveProjectByRow(i);
            refreshTrayVersionMenu();
        });
    }
}

void MainWindow::refreshTrayVersionMenu() {
    if (!m_trayVersionMenu) {
        return;
    }

    m_trayVersionMenu->clear();
    if (m_projectRoot.isEmpty()) {
        auto* noProject = m_trayVersionMenu->addAction(AppStrings::TraySelectProjectFirst);
        noProject->setEnabled(false);
        return;
    }

    const QString logPath = QDir(m_projectRoot).filePath(AppStrings::MusitVersionLogRelativePath);
    QFile logFile(logPath);
    if (!logFile.exists() || !logFile.open(QIODevice::ReadOnly | QIODevice::Text)) {
        auto* noHistory = m_trayVersionMenu->addAction(AppStrings::TrayNoVersionHistoryYet);
        noHistory->setEnabled(false);
        return;
    }

    QList<QJsonObject> entries;
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
        if (obj.value("staged").toString().isEmpty() || obj.value("path").toString().isEmpty()) {
            continue;
        }

        entries.append(obj);
    }

    if (entries.isEmpty()) {
        auto* noHistory = m_trayVersionMenu->addAction(AppStrings::TrayNoRestorableVersionsYet);
        noHistory->setEnabled(false);
        return;
    }

    const qsizetype limit = std::min<qsizetype>(entries.size(), 20);
    const qsizetype start = entries.size() - limit;
    for (qsizetype i = start; i < entries.size(); ++i) {
        const QJsonObject obj = entries.at(i);
        const QString relPath = obj.value("path").toString();
        const QString stagedPath = obj.value("staged").toString();
        QString versionId = obj.value("version").toString();
        if (versionId.isEmpty()) {
            versionId = QString::number(i + 1);
        }

        QAction* action = m_trayVersionMenu->addAction(QString("v%1  %2").arg(versionId, relPath));
        connect(action, &QAction::triggered, this, [this, relPath, stagedPath, versionId]() {
            if (restoreFromVersionAction(relPath, stagedPath, versionId)) {
                m_activityLog->addItem(QString("[%1] restored %2 from %3")
                    .arg(QDateTime::currentDateTime().toString(Qt::ISODateWithMs), relPath, stagedPath));
                m_activityLog->scrollToBottom();
                if (m_trayIcon) {
                    m_trayIcon->showMessage(AppStrings::AppName, QString(AppStrings::TrayRestoredFmt).arg(relPath), QSystemTrayIcon::Information, 2500);
                }
            } else {
                m_activityLog->addItem(QString("[%1] restore failed for %2")
                    .arg(QDateTime::currentDateTime().toString(Qt::ISODateWithMs), relPath));
                m_activityLog->scrollToBottom();
            }
        });
    }
}

bool MainWindow::restoreFromVersionAction(const QString& relativePath, const QString& stagedPath, const QString& restoredVersionId) {
    if (m_projectRoot.isEmpty() || relativePath.isEmpty() || stagedPath.isEmpty()) {
        return false;
    }

    QFile source(stagedPath);
    if (!source.exists()) {
        return false;
    }

    const QString targetPath = QDir(m_projectRoot).filePath(relativePath);

    if (m_openedProjectProcess && m_openedProjectProcess->state() != QProcess::NotRunning) {
        m_suppressNextExternalCloseFocus = true;
        m_openedProjectProcess->kill();
        m_openedProjectProcess->waitForFinished(1500);
        m_openedProjectProcess = nullptr;
    }

    // First, try quitting the DAW we most recently launched for this project file.
    if (!m_lastOpenedAppBundleId.isEmpty()) {
        quitAppByBundleId(m_lastOpenedAppBundleId);
        forceKillAppByBundleId(m_lastOpenedAppBundleId);
    }

    // Fallback: if a DAW is currently frontmost, request it to quit as well.
    const QString frontmostBundleId = frontmostAppBundleId();
    if (!frontmostBundleId.isEmpty()
        && frontmostBundleId != QStringLiteral("com.apple.dt.Xcode")
        && frontmostBundleId != QStringLiteral("com.microsoft.VSCode")
        && frontmostBundleId != QStringLiteral("com.github.CopilotChat")
        && isLikelyDawBundleId(frontmostBundleId)) {
        quitAppByBundleId(frontmostBundleId);
        forceKillAppByBundleId(frontmostBundleId);
    }

    // Cross-platform hard fallback for DAWs that ignore polite quit requests.
    forceKillKnownDawProcessesCli();

    const QList<qint64> pids = pidsUsingFilePath(targetPath);
    if (!pids.isEmpty()) {
        const bool killed = killProcesses(pids);
        if (!killed) {
            m_activityLog->addItem(QString("[%1] restore blocked: failed to terminate process using %2")
                .arg(QDateTime::currentDateTime().toString(Qt::ISODateWithMs), targetPath));
            m_activityLog->scrollToBottom();
            return false;
        }

        const QList<qint64> remainingPids = pidsUsingFilePath(targetPath);
        if (!remainingPids.isEmpty()) {
            m_activityLog->addItem(QString("[%1] restore blocked: file still in use %2")
                .arg(QDateTime::currentDateTime().toString(Qt::ISODateWithMs), targetPath));
            m_activityLog->scrollToBottom();
            return false;
        }

        m_activityLog->addItem(QString("[%1] terminated process(es) using %2")
            .arg(QDateTime::currentDateTime().toString(Qt::ISODateWithMs), targetPath));
        m_activityLog->scrollToBottom();
    }

    const QFileInfo targetInfo(targetPath);
    if (!QDir().mkpath(targetInfo.dir().absolutePath())) {
        return false;
    }

    if (m_snapshotService) {
        m_snapshotService->setBranchBaseForPath(relativePath, restoredVersionId);
        // Restore writes can produce multiple filesystem notifications; suppress them.
        m_snapshotService->suppressNextEventsForPath(relativePath, 2);
    }

    QFile::remove(targetPath);
    if (!QFile::copy(stagedPath, targetPath)) {
        return false;
    }

#if defined(Q_OS_MAC)
    const bool reopened = QProcess::startDetached("/usr/bin/open", {targetPath});
#else
    const bool reopened = QDesktopServices::openUrl(QUrl::fromLocalFile(targetPath));
#endif

    if (reopened) {
        m_activityLog->addItem(QString("[%1] reopened %2 after restore")
            .arg(QDateTime::currentDateTime().toString(Qt::ISODateWithMs), targetPath));
        m_activityLog->scrollToBottom();
    }

    return reopened;
}

void MainWindow::closeEvent(QCloseEvent* event) {
    if (m_trayIcon && m_trayIcon->isVisible()) {
        hide();
        event->ignore();
        m_trayIcon->showMessage(AppStrings::AppName, AppStrings::TrayStillRunning, QSystemTrayIcon::Information, 2000);
        return;
    }

    QMainWindow::closeEvent(event);
}

void MainWindow::openProjectFromRow(int row) {
    if (row < 0 || row >= m_discoveredProjects.size()) {
        return;
    }

    if (m_openedProjectProcess && m_openedProjectProcess->state() != QProcess::NotRunning) {
        appendActivityLog(QString("[%1] launch ignored, waiting for previously opened app to quit")
            .arg(QDateTime::currentDateTime().toString(Qt::ISODateWithMs)),
            ActivityLogLevel::Debug);
        return;
    }

    m_projectList->setCurrentRow(row);
    setActiveProjectByRow(row);

    DiscoveredProject& project = m_discoveredProjects[row];
    if (project.primaryProjectFile.isEmpty() && !project.projectFiles.isEmpty()) {
        project.primaryProjectFile = project.projectFiles.first();
    }

    if (project.primaryProjectFile.isEmpty()) {
        m_activityLog->addItem(QString("[%1] open failed: no project file selected")
            .arg(QDateTime::currentDateTime().toString(Qt::ISODateWithMs)));
        m_activityLog->scrollToBottom();
        return;
    }

    const QString projectFilePath = QDir(project.rootPath).filePath(project.primaryProjectFile);
    if (!QFileInfo::exists(projectFilePath)) {
        m_activityLog->addItem(QString("[%1] open failed: missing file %2")
            .arg(QDateTime::currentDateTime().toString(Qt::ISODateWithMs), projectFilePath));
        m_activityLog->scrollToBottom();
        return;
    }

    if (!m_fileWatcher) {
        startMonitoring();
    }

    if (!m_fileWatcher) {
        m_activityLog->addItem(QString("[%1] open cancelled: monitoring did not start")
            .arg(QDateTime::currentDateTime().toString(Qt::ISODateWithMs)));
        m_activityLog->scrollToBottom();
        return;
    }

    if (!m_snapshotService) {
        m_activityLog->addItem(QString("[%1] open cancelled: snapshot service unavailable")
            .arg(QDateTime::currentDateTime().toString(Qt::ISODateWithMs)));
        m_activityLog->scrollToBottom();
        return;
    }

    QStringList baselineFiles = project.projectFiles;
    if (!project.primaryProjectFile.isEmpty() && !baselineFiles.contains(project.primaryProjectFile)) {
        baselineFiles.prepend(project.primaryProjectFile);
    }

    int baselineSuccess = 0;
    int baselineFail = 0;
    for (const QString& relativeProjectFile : baselineFiles) {
        const QString absoluteProjectFile = QDir(project.rootPath).filePath(relativeProjectFile);
        const bool baselineCaptured = m_snapshotService->snapshotFileNow(absoluteProjectFile, relativeProjectFile);
        if (baselineCaptured) {
            ++baselineSuccess;
        } else {
            ++baselineFail;
        }
    }

    if (baselineFiles.size() > 1) {
        appendActivityLog(QString("[%1] baseline initialized for %2 files (%3 ok, %4 skipped/failed)")
            .arg(QDateTime::currentDateTime().toString(Qt::ISODateWithMs))
            .arg(baselineFiles.size())
            .arg(baselineSuccess)
            .arg(baselineFail),
            ActivityLogLevel::Info);
    } else if (baselineFail > 0) {
        appendActivityLog(QString("[%1] warning: baseline snapshot failed for %2")
            .arg(QDateTime::currentDateTime().toString(Qt::ISODateWithMs), project.primaryProjectFile),
            ActivityLogLevel::Info);
    }

#if defined(Q_OS_MAC)
    auto* process = new QProcess(this);
    m_openedProjectProcess = process;
    m_lastOpenedAppBundleId = guessedBundleIdForProjectKind(project.kind);

    connect(process, &QProcess::finished, this, [this, process, projectFilePath](int, QProcess::ExitStatus) {
        if (m_openedProjectProcess == process) {
            m_openedProjectProcess = nullptr;
            m_activityLog->addItem(QString("[%1] app closed for %2")
                .arg(QDateTime::currentDateTime().toString(Qt::ISODateWithMs), projectFilePath));
            m_activityLog->scrollToBottom();
            if (m_suppressNextExternalCloseFocus) {
                m_suppressNextExternalCloseFocus = false;
            } else {
                showNormal();
                raise();
                activateWindow();
            }
        }
        process->deleteLater();
    });

    process->start("/usr/bin/open", {"-W", projectFilePath});
    if (!process->waitForStarted(3000)) {
        m_openedProjectProcess = nullptr;
        m_lastOpenedAppBundleId.clear();
        m_activityLog->addItem(QString("[%1] open failed for %2")
            .arg(QDateTime::currentDateTime().toString(Qt::ISODateWithMs), projectFilePath));
        m_activityLog->scrollToBottom();
        process->deleteLater();
        return;
    }

    const QString frontmostBundleId = frontmostAppBundleId();
    if (isLikelyDawBundleId(frontmostBundleId)) {
        m_lastOpenedAppBundleId = frontmostBundleId;
    }

    m_activityLog->addItem(QString("[%1] opened %2 and waiting for app to quit")
        .arg(QDateTime::currentDateTime().toString(Qt::ISODateWithMs), projectFilePath));
    m_activityLog->scrollToBottom();
#else
    const bool opened = QDesktopServices::openUrl(QUrl::fromLocalFile(projectFilePath));
    if (!opened) {
        m_activityLog->addItem(QString("[%1] open failed for %2")
            .arg(QDateTime::currentDateTime().toString(Qt::ISODateWithMs), projectFilePath));
        m_activityLog->scrollToBottom();
        return;
    }

    m_activityLog->addItem(QString("[%1] opened %2 (auto-stop on app quit is currently macOS-only)")
        .arg(QDateTime::currentDateTime().toString(Qt::ISODateWithMs), projectFilePath));
    m_activityLog->scrollToBottom();
#endif

    m_lastOpenedAtByProjectRoot.insert(project.rootPath, QDateTime::currentDateTime());
    if (m_projectSortMode == ProjectSortMode::LastOpened) {
        refreshProjectListFilterAndSort();
    }
}

void MainWindow::onProjectSearchChanged(const QString& text) {
    Q_UNUSED(text);
    refreshProjectListFilterAndSort();
}

void MainWindow::onProjectSortChanged(int index) {
    if (!m_projectSortCombo || index < 0) {
        return;
    }

    const int rawMode = m_projectSortCombo->itemData(index).toInt();
    m_projectSortMode = static_cast<ProjectSortMode>(rawMode);
    refreshProjectListFilterAndSort();
}

QString MainWindow::summarizeTypeCounts() const {
    if (m_totalProjectFileCount <= 0) {
        return "none";
    }

    auto pct = [this](int count) {
        return static_cast<double>(count * 100.0) / static_cast<double>(m_totalProjectFileCount);
    };

    QStringList parts;
    for (const ProjectKind kind : ProjectDiscovery::knownKinds()) {
        const int count = m_activeTypeCounts.value(kind);
        if (count <= 0) {
            continue;
        }
        parts << QString("%1=%2 (%3%)")
            .arg(ProjectDiscovery::kindToString(kind))
            .arg(count)
            .arg(pct(count), 0, 'f', 1);
    }

    if (parts.isEmpty()) {
        return "none";
    }
    return parts.join(", ");
}
