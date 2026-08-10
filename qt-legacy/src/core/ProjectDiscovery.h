#pragma once

#include <QHash>
#include <QList>
#include <QString>
#include <QStringList>

#include <atomic>
#include <functional>
#include <memory>
#include <optional>

#include "../persistence/ProjectsFolderSettings.h"

enum class ProjectKind {
    // DawElectronic — clip/pattern-oriented
    Ableton,
    Bitwig,
    FLStudio,
    Reason,
    Renoise,
    LMMS,
    SunVox,
    MuLab,

    // DawTraditional — linear/recording-oriented
    Logic,
    GarageBand,
    Cubase,
    Nuendo,
    Reaper,
    ProTools,
    StudioOne,
    Cakewalk,
    Ardour,
    Samplitude,
    Tracktion,

    // Video — NLE timelines
    DaVinciResolve,
    FinalCutPro,
    PremierePro,
    MediaComposer,
    VEGASPro,
    HitFilm,

    // MotionVfx — compositing & motion
    AfterEffects,
    Nuke,
    Fusion,

    // ThreeD — scenes & meshes
    Blender,
    Cinema4D,
    Houdini,
    Maya,
    Lightwave,

    // Photo — raster editing
    Photoshop,
    GIMP,
    Krita,
    AffinityPhoto,
    ClipStudioPaint,
    CaptureOne,

    // VectorIllustration — vector & layout art
    Illustrator,
    AffinityDesigner,
    Inkscape,
    CorelDRAW,

    // Publishing — long-form documents
    InDesign,
    Scrivener,
    AffinityPublisher,
    LaTeX,

    // GameDev — engines & editor projects
    Unreal,
    Godot,
    Unity,

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
    using ScanCallback = std::function<void(const QString& directoryPath)>;
    using ProgressCallback = std::function<void(const QList<DiscoveredProject>& partialProjects,
                                                int directoriesScanned)>;

    QList<DiscoveredProject> discoverAll(
        const QString& selectedFolder,
        const ScanCallback& onDirectoryScanned = {},
        const std::shared_ptr<std::atomic<bool>>& cancelled = {},
        const ProgressCallback& onProgress = {},
        int progressEveryDirectories = 20,
        ProjectsFolderLayout layout = ProjectsFolderLayout::Bundles) const;

    // Resolve a single project from a changed file path (for watcher rediscovery).
    std::optional<DiscoveredProject> discoverProjectForChangedPath(
        const QString& selectedFolder,
        const QString& absoluteChangedPath,
        ProjectsFolderLayout layout = ProjectsFolderLayout::Bundles) const;

    static QList<ProjectKind> knownKinds();
    static QString kindToString(ProjectKind kind);
};
