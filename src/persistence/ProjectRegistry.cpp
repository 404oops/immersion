#include "ProjectRegistry.h"

#include "../core/PathCleanup.h"

#include <QDateTime>
#include <QDir>
#include <QFile>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QFileInfo>
#include <QSaveFile>
#include <QStandardPaths>

namespace {

bool writeJsonAtomically(const QString& filePath, const QJsonObject& root) {
    QSaveFile file(filePath);
    if (!file.open(QIODevice::WriteOnly | QIODevice::Truncate)) {
        return false;
    }

    const QByteArray encoded = QJsonDocument(root).toJson(QJsonDocument::Indented);
    if (file.write(encoded) != encoded.size()) {
        return false;
    }

    return file.commit();
}

QJsonObject toJson(const DiscoveredProject& project, const QString& note = {}) {
    QJsonObject counts;
    for (const ProjectKind kind : ProjectDiscovery::knownKinds()) {
        counts.insert(ProjectDiscovery::kindToString(kind), project.typeCounts.value(kind));
    }

    QJsonObject item;
    QJsonArray projectFiles;
    for (const QString& fileName : project.projectFiles) {
        projectFiles.append(fileName);
    }

    item.insert("name", project.name);
    item.insert("root_path", project.rootPath);
    item.insert("kind", ProjectDiscovery::kindToString(project.kind));
    item.insert("primary_project_file", project.primaryProjectFile);
    item.insert("project_files", projectFiles);
    item.insert("project_file_count", project.totalProjectFiles);
    item.insert("type_counts", counts);
    item.insert("last_seen_utc", QDateTime::currentDateTimeUtc().toString(Qt::ISODateWithMs));
    if (!note.trimmed().isEmpty()) {
        item.insert("note", note.trimmed());
    }
    return item;
}

QJsonArray readProjectsArray(QFile& file) {
    QJsonArray projects;
    if (!file.exists() || !file.open(QIODevice::ReadOnly)) {
        return projects;
    }

    const QJsonDocument existingDoc = QJsonDocument::fromJson(file.readAll());
    file.close();
    if (existingDoc.isObject()) {
        projects = existingDoc.object().value("projects").toArray();
    }
    return projects;
}

// Older builds used hand-rolled per-OS paths (and Windows accidentally nested
// an extra "musit" segment). Copy config forward once so upgrades keep their
// saved folder and project notes.
void migrateLegacyConfigIfNeeded(const QString& canonicalDir) {
    if (QFileInfo::exists(QDir(canonicalDir).filePath(QStringLiteral("config.json")))
        || QFileInfo::exists(QDir(canonicalDir).filePath(QStringLiteral("projects.json")))) {
        return;
    }

    QStringList legacyDirs;
#if defined(Q_OS_MAC)
    legacyDirs << QDir::home().filePath(QStringLiteral("Library/Application Support/musit"));
#elif defined(Q_OS_WIN)
    const QString roaming = QStandardPaths::writableLocation(QStandardPaths::GenericConfigLocation);
    if (!roaming.isEmpty()) {
        legacyDirs << QDir(roaming).filePath(QStringLiteral("musit"));
    }
    // Buggy path from appending "musit" to AppDataLocation twice.
    const QString appData = QStandardPaths::writableLocation(QStandardPaths::AppDataLocation);
    if (!appData.isEmpty()) {
        legacyDirs << QDir(appData).filePath(QStringLiteral("musit"));
    }
#else
    legacyDirs << QDir::home().filePath(QStringLiteral(".config/musit"));
    legacyDirs << QDir::home().filePath(QStringLiteral(".local/share/musit/musit"));
#endif

    for (const QString& legacyDir : legacyDirs) {
        if (pathEquals(legacyDir, canonicalDir) || !QDir(legacyDir).exists()) {
            continue;
        }

        QDir().mkpath(canonicalDir);
        for (const QString& fileName : {QStringLiteral("config.json"), QStringLiteral("projects.json")}) {
            const QString sourcePath = QDir(legacyDir).filePath(fileName);
            const QString destPath = QDir(canonicalDir).filePath(fileName);
            if (QFileInfo::exists(sourcePath) && !QFileInfo::exists(destPath)) {
                QFile::copy(sourcePath, destPath);
            }
        }
        return;
    }
}

} // namespace

