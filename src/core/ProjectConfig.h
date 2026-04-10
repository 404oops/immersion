#pragma once

#include <QSet>
#include <QString>

class ProjectConfig {
public:
    explicit ProjectConfig(QString rootPath = {});

    void setRootPath(const QString& rootPath);
    const QString& rootPath() const;
    QString musitPath() const;
    bool isReady() const;

    bool shouldTrack(const QString& relativePath) const;

private:
    QString m_rootPath;
    QSet<QString> m_excludedExtensions;
};
