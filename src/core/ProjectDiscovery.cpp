#include "ProjectDiscovery.h"

#include "BackupTemplate.h"
#include "PathCleanup.h"
#include "ProjectConfig.h"

#include <QDir>
#include <QFileInfo>
#include <QRegularExpression>
#include <QtGlobal>

#include <algorithm>

namespace {

ProjectKind classifyPath(const QFileInfo& info) {
    const ProjectKind kind = BackupTemplates::kindForFileName(info.fileName());
    if (kind == ProjectKind::Unknown) {
        return kind;
    }

    if (BackupTemplates::kindIsBundle(kind) != info.isDir()) {
        return ProjectKind::Unknown;
    }

    return kind;
}

void registerMarkerProject(
    const QString& rootFolder,
    const QFileInfo& markerFile,
    QHash<QString, QHash<ProjectKind, int>>& countsByFolder,
    QHash<QString, QStringList>& filesByFolder,
    QHash<QString, QHash<ProjectKind, QStringList>>& filesByFolderAndKind,
    QHash<QString, QHash<QString, QFileInfo>>& fileInfosByFolder) {
    const QString relativePath = QDir(rootFolder).relativeFilePath(markerFile.absoluteFilePath());
    const ProjectKind kind = BackupTemplates::kindForRootMarkerPath(relativePath);
    if (kind == ProjectKind::Unknown) {
        return;
    }

    QString markerPattern;
    for (const BackupTemplate& tpl : BackupTemplates::all()) {
        if (tpl.kind != kind) {
            continue;
        }
        for (const QString& marker : tpl.projectRootMarkers) {
            if (relativePath.compare(marker, Qt::CaseInsensitive) == 0) {
                markerPattern = marker;
                break;
            }
        }
        if (!markerPattern.isEmpty()) {
            break;
        }
    }
    if (markerPattern.isEmpty()) {
        return;
    }

    const QString projectRoot = BackupTemplates::projectRootForMarker(
        markerPattern, markerFile.absoluteFilePath());
    const QString fileName = markerFile.fileName();

    countsByFolder[projectRoot][kind] = countsByFolder[projectRoot].value(kind) + 1;
    filesByFolder[projectRoot].append(fileName);
    filesByFolderAndKind[projectRoot][kind].append(fileName);
    fileInfosByFolder[projectRoot].insert(fileName, markerFile);
}

int extractVersionNumber(const QString& fileName) {
    static const QRegularExpression versionRegex(
        "(?:^|[ _-])v(\\d+)(?:$|[ _.-])",
        QRegularExpression::CaseInsensitiveOption);

    const QString stem = QFileInfo(fileName).completeBaseName();
    const QRegularExpressionMatch match = versionRegex.match(stem);
    if (!match.hasMatch()) {
        return -1;
    }

    bool ok = false;
    const int version = match.captured(1).toInt(&ok);
    return ok ? version : -1;
}

QString choosePrimaryProjectFile(
    const QStringList& candidateFiles,
    const QHash<QString, QFileInfo>& fileInfosByName) {
    if (candidateFiles.isEmpty()) {
        return {};
    }

    QString bestFile;
    int bestVersion = -1;
    QDateTime bestModified;

    for (const QString& fileName : candidateFiles) {
        const int version = extractVersionNumber(fileName);
        const QFileInfo info = fileInfosByName.value(fileName);
        const QDateTime modified = info.lastModified();

        const bool isBetterVersion = version > bestVersion;
        const bool isSameVersionNewer = version == bestVersion && modified > bestModified;
        const bool isSameVersionSameTimeLexicographicallyEarlier =
            version == bestVersion
            && modified == bestModified
            && (bestFile.isEmpty() || fileName < bestFile);

        if (bestFile.isEmpty()
            || isBetterVersion
            || isSameVersionNewer
            || isSameVersionSameTimeLexicographicallyEarlier) {
            bestFile = fileName;
            bestVersion = version;
            bestModified = modified;
        }
    }

    return bestFile;
}

void scanDirectory(
    const QString& rootFolder,
    const QString& currentFolder,
    QHash<QString, QHash<ProjectKind, int>>& countsByFolder,
    QHash<QString, QStringList>& filesByFolder,
    QHash<QString, QHash<ProjectKind, QStringList>>& filesByFolderAndKind,
    QHash<QString, QHash<QString, QFileInfo>>& fileInfosByFolder,
    const ProjectDiscovery::ScanCallback& onDirectoryScanned,
    const std::shared_ptr<std::atomic<bool>>& cancelled) {
    if (cancelled && cancelled->load()) {
        return;
    }

    if (onDirectoryScanned) {
        onDirectoryScanned(currentFolder);
    }

    QDir dir(currentFolder);
    const QFileInfoList entries = dir.entryInfoList(
        QDir::NoDotAndDotDot | QDir::AllEntries | QDir::Hidden,
        QDir::NoSort);

    for (const QFileInfo& entry : entries) {
        if (entry.isFile()) {
            registerMarkerProject(rootFolder,
                entry,
                countsByFolder,
                filesByFolder,
                filesByFolderAndKind,
                fileInfosByFolder);
        }

        const ProjectKind kind = classifyPath(entry);

        if (entry.isDir() && kind == ProjectKind::Unknown) {
            if (ProjectConfig::isIgnoredDirectoryName(entry.fileName())) {
                continue;
            }

            scanDirectory(rootFolder,
                entry.absoluteFilePath(),
                countsByFolder,
                filesByFolder,
                filesByFolderAndKind,
                fileInfosByFolder,
                onDirectoryScanned,
                cancelled);
            continue;
        }

        if (kind == ProjectKind::Unknown) {
            continue;
        }

        const QString folderPath = entry.absolutePath();
        countsByFolder[folderPath][kind] = countsByFolder[folderPath].value(kind) + 1;
        filesByFolder[folderPath].append(entry.fileName());
        filesByFolderAndKind[folderPath][kind].append(entry.fileName());
        fileInfosByFolder[folderPath].insert(entry.fileName(), entry);
    }
}

ProjectKind dominantKind(const QHash<ProjectKind, int>& counts) {
    ProjectKind dominant = ProjectKind::Unknown;
    int dominantCount = -1;

    const QList<ProjectKind> preferenceOrder = ProjectDiscovery::knownKinds();

    for (const ProjectKind candidate : preferenceOrder) {
        const int count = counts.value(candidate);
        if (count > dominantCount) {
            dominant = candidate;
            dominantCount = count;
        }
    }

    return dominant;
}

QList<DiscoveredProject> buildProjectsFromState(
    const QHash<QString, QHash<ProjectKind, int>>& countsByFolder,
    const QHash<QString, QStringList>& filesByFolder,
    const QHash<QString, QHash<ProjectKind, QStringList>>& filesByFolderAndKind,
    const QHash<QString, QHash<QString, QFileInfo>>& fileInfosByFolder) {
    QList<DiscoveredProject> projects;
    for (auto itFolder = countsByFolder.constBegin(); itFolder != countsByFolder.constEnd(); ++itFolder) {
        const QString folderPath = itFolder.key();
        const QHash<ProjectKind, int>& counts = itFolder.value();

        const ProjectKind dominant = dominantKind(counts);
        if (dominant == ProjectKind::Unknown) {
            continue;
        }

        DiscoveredProject project;
        project.rootPath = folderPath;
        project.name = QFileInfo(folderPath).fileName();
        if (project.name.isEmpty()) {
            project.name = folderPath;
        }
        project.kind = dominant;
        project.typeCounts = counts;
        project.projectFiles = filesByFolder.value(folderPath);
        project.projectFiles.removeDuplicates();
        std::sort(project.projectFiles.begin(), project.projectFiles.end());

        const QStringList dominantFiles = filesByFolderAndKind.value(folderPath).value(dominant);
        const QString selectedPrimary = choosePrimaryProjectFile(
            dominantFiles.isEmpty() ? project.projectFiles : dominantFiles,
            fileInfosByFolder.value(folderPath));
        project.primaryProjectFile = selectedPrimary.isEmpty() ? project.projectFiles.value(0) : selectedPrimary;
        project.totalProjectFiles = project.projectFiles.size();

        projects.append(project);
    }

    std::sort(projects.begin(), projects.end(), [](const DiscoveredProject& left, const DiscoveredProject& right) {
        if (left.name == right.name) {
            return left.rootPath < right.rootPath;
        }
        return left.name < right.name;
    });

    return projects;
}

QString resolveProjectRootAbsolute(const QString& selectedFolder, const QString& relativePath) {
    const QString normalizedRel = QDir::fromNativeSeparators(relativePath);

    for (const BackupTemplate& tpl : BackupTemplates::all()) {
        for (const QString& marker : tpl.projectRootMarkers) {
            if (normalizedRel.compare(marker, Qt::CaseInsensitive) == 0) {
                const QString absolutePath = QDir(selectedFolder).filePath(relativePath);
                return QDir::cleanPath(BackupTemplates::projectRootForMarker(marker, absolutePath));
            }
        }
    }

    const QString artifactRel = BackupTemplates::artifactForPath(relativePath);
    if (artifactRel.isEmpty()) {
        return {};
    }

    const QString artifactAbs = QDir(selectedFolder).filePath(artifactRel);
    const QFileInfo artifactInfo(artifactAbs);
    const ProjectKind kind = classifyPath(artifactInfo);
    if (kind != ProjectKind::Unknown && BackupTemplates::kindIsBundle(kind)) {
        return QDir::cleanPath(artifactInfo.absolutePath());
    }

    return QDir::cleanPath(artifactInfo.absolutePath());
}

std::optional<DiscoveredProject> discoverProjectAt(
    const QString& selectedFolder,
    const QString& absoluteProjectRoot) {
    const QString cleanRoot = QDir::cleanPath(absoluteProjectRoot);
    if (!pathIsUnderRoot(cleanRoot, selectedFolder)) {
        return {};
    }

    QHash<QString, QHash<ProjectKind, int>> countsByFolder;
    QHash<QString, QStringList> filesByFolder;
    QHash<QString, QHash<ProjectKind, QStringList>> filesByFolderAndKind;
    QHash<QString, QHash<QString, QFileInfo>> fileInfosByFolder;

    QDir projectDir(cleanRoot);
    if (!projectDir.exists()) {
        return {};
    }

    const QFileInfoList entries = projectDir.entryInfoList(
        QDir::NoDotAndDotDot | QDir::AllEntries | QDir::Hidden,
        QDir::NoSort);

    for (const QFileInfo& entry : entries) {
        if (entry.isFile()) {
            registerMarkerProject(selectedFolder,
                                  entry,
                                  countsByFolder,
                                  filesByFolder,
                                  filesByFolderAndKind,
                                  fileInfosByFolder);
        }

        const ProjectKind kind = classifyPath(entry);
        if (kind == ProjectKind::Unknown) {
            continue;
        }

        const QString folderPath = QDir::cleanPath(entry.absolutePath());
        if (!pathEquals(folderPath, cleanRoot)) {
            continue;
        }

        countsByFolder[cleanRoot][kind] = countsByFolder[cleanRoot].value(kind) + 1;
        filesByFolder[cleanRoot].append(entry.fileName());
        filesByFolderAndKind[cleanRoot][kind].append(entry.fileName());
        fileInfosByFolder[cleanRoot].insert(entry.fileName(), entry);
    }

    const QList<DiscoveredProject> projects = buildProjectsFromState(countsByFolder,
                                                                     filesByFolder,
                                                                     filesByFolderAndKind,
                                                                     fileInfosByFolder);
    for (const DiscoveredProject& project : projects) {
        if (pathEquals(project.rootPath, cleanRoot)) {
            return project;
        }
    }

    return {};
}

} // namespace

