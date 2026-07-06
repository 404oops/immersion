#include "ProjectConfig.h"

#include "BackupTemplate.h"

#include <QDir>
#include <QSet>
#include <QStringList>

namespace {

bool containsIgnoredDirectory(const QString& normalizedLowerPath) {
    const QStringList parts = normalizedLowerPath.split('/', Qt::SkipEmptyParts);
    // Only look at directory components; the last part is the file name.
    for (int i = 0; i < parts.size() - 1; ++i) {
        if (ProjectConfig::isIgnoredDirectoryName(parts.at(i))) {
            return true;
        }
    }
    return false;
}

} // namespace

bool ProjectConfig::isIgnoredDirectoryName(const QString& name) {
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

    const QString lowerName = name.toLower();
    if (ignoredDirectories.contains(lowerName)) {
        return true;
    }

    // DAWs create auto-backup folders with varying names ("Backup", "Backups",
    // "Auto Backup", ...). Match per directory component, never on full paths,
    // so a projects root like "D:/Backups/Music" chosen by the user still works.
    return lowerName.contains("backup");
}

ProjectConfig::ProjectConfig(QString rootPath)
    : m_rootPath(std::move(rootPath)) {}

void ProjectConfig::setRootPath(const QString& rootPath) {
    m_rootPath = QDir::cleanPath(rootPath);
}

const QString& ProjectConfig::rootPath() const {
    return m_rootPath;
}

QString ProjectConfig::musitPath() const {
    if (m_rootPath.isEmpty()) {
        return {};
    }
    return QDir(m_rootPath).filePath(".musit");
}

bool ProjectConfig::isReady() const {
    return !m_rootPath.isEmpty();
}

bool ProjectConfig::shouldTrack(const QString& relativePath) const {
    if (relativePath.isEmpty()) {
        return false;
    }

    const QString normalized = QDir::fromNativeSeparators(relativePath).toLower();
    if (normalized.startsWith(".musit/") || normalized == ".musit") {
        return false;
    }

    if (containsIgnoredDirectory(normalized)) {
        return false;
    }

    // The backup templates decide what gets versioned: main project files,
    // accompanying files/bundle internals per include patterns, and never
    // audio/temp files.
    return BackupTemplates::shouldTrackPath(normalized);
}
