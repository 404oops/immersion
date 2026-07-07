#include "PlatformAgent.h"

namespace PlatformAgent {

void setBackgroundAgentMode(bool /*enabled*/) {}

void activateApplication() {}

void setLaunchAtStartup(bool /*enabled*/) {}

bool isLaunchAtStartupEnabled() {
    return false;
}

bool isLaunchAtStartupSupported() {
    return false;
}

} // namespace PlatformAgent
