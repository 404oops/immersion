// End-to-end test for universal backup templates + tiered compressed storage.
//
// Exercises the real production stack (discovery, watcher, snapshot service,
// metadata/object stores, QmlBackend restore) against a fake Logic bundle:
//   1. bundle internals (ProjectData, plists) are tracked, audio is not
//   2. bundle files saved together share one grouped version
//   3. staged copies are compacted past the 5 newest versions per file
//   4. a compacted version restores from the compressed object store
//   5. saving after a restore branches (1.1) on the artifact
//
// Build with -DMUSIT_BUILD_TESTS=ON; run the musit_e2e_test binary.

#include <QCoreApplication>
#include <QDeadlineTimer>
#include <QDir>
#include <QDirIterator>
#include <QFile>
#include <QFileInfo>
#include <QGuiApplication>
#include <QTemporaryDir>
#include <QThread>
#include <QUrl>
#include <QVariantList>
#include <QVariantMap>

#include <cstdint>

#include <cstdio>
#include <functional>

#include "../src/core/BackupTemplate.h"
#include "../src/core/SnapshotService.h"
#include "../src/persistence/ObjectStore.h"
#include "../src/ui/QmlBackend.h"

namespace {

int g_passed = 0;
int g_failed = 0;

void check(bool condition, const char* name) {
    if (condition) {
        ++g_passed;
        std::printf("PASS  %s\n", name);
    } else {
        ++g_failed;
        std::printf("FAIL  %s\n", name);
    }
    std::fflush(stdout);
}

bool waitFor(const std::function<bool()>& predicate, int timeoutMs) {
    QDeadlineTimer deadline(timeoutMs);
    while (!deadline.hasExpired()) {
        if (predicate()) {
            return true;
        }
        QCoreApplication::processEvents(QEventLoop::AllEvents, 50);
        QThread::msleep(25);
    }
    return predicate();
}

bool writeFile(const QString& path, const QByteArray& contents) {
    if (!QDir().mkpath(QFileInfo(path).dir().absolutePath())) {
        return false;
    }
    QFile file(path);
    if (!file.open(QIODevice::WriteOnly | QIODevice::Truncate)) {
        return false;
    }
    return file.write(contents) == contents.size();
}

QByteArray readFile(const QString& path) {
    QFile file(path);
    if (!file.open(QIODevice::ReadOnly)) {
        return {};
    }
    return file.readAll();
}

QVariantMap versionById(const QVariantList& versions, const QString& id) {
    for (const QVariant& value : versions) {
        const QVariantMap version = value.toMap();
        if (version.value(QStringLiteral("id")).toString() == id) {
            return version;
        }
    }
    return {};
}

int countStagedFilesNamed(const QString& projectRoot, const QString& fileName) {
    const QString stagingRoot = QDir(projectRoot).filePath(QStringLiteral(".musit/staging"));
    int count = 0;
    QDirIterator it(stagingRoot, QDir::Files | QDir::Hidden, QDirIterator::Subdirectories);
    while (it.hasNext()) {
        it.next();
        if (it.fileName() == fileName) {
            ++count;
        }
    }
    return count;
}

bool stagingContainsSuffix(const QString& projectRoot, const QString& suffix) {
    const QString stagingRoot = QDir(projectRoot).filePath(QStringLiteral(".musit/staging"));
    QDirIterator it(stagingRoot, QDir::Files | QDir::Hidden, QDirIterator::Subdirectories);
    while (it.hasNext()) {
        it.next();
        if (it.fileName().endsWith(suffix, Qt::CaseInsensitive)) {
            return true;
        }
    }
    return false;
}

int projectIndexByFile(const QmlBackend& backend, const QString& fileName) {
    const QVariantList projects = backend.projects();
    for (int i = 0; i < projects.size(); ++i) {
        if (projects.at(i).toMap().value(QStringLiteral("file")).toString() == fileName) {
            return i;
        }
    }
    return -1;
}

bool fileContains(const QString& path, const QByteArray& needle) {
    return readFile(path).contains(needle);
}

} // namespace