bool ProjectRegistry::saveProject(const DiscoveredProject& project) {
    const QString dirPath = appConfigDirectory();
    if (dirPath.isEmpty() || !QDir().mkpath(dirPath)) {
        return false;
    }

    const QString filePath = dataFilePath();
    QFile file(filePath);

    QJsonArray projects = readProjectsArray(file);

    QJsonArray updated;
    bool replaced = false;
    for (const QJsonValue& value : projects) {
        const QJsonObject obj = value.toObject();
        if (pathEquals(obj.value("root_path").toString(), project.rootPath)) {
            updated.append(toJson(project, obj.value("note").toString()));
            replaced = true;
        } else {
            updated.append(obj);
        }
    }

    if (!replaced) {
        updated.append(toJson(project));
    }

    QJsonObject root;
    root.insert("projects", updated);

    return writeJsonAtomically(filePath, root);
}

QString ProjectRegistry::loadProjectNote(const QString& rootPath) const {
    QFile file(dataFilePath());
    const QJsonArray projects = readProjectsArray(file);
    for (const QJsonValue& value : projects) {
        const QJsonObject obj = value.toObject();
        if (pathEquals(obj.value("root_path").toString(), rootPath)) {
            return obj.value("note").toString();
        }
    }

    return {};
}

bool ProjectRegistry::saveProjectNote(const QString& rootPath, const QString& note) {
    const QString dirPath = appConfigDirectory();
    if (dirPath.isEmpty() || !QDir().mkpath(dirPath)) {
        return false;
    }

    QFile file(dataFilePath());
    QJsonArray projects = readProjectsArray(file);

    bool updated = false;
    QJsonArray rewritten;
    for (const QJsonValue& value : projects) {
        QJsonObject obj = value.toObject();
        if (pathEquals(obj.value("root_path").toString(), rootPath)) {
            if (note.trimmed().isEmpty()) {
                obj.remove("note");
            } else {
                obj.insert("note", note.trimmed());
            }
            updated = true;
        }
        rewritten.append(obj);
    }

    if (!updated) {
        QJsonObject placeholder;
        placeholder.insert("root_path", rootPath);
        if (!note.trimmed().isEmpty()) {
            placeholder.insert("note", note.trimmed());
        }
        rewritten.append(placeholder);
    }

    QJsonObject root;
    root.insert("projects", rewritten);

    return writeJsonAtomically(dataFilePath(), root);
}

namespace {

QString configFilePathForApp() {
    return QDir(ProjectRegistry::appConfigDirectory()).filePath(QStringLiteral("config.json"));
}

QJsonObject readConfigRoot() {
    QJsonObject root;
    const QString configFilePath = configFilePathForApp();
    QFile configFile(configFilePath);
    if (!configFile.exists() || !configFile.open(QIODevice::ReadOnly)) {
        return root;
    }

    const QJsonDocument doc = QJsonDocument::fromJson(configFile.readAll());
    configFile.close();
    if (doc.isObject()) {
        root = doc.object();
    }
    return root;
}

} // namespace

bool ProjectRegistry::saveProjectsFolder(const QString& folderPath) {
    const QString dirPath = appConfigDirectory();
    if (dirPath.isEmpty() || !QDir().mkpath(dirPath)) {
        return false;
    }

    QJsonObject root = readConfigRoot();
    root.insert("projects_folder", QDir::cleanPath(folderPath));
    root.insert("updated_utc", QDateTime::currentDateTimeUtc().toString(Qt::ISODateWithMs));

    return writeJsonAtomically(configFilePathForApp(), root);
}

