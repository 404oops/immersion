#include "ProjectDiscoveryScanWorker.h"

#include <QElapsedTimer>
#include <QtGlobal>

ProjectDiscoveryScanWorker::ProjectDiscoveryScanWorker(QObject* parent)
    : QObject(parent) {}

void ProjectDiscoveryScanWorker::setCancelledFlag(
    const std::shared_ptr<std::atomic<bool>>& cancelled) {
    m_cancelled = cancelled;
}

void ProjectDiscoveryScanWorker::scan(const QString& folderPath) {
    QElapsedTimer timer;
    timer.start();

    int directoriesScanned = 0;
    const QList<DiscoveredProject> projects = m_discovery.discoverAll(
        folderPath,
        [this, &directoriesScanned](const QString& directoryPath) {
            ++directoriesScanned;
            emit directoryScanned(directoryPath, directoriesScanned);
        },
        m_cancelled,
        [this, folderPath](const QList<DiscoveredProject>& partialProjects, int scannedDirectories) {
            emit projectsUpdated(partialProjects, scannedDirectories, folderPath);
        },
        20);

    if (m_cancelled && m_cancelled->load()) {
        return;
    }

    emit scanCompleted(projects, timer.elapsed(), directoriesScanned, folderPath);
}
