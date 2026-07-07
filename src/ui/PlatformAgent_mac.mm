#include "PlatformAgent.h"

#import <AppKit/AppKit.h>

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

} // namespace PlatformAgent
