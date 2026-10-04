//! Sparkle is loaded only from a packaged application's signed bundle.
use objc2::rc::{Allocated, Retained};
use objc2::runtime::{AnyClass, AnyObject, NSObjectProtocol};
use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_foundation::{NSBundle, NSFileManager, NSObject, NSString};
use std::cell::RefCell;

thread_local! {
    static CONTROLLER: RefCell<Option<Retained<AnyObject>>> = const { RefCell::new(None) };
    static DELEGATE: RefCell<Option<Retained<UpdateDelegate>>> = const { RefCell::new(None) };
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "ImmersionUpdateDelegate"]
    struct UpdateDelegate;

    unsafe impl NSObjectProtocol for UpdateDelegate {}

    impl UpdateDelegate {
        // Take ownership of scheduling without invoking the immediate handler.
        // Sparkle still installs on normal termination, and won't later ask the
        // user to restart an app that is monitoring projects in the background.
        #[unsafe(method(updater:willInstallUpdateOnQuit:immediateInstallationBlock:))]
        fn install_on_quit(
            &self, _updater: &AnyObject, _item: &AnyObject,
            _handler: &block2::Block<dyn Fn()>,
        ) -> bool { true }
    }
);

pub fn start() {
    if super::disabled()
        || !cfg!(target_arch = "aarch64")
        || CONTROLLER.with(|slot| slot.borrow().is_some())
    {
        return;
    }
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let bundle = NSBundle::mainBundle();
    let path = bundle.bundlePath().to_string();
    // Bare cargo builds and apps running on a mounted installer are skipped.
    if !path.ends_with(".app") || path.starts_with("/Volumes/") {
        return;
    }
    if !NSFileManager::defaultManager().isWritableFileAtPath(&NSString::from_str(&path)) {
        return;
    }
    // Silent updates cannot acquire administrator privileges. Skip read-only
    // mounts, app translocation, and protected installation directories.
    let Some(parent) = std::path::Path::new(&path).parent() else {
        return;
    };
    if tempfile::Builder::new()
        .prefix(".immersion-update-")
        .tempdir_in(parent)
        .is_err()
    {
        return;
    }
    let framework_path =
        NSString::from_str(&format!("{path}/Contents/Frameworks/Sparkle.framework"));
    let Some(framework) = NSBundle::bundleWithPath(&framework_path) else {
        return;
    };
    if !unsafe { framework.load() } {
        eprintln!("Immersion updater: could not load Sparkle");
        return;
    }
    let Some(class) = AnyClass::get(c"SPUStandardUpdaterController") else {
        return;
    };
    // All Sparkle objects are main-thread confined and retained for app lifetime.
    let delegate: Retained<UpdateDelegate> = unsafe { msg_send![UpdateDelegate::alloc(mtm), init] };
    let delegate_obj: &AnyObject = &delegate;
    let controller: Retained<AnyObject> = unsafe {
        let allocated: Allocated<AnyObject> = msg_send![class, alloc];
        msg_send![allocated, initWithStartingUpdater: false, updaterDelegate: delegate_obj,
            userDriverDelegate: std::ptr::null::<AnyObject>()]
    };
    // Use the underlying updater's error-returning API instead of an alert on
    // configuration failure. Such failure must not prevent the app launching.
    let updater: Retained<AnyObject> = unsafe { msg_send![&controller, updater] };
    unsafe {
        let _: () = msg_send![&updater, setAutomaticallyChecksForUpdates: true];
        let _: () = msg_send![&updater, setAutomaticallyDownloadsUpdates: true];
    }
    let started: bool =
        unsafe { msg_send![&updater, startUpdater: std::ptr::null_mut::<*mut AnyObject>()] };
    if !started {
        eprintln!("Immersion updater: Sparkle configuration rejected");
        return;
    }
    DELEGATE.with(|slot| *slot.borrow_mut() = Some(delegate));
    CONTROLLER.with(|slot| *slot.borrow_mut() = Some(controller));
}

/// Called by Settings on the main thread. Sparkle owns any already prepared update.
pub fn settings_changed() {
    if !super::disabled() {
        start();
    }
    CONTROLLER.with(|slot| {
        if let Some(controller) = slot.borrow().as_ref() {
            let updater: Retained<AnyObject> = unsafe { msg_send![controller, updater] };
            let enabled = !super::disabled();
            unsafe {
                let _: () = msg_send![&updater, setAutomaticallyChecksForUpdates: enabled];
                let _: () = msg_send![&updater, setAutomaticallyDownloadsUpdates: enabled];
            }
        }
    });
}