QList<ProjectKind> ProjectDiscovery::knownKinds() {
    QList<ProjectKind> kinds;
    for (const BackupTemplate& tpl : BackupTemplates::all()) {
        kinds.append(tpl.kind);
    }
    return kinds;
}

QList<DiscoveredProject> ProjectDiscovery::discoverAll(
    const QString& selectedFolder,
    const ScanCallback& onDirectoryScanned,
    const std::shared_ptr<std::atomic<bool>>& cancelled,
    const ProgressCallback& onProgress,
    int progressEveryDirectories) const {
    if (selectedFolder.isEmpty()) {
        return {};
    }

    const QString rootFolder = QDir::cleanPath(selectedFolder);

    QHash<QString, QHash<ProjectKind, int>> countsByFolder;
    QHash<QString, QStringList> filesByFolder;
    QHash<QString, QHash<ProjectKind, QStringList>> filesByFolderAndKind;
    QHash<QString, QHash<QString, QFileInfo>> fileInfosByFolder;

    int directoriesScanned = 0;
    const int progressInterval = qMax(1, progressEveryDirectories);
    const auto wrappedDirectoryScanned = [&](const QString& directoryPath) {
        if (onDirectoryScanned) {
            onDirectoryScanned(directoryPath);
        }

        ++directoriesScanned;
        if (onProgress && directoriesScanned % progressInterval == 0) {
            onProgress(buildProjectsFromState(countsByFolder,
                                              filesByFolder,
                                              filesByFolderAndKind,
                                              fileInfosByFolder),
                       directoriesScanned);
        }
    };

    scanDirectory(rootFolder,
        rootFolder,
        countsByFolder,
        filesByFolder,
        filesByFolderAndKind,
        fileInfosByFolder,
        wrappedDirectoryScanned,
        cancelled);

    return buildProjectsFromState(countsByFolder,
                                  filesByFolder,
                                  filesByFolderAndKind,
                                  fileInfosByFolder);
}

