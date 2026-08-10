#include "FileWatcherFactory.h"

#include <QDir>
#include <QElapsedTimer>
#include <QFileInfo>
#include <QFileSystemWatcher>
#include <QHash>
#include <QMetaObject>
#include <QSet>
#include <QThread>
#include <QTimer>

#include <functional>

#include "IFileWatcher.h"
#include "ProjectConfig.h"

namespace {

constexpr int kDebounceMs = 400;
constexpr int kSafetyScanMs = 5000;

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

QString relativePathForRoot(const QString& rootPath, const QString& absolutePath) {
    return QDir(rootPath).relativeFilePath(QDir::cleanPath(absolutePath));
}

bool isUnderRoot(const QString& rootPath, const QString& absolutePath) {
    const QString relative = relativePathForRoot(rootPath, absolutePath);
    return !relative.startsWith(QStringLiteral(".."));
}

class HybridWatchWorker final : public QObject {
public:
    explicit HybridWatchWorker(QObject* parent = nullptr)
        : QObject(parent) {}

    void setScanLogHandler(FileWatcherScanLogFn logFn) {
        m_logScan = std::move(logFn);
    }

    void setEventHandler(std::function<void(const FileEvent&)> onEvent) {
        m_onEvent = std::move(onEvent);
    }

    void start(const QString& rootPath) {
        if (rootPath.isEmpty()) {
            return;
        }

        ensureTimersAndWatcher();

        m_rootPath = QDir::cleanPath(rootPath);
        m_projectConfig.setRootPath(m_rootPath);
        m_previous.clear();
        m_pending.clear();
        m_dirtyRelative.clear();
        m_scanSequence = 0;

        QElapsedTimer timer;
        timer.start();
        snapshotStateForRoot(m_rootPath, m_previous);
        logScan(QStringLiteral("baseline"), m_previous.size(), timer.elapsed());

        installWatchesRecursive(m_rootPath);
        m_safetyTimer->start();
    }

    void stop() {
        if (m_debounceTimer) {
            m_debounceTimer->stop();
        }
        if (m_safetyTimer) {
            m_safetyTimer->stop();
        }
        if (m_watcher) {
            const QStringList paths = m_watcher->directories() + m_watcher->files();
            if (!paths.isEmpty()) {
                m_watcher->removePaths(paths);
            }
        }
        m_rootPath.clear();
        m_previous.clear();
        m_pending.clear();
        m_dirtyRelative.clear();
    }

private:
    void ensureTimersAndWatcher() {
        if (m_watcher) {
            return;
        }

        m_watcher = new QFileSystemWatcher(this);
        connect(m_watcher,
                &QFileSystemWatcher::fileChanged,
                this,
                &HybridWatchWorker::onFileChanged);
        connect(m_watcher,
                &QFileSystemWatcher::directoryChanged,
                this,
                &HybridWatchWorker::onDirectoryChanged);

        m_debounceTimer = new QTimer(this);
        m_debounceTimer->setSingleShot(true);
        m_debounceTimer->setInterval(kDebounceMs);
        connect(m_debounceTimer, &QTimer::timeout, this, &HybridWatchWorker::onDebounceTimeout);

        m_safetyTimer = new QTimer(this);
        m_safetyTimer->setInterval(kSafetyScanMs);
        connect(m_safetyTimer, &QTimer::timeout, this, &HybridWatchWorker::onSafetyScan);
    }

    void installWatchesRecursive(const QString& dirPath) {
        if (m_rootPath.isEmpty() || !m_watcher) {
            return;
        }

        const QString normalizedDir = QDir::cleanPath(dirPath);
        if (!isUnderRoot(m_rootPath, normalizedDir)) {
            return;
        }

        m_watcher->addPath(normalizedDir);

        const QDir dir(normalizedDir);
        const QFileInfoList entries = dir.entryInfoList(
            QDir::NoDotAndDotDot | QDir::Dirs | QDir::Hidden | QDir::NoSymLinks,
            QDir::NoSort);
        for (const QFileInfo& info : entries) {
            if (ProjectConfig::isIgnoredDirectoryName(info.fileName())) {
                continue;
            }
            installWatchesRecursive(info.absoluteFilePath());
        }
    }

