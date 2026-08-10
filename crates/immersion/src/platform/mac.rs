//! macOS platform glue: NSStatusItem (MacStatusBar.mm), activation policy
//! (PlatformAgent_mac.mm), SMAppService launch-at-login, and
//! UNUserNotificationCenter save notifications.

use std::cell::RefCell;

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{AllocAnyThread, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSEventModifierFlags, NSEventType, NSImage,
    NSMenu, NSMenuItem, NSStatusBar, NSStatusItem, NSVariableStatusItemLength,
};
use objc2_foundation::{NSBundle, NSData, NSObject, NSPoint, NSString};

/// Menu bar template icon (qt-legacy/icons/menubar.png).
const MENUBAR_ICON: &[u8] = include_bytes!("../../assets/icons/menubar.png");
/// App icon for the Dock when running unbundled (qt-legacy/icons/app.png).
const APP_ICON: &[u8] = include_bytes!("../../assets/icons/app.png");

thread_local! {
    static OPEN_CALLBACK: RefCell<Option<Box<dyn Fn()>>> = const { RefCell::new(None) };
    static QUIT_CALLBACK: RefCell<Option<Box<dyn Fn()>>> = const { RefCell::new(None) };
    static STATUS_ITEM: RefCell<Option<Retained<NSStatusItem>>> = const { RefCell::new(None) };
    static STATUS_MENU: RefCell<Option<Retained<NSMenu>>> = const { RefCell::new(None) };
    static STATUS_TARGET: RefCell<Option<Retained<StatusTarget>>> = const { RefCell::new(None) };
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "MusitStatusTarget"]
    struct StatusTarget;

    impl StatusTarget {
        #[unsafe(method(statusBarButtonClicked:))]
        fn status_bar_button_clicked(&self, sender: &AnyObject) {
            let Some(mtm) = MainThreadMarker::new() else {
                return;
            };
            let app = NSApplication::sharedApplication(mtm);
            let is_right_click = app.currentEvent().is_some_and(|event| {
                event.r#type() == NSEventType::RightMouseDown
                    || (event.r#type() == NSEventType::LeftMouseDown
                        && event
                            .modifierFlags()
                            .contains(NSEventModifierFlags::Control))
            });

            if is_right_click {
                STATUS_MENU.with(|menu| {
                    if let Some(menu) = menu.borrow().as_ref() {
                        unsafe {
                            let bounds: objc2_foundation::NSRect = msg_send![sender, bounds];
                            let location =
                                NSPoint::new(0.0, bounds.size.height + 4.0);
                            let view: &AnyObject = sender;
                            let _: () = msg_send![
                                &**menu,
                                popUpMenuPositioningItem: std::ptr::null::<AnyObject>(),
                                atLocation: location,
                                inView: view
                            ];
                        }
                    }
                });
                return;
            }

            OPEN_CALLBACK.with(|callback| {
                if let Some(callback) = callback.borrow().as_ref() {
                    callback();
                }
            });
        }

        #[unsafe(method(openFromMenu:))]
        fn open_from_menu(&self, _sender: &AnyObject) {
            OPEN_CALLBACK.with(|callback| {
                if let Some(callback) = callback.borrow().as_ref() {
                    callback();
                }
            });
        }

        #[unsafe(method(quitFromMenu:))]
        fn quit_from_menu(&self, _sender: &AnyObject) {
            QUIT_CALLBACK.with(|callback| {
                if let Some(callback) = callback.borrow().as_ref() {
                    callback();
                }
            });
        }
    }
);

/// Whether we are running from a .app bundle (needed for SMAppService and
/// user notifications).
pub fn is_bundled() -> bool {
    NSBundle::mainBundle().bundleIdentifier().is_some()
}

/// PlatformAgent::setBackgroundAgentMode: Accessory hides the Dock icon.
pub fn set_background_agent_mode(enabled: bool) {
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let app = NSApplication::sharedApplication(mtm);
    let policy = if enabled {
        NSApplicationActivationPolicy::Accessory
    } else {
        NSApplicationActivationPolicy::Regular
    };
    app.setActivationPolicy(policy);
}

fn image_from_bytes(bytes: &[u8]) -> Option<Retained<NSImage>> {
    let data = NSData::with_bytes(bytes);
    unsafe { msg_send![NSImage::alloc(), initWithData: &*data] }
}

fn status_bar_icon(mtm: MainThreadMarker) -> Option<Retained<NSImage>> {
    let _ = mtm;
    // Prefer a bundled template icon like the Qt build.
    unsafe {
        let bundle = NSBundle::mainBundle();
        let name = NSString::from_str("menubar");
        let kind = NSString::from_str("png");
        if let Some(path) = bundle.pathForResource_ofType(Some(&name), Some(&kind)) {
            let image: Option<Retained<NSImage>> =
                msg_send![NSImage::alloc(), initWithContentsOfFile: &*path];
            if let Some(image) = image {
                image.setSize(objc2_foundation::NSSize::new(18.0, 18.0));
                image.setTemplate(true);
                return Some(image);
            }
        }
    }

    // Embedded copy of the same icon for unbundled (dev) builds.
    if let Some(image) = image_from_bytes(MENUBAR_ICON) {
        image.setSize(objc2_foundation::NSSize::new(18.0, 18.0));
        image.setTemplate(true);
        return Some(image);
    }
    None
}

