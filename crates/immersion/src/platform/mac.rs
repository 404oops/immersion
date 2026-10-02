//! macOS platform glue: NSStatusItem menu bar item, activation policy
//! (background-agent mode), SMAppService launch-at-login, and
//! UNUserNotificationCenter save notifications.

use std::cell::{Cell, RefCell};
use std::time::{Duration, Instant};

use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject, Bool, NSObjectProtocol, ProtocolObject};
use objc2::{AllocAnyThread, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSApplicationDidBecomeActiveNotification,
    NSEventModifierFlags, NSEventType, NSImage, NSMenu, NSMenuItem, NSStatusBar, NSStatusItem,
    NSVariableStatusItemLength, NSWindow, NSWindowAnimationBehavior,
};
use objc2_foundation::{
    NSBundle, NSData, NSNotification, NSNotificationCenter, NSObject, NSPoint, NSString,
};
use objc2_user_notifications::{
    UNNotification, UNNotificationPresentationOptions, UNUserNotificationCenter,
    UNUserNotificationCenterDelegate,
};

/// Menu bar template icon.
const MENUBAR_ICON: &[u8] = include_bytes!("../../assets/icons/menubar.png");
/// App icon for the Dock when running unbundled.
const APP_ICON: &[u8] = include_bytes!("../../assets/icons/app.png");

thread_local! {
    static OPEN_CALLBACK: RefCell<Option<Box<dyn Fn()>>> = const { RefCell::new(None) };
    static QUIT_CALLBACK: RefCell<Option<Box<dyn Fn()>>> = const { RefCell::new(None) };
    static STATUS_ITEM: RefCell<Option<Retained<NSStatusItem>>> = const { RefCell::new(None) };
    static STATUS_MENU: RefCell<Option<Retained<NSMenu>>> = const { RefCell::new(None) };
    static STATUS_TARGET: RefCell<Option<Retained<StatusTarget>>> = const { RefCell::new(None) };
    // UNUserNotificationCenter.delegate is weak; keep the strong reference.
    static NOTIFICATION_DELEGATE: RefCell<Option<Retained<NotificationDelegate>>> =
        const { RefCell::new(None) };
    static ACTIVATION_CALLBACK: RefCell<Option<Box<dyn Fn()>>> = const { RefCell::new(None) };
    static ACTIVATION_OBSERVER: RefCell<Option<Retained<ProtocolObject<dyn NSObjectProtocol>>>> =
        const { RefCell::new(None) };
    static ACTIVATION_SUPPRESSED_UNTIL: Cell<Option<Instant>> = const { Cell::new(None) };
    static MAIN_NS_WINDOW: RefCell<Option<Retained<NSWindow>>> = const { RefCell::new(None) };
}

/// Adopts the window GPUI just opened, so hiding to the menu bar can order it
/// out and a status-item click can order it back in.
///
/// `DocumentWindow` animation behaviour is what makes AppKit animate those:
/// left at the default, a window ordered in from the status item simply
/// appears. GPUI only sets a behaviour on popup windows, never on this one.
pub fn adopt_main_window(title: &str) {
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let app = NSApplication::sharedApplication(mtm);
    for window in app.windows().iter() {
        if window.title().to_string() != title {
            continue;
        }
        window.setAnimationBehavior(NSWindowAnimationBehavior::DocumentWindow);
        MAIN_NS_WINDOW.with(|slot| *slot.borrow_mut() = Some(window.clone()));
        return;
    }
}

fn with_main_window(action: impl FnOnce(&NSWindow)) -> bool {
    MAIN_NS_WINDOW.with(|slot| {
        let slot = slot.borrow();
        let Some(window) = slot.as_ref() else {
            return false;
        };
        action(window);
        true
    })
}

/// Orders the main window out (animated), leaving the app in the menu bar.
/// Returns false when there is no adopted window to hide.
pub fn hide_main_window() -> bool {
    with_main_window(|window| window.orderOut(None))
}

/// Orders it back in, animated, as a freshly opened window would be.
pub fn show_main_window() -> bool {
    with_main_window(|window| {
        if window.isMiniaturized() {
            window.deminiaturize(None);
        }
        window.makeKeyAndOrderFront(None);
    })
}

/// Skips the next activation-driven window reveal for a moment; used around
/// status-item interactions so opening the tray menu doesn't pop the window.
fn suppress_activation_reveal() {
    ACTIVATION_SUPPRESSED_UNTIL
        .with(|cell| cell.set(Some(Instant::now() + Duration::from_secs(1))));
}

/// Skips activation-driven reveals while a login launch settles: the system
/// can activate a login item as it opens, which would show the window.
pub fn suppress_launch_activation_reveal() {
    ACTIVATION_SUPPRESSED_UNTIL
        .with(|cell| cell.set(Some(Instant::now() + Duration::from_secs(5))));
}

