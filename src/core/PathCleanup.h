#pragma once

#include <QDir>
#include <QFileInfo>
#include <QString>

// Normalizes a filesystem path for stable comparisons across platforms.
// Always uses '/' separators (Qt convention after cleanPath).
inline QString normalizeAbsolutePath(const QString& path) {
    return QDir::cleanPath(QDir::fromNativeSeparators(path));
}

// Case-insensitive on Windows and macOS default volumes; exact on Linux.
// User-facing paths (project roots, registry keys) must compare equal even
// when the OS or file picker varies drive-letter / folder casing.
inline Qt::CaseSensitivity pathCompareSensitivity() {
#if defined(Q_OS_WIN) || defined(Q_OS_MAC)
    return Qt::CaseInsensitive;
#else
    return Qt::CaseSensitive;
#endif
}

inline bool pathEquals(const QString& a, const QString& b) {
    return QString::compare(
               normalizeAbsolutePath(a),
               normalizeAbsolutePath(b),
               pathCompareSensitivity())
        == 0;
}

inline bool pathIsUnderRoot(const QString& path, const QString& root) {
    const QString normalizedPath = normalizeAbsolutePath(path);
    const QString normalizedRoot = normalizeAbsolutePath(root);
    if (pathEquals(normalizedPath, normalizedRoot)) {
        return true;
    }
    return normalizedPath.startsWith(
        normalizedRoot + QLatin1Char('/'), pathCompareSensitivity());
}

// Hash-map key for absolute project roots (stable across casing drift).
inline QString pathKey(const QString& path) {
    QString key = normalizeAbsolutePath(path);
#if defined(Q_OS_WIN) || defined(Q_OS_MAC)
    key = key.toLower();
#endif
    return key;
}

// Project-relative artifact ids (e.g. "Song.logicx", "track.als").
inline bool artifactEquals(const QString& a, const QString& b) {
    const QString left = QDir::fromNativeSeparators(a).trimmed();
    const QString right = QDir::fromNativeSeparators(b).trimmed();
    return QString::compare(left, right, pathCompareSensitivity()) == 0;
}

// Removes now-empty directories from startDirPath upwards, never crossing
// above stopDirPath. Used to keep .musit/staging tidy after staged copies
// are deleted (compaction or version deletion).
inline void removeEmptyParentDirs(const QString& startDirPath, const QString& stopDirPath) {
    const QString stop = normalizeAbsolutePath(stopDirPath);
    QString current = normalizeAbsolutePath(startDirPath);

    const auto isSameOrBelow = [&stop](const QString& path) {
        return pathEquals(path, stop) || path.startsWith(stop + QLatin1Char('/'), pathCompareSensitivity());
    };

    while (!current.isEmpty() && isSameOrBelow(current)) {
        QDir dir(current);
        if (!dir.exists()) {
            break;
        }

        const QStringList entries = dir.entryList(QDir::NoDotAndDotDot | QDir::AllEntries);
        if (!entries.isEmpty()) {
            break;
        }

        const QString parent = QFileInfo(current).dir().absolutePath();
        QDir().rmdir(current);

        if (pathEquals(current, stop)) {
            break;
        }
        current = parent;
    }
}
