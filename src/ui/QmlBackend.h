#pragma once

#include <QDateTime>
#include <QHash>
#include <QObject>
#include <QString>
#include <QStringList>
#include <QVariantList>

#include <memory>
#include <vector>

#include "../core/ProjectDiscovery.h"

class ProjectRegistry;
class ProjectDiscovery;

class IFileWatcher;
class SnapshotService;

class QmlBackend : public QObject {
    Q_OBJECT
    Q_PROPERTY(QString statusMessage READ statusMessage NOTIFY statusMessageChanged)
    Q_PROPERTY(QVariantList projects READ projects NOTIFY projectsChanged)
    Q_PROPERTY(QStringList activity READ activity NOTIFY activityChanged)
    Q_PROPERTY(QString logLevel READ logLevel WRITE setLogLevel NOTIFY logLevelChanged)
    Q_PROPERTY(QString searchText READ searchText WRITE setSearchText NOTIFY searchTextChanged)
    Q_PROPERTY(QString sortMode READ sortMode WRITE setSortMode NOTIFY sortModeChanged)
    Q_PROPERTY(int selectedProjectIndex READ selectedProjectIndex WRITE setSelectedProjectIndex NOTIFY selectedProjectIndexChanged)
    Q_PROPERTY(QVariantList selectedProjectVersionGraph READ selectedProjectVersionGraph NOTIFY selectedProjectVersionGraphChanged)
    Q_PROPERTY(QString selectedProjectNote READ selectedProjectNote WRITE setSelectedProjectNote NOTIFY selectedProjectNoteChanged)

public:
    explicit QmlBackend(QObject* parent = nullptr);
    ~QmlBackend() override;

    QString statusMessage() const;
    QVariantList projects() const;
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

    Q_INVOKABLE void loadProjectsFromFolder(const QString& folderPath);
    Q_INVOKABLE void openProject(int visibleIndex);
    Q_INVOKABLE void manageProjectVersions(int visibleIndex);
    Q_INVOKABLE QVariantList getProjectVersions(int visibleIndex) const;
    Q_INVOKABLE bool restoreVersionById(const QString& versionId);
    Q_INVOKABLE bool saveVersionNote(const QString& versionId, const QString& note);
    Q_INVOKABLE bool deleteVersionById(const QString& versionId);

signals:
    void statusMessageChanged();
    void projectsChanged();
    void activityChanged();
    void logLevelChanged();
    void searchTextChanged();
    void sortModeChanged();
    void selectedProjectIndexChanged();
    void selectedProjectVersionGraphChanged();
    void selectedProjectNoteChanged();

private:
    enum class SortMode {
        Name,
        LastOpened,
    };

    enum class LogLevel {
        Info,
        Debug,
    };

    void runStartupSelfCheck();

    void appendActivityWithLevel(const QString& line, LogLevel level);
    void appendActivity(const QString& line);
    void appendDebugActivity(const QString& line);
    void rebuildVisibleActivity();
    void rebuildVisibleProjects();
    void startMonitoring();
    void stopMonitoring();
    SnapshotService* snapshotServiceForRoot(const QString& projectRoot);

    QString m_statusMessage;
    QString m_projectsFolderRoot;
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

    QList<DiscoveredProject> m_discoveredProjects;
    QList<int> m_visibleProjectIndexes;
    QHash<QString, QDateTime> m_lastOpenedAtByProjectRoot;
    QHash<QString, QString> m_projectNotes;

    std::unique_ptr<ProjectDiscovery> m_projectDiscovery;
    std::unique_ptr<ProjectRegistry> m_projectRegistry;
    std::unique_ptr<IFileWatcher> m_fileWatcher;
    std::vector<std::unique_ptr<SnapshotService>> m_snapshotServices;
    QHash<QString, SnapshotService*> m_snapshotServiceByRoot;
};
