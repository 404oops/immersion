#include "PlatformAgent.h"

#import <AppKit/AppKit.h>
#import <ServiceManagement/ServiceManagement.h>

namespace PlatformAgent {

void setBackgroundAgentMode(const bool enabled) {
    if (!NSApp) {
        return;
    }

    const NSApplicationActivationPolicy policy = enabled
        ? NSApplicationActivationPolicyAccessory
        : NSApplicationActivationPolicyRegular;
    [NSApp setActivationPolicy:policy];
}

void activateApplication() {
    if (!NSApp) {
        return;
    }
    [NSApp activateIgnoringOtherApps:YES];
}

void setLaunchAtStartup(const bool enabled) {
    if (@available(macOS 13.0, *)) {
        SMAppService* const service = [SMAppService mainAppService];
        NSError* error = nil;
        if (enabled) {
            [service registerAndReturnError:&error];
        } else {
            [service unregisterAndReturnError:&error];
        }
        (void)error;
    }
}

bool isLaunchAtStartupEnabled() {
    if (@available(macOS 13.0, *)) {
        return [SMAppService mainAppService].status == SMAppServiceStatusEnabled;
    }
    return false;
}

bool isLaunchAtStartupSupported() {
    if (@available(macOS 13.0, *)) {
        return true;
    }
    return false;
}

} // namespace PlatformAgent
