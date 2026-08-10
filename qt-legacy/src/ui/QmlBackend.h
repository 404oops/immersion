#pragma once

#include <QDateTime>
#include <QHash>
#include <QObject>
#include <QString>
#include <QStringList>
#include <QThread>
#include <QTimer>
#include <QUrl>
#include <QVariantList>

#include <atomic>
#include <memory>
#include <vector>

#include "../core/ProjectDiscovery.h"
#include "../core/ProjectDiscoveryScanWorker.h"
#include "../core/BackupTemplate.h"
#include "../persistence/ProjectsFolderSettings.h"

struct FileEvent;

class ProjectRegistry;
class ProjectDiscovery;

class IFileWatcher;
class SnapshotService;

class QmlBackend : public QObject {
    Q_OBJECT
    Q_PROPERTY(QString statusMessage READ statusMessage NOTIFY statusMessageChanged)
    Q_PROPERTY(QVariantList projects READ projects NOTIFY projectsChanged)
    Q_PROPERTY(bool hasDiscoveredProjects READ hasDiscoveredProjects NOTIFY projectsChanged)
    Q_PROPERTY(bool hasProjectsFolder READ hasProjectsFolder NOTIFY projectsFolderChanged)
    Q_PROPERTY(bool isScanningProjects READ isScanningProjects NOTIFY isScanningProjectsChanged)
    Q_PROPERTY(QStringList activity READ activity NOTIFY activityChanged)
    Q_PROPERTY(QString logLevel READ logLevel WRITE setLogLevel NOTIFY logLevelChanged)
    Q_PROPERTY(QString searchText READ searchText WRITE setSearchText NOTIFY searchTextChanged)
    Q_PROPERTY(QString sortMode READ sortMode WRITE setSortMode NOTIFY sortModeChanged)
    Q_PROPERTY(int selectedProjectIndex READ selectedProjectIndex WRITE setSelectedProjectIndex NOTIFY selectedProjectIndexChanged)
    Q_PROPERTY(QVariantList selectedProjectVersionGraph READ selectedProjectVersionGraph NOTIFY selectedProjectVersionGraphChanged)
    Q_PROPERTY(QString selectedProjectNote READ selectedProjectNote WRITE setSelectedProjectNote NOTIFY selectedProjectNoteChanged)
    Q_PROPERTY(QStringList selectedProjectFiles READ selectedProjectFiles NOTIFY selectedProjectFilesChanged)
    Q_PROPERTY(QString selectedProjectPrimaryFile READ selectedProjectPrimaryFile WRITE setSelectedProjectPrimaryFile NOTIFY selectedProjectPrimaryFileChanged)
    Q_PROPERTY(QString projectsFolderPath READ projectsFolderPath NOTIFY projectsFolderChanged)
    Q_PROPERTY(QString projectsFolderLayout READ projectsFolderLayout NOTIFY projectsFolderLayoutChanged)
    Q_PROPERTY(QString pendingProjectsFolderSetup READ pendingProjectsFolderSetup NOTIFY pendingProjectsFolderSetupChanged)
    Q_PROPERTY(double themeHue READ themeHue WRITE setThemeHue NOTIFY themeHueChanged)
    Q_PROPERTY(bool launchAtStartup READ launchAtStartup WRITE setLaunchAtStartup NOTIFY launchAtStartupChanged)
    Q_PROPERTY(bool launchAtStartupSupported READ launchAtStartupSupported CONSTANT)
    Q_PROPERTY(int snapshotRetention READ snapshotRetention WRITE setSnapshotRetention NOTIFY snapshotRetentionChanged)
    Q_PROPERTY(bool notificationsEnabled READ notificationsEnabled WRITE setNotificationsEnabled NOTIFY notificationsEnabledChanged)
    Q_PROPERTY(QString colorSchemeMode READ colorSchemeMode WRITE setColorSchemeMode NOTIFY colorSchemeModeChanged)

public:
    explicit QmlBackend(QObject* parent = nullptr);
    ~QmlBackend() override;

