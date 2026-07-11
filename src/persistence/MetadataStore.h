#pragma once

#include <QJsonObject>
#include <QString>
#include <QStringList>

#include "../core/FileEvent.h"

class MetadataStore {
public:
    explicit MetadataStore(QString musitRoot = {});

    void setMusitRoot(const QString& musitRoot);
    bool init();

    // Versions are numbered per artifact (a project file, or a bundle root
    // like "Song.logicx" whose internal files share one version per save).
    QString nextVersionIdForArtifact(const QString& artifact, const QString& branchBaseVersion) const;
    QString latestStagedVersionForArtifact(const QString& artifact) const;

    // Appends one snapshot line. versionId/parentVersion are pre-allocated
    // by the caller so several files of one save can share a version.
    bool appendSnapshotEvent(
        const FileEvent& event,
        const QString& artifact,
        const QString& objectHash,
        const QString& stagedPath,
        const QString& versionId = {},
        const QString& parentVersion = {});

    // Staged paths (as stored in the log) of all but the `keepCount` most
    // recently appended versions of a path; used to compact old versions
    // down to their compressed objects.
    QStringList stagedPathsBeyondNewest(const QString& relativePath, int keepCount) const;
    QStringList snapshotPaths() const;

    // Artifact recorded on a log line, with fallback for legacy lines that
    // predate the artifact field (they were always single-file versions).
    static QString artifactOfLogLine(const QJsonObject& obj);

private:
    QString m_musitRoot;
};
