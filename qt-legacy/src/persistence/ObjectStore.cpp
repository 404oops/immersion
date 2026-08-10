#include "ObjectStore.h"

#include <QCryptographicHash>
#include <QDateTime>
#include <QDir>
#include <QFile>
#include <QSaveFile>

namespace {

bool compressedObjectMatchesHash(const QByteArray& compressed, const QString& hashHex) {
    if (compressed.size() < 4 || hashHex.size() != 64) {
        return false;
    }

    const QByteArray decompressed = qUncompress(compressed);
    const QByteArray actualHash =
        QCryptographicHash::hash(decompressed, QCryptographicHash::Sha256).toHex();
    return actualHash == hashHex.toLatin1();
}

} // namespace

ObjectStore::ObjectStore(QString musitRoot)
    : m_musitRoot(std::move(musitRoot)) {}

void ObjectStore::setMusitRoot(const QString& musitRoot) {
    m_musitRoot = musitRoot;
}

bool ObjectStore::init() {
    if (m_musitRoot.isEmpty()) {
        return false;
    }

    QDir root;
    const bool ok = root.mkpath(QDir(m_musitRoot).filePath("objects"))
        && root.mkpath(QDir(m_musitRoot).filePath("staging"));

    return ok;
}

QString ObjectStore::stageFile(const QString& absolutePath, const QString& relativePath) {
    // The relative path is used to build a path inside the staging area;
    // refuse anything that could escape it.
    const QString normalizedRelative = QDir::cleanPath(QDir::fromNativeSeparators(relativePath));
    if (normalizedRelative.isEmpty()
        || normalizedRelative.startsWith("..")
        || normalizedRelative.contains("/../")
        || QDir::isAbsolutePath(normalizedRelative)) {
        return {};
    }

    QFile source(absolutePath);
    if (!source.open(QIODevice::ReadOnly)) {
        return {};
    }

    const QByteArray bytes = source.readAll();
    source.close();

    const QString stamp = QString::number(QDateTime::currentMSecsSinceEpoch());

    const QString targetDirPath = QDir(m_musitRoot).filePath(QDir("staging").filePath(stamp));
    if (!QDir().mkpath(targetDirPath)) {
        return {};
    }

    const QString targetFilePath = QDir(targetDirPath).filePath(normalizedRelative);
    const QFileInfo outInfo(targetFilePath);
    if (!QDir().mkpath(outInfo.dir().absolutePath())) {
        return {};
    }

    QFile target(targetFilePath);
    if (!target.open(QIODevice::WriteOnly)) {
        return {};
    }

    if (target.write(bytes) != bytes.size()) {
        target.close();
        QFile::remove(targetFilePath);
        return {};
    }

    target.close();
    return targetFilePath;
}

QString ObjectStore::objectPathForHash(const QString& objectHash) const {
    if (m_musitRoot.isEmpty() || objectHash.size() <= 2) {
        return {};
    }

    return QDir(m_musitRoot).filePath(
        QStringLiteral("objects/%1/%2.z").arg(objectHash.left(2), objectHash.mid(2)));
}

bool ObjectStore::extractObject(const QString& objectHash, const QString& destPath) const {
    const QString objectPath = objectPathForHash(objectHash);
    if (objectPath.isEmpty()) {
        return false;
    }

    QFile object(objectPath);
    if (!object.open(QIODevice::ReadOnly)) {
        return false;
    }

    const QByteArray compressed = object.readAll();
    object.close();
    if (!compressedObjectMatchesHash(compressed, objectHash)) {
        return false;
    }
    const QByteArray decompressed = qUncompress(compressed);

    QFile dest(destPath);
    if (!dest.open(QIODevice::WriteOnly | QIODevice::Truncate)) {
        return false;
    }

    if (dest.write(decompressed) != decompressed.size()) {
        dest.close();
        QFile::remove(destPath);
        return false;
    }

    dest.close();
    return true;
}

QString ObjectStore::storeFile(const QString& absolutePath) {
    QFile source(absolutePath);
    if (!source.open(QIODevice::ReadOnly)) {
        return {};
    }

    const QByteArray bytes = source.readAll();
    source.close();

    const QByteArray hashBytes = QCryptographicHash::hash(bytes, QCryptographicHash::Sha256);
    const QString hashHex = QString::fromLatin1(hashBytes.toHex());

    const QString shard = hashHex.left(2);
    const QString filePart = hashHex.mid(2);

    const QString shardDir = QDir(m_musitRoot).filePath(QDir("objects").filePath(shard));
    if (!QDir().mkpath(shardDir)) {
        return {};
    }

    const QString objectPath = QDir(shardDir).filePath(filePart + ".z");
    if (QFileInfo::exists(objectPath)) {
        QFile existing(objectPath);
        if (existing.open(QIODevice::ReadOnly)
            && compressedObjectMatchesHash(existing.readAll(), hashHex)) {
            return hashHex;
        }
    }

    const QByteArray compressed = qCompress(bytes, 6);
    QSaveFile object(objectPath);
    if (!object.open(QIODevice::WriteOnly | QIODevice::Truncate)) {
        return {};
    }

    if (object.write(compressed) != compressed.size() || !object.commit()) {
        return {};
    }

    return hashHex;
}
