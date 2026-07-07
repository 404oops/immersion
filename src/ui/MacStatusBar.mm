#include "MacStatusBar.h"

#include <QMetaObject>

#import <AppKit/AppKit.h>
#import <UserNotifications/UserNotifications.h>

namespace {

NSImage* statusBarIconImage() {
    const CGFloat size = 18.0;
    NSImage* image = [[NSImage alloc] initWithSize:NSMakeSize(size, size)];
    [image lockFocus];

    [[NSColor controlTextColor] setFill];
    [[NSBezierPath bezierPathWithOvalInRect:NSMakeRect(1.0, 1.0, size - 2.0, size - 2.0)] fill];

    const NSFont* font = [NSFont boldSystemFontOfSize:11.0];
    NSDictionary* attributes = @{
        NSFontAttributeName: font,
        NSForegroundColorAttributeName: [NSColor controlBackgroundColor],
    };
    const NSSize textSize = [@"M" sizeWithAttributes:attributes];
    const NSPoint textOrigin = NSMakePoint(
        (size - textSize.width) / 2.0,
        (size - textSize.height) / 2.0 - 0.5);
    [@"M" drawAtPoint:textOrigin withAttributes:attributes];

    [image unlockFocus];
    [image setTemplate:YES];
    return image;
}

void requestNotificationAuthorizationOnce() {
    static dispatch_once_t onceToken;
    dispatch_once(&onceToken, ^{
        UNUserNotificationCenter* center = [UNUserNotificationCenter currentNotificationCenter];
        [center requestAuthorizationWithOptions:(UNAuthorizationOptionAlert | UNAuthorizationOptionSound)
                              completionHandler:^(__unused BOOL granted, __unused NSError* error) {}];
    });
}

} // namespace

@interface MusitNotificationDelegate : NSObject <UNUserNotificationCenterDelegate>
@end

@implementation MusitNotificationDelegate

- (void)userNotificationCenter:(UNUserNotificationCenter*)center
       willPresentNotification:(UNNotification*)notification
         withCompletionHandler:(void (^)(UNNotificationPresentationOptions options))completionHandler {
    Q_UNUSED(center);
    Q_UNUSED(notification);
    if (@available(macOS 11.0, *)) {
        completionHandler(UNNotificationPresentationOptionBanner
                          | UNNotificationPresentationOptionSound
                          | UNNotificationPresentationOptionList);
    } else {
        completionHandler(UNNotificationPresentationOptionBanner | UNNotificationPresentationOptionSound);
    }
}

@end

namespace {

MusitNotificationDelegate* notificationDelegateInstance() {
    static MusitNotificationDelegate* delegate = [[MusitNotificationDelegate alloc] init];
    return delegate;
}

} // namespace

struct MacStatusBar::Private {
    NSStatusItem* statusItem {nil};
    id target {nil};
};

@interface MusitStatusBarTarget : NSObject
@property (nonatomic, assign) MacStatusBar* controller;
- (void)openFromMenu:(id)sender;
- (void)quitFromMenu:(id)sender;
@end

@implementation MusitStatusBarTarget

- (void)openFromMenu:(__unused id)sender {
    if (self.controller) {
        QMetaObject::invokeMethod(self.controller, "openFromMenu", Qt::QueuedConnection);
    }
}

- (void)quitFromMenu:(__unused id)sender {
    if (self.controller) {
        QMetaObject::invokeMethod(self.controller, "quitFromMenu", Qt::QueuedConnection);
    }
}

@end

MacStatusBar::MacStatusBar(QObject* parent)
    : QObject(parent)
    , d(new Private) {}

MacStatusBar::~MacStatusBar() {
    if (d->statusItem) {
        [[NSStatusBar systemStatusBar] removeStatusItem:d->statusItem];
        d->statusItem = nil;
    }
    delete d;
}

void MacStatusBar::install() {
    if (d->statusItem) {
        return;
    }

    requestNotificationAuthorizationOnce();

    UNUserNotificationCenter* center = [UNUserNotificationCenter currentNotificationCenter];
    center.delegate = notificationDelegateInstance();

    MusitStatusBarTarget* target = [[MusitStatusBarTarget alloc] init];
    target.controller = this;
    d->target = target;

    d->statusItem = [[NSStatusBar systemStatusBar] statusItemWithLength:NSVariableStatusItemLength];
    d->statusItem.button.image = statusBarIconImage();
    d->statusItem.button.toolTip = @"Immersion";

    NSMenu* menu = [[NSMenu alloc] init];
    [menu addItemWithTitle:@"Open Immersion"
                    action:@selector(openFromMenu:)
             keyEquivalent:@"o"].target = target;
    [menu addItem:[NSMenuItem separatorItem]];
    [menu addItemWithTitle:@"Quit"
                    action:@selector(quitFromMenu:)
             keyEquivalent:@"q"].target = target;
    d->statusItem.menu = menu;
}

void MacStatusBar::showNotification(const QString& title, const QString& body) {
    UNMutableNotificationContent* content = [[UNMutableNotificationContent alloc] init];
    content.title = title.toNSString();
    content.body = body.toNSString();
    content.sound = [UNNotificationSound defaultSound];

    NSString* identifier = [[NSUUID UUID] UUIDString];
    UNNotificationRequest* request =
        [UNNotificationRequest requestWithIdentifier:identifier content:content trigger:nil];

    [[UNUserNotificationCenter currentNotificationCenter]
        addNotificationRequest:request
         withCompletionHandler:nil];
}

void MacStatusBar::openFromMenu() {
    emit openRequested();
}

void MacStatusBar::quitFromMenu() {
    emit quitRequested();
}

void MacStatusBar::toggleFromMenu() {
    emit toggleWindowRequested();
}
