#include "FileWatcherFactory.h"

#include <QDateTime>
#include <QDir>
#include <QFileInfo>
#include <QHash>
#include <QMetaObject>
#include <QThread>
#include <QTimer>

#include <functional>

#include "IFileWatcher.h"
#include "ProjectConfig.h"

struct FileState {
    qint64 size {0};
    qint64 msecsSinceEpoch {0};

    bool operator==(const FileState& other) const {
        return size == other.size && msecsSinceEpoch == other.msecsSinceEpoch;
    }
};

void scanTreeState(const QDir& rootDir, const QString& currentPath, QHash<QString, FileState>& out) {
    const QDir dir(currentPath);
    const QFileInfoList entries = dir.entryInfoList(
        QDir::NoDotAndDotDot | QDir::AllEntries | QDir::Hidden | QDir::NoSymLinks,
        QDir::NoSort);

    for (const QFileInfo& info : entries) {
        if (info.isDir()) {
            // Skip sample libraries, renders, backups, .musit, ... here
            // already; scanning them every tick is wasted disk churn.
            if (ProjectConfig::isIgnoredDirectoryName(info.fileName())) {
                continue;
            }
            scanTreeState(rootDir, info.absoluteFilePath(), out);
            continue;
        }

        const QString relativePath = rootDir.relativeFilePath(info.absoluteFilePath());
        out.insert(relativePath, FileState {info.size(), info.lastModified().toMSecsSinceEpoch()});
    }
}

void snapshotStateForRoot(const QString& rootPath, QHash<QString, FileState>& out) {
    scanTreeState(QDir(rootPath), rootPath, out);
}

class PollingScanWorker final : public QObject {
public:
    explicit PollingScanWorker(QObject* parent = nullptr)
        : QObject(parent) {}

    void setEventHandler(std::function<void(const FileEvent&)> onEvent) {
        m_onEvent = std::move(onEvent);
    }

    void start(const QString& rootPath) {
        if (rootPath.isEmpty()) {
            return;
        }

        if (!m_timer) {
            m_timer = new QTimer(this);
            m_timer->setInterval(400);
            connect(m_timer, &QTimer::timeout, this, [this]() {
                scan();
            });
        }

        m_rootPath = rootPath;
        m_previous.clear();
        m_pending.clear();
        snapshotStateForRoot(m_rootPath, m_previous);
        m_timer->start();
    }

    void stop() {
        if (m_timer) {
            m_timer->stop();
        }
        m_rootPath.clear();
        m_previous.clear();
        m_pending.clear();
    }

private:
    void scan() {
        if (m_rootPath.isEmpty()) {
            return;
        }

        // Files that stabilize in the same scan carry the same sequence, so
        // multi-file saves (e.g. inside .logicx bundles) can be grouped into
        // one version downstream.
        ++m_scanSequence;

        QHash<QString, FileState> current;
        snapshotStateForRoot(m_rootPath, current);

        for (auto it = current.constBegin(); it != current.constEnd(); ++it) {
            const QString& path = it.key();
            const FileState& state = it.value();

            const auto knownIt = m_previous.constFind(path);
            if (knownIt != m_previous.constEnd() && knownIt.value() == state) {
                m_pending.remove(path);
                continue;
            }

            // Only report a change once the file has been stable for a full
            // scan interval. DAWs can take seconds to write large project
            // files; snapshotting mid-write would capture a torn file.
            const auto pendingIt = m_pending.constFind(path);
            if (pendingIt != m_pending.constEnd() && pendingIt.value() == state) {
                const bool isNew = knownIt == m_previous.constEnd();
                m_previous.insert(path, state);
                m_pending.remove(path);
                emitChange(isNew ? FileEvent::Type::Created : FileEvent::Type::Modified, path);
            } else {
                m_pending.insert(path, state);
            }
        }

        for (auto it = m_previous.begin(); it != m_previous.end();) {
            if (!current.contains(it.key())) {
                const QString path = it.key();
                it = m_previous.erase(it);
                m_pending.remove(path);
                emitChange(FileEvent::Type::Deleted, path);
            } else {
                ++it;
            }
        }
    }

    void emitChange(FileEvent::Type type, const QString& relativePath) {
        FileEvent event;
        event.type = type;
        event.relativePath = relativePath;
        event.absolutePath = QDir(m_rootPath).filePath(relativePath);
        event.scanSequence = m_scanSequence;
        if (m_onEvent) {
            m_onEvent(event);
        }
    }

    QString m_rootPath;
    qint64 m_scanSequence {0};
    QTimer* m_timer {nullptr};
    QHash<QString, FileState> m_previous;
    QHash<QString, FileState> m_pending;
    std::function<void(const FileEvent&)> m_onEvent;
};

class PollingFileWatcher final : public IFileWatcher {
public:
    PollingFileWatcher() {
        qRegisterMetaType<FileEvent>("FileEvent");

        m_worker = new PollingScanWorker();
        m_worker->moveToThread(&m_workerThread);
        connect(&m_workerThread, &QThread::finished, m_worker, &QObject::deleteLater);

        m_worker->setEventHandler([this](const FileEvent& event) {
            QMetaObject::invokeMethod(this,
                                      [this, event]() {
                                          emit fileEvent(event);
                                      },
                                      Qt::QueuedConnection);
        });

        m_workerThread.start();
    }

    ~PollingFileWatcher() override {
        if (m_worker) {
            QMetaObject::invokeMethod(m_worker,
                                      [worker = m_worker]() {
                                          worker->stop();
                                      },
                                      Qt::BlockingQueuedConnection);
        }
        m_workerThread.quit();
        m_workerThread.wait();
    }

    bool startWatching(const QString& rootPath) override {
        if (rootPath.isEmpty()) {
            return false;
        }

        if (!m_worker) {
            return false;
        }

        const QString normalizedRoot = QDir::cleanPath(rootPath);
        QMetaObject::invokeMethod(m_worker,
                                  [worker = m_worker, normalizedRoot]() {
                                      worker->start(normalizedRoot);
                                  },
                                  Qt::QueuedConnection);
        return true;
    }

    void stopWatching() override {
        if (!m_worker) {
            return;
        }

        QMetaObject::invokeMethod(m_worker,
                                  [worker = m_worker]() {
                                      worker->stop();
                                  },
                                  Qt::QueuedConnection);
    }

private:
    QThread m_workerThread;
    PollingScanWorker* m_worker {nullptr};
};

std::unique_ptr<IFileWatcher> createFileWatcher() {
    return std::make_unique<PollingFileWatcher>();
}