/// Whether the system opened the app as a login item. Only valid while the
/// launch is handled, i.e. during applicationDidFinishLaunching, when the
/// open-application Apple event is still current.
pub fn launched_at_login() -> bool {
    const OPEN_APPLICATION: u32 = u32::from_be_bytes(*b"oapp");
    const PROP_DATA: u32 = u32::from_be_bytes(*b"prdt");
    const LAUNCHED_AS_LOGIN_ITEM: u32 = u32::from_be_bytes(*b"lgit");
    unsafe {
        let manager: Retained<AnyObject> =
            msg_send![objc2::class!(NSAppleEventManager), sharedAppleEventManager];
        let event: Option<Retained<AnyObject>> = msg_send![&*manager, currentAppleEvent];
        let Some(event) = event else {
            return false;
        };
        let event_id: u32 = msg_send![&*event, eventID];
        if event_id != OPEN_APPLICATION {
            return false;
        }
        let property: Option<Retained<AnyObject>> =
            msg_send![&*event, paramDescriptorForKeyword: PROP_DATA];
        property.is_some_and(|property| {
            let code: u32 = msg_send![&*property, enumCodeValue];
            code == LAUNCHED_AS_LOGIN_ITEM
        })
    }
}

fn activation_reveal_suppressed() -> bool {
    ACTIVATION_SUPPRESSED_UNTIL
        .with(|cell| cell.get())
        .is_some_and(|until| Instant::now() < until)
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
            // Status-item clicks activate the app; don't let the activation
            // observer treat that as a Spotlight-style reveal request.
            suppress_activation_reveal();
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
                            // Returns BOOL; declaring () would fail objc2's
                            // debug-build encoding verification.
                            let _: Bool = msg_send![
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
            suppress_activation_reveal();
            QUIT_CALLBACK.with(|callback| {
                if let Some(callback) = callback.borrow().as_ref() {
                    callback();
                }
            });
        }
    }
);

define_class!(
    #[unsafe(super(NSObject))]
    #[name = "MusitNotificationDelegate"]
    struct NotificationDelegate;

    unsafe impl NSObjectProtocol for NotificationDelegate {}

    unsafe impl UNUserNotificationCenterDelegate for NotificationDelegate {
        // Present banner + sound + list even while Immersion is the frontmost
        // app (the macOS default suppresses notifications from the active
        // app).
        #[unsafe(method(userNotificationCenter:willPresentNotification:withCompletionHandler:))]
        fn will_present_notification(
            &self,
            _center: &UNUserNotificationCenter,
            _notification: &UNNotification,
            completion_handler: &block2::Block<dyn Fn(UNNotificationPresentationOptions)>,
        ) {
            let options = UNNotificationPresentationOptions::Banner
                | UNNotificationPresentationOptions::Sound
                | UNNotificationPresentationOptions::List;
            completion_handler.call((options,));
        }
    }
);

/// Reveals the window when the app is activated without going through
/// applicationShouldHandleReopen — Spotlight and Dock often activate an
/// already-running process directly, so this observes
/// NSApplicationDidBecomeActiveNotification instead.
pub fn install_activation_observer(on_activate: Box<dyn Fn()>) {
    if MainThreadMarker::new().is_none() {
        return;
    }
    ACTIVATION_CALLBACK.with(|cell| *cell.borrow_mut() = Some(on_activate));
    let already_installed = ACTIVATION_OBSERVER.with(|cell| cell.borrow().is_some());
    if already_installed {
        return;
    }
    unsafe {
        let center = NSNotificationCenter::defaultCenter();
        let block = block2::RcBlock::new(|_notification: std::ptr::NonNull<NSNotification>| {
            if activation_reveal_suppressed() {
                return;
            }
            ACTIVATION_CALLBACK.with(|cell| {
                if let Some(callback) = cell.borrow().as_ref() {
                    callback();
                }
            });
        });
        let token = center.addObserverForName_object_queue_usingBlock(
            Some(NSApplicationDidBecomeActiveNotification),
            None,
            None,
            &block,
        );
        ACTIVATION_OBSERVER.with(|cell| *cell.borrow_mut() = Some(token));
    }
}

/// Whether we are running from a .app bundle (needed for SMAppService and
/// user notifications).
pub fn is_bundled() -> bool {
    NSBundle::mainBundle().bundleIdentifier().is_some()
}

/// Background-agent mode: the Accessory activation policy hides the Dock icon.
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
    // Prefer the template icon shipped in the app bundle's resources.
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

    let target: Retained<StatusTarget> = unsafe { msg_send![StatusTarget::alloc(mtm), init] };

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
    // SMAppService was introduced in macOS 13, while the bundle still runs
    // on macOS 12. Looking up the class avoids messaging an unavailable API.
    is_bundled() && AnyClass::get(c"SMAppService").is_some()
}

