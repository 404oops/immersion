#pragma once

#include <QString>

class ObjectStore {
public:
    explicit ObjectStore(QString musitRoot = {});

    void setMusitRoot(const QString& musitRoot);
    bool init();

    QString storeFile(const QString& absolutePath);
    QString stageFile(const QString& absolutePath, const QString& relativePath);

    // Decompresses the object with the given hash to destPath. Used to
    // restore versions whose uncompressed staged copy has been compacted.
    bool extractObject(const QString& objectHash, const QString& destPath) const;
    QString objectPathForHash(const QString& objectHash) const;

private:
    QString m_musitRoot;
};
