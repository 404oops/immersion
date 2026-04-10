#pragma once

#include <QString>

namespace AppStrings {

inline const QString AppName = QStringLiteral("musit");
inline const QString StatusSelectProjectsFolder =
    QStringLiteral("Please select the folder where all of your project files sit.");
inline const QString DialogSelectProjectsFolder =
    QStringLiteral("Select Folder Containing Project Files");

inline const QString SortName = QStringLiteral("Name");
inline const QString SortLastOpened = QStringLiteral("Last Opened");
inline const QString LogLevelInfo = QStringLiteral("Info");
inline const QString LogLevelDebug = QStringLiteral("Debug");

inline const QString LabelDiscoveredProjects = QStringLiteral("Discovered Projects");
inline const QString LabelActivity = QStringLiteral("Activity");
inline const QString LabelSort = QStringLiteral("Sort:");
inline const QString LabelLogLevel = QStringLiteral("Log level:");
inline const QString LabelSearchPlaceholder = QStringLiteral("Search projects...");
inline const QString LabelChooseProjectFolder = QStringLiteral("Choose Project Folder");

inline const QString StatusNoSupportedProjects =
    QStringLiteral("No supported project files found (.bwproject, .flp, .als, .logicx, .cpr, .rpp, .ardour, .xrns, .ptx, .song, .cwp, .reason, .rsn, .band)");
inline const QString StatusFirstTimeSetupFmt =
    QStringLiteral("First-time setup complete. Found %1 project folders.");

inline const QString ActivityNoProjectFilesFmt =
    QStringLiteral("[%1] first-day setup found no project files under %2");
inline const QString ActivityDiscoveredFmt =
    QStringLiteral("[%1] discovered %2 projects from %3");
inline const QString ActivityRegistryStatusFmt =
    QStringLiteral("[%1] project registry %2: %3");
inline const QString RegistrySaved = QStringLiteral("saved");
inline const QString RegistryPartiallySaved = QStringLiteral("partially saved");
inline const QString ActivityDefaultSelectionFmt =
    QStringLiteral("[%1] default selection=%2 (%3)");

inline const QString StatusNoProjectSelected = QStringLiteral("No project folder selected");
inline const QString StatusMonitoringActiveFmt = QStringLiteral("Monitoring already active: %1");
inline const QString StatusFailedInitStorage = QStringLiteral("Failed to initialize storage under .musit");
inline const QString StatusFailedCreateWatcher = QStringLiteral("Failed to create file watcher");
inline const QString StatusFailedStartWatcher = QStringLiteral("Failed to start watcher");
inline const QString StatusMonitoringFmt = QStringLiteral("Monitoring: %1");
inline const QString StatusMonitoringStopped = QStringLiteral("Monitoring stopped");
inline const QString ActivityMonitoringStartedFmt = QStringLiteral("[%1] monitoring started");
inline const QString ActivityMonitoringStoppedFmt = QStringLiteral("[%1] monitoring stopped");

inline const QString StatusNoProjectSelectedForRestore = QStringLiteral("No project selected for restore");
inline const QString StatusNoProjectFileSelectedForRestore = QStringLiteral("No project file selected for restore");
inline const QString StatusNoVersionHistoryForProject = QStringLiteral("No version history found for this project");
inline const QString StatusNoRestorableVersionsForFile = QStringLiteral("No restorable versions found for selected file");
inline const QString StatusRestoredFmt = QStringLiteral("Restored: %1");
inline const QString StatusRestoreFailedFmt = QStringLiteral("Restore failed: %1");
inline const QString ActivityRestoredFmt = QStringLiteral("[%1] restored %2 from %3");
inline const QString ActivityRestoreFailedFmt = QStringLiteral("[%1] restore failed for %2");

inline const QString StatusNoProjectSelectedForManage = QStringLiteral("No project selected for managing versions");
inline const QString StatusNoProjectFileSelectedForManage = QStringLiteral("No project file selected for managing versions");
inline const QString StatusNoVersionsToManage = QStringLiteral("No versions available to manage");
inline const QString DialogManageTitle = QStringLiteral("Manage");
inline const QString DialogManageVersionsForFmt = QStringLiteral("Manage versions for %1");
inline const QString ButtonCancel = QStringLiteral("Cancel");
inline const QString ButtonAddEditNote = QStringLiteral("Add/Edit Note");
inline const QString ButtonDeleteSelectedVersions = QStringLiteral("Delete Selected Versions");
inline const QString StatusSelectVersionRowForNote = QStringLiteral("Select a version row first to add/edit note");
inline const QString DialogVersionNoteTitle = QStringLiteral("Version Note");
inline const QString DialogVersionNotePrompt = QStringLiteral("Track change for this version:");
inline const QString StatusFailedSaveVersionNote = QStringLiteral("Failed to save version note");
inline const QString StatusNoVersionsSelectedForDeletion = QStringLiteral("No versions selected for deletion");
inline const QString StatusFailedRewriteVersionLog = QStringLiteral("Failed to rewrite version log");
inline const QString StatusFailedPersistDeletedVersions = QStringLiteral("Failed to persist deleted versions");
inline const QString StatusDeletedVersionsFmt = QStringLiteral("Deleted %1 versions for %2");

inline const QString TrayShowApp = QStringLiteral("Show musit");
inline const QString TraySelectProject = QStringLiteral("Select Project");
inline const QString TrayPastVersions = QStringLiteral("Past Versions");
inline const QString TrayQuit = QStringLiteral("Quit");
inline const QString TrayNoProjectsDiscovered = QStringLiteral("No projects discovered");
inline const QString TraySelectProjectFirst = QStringLiteral("Select a project first");
inline const QString TrayNoVersionHistoryYet = QStringLiteral("No version history yet");
inline const QString TrayNoRestorableVersionsYet = QStringLiteral("No restorable versions yet");
inline const QString TrayStillRunning = QStringLiteral("Still running in tray.");
inline const QString TraySaveDetectedAndVersionedFmt = QStringLiteral("Save detected and versioned: %1");
inline const QString TrayRestoredFmt = QStringLiteral("Restored %1");

inline const QString PlatformMacOS = QStringLiteral("macOS");
inline const QString PlatformWindows = QStringLiteral("Windows");
inline const QString PlatformLinux = QStringLiteral("Linux");
inline const QString PermissionHintMacOS = QStringLiteral("Grant Files and Folders access in System Settings if prompted, and ensure the project folder is writable.");
inline const QString PermissionHintWindows = QStringLiteral("Run from a writable user folder and ensure Controlled Folder Access/antivirus is not blocking file writes.");
inline const QString PermissionHintLinux = QStringLiteral("Ensure the project folder and config directory are writable for this user.");

inline const QString StatusNoSupportedProjectFiles = QStringLiteral("No supported project files found.");
inline const QString StatusSelectedFmt = QStringLiteral("Selected: %1 (%2)");
inline const QString StatusFailedInitializeVersioning = QStringLiteral("Failed to initialize versioning");
inline const QString StatusRestoredToVersionFmt = QStringLiteral("Restored %1 to version %2");
inline const QString ActivityFirstDaySetupNoProjectsFmt = QStringLiteral("[%1] first-day setup found no project files under %2");
inline const QString ActivityDiscoveredProjectsFmt = QStringLiteral("[%1] discovered %2 projects from %3");
inline const QString ActivityOpenFailedNoFileSelected = QStringLiteral("[%1] open failed: no project file selected");
inline const QString ActivityOpenFailedMissingFileFmt = QStringLiteral("[%1] open failed: missing file %2");
inline const QString ActivityOpenFailedFmt = QStringLiteral("[%1] open failed for %2");
inline const QString ActivityFileAssociationUnavailableOpenedFolderFmt = QStringLiteral("[%1] file association unavailable; opened folder %2");
inline const QString ActivityOpenedFmt = QStringLiteral("[%1] opened %2");
inline const QString ActivityNoVersionHistoryAvailableFmt = QStringLiteral("[%1] no version history available for %2");
inline const QString DialogRestoreProjectVersionTitle = QStringLiteral("Restore Project Version");
inline const QString DialogChooseVersionToRestoreFmt = QStringLiteral("Choose a version to restore for %1:");
inline const QString DialogRestoreFailedTitle = QStringLiteral("Restore Failed");
inline const QString DialogSnapshotDataUnavailableFmt = QStringLiteral("Snapshot data for version %1 is unavailable.");
inline const QString DialogCouldNotRestoreVersionFmt = QStringLiteral("Could not restore version %1.");
inline const QString DialogRestoreCompleteTitle = QStringLiteral("Restore Complete");
inline const QString DialogRestoredHasBeenFmt = QStringLiteral("%1 has been restored to version %2.");
inline const QString ActivityNoVersionsToManageFmt = QStringLiteral("[%1] no versions to manage for %2");
inline const QString DialogManageProjectVersionsTitle = QStringLiteral("Manage Project Versions");
inline const QString DialogHasTrackedVersionsFmt = QStringLiteral("%1 has %2 tracked versions.");
inline const QString VersionChoiceFmt = QStringLiteral("v%1  (%2)");
inline const QString VersionDetailsLineFmt = QStringLiteral("v%1  %2\n");
inline const QString ButtonRestoreVersion = QStringLiteral("Restore Version");
inline const QString ButtonDeleteVersion = QStringLiteral("Manage Versions");
inline const QString ButtonOpenStagingFolder = QStringLiteral("Open Staging Folder");
inline const QString DialogDeleteVersionsTitle = QStringLiteral("Manage Versions");
inline const QString DialogSelectStagedVariationsFmt = QStringLiteral("Select staged variation(s) to delete for %1:");
inline const QString ButtonSelectAll = QStringLiteral("Select All");
inline const QString ButtonClear = QStringLiteral("Clear");
inline const QString ButtonDeleteSelected = QStringLiteral("Delete Selected");
inline const QString DialogNoVersionsSelected = QStringLiteral("No versions were selected.");
inline const QString DialogConfirmDeleteTitle = QStringLiteral("Confirm Delete");
inline const QString DialogConfirmDeleteFmt = QStringLiteral("Delete %1 staged variation(s): v%2?\n\nThis removes backup files from .musit/staging and cannot be undone.");
inline const QString DialogDeleteVersionsResultTitle = QStringLiteral("Manage Versions Result");
inline const QString DialogDeleteVersionsResultFmt = QStringLiteral("Deleted: %1\nAlready missing: %2\nFailed: %3\nNot found: %4");
inline const QString ActivityDeleteSkippedVersionNotFoundFmt = QStringLiteral("[%1] delete skipped: version v%2 not found for %3");
inline const QString ActivityDeleteSkippedMissingStagedFmt = QStringLiteral("[%1] delete skipped: staged file already missing for %2 (v%3)");
inline const QString ActivityDeleteFailedRemoveFmt = QStringLiteral("[%1] delete failed: could not remove staged file for %2 (v%3)");
inline const QString ActivityDeletedStagedVariationFmt = QStringLiteral("[%1] deleted staged variation v%2 for %3");
inline const QString ActivityRestoreFailedSnapshotMissingFmt = QStringLiteral("[%1] restore failed: staged snapshot missing for %2 (v%3)");
inline const QString ActivityRestoreFailedCreateFolderFmt = QStringLiteral("[%1] restore failed: could not create folder for %2");
inline const QString ActivityRestoreFailedReplaceFmt = QStringLiteral("[%1] restore failed: could not replace %2");
inline const QString ActivityRestoreFailedCopyFmt = QStringLiteral("[%1] restore failed: copy failed from %2 to %3");
inline const QString ActivityRestoredToVersionFmt = QStringLiteral("[%1] restored %2 to version %3");
inline const QString ActivityOpenedStagingFolderFmt = QStringLiteral("[%1] opened staging folder for %2");
inline const QString ActivityFailedOpenStagingFolderFmt = QStringLiteral("[%1] failed to open staging folder for %2");
inline const QString StatusCouldNotResolveAppDataDir = QStringLiteral("Could not resolve app config directory (AppDataLocation).");
inline const QString StatusCouldNotCreateConfigDirFmt = QStringLiteral("Could not create config directory: %1");
inline const QString StatusConfigDirectoryNotWritableFmt = QStringLiteral("Config directory is not writable: %1");
inline const QString StatusSavedProjectsFolderFmt = QStringLiteral("Saved projects folder: %1");
inline const QString StatusSavedProjectsFolderMissingFmt = QStringLiteral("Saved projects folder no longer exists: %1");
inline const QString StatusCouldNotInitializeMusitStorageFmt = QStringLiteral("Could not initialize .musit storage under: %1");
inline const QString StatusMusitStorageNotWritableFmt = QStringLiteral(".musit storage is not writable in: %1");
inline const QString ActivityStartupSelfCheckPassedFmt = QStringLiteral("[%1] startup self-check passed on %2");
inline const QString ActivityStartupSelfCheckFoundIssuesFmt = QStringLiteral("[%1] startup self-check found %2 issue(s) on %3");
inline const QString DialogStartupSelfCheckTitle = QStringLiteral("musit Startup Self-Check");
inline const QString DialogSomeEnvironmentChecksFailedFmt = QStringLiteral("Some environment checks failed on %1.");
inline const QString DialogIssuesAndSuggestedFixFmt = QStringLiteral("Issues:\n- %1\n\nSuggested fix:\n%2");
inline const QString DialogChecksFmt = QStringLiteral("Checks:\n- %1");
inline const QString DialogCouldNotResolveAppDataDir = QStringLiteral("Could not resolve app config directory (AppDataLocation).");
inline const QString DialogConfigDirectoryFmt = QStringLiteral("Config directory: %1");
inline const QString DialogCouldNotCreateConfigDirFmt = QStringLiteral("Could not create config directory: %1");
inline const QString DialogConfigDirectoryNotWritableFmt = QStringLiteral("Config directory is not writable: %1");
inline const QString DialogSavedProjectsFolderFmt = QStringLiteral("Saved projects folder: %1");
inline const QString DialogSavedProjectsFolderMissingFmt = QStringLiteral("Saved projects folder no longer exists: %1");
inline const QString DialogCouldNotInitializeMusitStorageFmt = QStringLiteral("Could not initialize .musit storage under: %1");
inline const QString DialogMusitStorageNotWritableFmt = QStringLiteral(".musit storage is not writable in: %1");
inline const QString StatusSelectedProjectFmt = QStringLiteral("Selected: %1 (%2)");

inline const QString MusitVersionLogRelativePath = QStringLiteral(".musit/versions/log.jsonl");
inline const QString MusitStagingRelativePath = QStringLiteral(".musit/staging");

} // namespace AppStrings
