#pragma once

#include <QString>

#include "../core/ProjectDiscovery.h"

struct AppSettings {
    double themeHue {280.0};
    bool launchAtStartup {false};
    QString sortMode;
    QString logLevel;
    int snapshotRetention {5};
    bool notificationsEnabled {true};
    QString colorSchemeMode;
};

class ProjectRegistry {
public:
    bool saveProject(const DiscoveredProject& project);
    bool saveProjectsFolder(const QString& folderPath);
    QString loadProjectsFolder() const;
    QString loadProjectNote(const QString& rootPath) const;
    bool saveProjectNote(const QString& rootPath, const QString& note);
    QString dataFilePath() const;

    AppSettings loadAppSettings() const;
    bool saveAppSettings(const AppSettings& settings);

    // Single source of truth for where app config lives; the startup
    // self-check and reset tooling must agree with this.
    static QString appConfigDirectory();
};
