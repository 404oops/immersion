#include "ProjectRegistry.h"

#include <QDateTime>
#include <QDir>
#include <QFile>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QFileInfo>
#include <QStandardPaths>

namespace {

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
        if (obj.value("root_path").toString() == project.rootPath) {
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

    if (!file.open(QIODevice::WriteOnly | QIODevice::Truncate)) {
        return false;
    }

    const QByteArray encoded = QJsonDocument(root).toJson(QJsonDocument::Indented);
    const qint64 written = file.write(encoded);
    file.close();

    return written == encoded.size();
}

QString ProjectRegistry::loadProjectNote(const QString& rootPath) const {
    QFile file(dataFilePath());
    const QJsonArray projects = readProjectsArray(file);
    for (const QJsonValue& value : projects) {
        const QJsonObject obj = value.toObject();
        if (obj.value("root_path").toString() == rootPath) {
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
        if (obj.value("root_path").toString() == rootPath) {
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

    if (!file.open(QIODevice::WriteOnly | QIODevice::Truncate)) {
        return false;
    }

    const QByteArray encoded = QJsonDocument(root).toJson(QJsonDocument::Indented);
    const qint64 written = file.write(encoded);
    file.close();

    return written == encoded.size();
}

bool ProjectRegistry::saveProjectsFolder(const QString& folderPath) {
    const QString dirPath = appConfigDirectory();
    if (dirPath.isEmpty() || !QDir().mkpath(dirPath)) {
        return false;
    }

    const QString configFilePath = QDir(dirPath).filePath("config.json");
    QFile configFile(configFilePath);

    QJsonObject root;
    if (configFile.exists()) {
        if (!configFile.open(QIODevice::ReadOnly)) {
            return false;
        }

        const QJsonDocument existingDoc = QJsonDocument::fromJson(configFile.readAll());
        configFile.close();
        if (existingDoc.isObject()) {
            root = existingDoc.object();
        }
    }

    root.insert("projects_folder", QDir::cleanPath(folderPath));
    root.insert("updated_utc", QDateTime::currentDateTimeUtc().toString(Qt::ISODateWithMs));

    if (!configFile.open(QIODevice::WriteOnly | QIODevice::Truncate)) {
        return false;
    }

    const QByteArray encoded = QJsonDocument(root).toJson(QJsonDocument::Indented);
    const qint64 written = configFile.write(encoded);
    configFile.close();

    return written == encoded.size();
}

QString ProjectRegistry::loadProjectsFolder() const {
    const QString configFilePath = QDir(appConfigDirectory()).filePath("config.json");
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

QString ProjectRegistry::appConfigDirectory() const {
#if defined(Q_OS_MAC)
    return QDir::home().filePath("Library/Application Support/musit");
#elif defined(Q_OS_WIN)
    const QString appData = QStandardPaths::writableLocation(QStandardPaths::AppDataLocation);
    if (appData.isEmpty()) {
        return {};
    }
    return QDir(appData).filePath("musit");
#else
    return QDir::home().filePath(".config/musit");
#endif
}
