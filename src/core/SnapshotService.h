#pragma once

#include <QObject>
#include <QHash>

#include "FileEvent.h"
#include "ProjectConfig.h"
#include "../persistence/MetadataStore.h"
#include "../persistence/ObjectStore.h"

class SnapshotService : public QObject {
    Q_OBJECT
public:
    explicit SnapshotService(QObject* parent = nullptr);

    bool setProjectRoot(const QString& rootPath);

    // Snapshots a file, or every tracked file inside a directory bundle
    // (.logicx/.band) as one grouped version. Used for baselines and for
    // preserving unversioned state before a restore.
    bool snapshotPathNow(const QString& absolutePath, const QString& relativePath);

    bool hasVersionForArtifact(const QString& artifact) const;
    void suppressNextEventsForPath(const QString& relativePath, int count = 1);
    void setBranchBaseForArtifact(const QString& artifact, const QString& versionId);

public slots:
    void onFileEvent(const FileEvent& event);

signals:
    void snapshotCreated(const QString& message);
    // Emitted once per user save (not baselines/seeds), including the first
    // file of a grouped bundle save.
    void saveRecorded(const QString& versionId, const QString& relativePath);
    void snapshotSkipped(const QString& reason);
    void snapshotError(const QString& error);

private:
    bool createSnapshot(const FileEvent& event, bool isBaseline);
    void compactStagedCopies(const QString& relativePath);
    QString branchStateFilePath() const;
    void loadBranchState();
    void saveBranchState() const;

    ProjectConfig m_projectConfig;
    ObjectStore m_objectStore;
    MetadataStore m_metadataStore;
    QHash<QString, int> m_suppressedEventsByPath;
    QHash<QString, QString> m_branchBaseByArtifact;

    // Files of one save (same artifact, same watcher scan) share a version.
    struct ActiveGroup {
        qint64 scanSequence {0};
        QString versionId;
        QString parentVersion;
    };
    QHash<QString, ActiveGroup> m_activeGroupByArtifact;
    qint64 m_syntheticScanSequence {0};
};
