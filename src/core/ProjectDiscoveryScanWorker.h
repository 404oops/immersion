#pragma once

#include <QObject>
#include <QString>

#include <atomic>
#include <memory>

#include "ProjectDiscovery.h"

class ProjectDiscoveryScanWorker final : public QObject {
    Q_OBJECT

public:
    explicit ProjectDiscoveryScanWorker(QObject* parent = nullptr);

    void setCancelledFlag(const std::shared_ptr<std::atomic<bool>>& cancelled);

public slots:
    void scan(const QString& folderPath, int layout, int scanGeneration);

signals:
    void directoryScanned(const QString& directoryPath, int directoriesScanned);
    void projectsUpdated(const QList<DiscoveredProject>& partialProjects,
                         int directoriesScanned,
                         const QString& folderPath,
                         int scanGeneration);
    void scanCompleted(const QList<DiscoveredProject>& projects,
                       qint64 elapsedMs,
                       int directoriesScanned,
                       const QString& folderPath,
                       int scanGeneration);

private:
    std::shared_ptr<std::atomic<bool>> m_cancelled;
    ProjectDiscovery m_discovery;
};