/// Registration can succeed while macOS still waits for the user's approval.
pub fn launch_at_startup_needs_approval() -> bool {
    if !is_launch_at_startup_supported() {
        return false;
    }
    use objc2_service_management::{SMAppService, SMAppServiceStatus};
    unsafe { SMAppService::mainAppService().status() == SMAppServiceStatus::RequiresApproval }
}

pub fn set_launch_at_startup(enabled: bool) {
    if !is_launch_at_startup_supported() {
        return;
    }
    unsafe {
        use objc2_service_management::{SMAppService, SMAppServiceStatus};
        let service = SMAppService::mainAppService();
        let status = service.status();
        let result = if enabled
            && matches!(
                status,
                SMAppServiceStatus::NotRegistered | SMAppServiceStatus::NotFound
            ) {
            service.registerAndReturnError()
        } else if !enabled
            && matches!(
                status,
                SMAppServiceStatus::Enabled | SMAppServiceStatus::RequiresApproval
            )
        {
            service.unregisterAndReturnError()
        } else {
            return;
        };
        if let Err(error) = result {
            eprintln!("Immersion launch at login could not be updated: {error}");
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
        use objc2_user_notifications::UNAuthorizationOptions;
        let center = UNUserNotificationCenter::currentNotificationCenter();
        // Install the foreground-presentation delegate (weak property, so
        // the strong reference lives in NOTIFICATION_DELEGATE).
        let delegate: Retained<NotificationDelegate> =
            unsafe { msg_send![NotificationDelegate::alloc(), init] };
        center.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
        NOTIFICATION_DELEGATE.with(|cell| *cell.borrow_mut() = Some(delegate));
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

/// The document icon macOS shows for files with `extension`, rendered to a
/// square PNG of `size_px` pixels. The lookup goes by content type, so it
/// never touches a file (which may sit on a slow share) and costs one call
/// per extension. Many creative project formats are undeclared, dynamic types that
/// still carry their app's document icon, so "unknown" is detected by
/// comparing against the generic document icon; None then, so the UI keeps
/// its own tile.
pub fn file_type_icon_png(extension: &str, size_px: usize) -> Option<Vec<u8>> {
    if extension.is_empty() || size_px == 0 {
        return None;
    }
    let icon = render_type_icon_png(extension, size_px)?;
    let generic = render_type_icon_png("immersion-unclaimed-type-probe", size_px)?;
    if icon == generic { None } else { Some(icon) }
}

fn render_type_icon_png(extension: &str, size_px: usize) -> Option<Vec<u8>> {
    use objc2_app_kit::{
        NSBitmapImageRep, NSCalibratedRGBColorSpace, NSCompositingOperation, NSGraphicsContext,
        NSWorkspace,
    };
    use objc2_foundation::{NSPoint, NSRect, NSSize};
    use objc2_uniform_type_identifiers::UTType;

    let _mtm = MainThreadMarker::new()?;

    let content_type = UTType::typeWithFilenameExtension(&NSString::from_str(extension))?;
    let icon = NSWorkspace::sharedWorkspace().iconForContentType(&content_type);

    let side = size_px as isize;
    let rep = unsafe {
        NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
            NSBitmapImageRep::alloc(),
            std::ptr::null_mut(),
            side,
            side,
            8,
            4,
            true,
            false,
            NSCalibratedRGBColorSpace,
            0,
            0,
        )
    }?;
    let context = NSGraphicsContext::graphicsContextWithBitmapImageRep(&rep)?;
    let rect = NSRect::new(
        NSPoint::new(0.0, 0.0),
        NSSize::new(size_px as f64, size_px as f64),
    );
    NSGraphicsContext::saveGraphicsState_class();
    NSGraphicsContext::setCurrentContext(Some(&context));
    icon.drawInRect_fromRect_operation_fraction(
        rect,
        NSRect::ZERO,
        NSCompositingOperation::Copy,
        1.0,
    );
    context.flushGraphics();
    NSGraphicsContext::restoreGraphicsState_class();

    // Read the pixels back: 8-bit RGBA, premultiplied (the default drawing
    // format), possibly with padded rows.
    let bytes_per_row = rep.bytesPerRow().max(0) as usize;
    let data = rep.bitmapData();
    if data.is_null() || bytes_per_row < size_px * 4 {
        return None;
    }
    let mut rgba = vec![0u8; size_px * size_px * 4];
    for y in 0..size_px {
        let row = unsafe { std::slice::from_raw_parts(data.add(y * bytes_per_row), size_px * 4) };
        rgba[y * size_px * 4..(y + 1) * size_px * 4].copy_from_slice(row);
    }
    super::icon_pixels::unpremultiply(&mut rgba);
    super::icon_pixels::trimmed_png(size_px as u32, size_px as u32, &rgba)
}