    QString statusMessage() const;
    QVariantList projects() const;
    bool hasDiscoveredProjects() const;
    bool hasProjectsFolder() const;
    bool isScanningProjects() const;
    QStringList activity() const;
    QString logLevel() const;
    void setLogLevel(const QString& value);

    QString searchText() const;
    void setSearchText(const QString& value);

    QString sortMode() const;
    void setSortMode(const QString& value);

    int selectedProjectIndex() const;
    void setSelectedProjectIndex(int value);
    QVariantList selectedProjectVersionGraph() const;
    QString selectedProjectNote() const;
    void setSelectedProjectNote(const QString& value);
    QStringList selectedProjectFiles() const;
    QString selectedProjectPrimaryFile() const;
    void setSelectedProjectPrimaryFile(const QString& value);

    QString projectsFolderPath() const;
    QString projectsFolderLayout() const;
    QString pendingProjectsFolderSetup() const;
    double themeHue() const;
    void setThemeHue(double value);
    bool launchAtStartup() const;
    void setLaunchAtStartup(bool value);
    bool launchAtStartupSupported() const;

    int snapshotRetention() const;
    void setSnapshotRetention(int value);
    bool notificationsEnabled() const;
    void setNotificationsEnabled(bool value);
    QString colorSchemeMode() const;
    void setColorSchemeMode(const QString& value);

    Q_INVOKABLE void loadProjectsFromFolder(const QString& folderPath,
                                            const QString& layoutOverride = QString());
    Q_INVOKABLE void confirmProjectsFolder(const QString& folderPath, const QString& layout);
    Q_INVOKABLE void reselectProjectsFolderLayout(const QString& layout);
    Q_INVOKABLE QString projectsFolderLayoutForPath(const QString& folderPath) const;
    Q_INVOKABLE QString displayLocalPath(const QString& urlOrPath) const;
    Q_INVOKABLE void openProject(int visibleIndex);
    Q_INVOKABLE void manageProjectVersions(int visibleIndex);
    Q_INVOKABLE QVariantList getProjectVersions(int visibleIndex) const;
    Q_INVOKABLE bool restoreVersionById(const QString& versionId);
    Q_INVOKABLE bool saveVersionNote(const QString& versionId, const QString& note);
    Q_INVOKABLE bool deleteVersionById(const QString& versionId);
    Q_INVOKABLE bool resetConfig();
    Q_INVOKABLE bool exportActivityLog(const QUrl& fileUrl);
    Q_INVOKABLE QUrl defaultActivityLogExportFolderUrl() const;
    Q_INVOKABLE QUrl defaultActivityLogExportFileUrl() const;

signals:
    void projectSaveRecorded(const QString& projectName,
                             const QString& versionLabel,
                             const QString& relativePath);
    void statusMessageChanged();
    void projectsChanged();
    void projectsFolderChanged();
    void isScanningProjectsChanged();
    void activityChanged();
    void logLevelChanged();
    void searchTextChanged();
    void sortModeChanged();
    void selectedProjectIndexChanged();
    void selectedProjectVersionGraphChanged();
    void selectedProjectNoteChanged();
    void selectedProjectFilesChanged();
    void selectedProjectPrimaryFileChanged();
    void projectsFolderLayoutChanged();
    void pendingProjectsFolderSetupChanged();
    void projectsFolderScanFinished(bool foundProjects);
    void themeHueChanged();
    void launchAtStartupChanged();
    void snapshotRetentionChanged();
    void notificationsEnabledChanged();
    void colorSchemeModeChanged();
    void configReset();

private:
    enum class SortMode {
        Name,
        LastOpened,
    };

    enum class LogLevel {
        Info,
        Debug,
    };

    enum class ColorSchemeMode {
        System,
        Light,
        Dark,
    };

    void runStartupSelfCheck();
    void applySelection(int visibleIndex);

