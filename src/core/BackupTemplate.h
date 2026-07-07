#pragma once

#include <QList>
#include <QString>
#include <QStringList>

#include "ProjectDiscovery.h"

// Category labels map to UI icon groups you can asset independently.
enum class BackupCategory {
    DawElectronic,
    DawTraditional,
    Video,
    MotionVfx,
    ThreeD,
    Photo,
    VectorIllustration,
    Publishing,
    GameDev,
};

// A BackupTemplate describes, for one data type, WHAT gets versioned:
// the main project file (or directory bundle) plus accompanying manifests,
// minus bulky media (audio/video/cache) via include/exclude patterns.
//
// HOW TO ADD A NEW DATA TYPE
// 1. Add ProjectKind + kindToString in ProjectDiscovery.
// 2. Append one entry below in BackupTemplate.cpp under the right category.
//
// Pattern syntax (case-insensitive, '/' separators):
//   **  matches anything including '/'
//   *   matches within one path segment
struct BackupTemplate {
    ProjectKind kind {ProjectKind::Unknown};
    BackupCategory category {BackupCategory::DawTraditional};
    QStringList projectFileExtensions;
    // Marker file relative to project root (e.g. "project.godot", "ProjectSettings/ProjectVersion.txt").
    QStringList projectRootMarkers;
    bool projectFileIsBundle {false};
    QStringList includePatterns;
    QStringList excludePatterns;
};

namespace BackupTemplates {

inline constexpr int kUncompressedRecentVersions = 5;

const QList<BackupTemplate>& all();

QString categoryToString(BackupCategory category);
BackupCategory categoryForKind(ProjectKind kind);

ProjectKind kindForFileName(const QString& fileName);
ProjectKind kindForRootMarkerPath(const QString& relativePath);
QString projectRootForMarker(const QString& marker, const QString& markerFilePath);
bool kindIsBundle(ProjectKind kind);
bool shouldTrackPath(const QString& relativePath);
QString artifactForPath(const QString& relativePath);

} // namespace BackupTemplates