int main(int argc, char** argv) {
    // Isolate from the user's real config and any window system.
    QTemporaryDir fakeHome;
    if (!fakeHome.isValid()) {
        std::printf("FATAL cannot create temp home\n");
        return 2;
    }

    qputenv("HOME", fakeHome.path().toUtf8());
    qputenv("USERPROFILE", fakeHome.path().toUtf8());

    const QString xdgDataHome = fakeHome.filePath(QStringLiteral(".local/share"));
    QDir().mkpath(xdgDataHome);
    qputenv("XDG_DATA_HOME", QDir::toNativeSeparators(xdgDataHome).toUtf8());

#if defined(Q_OS_WIN)
    const QString appDataRoaming = fakeHome.filePath(QStringLiteral("AppData/Roaming"));
    QDir().mkpath(appDataRoaming);
    qputenv("APPDATA", QDir::toNativeSeparators(appDataRoaming).toUtf8());
#endif

    qputenv("QT_QPA_PLATFORM", "offscreen");

    QCoreApplication::setOrganizationName(QStringLiteral("musit"));
    QCoreApplication::setOrganizationDomain(QStringLiteral("musit.app"));
    QCoreApplication::setApplicationName(QStringLiteral("musit"));

    QGuiApplication app(argc, argv);

    // ---- Template registry unit checks (pure functions) ----
    check(BackupTemplates::shouldTrackPath("Song.logicx/Alternatives/000/ProjectData"),
          "template: ProjectData inside .logicx bundle is tracked");
    check(BackupTemplates::shouldTrackPath("Song.logicx/Metadata.plist"),
          "template: plist inside .logicx bundle is tracked");
    check(!BackupTemplates::shouldTrackPath("Song.logicx/Media/kick.wav"),
          "template: audio inside .logicx Media is excluded");
    check(!BackupTemplates::shouldTrackPath("Song.logicx/Resources/stray.wav"),
          "template: stray audio inside bundle is excluded globally");
    check(BackupTemplates::shouldTrackPath("My Set.als"),
          "template: top-level .als project file is tracked");
    check(!BackupTemplates::shouldTrackPath("notes.txt"),
          "template: unrelated file is not tracked");
    check(BackupTemplates::artifactForPath("Song.logicx/Alternatives/000/ProjectData")
              == QStringLiteral("Song.logicx"),
          "template: bundle-internal path maps to bundle artifact");
    check(BackupTemplates::artifactForPath("My Set.als") == QStringLiteral("My Set.als"),
          "template: single-file path is its own artifact");

    // ---- Compressed object integrity ----
    QTemporaryDir objectStoreDir;
    const QString objectSource = objectStoreDir.filePath(QStringLiteral("source.bin"));
    const QString objectRestore = objectStoreDir.filePath(QStringLiteral("restored.bin"));
    ObjectStore objectStore(objectStoreDir.filePath(QStringLiteral(".musit")));
    check(objectStore.init() && writeFile(objectSource, "object contents"),
          "object store: fixture initialized");
    const QString objectHash = objectStore.storeFile(objectSource);
    check(!objectHash.isEmpty(), "object store: compressed object created");
    check(writeFile(objectStore.objectPathForHash(objectHash), "corrupt"),
          "object store: compressed object corrupted for recovery test");
    check(objectStore.storeFile(objectSource) == objectHash,
          "object store: corrupt existing object repaired");
    check(objectStore.extractObject(objectHash, objectRestore)
              && readFile(objectRestore) == QByteArray("object contents"),
          "object store: repaired object restores correctly");

    // ---- Loose-files layout discovery ----
    QTemporaryDir looseProjectsDir;
    const QString nestedFlp = QDir(looseProjectsDir.path()).filePath(
        QStringLiteral("Nested Song/Nested Song.flp"));
    const QString ignoredLogicBundleFile = QDir(looseProjectsDir.path()).filePath(
        QStringLiteral("Bundled Song.logicx/Alternatives/000/ProjectData"));
    check(writeFile(nestedFlp, "fake flp")
              && writeFile(ignoredLogicBundleFile, "fake bundled project"),
          "files layout: loose and bundled fixtures created");

    const QList<DiscoveredProject> looseProjects = ProjectDiscovery().discoverAll(
        looseProjectsDir.path(), {}, {}, {}, 20, ProjectsFolderLayout::Files);
    check(looseProjects.size() == 1,
          "files layout: recursively finds only naked project files");
    check(looseProjects.value(0).primaryProjectFile == QStringLiteral("Nested Song.flp"),
          "files layout: project entry uses matching file extension");

    // ---- Nested-project routing, migration, UI refresh, and restart ----
    QTemporaryDir routingProjectsDir;
    const QString routingRoot = routingProjectsDir.path();
    const QString rootProjectFile = QDir(routingRoot).filePath(QStringLiteral("Root.flp"));
    const QString legacyProjectRoot =
        QDir(routingRoot).filePath(QStringLiteral("Legacy Project"));
    const QString legacyProjectFile =
        QDir(legacyProjectRoot).filePath(QStringLiteral("Legacy Project.flp"));
    check(writeFile(rootProjectFile, "root project")
              && writeFile(legacyProjectFile, "legacy v1"),
          "rediscovery routing: fixtures created");

    // Reproduce history written by the old ancestor-routing bug.
    SnapshotService ancestorSnapshots;
    check(ancestorSnapshots.setProjectRoot(routingRoot)
              && ancestorSnapshots.snapshotPathNow(
                  legacyProjectFile,
                  QStringLiteral("Legacy Project/Legacy Project.flp"))
              && writeFile(legacyProjectFile, "legacy v2")
              && ancestorSnapshots.snapshotPathNow(
                  legacyProjectFile,
                  QStringLiteral("Legacy Project/Legacy Project.flp")),
          "rediscovery migration: ancestor history fixture created");

    const QString ancestorLog =
        QDir(routingRoot).filePath(QStringLiteral(".musit/versions/log.jsonl"));
    check(fileContains(ancestorLog, "Legacy Project/Legacy Project.flp"),
          "rediscovery migration: history starts under ancestor root");

    {
        QmlBackend routingBackend;
        int projectsChangedCount = 0;
        QObject::connect(&routingBackend, &QmlBackend::projectsChanged,
                         [&projectsChangedCount]() { ++projectsChangedCount; });
        routingBackend.confirmProjectsFolder(routingRoot, QStringLiteral("Bundles"));

        check(waitFor([&]() {
            return !routingBackend.property("isScanningProjects").toBool()
                && routingBackend.projects().size() == 2
                && routingBackend.statusMessage().startsWith(QStringLiteral("Monitoring:"));
        }, 20000), "rediscovery migration: initial monitoring ready");

        const int legacyIndex =
            projectIndexByFile(routingBackend, QStringLiteral("Legacy Project.flp"));
        check(legacyIndex >= 0
                  && routingBackend.getProjectVersions(legacyIndex).size() == 2,
              "rediscovery migration: ancestor versions moved into project");
        check(!fileContains(ancestorLog, "Legacy Project/Legacy Project.flp"),
              "rediscovery migration: ancestor log no longer owns project history");

        const int changesBeforeNewProject = projectsChangedCount;
        const QString newProjectRoot =
            QDir(routingRoot).filePath(QStringLiteral("New Project"));
        const QString newProjectFile =
            QDir(newProjectRoot).filePath(QStringLiteral("New Project.flp"));
        check(writeFile(newProjectFile, "new v1"),
              "rediscovery routing: new nested project created");

        check(waitFor([&]() {
            return projectIndexByFile(
                       routingBackend, QStringLiteral("New Project.flp")) >= 0;
        }, 20000), "rediscovery routing: new project initialized");
        check(projectsChangedCount > changesBeforeNewProject,
              "rediscovery UI: main projects model refreshed after initialization");

        int newProjectIndex =
            projectIndexByFile(routingBackend, QStringLiteral("New Project.flp"));
        check(newProjectIndex >= 0
                  && routingBackend.getProjectVersions(newProjectIndex).size() == 1,
              "rediscovery routing: initialization creates one baseline");

        check(writeFile(newProjectFile, "new v2"),
              "rediscovery routing: nested project save written");
        check(waitFor([&]() {
            newProjectIndex =
                projectIndexByFile(routingBackend, QStringLiteral("New Project.flp"));
            return newProjectIndex >= 0
                && routingBackend.getProjectVersions(newProjectIndex).size() == 2;
        }, 20000), "rediscovery routing: save recorded in nested project");
        check(!fileContains(ancestorLog, "New Project/New Project.flp"),
              "rediscovery routing: ancestor project did not claim nested saves");
    }

    {
        QmlBackend restartedBackend;
        restartedBackend.loadProjectsFromFolder(routingRoot, QStringLiteral("Bundles"));
        check(waitFor([&]() {
            return !restartedBackend.property("isScanningProjects").toBool()
                && restartedBackend.projects().size() == 3;
        }, 20000), "rediscovery restart: projects loaded");
        const int restartedIndex =
            projectIndexByFile(restartedBackend, QStringLiteral("New Project.flp"));
        check(restartedIndex >= 0
                  && restartedBackend.getProjectVersions(restartedIndex).size() == 2,
              "rediscovery restart: nested history persisted without new baseline");
        restartedBackend.resetConfig();
    }

    // ---- Fake Logic project ----
    QTemporaryDir projectsDir;
    if (!projectsDir.isValid()) {
        std::printf("FATAL cannot create temp projects dir\n");
        return 2;
    }

    const QString projectRoot = QDir(projectsDir.path()).filePath("MySong");
    const QString bundleRoot = QDir(projectRoot).filePath("Song.logicx");
    const QString projectDataPath = QDir(bundleRoot).filePath("Alternatives/000/ProjectData");
    const QString plistPath = QDir(bundleRoot).filePath("Metadata.plist");
    const QString audioPath = QDir(bundleRoot).filePath("Media/kick.wav");

    bool created = writeFile(projectDataPath, "projectdata v1");
    created = writeFile(plistPath, "plist v1") && created;
    created = writeFile(audioPath, "RIFFfakeaudio") && created;
    check(created, "setup: fake .logicx bundle created");

    // ---- Discovery + baseline seeding ----
    QmlBackend backend;
    backend.setLogLevel(QStringLiteral("debug")); // expose suppression events in activity
    backend.loadProjectsFromFolder(projectsDir.path());

    const bool scanReady = waitFor([&]() {
        return backend.hasDiscoveredProjects() && !backend.property("isScanningProjects").toBool();
    }, 20000);
    if (!scanReady) {
        std::printf("FAIL  project scan never completed\n");
        ++g_failed;
        return 1;
    }

    const QVariantList projects = backend.projects();
    check(projects.size() == 1, "discovery: exactly one project found (bundle dir, not its contents)");

    backend.confirmProjectsFolder(projectsDir.path(), QStringLiteral("Files"));
    check(waitFor([&]() {
        return !backend.property("isScanningProjects").toBool();
    }, 20000), "layout: Files scan completed");
    check(!backend.hasDiscoveredProjects(), "layout: bundle hidden in Files mode");

    backend.confirmProjectsFolder(projectsDir.path(), QStringLiteral("Bundles"));
    const bool bundlesRescanReady = waitFor([&]() {
        return backend.hasDiscoveredProjects() && !backend.property("isScanningProjects").toBool();
    }, 20000);
    check(bundlesRescanReady, "layout: Bundles rescan completed with projects");
    check(backend.projects().size() == 1, "layout: bundle project restored after rescan");

    backend.confirmProjectsFolder(projectsDir.path(), QStringLiteral("Files"));
    backend.confirmProjectsFolder(projectsDir.path(), QStringLiteral("Bundles"));
    const bool rapidRescanReady = waitFor([&]() {
        return backend.hasDiscoveredProjects() && !backend.property("isScanningProjects").toBool();
    }, 20000);
    check(rapidRescanReady, "layout: rapid switch back to Bundles still discovers projects");

    const QString fileUrl = QUrl::fromLocalFile(projectsDir.path()).toString();
    backend.confirmProjectsFolder(fileUrl, QStringLiteral("Files"));
    backend.confirmProjectsFolder(fileUrl, QStringLiteral("Bundles"));
    const bool urlRescanReady = waitFor([&]() {
        return backend.hasDiscoveredProjects() && !backend.property("isScanningProjects").toBool();
    }, 20000);
    check(urlRescanReady, "layout: file URL rescan still discovers projects");

    backend.reselectProjectsFolderLayout(QStringLiteral("Files"));
    backend.reselectProjectsFolderLayout(QStringLiteral("Bundles"));
    const bool reselectReady = waitFor([&]() {
        return backend.hasDiscoveredProjects() && !backend.property("isScanningProjects").toBool();
    }, 20000);
    check(reselectReady, "layout: reselectProjectsFolderLayout restores bundle projects");

    const bool monitoringReady = waitFor([&]() {
        const QVariantList versions = backend.getProjectVersions(0);
        return versions.size() >= 1;
    }, 20000);
    if (!monitoringReady) {
        std::printf("FAIL  monitoring baseline never seeded\n");
        ++g_failed;
        return 1;
    }

    QVariantList versions = backend.getProjectVersions(0);
    check(versions.size() == 1, "baseline: one grouped version seeded");
    {
        const QVariantMap v1 = versionById(versions, QStringLiteral("1"));
        const QVariantList files = v1.value(QStringLiteral("files")).toList();
        check(files.size() == 2, "baseline: v1 groups both bundle files (ProjectData + plist)");
    }
    check(!stagingContainsSuffix(projectRoot, QStringLiteral(".wav")),
          "baseline: no audio staged");

    // ---- Event-triggered rediscovery of a new project ----
    const QString projectRoot2 = QDir(projectsDir.path()).filePath("OtherSong");
    const QString bundleRoot2 = QDir(projectRoot2).filePath("Another.logicx");
    const QString projectDataPath2 = QDir(bundleRoot2).filePath("Alternatives/000/ProjectData");
    const QString plistPath2 = QDir(bundleRoot2).filePath("Metadata.plist");
    check(writeFile(projectDataPath2, "other projectdata v1")
              && writeFile(plistPath2, "other plist v1"),
          "rediscovery: second fake .logicx bundle created");

    const bool secondProjectAppeared = waitFor([&]() {
        return backend.projects().size() == 2;
    }, 15000);
    check(secondProjectAppeared, "rediscovery: watcher detected new project folder");

    // ---- Simulated saves through the real watcher ----
    // 7 saves -> 8 versions total; versions 1..3 must lose their staged
    // copies (5 newest kept per file).
    for (int save = 2; save <= 8; ++save) {
        const QByteArray payload = "projectdata v" + QByteArray::number(save);
        const QByteArray plistPayload = "plist v" + QByteArray::number(save);
        if (!writeFile(projectDataPath, payload) || !writeFile(plistPath, plistPayload)) {
            std::printf("FATAL cannot rewrite bundle files (save %d)\n", save);
            return 2;
        }

        const bool appeared = waitFor([&]() {
            versions = backend.getProjectVersions(0);
            return !versionById(versions, QString::number(save)).isEmpty();
        }, 20000);

        if (!appeared) {
            std::printf("FAIL  watcher: version %d never appeared\n", save);
            ++g_failed;
            return 1;
        }
    }
    check(versions.size() == 8, "watcher: 8 grouped versions after 7 saves");

    {
        const QVariantMap v8 = versionById(versions, QStringLiteral("8"));
        const QVariantList files = v8.value(QStringLiteral("files")).toList();
        check(files.size() == 2, "grouping: save touching 2 bundle files yields one 2-file version");
        check(v8.value(QStringLiteral("isCurrent")).toBool(), "current: newest version marked current");
    }

    // ---- Compaction ----
    check(countStagedFilesNamed(projectRoot, QStringLiteral("ProjectData"))
              == BackupTemplates::kUncompressedRecentVersions,
          "compaction: exactly 5 staged ProjectData copies remain");
    check(countStagedFilesNamed(projectRoot, QStringLiteral("Metadata.plist"))
              == BackupTemplates::kUncompressedRecentVersions,
          "compaction: exactly 5 staged plist copies remain");

    {
        // v1's staged copies must be gone from disk (only objects remain).
        const QVariantMap v1 = versionById(versions, QStringLiteral("1"));
        bool anyStagedLeft = false;
        const QVariantList files = v1.value(QStringLiteral("files")).toList();
        for (const QVariant& fileValue : files) {
            const QString staged = fileValue.toMap().value(QStringLiteral("stagedPath")).toString();
            const QString absoluteStaged = QFileInfo(staged).isAbsolute()
                ? staged
                : QDir(projectRoot).filePath(staged);
            anyStagedLeft = anyStagedLeft || QFileInfo::exists(absoluteStaged);
        }
        check(!files.isEmpty() && !anyStagedLeft, "compaction: v1 staged copies deleted");
    }

    backend.setSnapshotRetention(2);
    check(countStagedFilesNamed(projectRoot, QStringLiteral("ProjectData")) == 2,
          "retention change: existing ProjectData copies compact immediately");
    check(countStagedFilesNamed(projectRoot, QStringLiteral("Metadata.plist")) == 2,
          "retention change: existing plist copies compact immediately");

    // ---- Restore of a compacted version (object-store path) ----
    backend.setSelectedProjectIndex(0);
    const bool restored = backend.restoreVersionById(QStringLiteral("1"));
    check(restored, "restore: compacted v1 restore reported success");
    check(readFile(projectDataPath) == QByteArray("projectdata v1"),
          "restore: ProjectData content back to v1");
    check(readFile(plistPath) == QByteArray("plist v1"),
          "restore: plist content back to v1");
    check(readFile(audioPath) == QByteArray("RIFFfakeaudio"),
          "restore: audio file untouched by restore");

    versions = backend.getProjectVersions(0);
    check(versionById(versions, QStringLiteral("1")).value(QStringLiteral("isCurrent")).toBool(),
          "restore: v1 now marked current");

    // ---- Branching after restore ----
    // The restore rewrote both bundle files; the watcher will report those
    // changes and the snapshot service suppresses them (self-triggered).
    // Wait until both suppressions have happened, otherwise the branch save
    // below would coalesce with the restore change and be swallowed.
    const auto suppressedCount = [&backend]() {
        int count = 0;
        const QStringList activity = backend.activity();
        for (const QString& line : activity) {
            if (line.contains(QStringLiteral("Suppressed self-triggered update"))) {
                ++count;
            }
        }
        return count;
    };
    const bool suppressionConsumed = waitFor([&]() { return suppressedCount() >= 2; }, 20000);
    check(suppressionConsumed, "restore: watcher events for restored files were suppressed");

    if (!writeFile(projectDataPath, "projectdata branch")
        || !writeFile(plistPath, "plist branch")) {
        std::printf("FATAL cannot rewrite bundle files (branch save)\n");
        return 2;
    }

    const bool branched = waitFor([&]() {
        versions = backend.getProjectVersions(0);
        return !versionById(versions, QStringLiteral("1.1")).isEmpty();
    }, 20000);
    check(branched, "branch: save after restore creates version 1.1");
    if (branched) {
        const QVariantMap v11 = versionById(versions, QStringLiteral("1.1"));
        check(v11.value(QStringLiteral("parent")).toString() == QStringLiteral("1"),
              "branch: 1.1 has parent 1");
        check(v11.value(QStringLiteral("files")).toList().size() == 2,
              "branch: 1.1 groups both bundle files");
    }

    std::printf("\n%d passed, %d failed\n", g_passed, g_failed);
    return g_failed == 0 ? 0 : 1;
}