    void appendActivityWithLevel(const QString& line, LogLevel level);
    void appendActivity(const QString& line);
    void appendDebugActivity(const QString& line);
    void rebuildVisibleActivity();
    void rebuildVisibleProjects();
    void startMonitoringDeferred();
    void advanceMonitoringInit();
    bool initMonitoringForProject(const DiscoveredProject& project);
    void startFileWatcherIfReady();
    void stopMonitoring();
    void cancelProjectScan();
    void setScanningProjects(bool scanning);
    void applyDiscoveredProjects(const QList<DiscoveredProject>& projects);
    void applySavedPrimaryFileOverrides(QList<DiscoveredProject>& projects) const;
    void finishLoadingProjects(const QList<DiscoveredProject>& projects, const QString& cleanPath);
    void persistAppSettings();
    void flushPendingThemeHuePersist();
    void logConfigChange(const QString& detail);
    void trimActivityLog();
    void applySnapshotRetentionToServices(bool compactExisting = false);
    SortMode sortModeFromString(const QString& value) const;
    LogLevel logLevelFromString(const QString& value) const;
    ColorSchemeMode colorSchemeModeFromString(const QString& value) const;
    QString colorSchemeModeToString(ColorSchemeMode mode) const;
    void handleProjectScanDirectory(const QString& directoryPath, int directoriesScanned);
    void handleProjectsUpdated(const QList<DiscoveredProject>& partialProjects,
                               int directoriesScanned,
                               const QString& folderPath,
                               int scanGeneration);
    void handleProjectScanCompleted(const QList<DiscoveredProject>& projects,
                                    qint64 elapsedMs,
                                    int directoriesScanned,
                                    const QString& folderPath,
                                    int scanGeneration);
    bool adoptDiscoveredProject(const DiscoveredProject& project);
    bool containsDiscoveredProject(const DiscoveredProject& candidate) const;
    bool tryDiscoverProjectFromEvent(const FileEvent& event);
    bool dispatchFileEvent(const FileEvent& event);
    SnapshotService* snapshotServiceForRoot(const QString& projectRoot);

    QString m_statusMessage;
    QString m_projectsFolderRoot;
    QString m_pendingProjectsFolderSetup;
    ProjectsFolderLayout m_projectsFolderLayout {ProjectsFolderLayout::Bundles};
    QString m_projectRoot;
    QVariantList m_projects;
    QStringList m_activity;
    QStringList m_activityAll;
    QList<LogLevel> m_activityLevels;
    LogLevel m_logLevel {LogLevel::Info};
    QString m_searchText;
    SortMode m_sortMode {SortMode::Name};
    int m_selectedProjectIndex {-1};
    QString m_selectedProjectNote;
    QString m_selectedProjectPrimaryFile;
    double m_themeHue {280.0};
    QTimer m_themeHuePersistTimer;
    bool m_launchAtStartup {false};
    int m_snapshotRetention {BackupTemplates::kUncompressedRecentVersions};
    bool m_notificationsEnabled {true};
    ColorSchemeMode m_colorSchemeMode {ColorSchemeMode::System};

    QList<DiscoveredProject> m_discoveredProjects;
    QList<int> m_visibleProjectIndexes;
    QHash<QString, QDateTime> m_lastOpenedAtByProjectRoot;
    QHash<QString, QString> m_projectNotes;
    QHash<QString, QString> m_projectPrimaryFiles;

    std::unique_ptr<ProjectRegistry> m_projectRegistry;
    ProjectsFolderSettings m_projectsFolderSettings;
    QThread m_discoveryThread;
    ProjectDiscoveryScanWorker* m_discoveryWorker {nullptr};
    std::shared_ptr<std::atomic<bool>> m_scanCancelFlag;
    bool m_isScanningProjects {false};
    int m_lastScanStatusDirectories {0};
    QString m_activeScanFolder;
    int m_activeScanGeneration {0};
    int m_monitorInitIndex {0};
    bool m_monitorInitActive {false};
    ProjectDiscovery m_projectDiscovery;
    std::unique_ptr<IFileWatcher> m_fileWatcher;
    std::vector<std::unique_ptr<SnapshotService>> m_snapshotServices;
    QHash<QString, SnapshotService*> m_snapshotServiceByRoot;
    // pathKey(root) -> root as discovered (hash keys may be lowercased).
    QHash<QString, QString> m_canonicalRootByKey;
};
