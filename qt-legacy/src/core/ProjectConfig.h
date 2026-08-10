#pragma once

#include <QString>

class ProjectConfig {
public:
    explicit ProjectConfig(QString rootPath = {});

    void setRootPath(const QString& rootPath);
    const QString& rootPath() const;
    QString musitPath() const;
    bool isReady() const;

    bool shouldTrack(const QString& relativePath) const;

    // Directory names (single path components) that are never scanned or tracked.
    static bool isIgnoredDirectoryName(const QString& name);

private:
    QString m_rootPath;
};
