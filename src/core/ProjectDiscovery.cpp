#include "ProjectDiscovery.h"

#include <QDir>
#include <QFileInfo>
#include <QRegularExpression>
#include <QSet>

#include <algorithm>

namespace {

ProjectKind classifyPath(const QFileInfo& info) {
    const QString lowerName = info.fileName().toLower();
    if (lowerName.endsWith(".bwproject")) {
        return ProjectKind::Bitwig;
    }
    if (lowerName.endsWith(".flp")) {
        return ProjectKind::FLStudio;
    }
    if (lowerName.endsWith(".als")) {
        return ProjectKind::Ableton;
    }
    if (lowerName.endsWith(".logicx")) {
        return ProjectKind::Logic;
    }
    if (lowerName.endsWith(".cpr")) {
        return ProjectKind::Cubase;
    }
    if (lowerName.endsWith(".rpp")) {
        return ProjectKind::Reaper;
    }
    if (lowerName.endsWith(".ardour")) {
        return ProjectKind::Ardour;
    }
    if (lowerName.endsWith(".xrns")) {
        return ProjectKind::Renoise;
    }
    if (lowerName.endsWith(".ptx")) {
        return ProjectKind::ProTools;
    }
    if (lowerName.endsWith(".song")) {
        return ProjectKind::StudioOne;
    }
    if (lowerName.endsWith(".cwp")) {
        return ProjectKind::Cakewalk;
    }
    if (lowerName.endsWith(".reason") || lowerName.endsWith(".rsn")) {
        return ProjectKind::Reason;
    }
    if (lowerName.endsWith(".band")) {
        return ProjectKind::GarageBand;
    }

    return ProjectKind::Unknown;
}

bool shouldSkipPath(const QString& absolutePath) {
    const QString lowerPath = QDir::fromNativeSeparators(absolutePath).toLower();
    return lowerPath.contains("/.musit/")
        || lowerPath.contains("/.git/")
        || lowerPath.contains("/.idea/")
    || lowerPath.contains("/.vscode/")
    || lowerPath.contains("backup");
}

bool isIgnoredDirectoryName(const QString& name) {
    const QString lowerName = name.toLower();

    static const QSet<QString> ignoredDirectories {
        ".musit",
        ".git",
        ".idea",
        ".vscode",
        ".svn",
        ".hg",
        ".counts",
        "auto-backups",
        "bounce",
        "samples",
        "recordings",
        "master-recordings",
        "multi-samples",
        "stems",
        "exports",
        "renders"
    };

    if (ignoredDirectories.contains(lowerName)) {
        return true;
    }

    if (lowerName.contains("backup")) {
        return true;
    }

    return false;
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
    const QString& currentFolder,
    QHash<QString, QHash<ProjectKind, int>>& countsByFolder,
    QHash<QString, QStringList>& filesByFolder,
    QHash<QString, QHash<ProjectKind, QStringList>>& filesByFolderAndKind,
    QHash<QString, QHash<QString, QFileInfo>>& fileInfosByFolder) {
    QDir dir(currentFolder);
    const QFileInfoList entries = dir.entryInfoList(
        QDir::NoDotAndDotDot | QDir::AllEntries | QDir::Hidden,
        QDir::NoSort);

    for (const QFileInfo& entry : entries) {
        if (entry.isDir()) {
            if (isIgnoredDirectoryName(entry.fileName())) {
                continue;
            }

            scanDirectory(entry.absoluteFilePath(),
                countsByFolder,
                filesByFolder,
                filesByFolderAndKind,
                fileInfosByFolder);
            continue;
        }

        const QString absolutePath = entry.absoluteFilePath();
        if (shouldSkipPath(absolutePath)) {
            continue;
        }

        const ProjectKind kind = classifyPath(entry);
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

} // namespace

QList<ProjectKind> ProjectDiscovery::knownKinds() {
    return {
        ProjectKind::Bitwig,
        ProjectKind::FLStudio,
        ProjectKind::Ableton,
        ProjectKind::Logic,
        ProjectKind::Cubase,
        ProjectKind::Reaper,
        ProjectKind::Ardour,
        ProjectKind::Renoise,
        ProjectKind::ProTools,
        ProjectKind::StudioOne,
        ProjectKind::Cakewalk,
        ProjectKind::Reason,
        ProjectKind::GarageBand
    };
}

QList<DiscoveredProject> ProjectDiscovery::discoverAll(const QString& selectedFolder) const {
    if (selectedFolder.isEmpty()) {
        return {};
    }

    const QString rootFolder = QDir::cleanPath(selectedFolder);

    QHash<QString, QHash<ProjectKind, int>> countsByFolder;
    QHash<QString, QStringList> filesByFolder;
    QHash<QString, QHash<ProjectKind, QStringList>> filesByFolderAndKind;
    QHash<QString, QHash<QString, QFileInfo>> fileInfosByFolder;

    scanDirectory(rootFolder,
        countsByFolder,
        filesByFolder,
        filesByFolderAndKind,
        fileInfosByFolder);

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

std::optional<DiscoveredProject> ProjectDiscovery::discover(const QString& selectedFolder) const {
    const QList<DiscoveredProject> projects = discoverAll(selectedFolder);
    if (projects.isEmpty()) {
        return std::nullopt;
    }

    const auto best = std::max_element(projects.begin(), projects.end(), [](const DiscoveredProject& left, const DiscoveredProject& right) {
        return left.totalProjectFiles < right.totalProjectFiles;
    });
    return *best;
}

QString ProjectDiscovery::kindToString(ProjectKind kind) {
    switch (kind) {
    case ProjectKind::Bitwig:
        return "Bitwig";
    case ProjectKind::FLStudio:
        return "FL Studio";
    case ProjectKind::Ableton:
        return "Ableton";
    case ProjectKind::Logic:
        return "Logic";
    case ProjectKind::Cubase:
        return "Cubase";
    case ProjectKind::Reaper:
        return "Reaper";
    case ProjectKind::Ardour:
        return "Ardour";
    case ProjectKind::Renoise:
        return "Renoise";
    case ProjectKind::ProTools:
        return "Pro Tools";
    case ProjectKind::StudioOne:
        return "Studio One";
    case ProjectKind::Cakewalk:
        return "Cakewalk";
    case ProjectKind::Reason:
        return "Reason";
    case ProjectKind::GarageBand:
        return "GarageBand";
    case ProjectKind::Unknown:
    default:
        return "Unknown";
    }
}
