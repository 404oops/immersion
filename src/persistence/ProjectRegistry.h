#pragma once

#include <QString>

#include "../core/ProjectDiscovery.h"

class ProjectRegistry {
public:
    bool saveProject(const DiscoveredProject& project);
    bool saveProjectsFolder(const QString& folderPath);
    QString loadProjectsFolder() const;
    QString loadProjectNote(const QString& rootPath) const;
    bool saveProjectNote(const QString& rootPath, const QString& note);
    QString dataFilePath() const;

private:
    QString appConfigDirectory() const;
};
