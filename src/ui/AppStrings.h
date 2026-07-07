#pragma once

#include <QString>

namespace AppStrings {

inline const QString StatusSelectProjectsFolder =
    QStringLiteral("Please select the folder where all of your project files sit.");

inline const QString SortName = QStringLiteral("Name");
inline const QString SortLastOpened = QStringLiteral("Last Opened");
inline const QString LogLevelInfo = QStringLiteral("Info");
inline const QString LogLevelDebug = QStringLiteral("Debug");

inline const QString StatusFirstTimeSetupFmt =
    QStringLiteral("First-time setup complete. Found %1 project folders.");
inline const QString StatusScanningProjectsFmt =
    QStringLiteral("Scanning %1 for projects...");
inline const QString StatusScanningProjectsProgressFmt =
    QStringLiteral("Scanning %1 (%2 folders)...");
inline const QString StatusScanningProjectsWithCountFmt =
    QStringLiteral("Scanning %1 (%2 folders, %3 projects)...");
inline const QString StatusInitializingMonitoringFmt =
    QStringLiteral("Initializing versioning (%1/%2)...");
inline const QString StatusNoSupportedProjectFiles = QStringLiteral("No supported project files found.");
inline const QString StatusSelectedProjectFmt = QStringLiteral("Selected: %1 (%2)");

inline const QString StatusFailedInitializeVersioning = QStringLiteral("Failed to initialize versioning");
inline const QString StatusFailedCreateWatcher = QStringLiteral("Failed to create file watcher");
inline const QString StatusFailedStartWatcher = QStringLiteral("Failed to start watcher");
inline const QString StatusMonitoringFmt = QStringLiteral("Monitoring: %1");
inline const QString StatusRestoredToVersionFmt = QStringLiteral("Restored %1 to version %2");

inline const QString ActivityFirstDaySetupNoProjectsFmt = QStringLiteral("[%1] first-day setup found no project files under %2");
inline const QString ActivityDiscoveredProjectsFmt = QStringLiteral("[%1] discovered %2 projects from %3");
inline const QString ActivityProjectRediscoveredFmt =
    QStringLiteral("[%1] new project detected: %2 (%3)");
inline const QString ActivityProjectScanStartedFmt = QStringLiteral("[%1] project scan started: %2");
inline const QString ActivityProjectScanDirectoryFmt = QStringLiteral("[%1] scan folder: %2");
inline const QString ActivityProjectScanCompletedFmt =
    QStringLiteral("[%1] project scan finished: %2 folders in %3 ms (%4 projects)");
inline const QString ActivityProjectScanCancelledFmt = QStringLiteral("[%1] project scan cancelled: %2");
inline const QString ActivityWatcherScanFmt =
    QStringLiteral("[%1] watcher %2 scan: %3 items in %4 ms (%5)");
inline const QString ActivityOpenFailedNoFileSelected = QStringLiteral("[%1] open failed: no project file selected");
inline const QString ActivityOpenFailedMissingFileFmt = QStringLiteral("[%1] open failed: missing file %2");
inline const QString ActivityOpenFailedFmt = QStringLiteral("[%1] open failed for %2");
inline const QString ActivityFileAssociationUnavailableOpenedFolderFmt = QStringLiteral("[%1] file association unavailable; opened folder %2");
inline const QString ActivityOpenedFmt = QStringLiteral("[%1] opened %2");
inline const QString ActivityDeletedStagedVariationFmt = QStringLiteral("[%1] deleted staged variation v%2 for %3");
inline const QString ActivityRestoreFailedSnapshotMissingFmt = QStringLiteral("[%1] restore failed: staged snapshot missing for %2 (v%3)");
inline const QString ActivityRestoreFailedCreateFolderFmt = QStringLiteral("[%1] restore failed: could not create folder for %2");
inline const QString ActivityRestoreFailedReplaceFmt = QStringLiteral("[%1] restore failed: could not replace %2");
inline const QString ActivityRestoreFailedCopyFmt = QStringLiteral("[%1] restore failed: copy failed from %2 to %3");
inline const QString ActivityRestoredToVersionFmt = QStringLiteral("[%1] restored %2 to version %3");
inline const QString ActivityStartupSelfCheckPassedFmt = QStringLiteral("[%1] startup self-check passed on %2");
inline const QString ActivityStartupSelfCheckFoundIssuesFmt = QStringLiteral("[%1] startup self-check found %2 issue(s) on %3");

inline const QString DialogCouldNotResolveAppDataDir = QStringLiteral("Could not resolve app config directory.");
inline const QString DialogConfigDirectoryFmt = QStringLiteral("Config directory: %1");
inline const QString DialogCouldNotCreateConfigDirFmt = QStringLiteral("Could not create config directory: %1");
inline const QString DialogConfigDirectoryNotWritableFmt = QStringLiteral("Config directory is not writable: %1");
inline const QString DialogSavedProjectsFolderFmt = QStringLiteral("Saved projects folder: %1");
inline const QString DialogSavedProjectsFolderMissingFmt = QStringLiteral("Saved projects folder no longer exists: %1");
inline const QString DialogCouldNotInitializeMusitStorageFmt = QStringLiteral("Could not initialize .musit storage under: %1");
inline const QString DialogMusitStorageNotWritableFmt = QStringLiteral(".musit storage is not writable in: %1");

inline const QString PlatformMacOS = QStringLiteral("macOS");
inline const QString PlatformWindows = QStringLiteral("Windows");
inline const QString PlatformLinux = QStringLiteral("Linux");

inline const QString MusitVersionLogRelativePath = QStringLiteral(".musit/versions/log.jsonl");
inline const QString MusitStagingRelativePath = QStringLiteral(".musit/staging");

} // namespace AppStrings
