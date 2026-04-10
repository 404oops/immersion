#include "FileWatcherFactory.h"

#include <QDateTime>
#include <QDir>
#include <QDirIterator>
#include <QFileInfo>
#include <QHash>
#include <QMetaObject>
#include <QSet>
#include <QThread>
#include <QTimer>

#include <functional>

#include "IFileWatcher.h"

struct FileState {
    qint64 size {0};
    qint64 msecsSinceEpoch {0};
};

void snapshotStateForRoot(const QString& rootPath, QHash<QString, FileState>& out) {
    QDirIterator it(rootPath, QDir::Files, QDirIterator::Subdirectories);
    while (it.hasNext()) {
        const QString absolutePath = it.next();
        const QFileInfo info(absolutePath);

        if (!info.exists() || info.isSymLink()) {
            continue;
        }

        const QString relativePath = QDir(rootPath).relativeFilePath(absolutePath);
        out.insert(relativePath, FileState {info.size(), info.lastModified().toMSecsSinceEpoch()});
    }
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
        snapshotStateForRoot(m_rootPath, m_previous);
        m_timer->start();
    }

    void stop() {
        if (m_timer) {
            m_timer->stop();
        }
        m_rootPath.clear();
        m_previous.clear();
    }

private:
    void scan() {
        if (m_rootPath.isEmpty()) {
            return;
        }

        QHash<QString, FileState> current;
        snapshotStateForRoot(m_rootPath, current);

        for (auto it = current.constBegin(); it != current.constEnd(); ++it) {
            if (!m_previous.contains(it.key())) {
                emitChange(FileEvent::Type::Created, it.key());
                continue;
            }

            const FileState oldState = m_previous.value(it.key());
            if (oldState.size != it->size || oldState.msecsSinceEpoch != it->msecsSinceEpoch) {
                emitChange(FileEvent::Type::Modified, it.key());
            }
        }

        for (auto it = m_previous.constBegin(); it != m_previous.constEnd(); ++it) {
            if (!current.contains(it.key())) {
                emitChange(FileEvent::Type::Deleted, it.key());
            }
        }

        m_previous = std::move(current);
    }

    void emitChange(FileEvent::Type type, const QString& relativePath) {
        FileEvent event;
        event.type = type;
        event.relativePath = relativePath;
        event.absolutePath = QDir(m_rootPath).filePath(relativePath);
        if (m_onEvent) {
            m_onEvent(event);
        }
    }

    QString m_rootPath;
    QTimer* m_timer {nullptr};
    QHash<QString, FileState> m_previous;
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
