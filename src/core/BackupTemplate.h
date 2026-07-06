#pragma once

#include <QList>
#include <QString>
#include <QStringList>

#include "ProjectDiscovery.h"

// A BackupTemplate describes, for one data type, WHAT gets versioned:
// the main project file (or directory bundle) plus any accompanying files
// inside its folder structure, minus bulky assets like audio.
//
// HOW TO ADD A NEW DATA TYPE
// 1. Add a value to ProjectKind (ProjectDiscovery.h) and a display name in
//    ProjectDiscovery::kindToString / knownKinds.
// 2. Append one BackupTemplate entry to the registry in BackupTemplate.cpp.
//
// Pattern syntax (matched case-insensitively against the project-relative
// path with '/' separators):
//   **  matches anything, including '/'
//   *   matches anything within one path segment (no '/')
//   ?   matches a single character (no '/')
// Example: "**.logicx/**/projectdata" matches
//   "Song.logicx/Alternatives/000/ProjectData" and
//   "sub dir/Song.logicx/Alternatives/000/ProjectData".
//
// Files with audio/temp extensions (wav, aiff, flac, ...) are excluded
// globally and never need per-template exclude entries.
struct BackupTemplate {
    ProjectKind kind {ProjectKind::Unknown};
    // Extensions (lowercase, no dot) identifying the main project file.
    QStringList projectFileExtensions;
    // True when the "project file" is a directory bundle (.logicx, .band):
    // the bundle itself is discovered as the project file, and the files
    // inside it that match includePatterns are what actually gets versioned.
    bool projectFileIsBundle {false};
    // Accompanying files to version, e.g. manifests inside a bundle.
    QStringList includePatterns;
    // Carve-outs inside included areas, e.g. media folders inside a bundle.
    QStringList excludePatterns;
};

namespace BackupTemplates {

// How many of the newest versions per artifact keep an uncompressed staged
// copy; older versions exist only as compressed objects.
inline constexpr int kUncompressedRecentVersions = 5;

const QList<BackupTemplate>& all();

// Classify a file/bundle name by its extension; Unknown if no template matches.
ProjectKind kindForFileName(const QString& fileName);
bool kindIsBundle(ProjectKind kind);

// Whether a project-relative path should be versioned according to the
// templates (main project files, bundle internals per include/exclude,
// global audio/temp exclusions). Directory-ignore rules are the caller's
// concern (see ProjectConfig::shouldTrack).
bool shouldTrackPath(const QString& relativePath);

// The artifact a path belongs to: the enclosing bundle root (for paths
// inside .logicx/.band bundles) or the path itself. Versions are numbered
// and restored per artifact.
QString artifactForPath(const QString& relativePath);

} // namespace BackupTemplates
