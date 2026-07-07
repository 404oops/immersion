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
#include <QVariantList>
#include <QVariantMap>

#include <cstdint>

#include <cstdio>
#include <functional>

#include "../src/core/BackupTemplate.h"
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
