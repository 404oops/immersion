#include "ProjectConfig.h"

#include <QDir>
#include <QStringList>

namespace {

bool containsIgnoredDirectory(const QString& normalizedLowerPath) {
    static const QStringList ignoredDirectories {
        ".musit",
        ".git",
        ".idea",
        ".vscode",
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

    const QStringList parts = normalizedLowerPath.split('/', Qt::SkipEmptyParts);
    for (const QString& part : parts) {
        if (ignoredDirectories.contains(part)) {
            return true;
        }
        if (part.contains("backup")) {
            return true;
        }
    }

    return false;
}

} // namespace

ProjectConfig::ProjectConfig(QString rootPath)
    : m_rootPath(std::move(rootPath)),
      m_excludedExtensions({"wav", "aif", "aiff", "mp3", "flac", "ogg", "m4a", "tmp", "lock"}) {}

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
    if (normalized.startsWith(".musit/")) {
        return false;
    }

    if (normalized.endsWith(".bwproject")) {
        if (containsIgnoredDirectory(normalized)) {
            return false;
        }
        return true;
    }

    if (containsIgnoredDirectory(normalized)) {
        return false;
    }

    const int dotIndex = normalized.lastIndexOf('.');
    if (dotIndex > -1) {
        const QString extension = normalized.mid(dotIndex + 1);
        if (m_excludedExtensions.contains(extension)) {
            return false;
        }
    }

    if (normalized.endsWith("~") || normalized.endsWith(".swp") || normalized.contains(".tmp")) {
        return false;
    }

    return true;
}
