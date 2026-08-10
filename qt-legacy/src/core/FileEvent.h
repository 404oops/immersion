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
    // Watcher scan tick that produced this event. Files that stabilize in
    // the same tick (e.g. several files inside a .logicx bundle written by
    // one save) are grouped into a single version. 0 = no grouping info.
    qint64 scanSequence {0};
};

Q_DECLARE_METATYPE(FileEvent)
