#pragma once

#include <functional>
#include <memory>

#include <QString>

class IFileWatcher;

using FileWatcherScanLogFn = std::function<void(const QString& scanKind,
                                                const QString& rootPath,
                                                int itemCount,
                                                qint64 elapsedMs)>;

std::unique_ptr<IFileWatcher> createFileWatcher(FileWatcherScanLogFn logFn = {});
