#pragma once

#include <QHash>
#include <QList>
#include <QString>
#include <QStringList>

enum class ProjectKind {
    Bitwig,
    FLStudio,
    Ableton,
    Logic,
    Cubase,
    Reaper,
    Ardour,
    Renoise,
    ProTools,
    StudioOne,
    Cakewalk,
    Reason,
    GarageBand,
    Unknown
};

struct DiscoveredProject {
    QString name;
    QString rootPath;
    ProjectKind kind {ProjectKind::Unknown};
    QString primaryProjectFile;
    QStringList projectFiles;
    QHash<ProjectKind, int> typeCounts;
    int totalProjectFiles {0};
};

class ProjectDiscovery {
public:
    QList<DiscoveredProject> discoverAll(const QString& selectedFolder) const;

    static QList<ProjectKind> knownKinds();
    static QString kindToString(ProjectKind kind);
};
