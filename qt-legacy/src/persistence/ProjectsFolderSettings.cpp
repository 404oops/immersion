#include "ProjectsFolderSettings.h"

#include <QDateTime>
#include <QDir>
#include <QFile>
#include <QJsonDocument>
#include <QJsonObject>
#include <QSaveFile>

namespace {

constexpr QLatin1StringView kLayoutKey("layout");
constexpr QLatin1StringView kUpdatedUtcKey("updated_utc");
constexpr QLatin1StringView kBundlesValue("bundles");
constexpr QLatin1StringView kFilesValue("files");

} // namespace

QString ProjectsFolderSettings::settingsDirectoryName() {
    return QStringLiteral(".immersion");
}

QString ProjectsFolderSettings::settingsFilePath(const QString& projectsFolderPath) {
    if (projectsFolderPath.isEmpty()) {
        return {};
    }

    return QDir(projectsFolderPath).filePath(
        QStringLiteral("%1/settings.json").arg(settingsDirectoryName()));
}

ProjectsFolderLayout ProjectsFolderSettings::layoutFromString(const QString& value) {
    if (value.compare(kFilesValue, Qt::CaseInsensitive) == 0) {
        return ProjectsFolderLayout::Files;
    }

    return ProjectsFolderLayout::Bundles;
}

QString ProjectsFolderSettings::layoutToString(const ProjectsFolderLayout layout) {
    switch (layout) {
    case ProjectsFolderLayout::Files:
        return QString(kFilesValue);
    case ProjectsFolderLayout::Bundles:
    default:
        return QString(kBundlesValue);
    }
}

bool ProjectsFolderSettings::hasLayoutSetting(const QString& projectsFolderPath) const {
    const QString filePath = settingsFilePath(projectsFolderPath);
    return !filePath.isEmpty() && QFile::exists(filePath);
}

ProjectsFolderLayout ProjectsFolderSettings::loadLayout(const QString& projectsFolderPath) const {
    const QString filePath = settingsFilePath(projectsFolderPath);
    QFile file(filePath);
    if (!file.exists() || !file.open(QIODevice::ReadOnly)) {
        return ProjectsFolderLayout::Bundles;
    }

    const QJsonDocument doc = QJsonDocument::fromJson(file.readAll());
    file.close();
    if (!doc.isObject()) {
        return ProjectsFolderLayout::Bundles;
    }

    return layoutFromString(doc.object().value(QString(kLayoutKey)).toString());
}

bool ProjectsFolderSettings::saveLayout(const QString& projectsFolderPath,
                                      const ProjectsFolderLayout layout) const {
    if (projectsFolderPath.isEmpty()) {
        return false;
    }

    const QString settingsDir = QDir(projectsFolderPath).filePath(settingsDirectoryName());
    if (!QDir().mkpath(settingsDir)) {
        return false;
    }

    QJsonObject root;
    root.insert(QString(kLayoutKey), layoutToString(layout));
    root.insert(QString(kUpdatedUtcKey),
                QDateTime::currentDateTimeUtc().toString(Qt::ISODateWithMs));

    QSaveFile file(settingsFilePath(projectsFolderPath));
    if (!file.open(QIODevice::WriteOnly | QIODevice::Truncate)) {
        return false;
    }

    const QByteArray encoded = QJsonDocument(root).toJson(QJsonDocument::Indented);
    if (file.write(encoded) != encoded.size()) {
        return false;
    }

    return file.commit();
}
