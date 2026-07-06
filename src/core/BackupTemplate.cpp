#include "BackupTemplate.h"

#include <QHash>
#include <QRegularExpression>
#include <QSet>

namespace {

// Bulky or regenerable files that are never versioned, no matter what an
// include pattern says (media inside bundles, temp/lock files, caches).
const QSet<QString>& globallyExcludedExtensions() {
    static const QSet<QString> extensions {
        "wav", "aif", "aiff", "aifc", "mp3", "flac", "ogg", "m4a", "wma",
        "asd", // Ableton analysis cache
        "tmp", "lock", "swp"
    };
    return extensions;
}

QString extensionOf(const QString& lowerPath) {
    const int slashIndex = lowerPath.lastIndexOf('/');
    const int dotIndex = lowerPath.lastIndexOf('.');
    if (dotIndex <= slashIndex) {
        return {};
    }
    return lowerPath.mid(dotIndex + 1);
}

// Wildcard-to-regex with '**' support:
//   **  matches anything, including '/'
//   *   matches within one path segment
//   ?   matches one character within a segment
QRegularExpression compilePattern(const QString& pattern) {
    QString regex = QStringLiteral("^");
    for (int i = 0; i < pattern.size(); ++i) {
        const QChar c = pattern.at(i);
        if (c == QLatin1Char('*')) {
            if (i + 1 < pattern.size() && pattern.at(i + 1) == QLatin1Char('*')) {
                regex += QStringLiteral(".*");
                ++i;
            } else {
                regex += QStringLiteral("[^/]*");
            }
        } else if (c == QLatin1Char('?')) {
            regex += QStringLiteral("[^/]");
        } else {
            regex += QRegularExpression::escape(QString(c));
        }
    }
    regex += QLatin1Char('$');
    return QRegularExpression(regex, QRegularExpression::CaseInsensitiveOption);
}

bool matchesAnyPattern(const QString& normalizedLowerPath, const QStringList& patterns) {
    static QHash<QString, QRegularExpression> cache;
    for (const QString& pattern : patterns) {
        auto it = cache.constFind(pattern);
        if (it == cache.constEnd()) {
            it = cache.insert(pattern, compilePattern(pattern));
        }
        if (it->match(normalizedLowerPath).hasMatch()) {
            return true;
        }
    }
    return false;
}

QString normalizeLower(const QString& relativePath) {
    QString normalized = relativePath;
    normalized.replace(QLatin1Char('\\'), QLatin1Char('/'));
    return normalized.toLower();
}

} // namespace

namespace BackupTemplates {

const QList<BackupTemplate>& all() {
    // Registry of everything musit knows how to back up. Order matters: it
    // is the preference order when a folder contains several project kinds.
    // See BackupTemplate.h for how to add a new data type.
    static const QList<BackupTemplate> templates {
        BackupTemplate {
            ProjectKind::Bitwig,
            {"bwproject"},
            false,
            {},
            {}
        },
        BackupTemplate {
            ProjectKind::FLStudio,
            {"flp"},
            false,
            {},
            {}
        },
        BackupTemplate {
            ProjectKind::Ableton,
            {"als"},
            false,
            // Project settings that live next to the .als file.
            {"**ableton project info/**"},
            {}
        },
        BackupTemplate {
            ProjectKind::Logic,
            {"logicx"},
            true,
            // Everything inside the bundle except media; the global audio
            // extension excludes catch stray audio elsewhere in the bundle.
            {"**.logicx/**"},
            {"**.logicx/media/**", "**.logicx/freeze files/**", "**.logicx/undo data**"}
        },
        BackupTemplate {
            ProjectKind::Cubase,
            {"cpr"},
            false,
            {},
            {}
        },
        BackupTemplate {
            ProjectKind::Reaper,
            {"rpp"},
            false,
            {},
            {}
        },
        BackupTemplate {
            ProjectKind::Ardour,
            {"ardour"},
            false,
            {},
            {}
        },
        BackupTemplate {
            ProjectKind::Renoise,
            {"xrns"},
            false,
            {},
            {}
        },
        BackupTemplate {
            ProjectKind::ProTools,
            {"ptx"},
            false,
            {},
            {}
        },
        BackupTemplate {
            ProjectKind::StudioOne,
            {"song"},
            false,
            {},
            {}
        },
        BackupTemplate {
            ProjectKind::Cakewalk,
            {"cwp"},
            false,
            {},
            {}
        },
        BackupTemplate {
            ProjectKind::Reason,
            {"reason", "rsn"},
            false,
            {},
            {}
        },
        BackupTemplate {
            ProjectKind::GarageBand,
            {"band"},
            true,
            {"**.band/**"},
            {"**.band/media/**", "**.band/output/**", "**.band/freeze files/**"}
        }
    };
    return templates;
}

ProjectKind kindForFileName(const QString& fileName) {
    const QString lowerName = fileName.toLower();
    for (const BackupTemplate& tpl : all()) {
        for (const QString& extension : tpl.projectFileExtensions) {
            if (lowerName.endsWith(QLatin1Char('.') + extension)) {
                return tpl.kind;
            }
        }
    }
    return ProjectKind::Unknown;
}

bool kindIsBundle(ProjectKind kind) {
    for (const BackupTemplate& tpl : all()) {
        if (tpl.kind == kind) {
            return tpl.projectFileIsBundle;
        }
    }
    return false;
}

bool shouldTrackPath(const QString& relativePath) {
    if (relativePath.isEmpty()) {
        return false;
    }

    const QString normalized = normalizeLower(relativePath);

    if (globallyExcludedExtensions().contains(extensionOf(normalized))) {
        return false;
    }

    for (const BackupTemplate& tpl : all()) {
        if (matchesAnyPattern(normalized, tpl.excludePatterns)) {
            return false;
        }
    }

    // Main project files of non-bundle types (a bundle "file" is a
    // directory; only its contents produce file events).
    const QString extension = extensionOf(normalized);
    for (const BackupTemplate& tpl : all()) {
        if (!tpl.projectFileIsBundle && tpl.projectFileExtensions.contains(extension)) {
            return true;
        }
    }

    for (const BackupTemplate& tpl : all()) {
        if (matchesAnyPattern(normalized, tpl.includePatterns)) {
            return true;
        }
    }

    return false;
}

QString artifactForPath(const QString& relativePath) {
    const QString normalized = normalizeLower(relativePath);

    QStringList bundleSuffixes;
    for (const BackupTemplate& tpl : all()) {
        if (tpl.projectFileIsBundle) {
            for (const QString& extension : tpl.projectFileExtensions) {
                bundleSuffixes.append(QLatin1Char('.') + extension);
            }
        }
    }

    // Find the first path segment that is a bundle root and cut there.
    // Compare against the normalized path but return the original casing.
    int segmentStart = 0;
    for (int i = 0; i <= normalized.size(); ++i) {
        if (i != normalized.size() && normalized.at(i) != QLatin1Char('/')) {
            continue;
        }
        const QString segment = normalized.mid(segmentStart, i - segmentStart);
        for (const QString& suffix : bundleSuffixes) {
            if (segment.endsWith(suffix)) {
                return relativePath.left(i);
            }
        }
        segmentStart = i + 1;
    }

    return relativePath;
}

} // namespace BackupTemplates
