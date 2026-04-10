#pragma once

#include <QMetaType>
#include <QString>

struct FileEvent {
    enum class Type {
        Created,
        Modified,
        Deleted,
        Renamed
    };

    Type type {Type::Modified};
    QString absolutePath;
    QString relativePath;
};

Q_DECLARE_METATYPE(FileEvent)
