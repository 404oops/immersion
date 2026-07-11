#include "ProjectDiscoveryScanWorker.h"

#include <QElapsedTimer>
#include <QtGlobal>

ProjectDiscoveryScanWorker::ProjectDiscoveryScanWorker(QObject* parent)
    : QObject(parent) {}

void ProjectDiscoveryScanWorker::setCancelledFlag(
    const std::shared_ptr<std::atomic<bool>>& cancelled) {
    m_cancelled = cancelled;
}

void ProjectDiscoveryScanWorker::scan(const QString& folderPath, int layout, int scanGeneration) {
    QElapsedTimer timer;
    timer.start();

    // Keep this scan tied to the cancellation token it started with. A later
    // layout rescan replaces m_cancelled for the next queued scan.
    const std::shared_ptr<std::atomic<bool>> cancelled = m_cancelled;
    int directoriesScanned = 0;
    const ProjectsFolderLayout folderLayout = static_cast<ProjectsFolderLayout>(layout);
    const QList<DiscoveredProject> projects = m_discovery.discoverAll(
        folderPath,
        [this, &directoriesScanned](const QString& directoryPath) {
            ++directoriesScanned;
            emit directoryScanned(directoryPath, directoriesScanned);
        },
        cancelled,
        [this, folderPath, scanGeneration](const QList<DiscoveredProject>& partialProjects,
                                           int scannedDirectories) {
            emit projectsUpdated(partialProjects, scannedDirectories, folderPath, scanGeneration);
        },
        20,
        folderLayout);

    if (cancelled && cancelled->load()) {
        return;
    }

    emit scanCompleted(projects, timer.elapsed(), directoriesScanned, folderPath, scanGeneration);
}
