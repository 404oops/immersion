#pragma once

#include <QObject>
#include <QString>

#include "FileEvent.h"

class IFileWatcher : public QObject {
    Q_OBJECT
public:
    using QObject::QObject;
    ~IFileWatcher() override = default;

    virtual bool startWatching(const QString& rootPath) = 0;
    virtual void stopWatching() = 0;

signals:
    void fileEvent(const FileEvent& event);
};
