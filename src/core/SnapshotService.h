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
    bool snapshotFileNow(const QString& absolutePath, const QString& relativePath);
    bool hasStagedVersionForPath(const QString& relativePath) const;
    void suppressNextEventsForPath(const QString& relativePath, int count = 1);
    void setBranchBaseForPath(const QString& relativePath, const QString& versionId);

public slots:
    void onFileEvent(const FileEvent& event);

signals:
    void snapshotCreated(const QString& message);
    void snapshotSkipped(const QString& reason);
    void snapshotError(const QString& error);

private:
    ProjectConfig m_projectConfig;
    ObjectStore m_objectStore;
    MetadataStore m_metadataStore;
    QHash<QString, int> m_suppressedEventsByPath;
    QHash<QString, QString> m_branchBaseByPath;
};
