#pragma once

#include <QString>

class ObjectStore {
public:
    explicit ObjectStore(QString musitRoot = {});

    void setMusitRoot(const QString& musitRoot);
    bool init();

    QString storeFile(const QString& absolutePath);
    QString stageFile(const QString& absolutePath, const QString& relativePath);

private:
    QString m_musitRoot;
};
