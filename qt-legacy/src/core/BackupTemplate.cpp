#include "BackupTemplate.h"

#include <QDir>
#include <QFileInfo>
#include <QHash>
#include <QRegularExpression>
#include <QSet>

namespace {

const QSet<QString>& globallyExcludedExtensions() {
    static const QSet<QString> extensions {
        // Audio
        "wav", "aif", "aiff", "aifc", "mp3", "flac", "ogg", "m4a", "wma", "aac",
        "asd", // Ableton analysis cache
        // Video
        "mp4", "mov", "m4v", "avi", "mkv", "webm", "mxf", "r3d", "braw", "mts",
        "m2ts", "mpg", "mpeg", "wmv", "prores",
        // Sidecar / cache
        "cfa", "pek", "xmp",
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

QString baseNameOf(const QString& lowerPath) {
    const int slashIndex = lowerPath.lastIndexOf('/');
    return slashIndex < 0 ? lowerPath : lowerPath.mid(slashIndex + 1);
}

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
    static const QList<BackupTemplate> templates {
        // ── DawElectronic ────────────────────────────────────────────────
        BackupTemplate {
            ProjectKind::Ableton,
            BackupCategory::DawElectronic,
            {"als"},
            {},
            false,
            {"**Ableton Project Info/**", "**/Ableton Project Info/**"},
            {"**/Backup/**", "**/Samples/**"}
        },
        BackupTemplate {
            ProjectKind::Bitwig,
            BackupCategory::DawElectronic,
            {"bwproject"},
            {},
            false,
            {},
            {}
        },
        BackupTemplate {
            ProjectKind::FLStudio,
            BackupCategory::DawElectronic,
            {"flp"},
            {},
            false,
            {},
            {}
        },
        BackupTemplate {
            ProjectKind::Reason,
            BackupCategory::DawElectronic,
            {"reason", "rsn"},
            {},
            true,
            {"**.reason/**", "**.rsn/**"},
            {"**.reason/Cache/**", "**.reason/Samples/**", "**.rsn/Cache/**"}
        },
        BackupTemplate {
            ProjectKind::Renoise,
            BackupCategory::DawElectronic,
            {"xrns"},
            {},
            false,
            {},
            {}
        },
        BackupTemplate {
            ProjectKind::LMMS,
            BackupCategory::DawElectronic,
            {"mmp", "mmpz"},
            {},
            false,
            {},
            {}
        },
        BackupTemplate {
            ProjectKind::SunVox,
            BackupCategory::DawElectronic,
            {"sunvox"},
            {},
            false,
            {},
            {}
        },
        BackupTemplate {
            ProjectKind::MuLab,
            BackupCategory::DawElectronic,
            {"muprj"},
            {},
            false,
            {},
            {}
        },

        // ── DawTraditional ───────────────────────────────────────────────
        BackupTemplate {
            ProjectKind::Logic,
            BackupCategory::DawTraditional,
            {"logicx"},
            {},
            true,
            {"**.logicx/**"},
            {"**.logicx/Media/**", "**.logicx/Freeze Files/**", "**.logicx/Undo Data/**",
             "**.logicx/Alternatives/**/Undo Data/**"}
        },
        BackupTemplate {
            ProjectKind::GarageBand,
            BackupCategory::DawTraditional,
            {"band"},
            {},
            true,
            {"**.band/**"},
            {"**.band/Media/**", "**.band/Output/**", "**.band/Freeze Files/**"}
        },
        BackupTemplate {
            ProjectKind::Cubase,
            BackupCategory::DawTraditional,
            {"cpr"},
            {},
            false,
            {"**/*.cpr", "**/Images/**"},
            {"**/Audio/**", "**/Edits/**", "**/Track Pictures/**"}
        },
        BackupTemplate {
            ProjectKind::Nuendo,
            BackupCategory::DawTraditional,
            {"npr"},
            {},
            false,
            {"**/*.npr", "**/Images/**"},
            {"**/Audio/**", "**/Edits/**"}
        },
        BackupTemplate {
            ProjectKind::Reaper,
            BackupCategory::DawTraditional,
            {"rpp"},
            {},
            false,
            {"**/*.rpp", "**/*.RPP", "**/reaper_backups/**"},
            {"**/Media/**", "**/Peaks/**", "**/reaper_peaks/**"}
        },
        BackupTemplate {
            ProjectKind::ProTools,
            BackupCategory::DawTraditional,
            {"ptx", "ptf"},
            {},
            false,
            {"**/*.ptx", "**/*.ptf", "**/Session File Backups/**"},
            {"**/Audio Files/**", "**/Fade Files/**", "**/WaveCache/**"}
        },
        BackupTemplate {
            ProjectKind::StudioOne,
            BackupCategory::DawTraditional,
            {"song"},
            {},
            false,
            {"**/*.song", "**/History/**"},
            {"**/Media/**", "**/Cache/**"}
        },
        BackupTemplate {
            ProjectKind::Cakewalk,
            BackupCategory::DawTraditional,
            {"cwp"},
            {},
            false,
            {},
            {"**/Audio/**", "**/Bounce/**"}
        },
        BackupTemplate {
            ProjectKind::Ardour,
            BackupCategory::DawTraditional,
            {"ardour"},
            {},
            false,
            {"**/*.ardour", "**/instant.xml", "**/session.state"},
            {"**/interchange/**", "**/export/**", "**/peak/**"}
        },
        BackupTemplate {
            ProjectKind::Samplitude,
            BackupCategory::DawTraditional,
            {"sam"},
            {},
            false,
            {},
            {"**/Audio/**", "**/Cache/**"}
        },
        BackupTemplate {
            ProjectKind::Tracktion,
            BackupCategory::DawTraditional,
            {"tracktion"},
            {},
            false,
            {},
            {"**/Recordings/**", "**/Cache/**"}
        },

        // ── Video ────────────────────────────────────────────────────────
        BackupTemplate {
            ProjectKind::DaVinciResolve,
            BackupCategory::Video,
            {"drp"},
            {},
            false,
            {"**/*.drp", "**/Resolve Project Backups/**"},
            {"**/CacheClip/**", "**/Render Cache/**", "**/Proxy/**", "**/Gallery/**"}
        },
        BackupTemplate {
            ProjectKind::FinalCutPro,
            BackupCategory::Video,
            {"fcpx", "fcpxbundle", "fcpbundle"},
            {},
            true,
            {"**.fcpx/**", "**.fcpxbundle/**", "**.fcpbundle/**"},
            {"**/Render Files/**", "**/Transcoded Media/**", "**/High Quality Media/**",
             "**/Proxy Media/**", "**/Original Media/**"}
        },
        BackupTemplate {
            ProjectKind::PremierePro,
            BackupCategory::Video,
            {"prproj"},
            {},
            false,
            {"**/*.prproj", "**/Adobe Premiere Pro Auto-Save/**"},
            {"**/Adobe Premiere Pro Audio Previews/**", "**/Adobe Premiere Pro Video Previews/**",
             "**/Media Cache/**", "**/Peak Files/**", "**/Captured Audio/**",
             "**/Captured Video/**"}
        },
        BackupTemplate {
            ProjectKind::MediaComposer,
            BackupCategory::Video,
            {"avp", "avb"},
            {},
            false,
            {"**/*.avp", "**/*.avb", "**/Avid Attic/**"},
            {"**/Avid MediaFiles/**", "**/Audio/**", "**/Rendered/**"}
        },
        BackupTemplate {
            ProjectKind::VEGASPro,
            BackupCategory::Video,
            {"veg"},
            {},
            false,
            {"**/*.veg", "**/Archive/**"},
            {"**/Media/**", "**/Proxy/**", "**/Render/**"}
        },
        BackupTemplate {
            ProjectKind::HitFilm,
            BackupCategory::Video,
            {"hfcs"},
            {},
            false,
            {},
            {"**/Media/**", "**/Cache/**", "**/Proxy/**"}
        },

        // ── MotionVfx ────────────────────────────────────────────────────
        BackupTemplate {
            ProjectKind::AfterEffects,
            BackupCategory::MotionVfx,
            {"aep", "aepx"},
            {},
            false,
            {"**/*.aep", "**/*.aepx", "**/Adobe After Effects Auto-Save/**"},
            {"**/Adobe After Effects Auto-Save/Peak Files/**", "**/Media Cache/**"}
        },
        BackupTemplate {
            ProjectKind::Nuke,
            BackupCategory::MotionVfx,
            {"nk"},
            {},
            false,
            {"**/*.nk", "**/autosave/**"},
            {"**/Cache/**", "**/Render/**", "**/Previews/**"}
        },
        BackupTemplate {
            ProjectKind::Fusion,
            BackupCategory::MotionVfx,
            {"comp"},
            {},
            false,
            {"**/*.comp"},
            {"**/Cache/**", "**/Render/**", "**/Proxy/**"}
        },

        // ── ThreeD ───────────────────────────────────────────────────────
        BackupTemplate {
            ProjectKind::Blender,
            BackupCategory::ThreeD,
            {"blend"},
            {},
            false,
            {"**/*.blend", "**/*.blend1"},
            {"**/cache/**", "**/tmp/**"}
        },
        BackupTemplate {
            ProjectKind::Cinema4D,
            BackupCategory::ThreeD,
            {"c4d"},
            {},
            false,
            {"**/*.c4d", "**/auto saves/**"},
            {"**/tex/**", "**/cache/**", "**/render/**"}
        },
        BackupTemplate {
            ProjectKind::Houdini,
            BackupCategory::ThreeD,
            {"hip", "hiplc", "hipnc", "hipsc"},
            {},
            false,
            {"**/*.hip", "**/*.hiplc", "**/*.hipnc", "**/*.hipsc", "**/backup/**"},
            {"**/render/**", "**/geo/**/cache/**", "**/sim/**/cache/**"}
        },
        BackupTemplate {
            ProjectKind::Maya,
            BackupCategory::ThreeD,
            {"ma", "mb"},
            {},
            false,
            {"**/*.ma", "**/*.mb", "**/incrementalSave/**", "**/scenes/**"},
            {"**/renderData/**", "**/cache/**", "**/sourceimages/**/proxy/**"}
        },
        BackupTemplate {
            ProjectKind::Lightwave,
            BackupCategory::ThreeD,
            {"lws"},
            {},
            false,
            {"**/*.lws", "**/Scenes/**"},
            {"**/Render/**", "**/Cache/**"}
        },

        // ── Photo ────────────────────────────────────────────────────────
        BackupTemplate {
            ProjectKind::Photoshop,
            BackupCategory::Photo,
            {"psd", "psb"},
            {},
            false,
            {},
            {}
        },
        BackupTemplate {
            ProjectKind::GIMP,
            BackupCategory::Photo,
            {"xcf"},
            {},
            false,
            {},
            {}
        },
        BackupTemplate {
            ProjectKind::Krita,
            BackupCategory::Photo,
            {"kra", "krz"},
            {},
            false,
            {},
            {"**/Thumbnails/**"}
        },
        BackupTemplate {
            ProjectKind::AffinityPhoto,
            BackupCategory::Photo,
            {"afphoto", "afphoto~"},
            {},
            true,
            {"**.afphoto/**", "**.afphoto~/**"},
            {"**/Cache/**", "**/Previews/**"}
        },
        BackupTemplate {
            ProjectKind::ClipStudioPaint,
            BackupCategory::Photo,
            {"clip"},
            {},
            false,
            {"**/*.clip", "**/DocumentBackup/**"},
            {"**/Cache/**", "**/Export/**"}
        },
        BackupTemplate {
            ProjectKind::CaptureOne,
            BackupCategory::Photo,
            {"cocatalog", "cosession"},
            {},
            true,
            {"**.cocatalog/**", "**.cosession/**"},
            {"**/Cache/**", "**/Previews/**", "**/Proxies/**", "**/Trash/**"}
        },

        // ── VectorIllustration ───────────────────────────────────────────
        BackupTemplate {
            ProjectKind::Illustrator,
            BackupCategory::VectorIllustration,
            {"ai"},
            {},
            false,
            {},
            {}
        },
        BackupTemplate {
            ProjectKind::AffinityDesigner,
            BackupCategory::VectorIllustration,
            {"afdesign", "afdesign~"},
            {},
            true,
            {"**.afdesign/**", "**.afdesign~/**"},
            {"**/Cache/**", "**/Previews/**"}
        },
        BackupTemplate {
            ProjectKind::Inkscape,
            BackupCategory::VectorIllustration,
            {"svg"},
            {},
            false,
            {},
            {}
        },
        BackupTemplate {
            ProjectKind::CorelDRAW,
            BackupCategory::VectorIllustration,
            {"cdr"},
            {},
            false,
            {},
            {"**/Bitmaps/**", "**/Previews/**"}
        },

        // ── Publishing ───────────────────────────────────────────────────
        BackupTemplate {
            ProjectKind::InDesign,
            BackupCategory::Publishing,
            {"indd", "indt", "idml"},
            {},
            false,
            {"**/*.indd", "**/*.indt", "**/*.idml", "**/Document fonts/**"},
            {"**/Links/**", "**/Media/**"}
        },
        BackupTemplate {
            ProjectKind::Scrivener,
            BackupCategory::Publishing,
            {"scriv", "scrivx"},
            {},
            true,
            {"**.scriv/**", "**.scrivx/**"},
            {"**/Snapshots/**/Attachments/**", "**/QuickLook/**"}
        },
        BackupTemplate {
            ProjectKind::AffinityPublisher,
            BackupCategory::Publishing,
            {"afpub", "afpub~"},
            {},
            true,
            {"**.afpub/**", "**.afpub~/**"},
            {"**/Cache/**", "**/Previews/**"}
        },
        BackupTemplate {
            ProjectKind::LaTeX,
            BackupCategory::Publishing,
            {"tex"},
            {},
            false,
            {"**/*.tex", "**/*.bib", "**/*.sty", "**/*.cls", "**/*.bst"},
            {"**/*.aux", "**/*.log", "**/*.out", "**/*.toc", "**/*.synctex.gz",
             "**/*.fls", "**/*.fdb_latexmk", "**/*.bbl", "**/*.blg"}
        },

        // ── GameDev ──────────────────────────────────────────────────────
        BackupTemplate {
            ProjectKind::Unreal,
            BackupCategory::GameDev,
            {"uproject"},
            {},
            false,
            {"**/*.uproject", "**/Config/**", "**/Content/**", "**/Source/**",
             "**/Plugins/**/Config/**", "**/Plugins/**/Source/**"},
            {"**/Binaries/**", "**/Intermediate/**", "**/DerivedDataCache/**",
             "**/Saved/**/Autosaves/**", "**/Saved/Logs/**", "**/Saved/Crashes/**"}
        },
        BackupTemplate {
            ProjectKind::Godot,
            BackupCategory::GameDev,
            {},
            {"project.godot"},
            false,
            {"**/project.godot", "**/*.tscn", "**/*.tres", "**/*.gd", "**/*.gdshader",
             "**/export_presets.cfg"},
            {"**/.import/**", "**/.godot/**/cache/**", "**/.godot/**/shader_cache/**"}
        },
        BackupTemplate {
            ProjectKind::Unity,
            BackupCategory::GameDev,
            {},
            {"ProjectSettings/ProjectVersion.txt"},
            false,
            {"**/ProjectSettings/**", "**/Packages/manifest.json", "**/Packages/packages-lock.json",
             "**/Assets/**", "**/Packages/**/package.json"},
            {"**/Library/**", "**/Temp/**", "**/Logs/**", "**/Obj/**", "**/Build/**",
             "**/UserSettings/Layouts/**"}
        },
    };
    return templates;
}

QString categoryToString(BackupCategory category) {
    switch (category) {
    case BackupCategory::DawElectronic:
        return QStringLiteral("DawElectronic");
    case BackupCategory::DawTraditional:
        return QStringLiteral("DawTraditional");
    case BackupCategory::Video:
        return QStringLiteral("Video");
    case BackupCategory::MotionVfx:
        return QStringLiteral("MotionVfx");
    case BackupCategory::ThreeD:
        return QStringLiteral("ThreeD");
    case BackupCategory::Photo:
        return QStringLiteral("Photo");
    case BackupCategory::VectorIllustration:
        return QStringLiteral("VectorIllustration");
    case BackupCategory::Publishing:
        return QStringLiteral("Publishing");
    case BackupCategory::GameDev:
        return QStringLiteral("GameDev");
    }
    return QStringLiteral("Unknown");
}

BackupCategory categoryForKind(ProjectKind kind) {
    for (const BackupTemplate& tpl : all()) {
        if (tpl.kind == kind) {
            return tpl.category;
        }
    }
    return BackupCategory::DawTraditional;
}

ProjectKind kindForFileName(const QString& fileName) {
    const QString lowerName = fileName.toLower();
    const QString baseName = baseNameOf(lowerName);

    for (const BackupTemplate& tpl : all()) {
        for (const QString& marker : tpl.projectRootMarkers) {
            if (baseName == baseNameOf(normalizeLower(marker))) {
                return tpl.kind;
            }
        }
        for (const QString& extension : tpl.projectFileExtensions) {
            if (lowerName.endsWith(QLatin1Char('.') + extension)) {
                return tpl.kind;
            }
        }
    }
    return ProjectKind::Unknown;
}

ProjectKind kindForRootMarkerPath(const QString& relativePath) {
    const QString normalized = normalizeLower(relativePath);
    for (const BackupTemplate& tpl : all()) {
        for (const QString& marker : tpl.projectRootMarkers) {
            if (normalized == normalizeLower(marker)) {
                return tpl.kind;
            }
        }
    }
    return ProjectKind::Unknown;
}

QString projectRootForMarker(const QString& marker, const QString& markerFilePath) {
    QDir dir = QFileInfo(markerFilePath).absoluteDir();
    const QString markerDir = QFileInfo(marker).path();
    if (markerDir.isEmpty() || markerDir == QLatin1String(".")) {
        return dir.absolutePath();
    }

    const QStringList segments = markerDir.split(QLatin1Char('/'), Qt::SkipEmptyParts);
    QString root = dir.absolutePath();
    for (int i = 0; i < segments.size(); ++i) {
        root = QDir(root).absoluteFilePath(QStringLiteral(".."));
    }
    return QDir::cleanPath(root);
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

    const QString baseName = baseNameOf(normalized);
    for (const BackupTemplate& tpl : all()) {
        for (const QString& marker : tpl.projectRootMarkers) {
            if (baseName == baseNameOf(normalizeLower(marker))) {
                return true;
            }
        }
    }

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