std::optional<DiscoveredProject> ProjectDiscovery::discoverProjectForChangedPath(
    const QString& selectedFolder,
    const QString& absoluteChangedPath) const {
    const QString cleanFolder = QDir::cleanPath(selectedFolder);
    const QString cleanPath = normalizeAbsolutePath(absoluteChangedPath);
    if (!pathIsUnderRoot(cleanPath, cleanFolder)) {
        return {};
    }

    const QString relativePath = QDir(cleanFolder).relativeFilePath(cleanPath);
    ProjectConfig config(cleanFolder);
    if (!config.shouldTrack(relativePath)) {
        return {};
    }

    const QString projectRoot = resolveProjectRootAbsolute(cleanFolder, relativePath);
    if (projectRoot.isEmpty()) {
        return {};
    }

    return discoverProjectAt(cleanFolder, projectRoot);
}

QString ProjectDiscovery::kindToString(ProjectKind kind) {
    switch (kind) {
    case ProjectKind::Ableton:
        return QStringLiteral("Ableton Live");
    case ProjectKind::Bitwig:
        return QStringLiteral("Bitwig Studio");
    case ProjectKind::FLStudio:
        return QStringLiteral("FL Studio");
    case ProjectKind::Reason:
        return QStringLiteral("Reason");
    case ProjectKind::Renoise:
        return QStringLiteral("Renoise");
    case ProjectKind::LMMS:
        return QStringLiteral("LMMS");
    case ProjectKind::SunVox:
        return QStringLiteral("SunVox");
    case ProjectKind::MuLab:
        return QStringLiteral("MuLab");
    case ProjectKind::Logic:
        return QStringLiteral("Logic Pro");
    case ProjectKind::GarageBand:
        return QStringLiteral("GarageBand");
    case ProjectKind::Cubase:
        return QStringLiteral("Cubase");
    case ProjectKind::Nuendo:
        return QStringLiteral("Nuendo");
    case ProjectKind::Reaper:
        return QStringLiteral("REAPER");
    case ProjectKind::ProTools:
        return QStringLiteral("Pro Tools");
    case ProjectKind::StudioOne:
        return QStringLiteral("Studio One");
    case ProjectKind::Cakewalk:
        return QStringLiteral("Cakewalk");
    case ProjectKind::Ardour:
        return QStringLiteral("Ardour");
    case ProjectKind::Samplitude:
        return QStringLiteral("Samplitude");
    case ProjectKind::Tracktion:
        return QStringLiteral("Tracktion");
    case ProjectKind::DaVinciResolve:
        return QStringLiteral("DaVinci Resolve");
    case ProjectKind::FinalCutPro:
        return QStringLiteral("Final Cut Pro");
    case ProjectKind::PremierePro:
        return QStringLiteral("Premiere Pro");
    case ProjectKind::MediaComposer:
        return QStringLiteral("Media Composer");
    case ProjectKind::VEGASPro:
        return QStringLiteral("VEGAS Pro");
    case ProjectKind::HitFilm:
        return QStringLiteral("HitFilm");
    case ProjectKind::AfterEffects:
        return QStringLiteral("After Effects");
    case ProjectKind::Nuke:
        return QStringLiteral("Nuke");
    case ProjectKind::Fusion:
        return QStringLiteral("Fusion");
    case ProjectKind::Blender:
        return QStringLiteral("Blender");
    case ProjectKind::Cinema4D:
        return QStringLiteral("Cinema 4D");
    case ProjectKind::Houdini:
        return QStringLiteral("Houdini");
    case ProjectKind::Maya:
        return QStringLiteral("Maya");
    case ProjectKind::Lightwave:
        return QStringLiteral("LightWave 3D");
    case ProjectKind::Photoshop:
        return QStringLiteral("Photoshop");
    case ProjectKind::GIMP:
        return QStringLiteral("GIMP");
    case ProjectKind::Krita:
        return QStringLiteral("Krita");
    case ProjectKind::AffinityPhoto:
        return QStringLiteral("Affinity Photo");
    case ProjectKind::ClipStudioPaint:
        return QStringLiteral("Clip Studio Paint");
    case ProjectKind::CaptureOne:
        return QStringLiteral("Capture One");
    case ProjectKind::Illustrator:
        return QStringLiteral("Illustrator");
    case ProjectKind::AffinityDesigner:
        return QStringLiteral("Affinity Designer");
    case ProjectKind::Inkscape:
        return QStringLiteral("Inkscape");
    case ProjectKind::CorelDRAW:
        return QStringLiteral("CorelDRAW");
    case ProjectKind::InDesign:
        return QStringLiteral("InDesign");
    case ProjectKind::Scrivener:
        return QStringLiteral("Scrivener");
    case ProjectKind::AffinityPublisher:
        return QStringLiteral("Affinity Publisher");
    case ProjectKind::LaTeX:
        return QStringLiteral("LaTeX");
    case ProjectKind::Unreal:
        return QStringLiteral("Unreal Engine");
    case ProjectKind::Godot:
        return QStringLiteral("Godot");
    case ProjectKind::Unity:
        return QStringLiteral("Unity");
    case ProjectKind::Unknown:
    default:
        return QStringLiteral("Unknown");
    }
}
