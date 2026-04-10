#pragma once

#include <QMainWindow>
#include <QList>
#include <QHash>
#include <QDateTime>
#include <memory>

#include "../core/ProjectDiscovery.h"

class QLabel;
class QLineEdit;
class QListWidget;
class QListWidgetItem;
class QPushButton;
class QComboBox;
class QIcon;
class QMenu;
class QSystemTrayIcon;
class QAction;
class QCloseEvent;
class QProcess;

class IFileWatcher;
class SnapshotService;
class ProjectDiscovery;
class ProjectRegistry;

class MainWindow : public QMainWindow {
    Q_OBJECT
public:
    MainWindow();
    ~MainWindow() override;

private slots:
    void chooseProjectFolder();
    void startMonitoring();
    void stopMonitoring();
    void onSnapshotMessage(const QString& message);
    void onDiscoveredProjectSelected(int row);
    void onLogLevelChanged(int index);
    void onProjectSearchChanged(const QString& text);
    void onProjectSortChanged(int index);

private:
    enum class ActivityLogLevel {
        Debug = 0,
        Info = 1,
        Error = 2,
    };

    enum class ProjectSortMode {
        Name = 0,
        LastOpened = 1,
    };

    bool loadProjectsFromFolder(const QString& folder, bool persistFolder);
    void setStatus(const QString& status);
    void appendActivityLog(const QString& line, ActivityLogLevel level = ActivityLogLevel::Info);
    QString summarizeTypeCounts() const;
    void populateProjectList();
    void refreshProjectListFilterAndSort();
    int visibleRowForProjectIndex(int projectIndex) const;
    QWidget* createProjectRowWidget(int index);
    void setActiveProjectByRow(int row);
    QIcon iconForProjectKind(ProjectKind kind) const;
    void setupTray();
    void refreshTrayProjectMenu();
    void refreshTrayVersionMenu();
    void showRestoreVersionMenuForRow(int row, QWidget* anchorWidget);
    void showManageVersionsDialogForRow(int row, QWidget* anchorWidget);
    bool restoreFromVersionAction(const QString& relativePath, const QString& stagedPath, const QString& restoredVersionId);
    void openProjectFromRow(int row);

    QString m_projectRoot;
    QString m_registryPath;
    QString m_lastOpenedAppBundleId;
    QLabel* m_statusLabel {nullptr};
    QLineEdit* m_projectSearchEdit {nullptr};
    QComboBox* m_projectSortCombo {nullptr};
    QListWidget* m_projectList {nullptr};
    QListWidget* m_activityLog {nullptr};
    QComboBox* m_logLevelCombo {nullptr};
    QPushButton* m_chooseButton {nullptr};
    ActivityLogLevel m_minLogLevel {ActivityLogLevel::Info};

    std::unique_ptr<IFileWatcher> m_fileWatcher;
    std::unique_ptr<SnapshotService> m_snapshotService;
    std::unique_ptr<ProjectDiscovery> m_projectDiscovery;
    std::unique_ptr<ProjectRegistry> m_projectRegistry;
    QList<DiscoveredProject> m_discoveredProjects;
    QList<int> m_visibleProjectIndexes;
    QHash<QString, QDateTime> m_lastOpenedAtByProjectRoot;
    ProjectSortMode m_projectSortMode {ProjectSortMode::Name};
    QHash<ProjectKind, int> m_activeTypeCounts;
    int m_totalProjectFileCount {0};
    QSystemTrayIcon* m_trayIcon {nullptr};
    QMenu* m_trayMenu {nullptr};
    QMenu* m_trayProjectMenu {nullptr};
    QMenu* m_trayVersionMenu {nullptr};
    QProcess* m_openedProjectProcess {nullptr};
    bool m_suppressNextExternalCloseFocus {false};

protected:
    void closeEvent(QCloseEvent* event) override;
};