/// Sets the Dock/app-switcher icon when running outside a .app bundle
/// (bundled builds get it from the asset catalog).
pub fn set_dock_icon_if_unbundled() {
    if is_bundled() {
        return;
    }
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    if let Some(image) = image_from_bytes(APP_ICON) {
        let app = NSApplication::sharedApplication(mtm);
        unsafe {
            let _: () = msg_send![&*app, setApplicationIconImage: &*image];
        }
    }
}

/// MacStatusBar::install: status item with left-click open and a
/// right-click menu (Open Immersion / Quit).
pub fn install_status_item(on_open: Box<dyn Fn()>, on_quit: Box<dyn Fn()>) {
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };

    OPEN_CALLBACK.with(|callback| *callback.borrow_mut() = Some(on_open));
    QUIT_CALLBACK.with(|callback| *callback.borrow_mut() = Some(on_quit));

    let already_installed = STATUS_ITEM.with(|item| item.borrow().is_some());
    if already_installed {
        return;
    }

    request_notification_authorization_once();

    let target: Retained<StatusTarget> =
        unsafe { msg_send![StatusTarget::alloc(mtm), init] };

    unsafe {
        let status_bar = NSStatusBar::systemStatusBar();
        let item = status_bar.statusItemWithLength(NSVariableStatusItemLength);

        if let Some(button) = item.button(mtm) {
            if let Some(icon) = status_bar_icon(mtm) {
                button.setImage(Some(&icon));
            } else {
                button.setTitle(&NSString::from_str("M"));
            }
            button.setToolTip(Some(&NSString::from_str("Immersion")));
            let target_obj: &AnyObject = &target;
            let _: () = msg_send![&*button, setTarget: target_obj];
            let _: () = msg_send![&*button, setAction: sel!(statusBarButtonClicked:)];
            // Left + right mouse down (1 << 1 | 1 << 3). Returns the
            // previous event mask (NSInteger).
            let mask: u64 = (1 << 1) | (1 << 3);
            let _previous: i64 = msg_send![&*button, sendActionOn: mask];
        }

        let menu = NSMenu::new(mtm);
        let open_item = NSMenuItem::new(mtm);
        open_item.setTitle(&NSString::from_str("Open Immersion"));
        open_item.setKeyEquivalent(&NSString::from_str("o"));
        open_item.setAction(Some(sel!(openFromMenu:)));
        let target_obj: &AnyObject = &target;
        let _: () = msg_send![&*open_item, setTarget: target_obj];
        menu.addItem(&open_item);

        menu.addItem(&NSMenuItem::separatorItem(mtm));

        let quit_item = NSMenuItem::new(mtm);
        quit_item.setTitle(&NSString::from_str("Quit"));
        quit_item.setKeyEquivalent(&NSString::from_str("q"));
        quit_item.setAction(Some(sel!(quitFromMenu:)));
        let _: () = msg_send![&*quit_item, setTarget: target_obj];
        menu.addItem(&quit_item);

        STATUS_MENU.with(|slot| *slot.borrow_mut() = Some(menu));
        STATUS_ITEM.with(|slot| *slot.borrow_mut() = Some(item));
        STATUS_TARGET.with(|slot| *slot.borrow_mut() = Some(target));
    }
}

// ---- Launch at login (SMAppService) ---------------------------------------

pub fn is_launch_at_startup_supported() -> bool {
    is_bundled()
}

pub fn set_launch_at_startup(enabled: bool) {
    if !is_bundled() {
        return;
    }
    unsafe {
        use objc2_service_management::SMAppService;
        let service = SMAppService::mainAppService();
        if enabled {
            let _ = service.registerAndReturnError();
        } else {
            let _ = service.unregisterAndReturnError();
        }
    }
}

// ---- User notifications ----------------------------------------------------

fn request_notification_authorization_once() {
    if !is_bundled() {
        return;
    }
    use std::sync::Once;
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        use objc2_user_notifications::{UNAuthorizationOptions, UNUserNotificationCenter};
        let center = UNUserNotificationCenter::currentNotificationCenter();
        let options = UNAuthorizationOptions::Alert | UNAuthorizationOptions::Sound;
        let handler = block2::StackBlock::new(
            |_granted: objc2::runtime::Bool, _error: *mut objc2_foundation::NSError| {},
        )
        .copy();
        center.requestAuthorizationWithOptions_completionHandler(options, &handler);
    });
}

/// MacStatusBar::showNotification.
pub fn show_notification(title: &str, body: &str) {
    if !is_bundled() {
        return;
    }
    use objc2_user_notifications::{
        UNMutableNotificationContent, UNNotificationRequest, UNNotificationSound,
        UNUserNotificationCenter,
    };
    let content = UNMutableNotificationContent::new();
    content.setTitle(&NSString::from_str(title));
    content.setBody(&NSString::from_str(body));
    content.setSound(Some(&UNNotificationSound::defaultSound()));

    let identifier = NSString::from_str(&uuid_string());
    let request =
        UNNotificationRequest::requestWithIdentifier_content_trigger(&identifier, &content, None);
    UNUserNotificationCenter::currentNotificationCenter()
        .addNotificationRequest_withCompletionHandler(&request, None);
}

fn uuid_string() -> String {
    // Cheap unique id without pulling in a uuid crate.
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("musit-{nanos}-{}", std::process::id())
}
