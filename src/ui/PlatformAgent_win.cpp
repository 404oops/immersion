#include "PlatformAgent.h"

#include <QCoreApplication>
#include <QDir>
#include <QGuiApplication>
#include <QSettings>
#include <QWindow>

#ifndef NOMINMAX
#define NOMINMAX
#endif
#include <windows.h>

namespace {

constexpr auto kRunKeyPath = "HKEY_CURRENT_USER\\Software\\Microsoft\\Windows\\CurrentVersion\\Run";
constexpr auto kRunValueName = "Immersion";

QString currentExecutablePath() {
    return QDir::toNativeSeparators(QCoreApplication::applicationFilePath());
}

QWindow* primaryWindow() {
    if (QWindow* focused = QGuiApplication::focusWindow()) {
        return focused;
    }

    const QList<QWindow*> windows = QGuiApplication::topLevelWindows();
    for (QWindow* window : windows) {
        if (window) {
            return window;
        }
    }

    return nullptr;
}

} // namespace

namespace PlatformAgent {

void setBackgroundAgentMode(bool /*enabled*/) {}

void activateApplication() {
    QWindow* window = primaryWindow();
    if (!window || window->winId() == 0) {
        return;
    }

    HWND hwnd = reinterpret_cast<HWND>(window->winId());
    if (IsIconic(hwnd)) {
        ShowWindow(hwnd, SW_RESTORE);
    }
    ShowWindow(hwnd, SW_SHOWNA);
    SetForegroundWindow(hwnd);
}

void setLaunchAtStartup(const bool enabled) {
    QSettings settings(kRunKeyPath, QSettings::NativeFormat);
    if (enabled) {
        settings.setValue(QString::fromLatin1(kRunValueName), currentExecutablePath());
    } else {
        settings.remove(QString::fromLatin1(kRunValueName));
    }
    settings.sync();
}

bool isLaunchAtStartupEnabled() {
    QSettings settings(kRunKeyPath, QSettings::NativeFormat);
    const QString registeredPath = settings.value(QString::fromLatin1(kRunValueName)).toString();
    if (registeredPath.isEmpty()) {
        return false;
    }

    return QDir::toNativeSeparators(registeredPath).compare(currentExecutablePath(), Qt::CaseInsensitive) == 0;
}

bool isLaunchAtStartupSupported() {
    return true;
}

} // namespace PlatformAgent
