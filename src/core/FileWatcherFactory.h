#pragma once

#include <memory>

class IFileWatcher;

std::unique_ptr<IFileWatcher> createFileWatcher();