    void refreshDirectoryWatch(const QString& dirPath) {
        if (!m_watcher) {
            return;
        }

        const QString normalizedDir = QDir::cleanPath(dirPath);
        if (!isUnderRoot(m_rootPath, normalizedDir)) {
            return;
        }

        if (!m_watcher->directories().contains(normalizedDir)) {
            m_watcher->addPath(normalizedDir);
        }
    }

    void markDirtyRelative(const QString& relativePath) {
        if (relativePath.isEmpty() || relativePath.startsWith(QStringLiteral(".."))) {
            return;
        }
        if (!m_projectConfig.shouldTrack(relativePath)) {
            return;
        }

        m_dirtyRelative.insert(relativePath);
        if (m_debounceTimer) {
            m_debounceTimer->start();
        }
    }

    void onFileChanged(const QString& path) {
        if (m_rootPath.isEmpty()) {
            return;
        }

        const QFileInfo info(path);
        if (!info.exists()) {
            markDirtyRelative(relativePathForRoot(m_rootPath, path));
            return;
        }

        if (info.isDir()) {
            onDirectoryChanged(path);
            return;
        }

        markDirtyRelative(relativePathForRoot(m_rootPath, path));
    }

    void onDirectoryChanged(const QString& path) {
        if (m_rootPath.isEmpty()) {
            return;
        }

        refreshDirectoryWatch(path);
        installWatchesRecursive(path);
        queueSubtreeReconcile(path);
    }

    void queueSubtreeReconcile(const QString& dirPath) {
        const QString normalizedDir = QDir::cleanPath(dirPath);
        const QString dirPrefix = relativePathForRoot(m_rootPath, normalizedDir);
        if (dirPrefix.startsWith(QStringLiteral(".."))) {
            return;
        }

        const QString prefix = dirPrefix.isEmpty() ? QString() : dirPrefix + QLatin1Char('/');

        QHash<QString, FileState> currentSubtree;
        scanTreeState(QDir(m_rootPath), normalizedDir, currentSubtree);

        for (auto it = currentSubtree.constBegin(); it != currentSubtree.constEnd(); ++it) {
            markDirtyRelative(it.key());
        }

        for (auto it = m_previous.constBegin(); it != m_previous.constEnd(); ++it) {
            if (prefix.isEmpty() || it.key().startsWith(prefix)) {
                if (!currentSubtree.contains(it.key())) {
                    markDirtyRelative(it.key());
                }
            }
        }
    }

    void onDebounceTimeout() {
        processDirtyBatch();
    }

    void onSafetyScan() {
        QElapsedTimer timer;
        timer.start();
        const int beforeDirty = m_dirtyRelative.size();
        queueFullTreeReconcile();
        logScan(QStringLiteral("safety"), beforeDirty, timer.elapsed());
    }

    void queueFullTreeReconcile() {
        if (m_rootPath.isEmpty()) {
            return;
        }

        QHash<QString, FileState> current;
        snapshotStateForRoot(m_rootPath, current);

        for (auto it = current.constBegin(); it != current.constEnd(); ++it) {
            const QString& path = it.key();
            if (!m_projectConfig.shouldTrack(path)) {
                continue;
            }

            const auto knownIt = m_previous.constFind(path);
            if (knownIt == m_previous.constEnd() || knownIt.value() != it.value()) {
                markDirtyRelative(path);
            }
        }

        for (auto it = m_previous.constBegin(); it != m_previous.constEnd(); ++it) {
            if (!current.contains(it.key()) && m_projectConfig.shouldTrack(it.key())) {
                markDirtyRelative(it.key());
            }
        }
    }

