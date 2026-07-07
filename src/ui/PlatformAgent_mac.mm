#include "PlatformAgent.h"

#include <QWindow>

#import <AppKit/AppKit.h>
#import <ApplicationServices/ApplicationServices.h>
#import <ServiceManagement/ServiceManagement.h>

namespace {

std::function<void()> g_showWindowHandler;
bool g_presentationSuppressed = false;
bool g_skipNextActivationPresent = false;

void dispatchShowWindowHandler() {
    if (g_showWindowHandler) {
        g_showWindowHandler();
    }
}

void keyNativeWindow(QWindow* window) {
    if (!window || window->winId() == 0) {
        return;
    }

    NSView* const view = reinterpret_cast<NSView*>(window->winId());
    NSWindow* const nsWindow = [view window];
    if (nsWindow) {
        [nsWindow makeKeyAndOrderFront:nil];
    }
}

bool anyVisibleAppWindow() {
    for (NSWindow* window in [NSApp windows]) {
        if (window.isVisible) {
            return true;
        }
    }
    return false;
}

} // namespace

@interface MusitAppDelegateForwarder : NSObject <NSApplicationDelegate>
@property (nonatomic, assign) id<NSApplicationDelegate> innerDelegate;
@end

@implementation MusitAppDelegateForwarder

- (BOOL)respondsToSelector:(SEL)selector {
    if (selector == @selector(applicationShouldHandleReopen:hasVisibleWindows:)
        || selector == @selector(applicationDidBecomeActive:)) {
        return YES;
    }
    if ([self.innerDelegate respondsToSelector:selector]) {
        return YES;
    }
    return [super respondsToSelector:selector];
}

- (id)forwardingTargetForSelector:(SEL)selector {
    if ([self.innerDelegate respondsToSelector:selector]) {
        return self.innerDelegate;
    }
    return [super forwardingTargetForSelector:selector];
}

- (BOOL)applicationShouldHandleReopen:(NSApplication*)application hasVisibleWindows:(BOOL)flag {
    Q_UNUSED(application);
    Q_UNUSED(flag);
    dispatchShowWindowHandler();

    if ([self.innerDelegate respondsToSelector:@selector(applicationShouldHandleReopen:hasVisibleWindows:)]) {
        return [self.innerDelegate applicationShouldHandleReopen:application hasVisibleWindows:flag];
    }
    return YES;
}

- (void)applicationDidBecomeActive:(NSNotification*)notification {
    if ([self.innerDelegate respondsToSelector:@selector(applicationDidBecomeActive:)]) {
        [self.innerDelegate applicationDidBecomeActive:notification];
    }

    // Spotlight/Dock often activate the existing process without spawning a new one
    // and without calling applicationShouldHandleReopen.
    if (!g_presentationSuppressed && !anyVisibleAppWindow()) {
        dispatchShowWindowHandler();
    }
}

@end

@interface MusitAppleEventHandler : NSObject
@end

@implementation MusitAppleEventHandler

- (void)handleOpenApplication:(NSAppleEventDescriptor*)event
                 withReplyEvent:(NSAppleEventDescriptor*)replyEvent {
    Q_UNUSED(event);
    Q_UNUSED(replyEvent);
    dispatchShowWindowHandler();
}

@end

namespace {

MusitAppDelegateForwarder* g_delegateForwarder = nil;

void ensureDelegateForwarderInstalled() {
    if (!NSApp) {
        return;
    }

    id const currentDelegate = [NSApp delegate];
    if (currentDelegate == g_delegateForwarder) {
        return;
    }

    if (!g_delegateForwarder) {
        g_delegateForwarder = [[MusitAppDelegateForwarder alloc] init];
    }

    g_delegateForwarder.innerDelegate = currentDelegate;
    [NSApp setDelegate:g_delegateForwarder];
}

void installAppleEventHandlers() {
    static dispatch_once_t onceToken;
    dispatch_once(&onceToken, ^{
        MusitAppleEventHandler* const handlerObject = [[MusitAppleEventHandler alloc] init];
        [[NSAppleEventManager sharedAppleEventManager]
            setEventHandler:handlerObject
                andSelector:@selector(handleOpenApplication:withReplyEvent:)
                forEventClass:kCoreEventClass
                andEventID:kAEOpenApplication];

        const pid_t pid = [[NSProcessInfo processInfo] processIdentifier];
        [[[NSWorkspace sharedWorkspace] notificationCenter]
            addObserverForName:NSWorkspaceDidActivateApplicationNotification
                        object:nil
                         queue:[NSOperationQueue mainQueue]
                    usingBlock:^(NSNotification* notification) {
                        NSRunningApplication* const app =
                            notification.userInfo[NSWorkspaceApplicationKey];
                        if (!app || app.processIdentifier != pid) {
                            return;
                        }
                        if (g_skipNextActivationPresent) {
                            g_skipNextActivationPresent = false;
                            return;
                        }
                        if (g_presentationSuppressed || anyVisibleAppWindow()) {
                            return;
                        }
                        dispatchShowWindowHandler();
                    }];
    });
}

} // namespace

namespace PlatformAgent {

void initActivationHandling() {
    ensureDelegateForwarderInstalled();
    installAppleEventHandlers();
}

void setShowWindowHandler(std::function<void()> handler) {
    g_showWindowHandler = std::move(handler);
    ensureDelegateForwarderInstalled();
}

void setPresentationSuppressed(const bool suppressed) {
    g_presentationSuppressed = suppressed;
}

bool isPresentationSuppressed() {
    return g_presentationSuppressed;
}

void setSkipNextActivationPresent(const bool skip) {
    g_skipNextActivationPresent = skip;
}

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

void presentMainWindow(QWindow* window) {
    setBackgroundAgentMode(false);

    if (!window) {
        activateApplication();
        return;
    }

    window->setVisible(true);
    window->show();
    window->raise();
    window->requestActivate();

    dispatch_async(dispatch_get_main_queue(), ^{
        keyNativeWindow(window);
        activateApplication();
    });
}

void installShowWindowHandler(std::function<void()> handler) {
    initActivationHandling();
    setShowWindowHandler(std::move(handler));
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
