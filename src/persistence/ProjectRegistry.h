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

    // Single source of truth for where app config lives; the startup
    // self-check and reset tooling must agree with this.
    static QString appConfigDirectory();
};