    void processDirtyBatch() {
        if (m_rootPath.isEmpty() || m_dirtyRelative.isEmpty()) {
            return;
        }

        QElapsedTimer timer;
        timer.start();

        ++m_scanSequence;
        const QSet<QString> batch = m_dirtyRelative;
        m_dirtyRelative.clear();

        bool needsAnotherPass = false;
        for (const QString& relativePath : batch) {
            if (observePath(relativePath, m_scanSequence)) {
                needsAnotherPass = true;
            }
        }

        logScan(QStringLiteral("debounce"), batch.size(), timer.elapsed());

        if (needsAnotherPass && m_debounceTimer) {
            m_debounceTimer->start();
        }
    }

    // Returns true when the path still needs another stabilization pass.
    bool observePath(const QString& relativePath, qint64 sequence) {
        const QString absolutePath = QDir(m_rootPath).filePath(relativePath);
        const QFileInfo info(absolutePath);

        if (!info.exists() || !info.isFile()) {
            if (m_previous.contains(relativePath)) {
                m_previous.remove(relativePath);
                m_pending.remove(relativePath);
                emitChange(FileEvent::Type::Deleted, relativePath, sequence);
            }
            return false;
        }

        const FileState state {info.size(), info.lastModified().toMSecsSinceEpoch()};
        const auto knownIt = m_previous.constFind(relativePath);
        if (knownIt != m_previous.constEnd() && knownIt.value() == state) {
            m_pending.remove(relativePath);
            return false;
        }

        const auto pendingIt = m_pending.constFind(relativePath);
        if (pendingIt != m_pending.constEnd() && pendingIt.value() == state) {
            const bool isNew = knownIt == m_previous.constEnd();
            m_previous.insert(relativePath, state);
            m_pending.remove(relativePath);
            emitChange(isNew ? FileEvent::Type::Created : FileEvent::Type::Modified, relativePath, sequence);
            return false;
        }

        m_pending.insert(relativePath, state);
        m_dirtyRelative.insert(relativePath);
        return true;
    }

    void emitChange(FileEvent::Type type, const QString& relativePath, qint64 sequence) {
        FileEvent event;
        event.type = type;
        event.relativePath = relativePath;
        event.absolutePath = QDir(m_rootPath).filePath(relativePath);
        event.scanSequence = sequence;
        if (m_onEvent) {
            m_onEvent(event);
        }
    }

    void logScan(const QString& scanKind, int itemCount, qint64 elapsedMs) {
        if (m_logScan) {
            m_logScan(scanKind, m_rootPath, itemCount, elapsedMs);
        }
    }

    QString m_rootPath;
    ProjectConfig m_projectConfig;
    qint64 m_scanSequence {0};
    QFileSystemWatcher* m_watcher {nullptr};
    QTimer* m_debounceTimer {nullptr};
    QTimer* m_safetyTimer {nullptr};
    QSet<QString> m_dirtyRelative;
    QHash<QString, FileState> m_previous;
    QHash<QString, FileState> m_pending;
    std::function<void(const FileEvent&)> m_onEvent;
    FileWatcherScanLogFn m_logScan;
};

class HybridFileWatcher final : public IFileWatcher {
public:
    explicit HybridFileWatcher(FileWatcherScanLogFn logFn)
        : m_logFn(std::move(logFn)) {
        qRegisterMetaType<FileEvent>("FileEvent");

        m_worker = new HybridWatchWorker();
        m_worker->moveToThread(&m_workerThread);
        connect(&m_workerThread, &QThread::finished, m_worker, &QObject::deleteLater);

        m_worker->setScanLogHandler(m_logFn);

        m_worker->setEventHandler([this](const FileEvent& event) {
            QMetaObject::invokeMethod(this,
                                      [this, event]() {
                                          emit fileEvent(event);
                                      },
                                      Qt::QueuedConnection);
        });

        m_workerThread.start();
    }

    ~HybridFileWatcher() override {
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
                                  Qt::BlockingQueuedConnection);
    }

private:
    QThread m_workerThread;
    HybridWatchWorker* m_worker {nullptr};
    FileWatcherScanLogFn m_logFn;
};

} // namespace

std::unique_ptr<IFileWatcher> createFileWatcher(FileWatcherScanLogFn logFn) {
    return std::make_unique<HybridFileWatcher>(std::move(logFn));
}
