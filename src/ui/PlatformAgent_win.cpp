#include "PlatformAgent.h"

#include <QCoreApplication>
#include <QDir>
#include <QSettings>

namespace {

constexpr auto kRunKeyPath = "HKEY_CURRENT_USER\\Software\\Microsoft\\Windows\\CurrentVersion\\Run";
constexpr auto kRunValueName = "Immersion";

QString currentExecutablePath() {
    return QDir::toNativeSeparators(QCoreApplication::applicationFilePath());
}

} // namespace

namespace PlatformAgent {

void setBackgroundAgentMode(bool /*enabled*/) {}

void activateApplication() {}

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
