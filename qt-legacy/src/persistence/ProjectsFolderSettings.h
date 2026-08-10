#pragma once

#include <QString>

enum class ProjectsFolderLayout {
    Bundles,
    Files,
};

class ProjectsFolderSettings {
public:
    static QString settingsDirectoryName();
    static QString settingsFilePath(const QString& projectsFolderPath);

    static ProjectsFolderLayout layoutFromString(const QString& value);
    static QString layoutToString(ProjectsFolderLayout layout);

    bool hasLayoutSetting(const QString& projectsFolderPath) const;
    ProjectsFolderLayout loadLayout(const QString& projectsFolderPath) const;
    bool saveLayout(const QString& projectsFolderPath, ProjectsFolderLayout layout) const;
};
