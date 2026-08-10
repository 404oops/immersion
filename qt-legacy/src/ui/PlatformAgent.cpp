#include "PlatformAgent.h"

#include <QWindow>

namespace PlatformAgent {

void setBackgroundAgentMode(bool /*enabled*/) {}

void activateApplication() {}

void presentMainWindow(QWindow* window) {
    if (!window) {
        return;
    }

    window->setVisible(true);
    window->show();
    window->raise();
    window->requestActivate();
}

void initActivationHandling() {}

void setShowWindowHandler(std::function<void()> /*handler*/) {}

void setPresentationSuppressed(bool /*suppressed*/) {}

bool isPresentationSuppressed() {
    return false;
}

void setSkipNextActivationPresent(bool /*skip*/) {}

void installShowWindowHandler(std::function<void()> /*handler*/) {}

void setLaunchAtStartup(bool /*enabled*/) {}

bool isLaunchAtStartupEnabled() {
    return false;
}

bool isLaunchAtStartupSupported() {
    return false;
}

} // namespace PlatformAgent
