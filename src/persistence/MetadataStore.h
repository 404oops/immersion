#pragma once

#include <QString>

#include "../core/FileEvent.h"

class MetadataStore {
public:
    explicit MetadataStore(QString musitRoot = {});

    void setMusitRoot(const QString& musitRoot);
    bool init();

    bool appendSnapshotEvent(
        const FileEvent& event,
        const QString& objectHash,
        const QString& stagedPath,
        const QString& branchBaseVersion = {});
    QString latestStagedVersionForPath(const QString& relativePath) const;

private:
    QString nextVersionIdForPath(const QString& relativePath, const QString& branchBaseVersion) const;
    QString m_musitRoot;
};