AppSettings ProjectRegistry::loadAppSettings() const {
    AppSettings settings;
    const QJsonObject root = readConfigRoot();
    if (root.contains(QStringLiteral("theme_hue"))) {
        settings.themeHue = root.value(QStringLiteral("theme_hue")).toDouble(settings.themeHue);
    }
    if (root.contains(QStringLiteral("launch_at_startup"))) {
        settings.launchAtStartup = root.value(QStringLiteral("launch_at_startup")).toBool(false);
    }
    if (root.contains(QStringLiteral("sort_mode"))) {
        settings.sortMode = root.value(QStringLiteral("sort_mode")).toString();
    }
    if (root.contains(QStringLiteral("log_level"))) {
        settings.logLevel = root.value(QStringLiteral("log_level")).toString();
    }
    if (root.contains(QStringLiteral("snapshot_retention"))) {
        settings.snapshotRetention = root.value(QStringLiteral("snapshot_retention")).toInt(settings.snapshotRetention);
    }
    if (root.contains(QStringLiteral("notifications_enabled"))) {
        settings.notificationsEnabled = root.value(QStringLiteral("notifications_enabled")).toBool(true);
    }
    if (root.contains(QStringLiteral("color_scheme_mode"))) {
        settings.colorSchemeMode = root.value(QStringLiteral("color_scheme_mode")).toString();
    }
    return settings;
}

bool ProjectRegistry::saveAppSettings(const AppSettings& settings) {
    const QString dirPath = appConfigDirectory();
    if (dirPath.isEmpty() || !QDir().mkpath(dirPath)) {
        return false;
    }

    QJsonObject root = readConfigRoot();
    root.insert(QStringLiteral("theme_hue"), settings.themeHue);
    root.insert(QStringLiteral("launch_at_startup"), settings.launchAtStartup);
    if (!settings.sortMode.isEmpty()) {
        root.insert(QStringLiteral("sort_mode"), settings.sortMode);
    }
    if (!settings.logLevel.isEmpty()) {
        root.insert(QStringLiteral("log_level"), settings.logLevel);
    }
    root.insert(QStringLiteral("snapshot_retention"), settings.snapshotRetention);
    root.insert(QStringLiteral("notifications_enabled"), settings.notificationsEnabled);
    if (!settings.colorSchemeMode.isEmpty()) {
        root.insert(QStringLiteral("color_scheme_mode"), settings.colorSchemeMode);
    }
    root.insert(QStringLiteral("updated_utc"), QDateTime::currentDateTimeUtc().toString(Qt::ISODateWithMs));

    return writeJsonAtomically(configFilePathForApp(), root);
}

QString ProjectRegistry::loadProjectsFolder() const {
    const QString configFilePath = configFilePathForApp();
    QFile configFile(configFilePath);
    if (!configFile.exists() || !configFile.open(QIODevice::ReadOnly)) {
        return {};
    }

    const QJsonDocument doc = QJsonDocument::fromJson(configFile.readAll());
    configFile.close();
    if (!doc.isObject()) {
        return {};
    }

    const QString savedFolder = doc.object().value("projects_folder").toString();
    if (savedFolder.isEmpty()) {
        return {};
    }

    const QString cleanFolder = QDir::cleanPath(savedFolder);
    const QFileInfo info(cleanFolder);
    if (!info.exists() || !info.isDir()) {
        return {};
    }

    return cleanFolder;
}

QString ProjectRegistry::dataFilePath() const {
    return QDir(appConfigDirectory()).filePath("projects.json");
}

QString ProjectRegistry::appConfigDirectory() {
    // Single cross-platform location derived from org/app names set in main().
    // Do not append extra path segments here — AppDataLocation already includes
    // the application name.
    const QString canonical = QStandardPaths::writableLocation(QStandardPaths::AppDataLocation);
    if (canonical.isEmpty()) {
        return {};
    }

    migrateLegacyConfigIfNeeded(canonical);
    return canonical;
}
