//! Linux StatusNotifierItem and save notifications. The tray's D-Bus worker
//! sends commands to the GPUI thread; GPUI callbacks must never run on the
//! worker.

use std::cell::RefCell;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};

use ksni::blocking::TrayMethods;
use ksni::menu::StandardItem;

enum Command {
    Open,
    Quit,
}

/// The tray icon's name inside [`icon_theme_dir`], distinct from the
/// installed app icon so hosts draw the tray glyph, not the app icon. No
/// dashes: GTK falls back from `a-b` to `a`, and a themed fallback (the
/// installed app icon) beats an exact match in the host's search path.
const TRAY_ICON_NAME: &str = "io.github._404oops.immersion.tray";
/// The macOS menu bar template: a black glyph whose alpha is the artwork.
const TEMPLATE_PNG: &[u8] = include_bytes!("../../assets/icons/menubar.png");
/// Linux panels draw tray icons as they are rather than tinting templates
/// the way macOS does, and the common ones (Unity, GNOME, Cinnamon) are
/// dark, so the glyph takes the Ubuntu panel icon colour.
const GLYPH_RGB: [u8; 3] = [0xdf, 0xdb, 0xd2];

struct ImmersionTray {
    commands: Sender<Command>,
    /// Whether a tray host's watcher has the item registered.
    online: Arc<AtomicBool>,
    icon: ksni::Icon,
    icon_theme_path: String,
}

impl ksni::Tray for ImmersionTray {
    fn id(&self) -> String {
        "immersion".into()
    }

    fn title(&self) -> String {
        "Immersion".into()
    }

    // Hosts such as Unity's indicator-application ignore the pixmap and
    // draw only a themed icon name, so the name has to resolve somewhere.
    fn icon_name(&self) -> String {
        if self.icon_theme_path.is_empty() {
            "io.github._404oops.immersion".into()
        } else {
            TRAY_ICON_NAME.into()
        }
    }

    fn icon_theme_path(&self) -> String {
        self.icon_theme_path.clone()
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        vec![self.icon.clone()]
    }

    fn watcher_online(&self) {
        self.online.store(true, Ordering::Relaxed);
    }

    fn watcher_offline(&self, _reason: ksni::OfflineReason) -> bool {
        self.online.store(false, Ordering::Relaxed);
        true
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        let _ = self.commands.send(Command::Open);
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        vec![
            StandardItem {
                label: "Open Immersion".into(),
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray.commands.send(Command::Open);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Quit Immersion".into(),
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray.commands.send(Command::Quit);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}

struct StatusItem {
    _handle: ksni::blocking::Handle<ImmersionTray>,
    online: Arc<AtomicBool>,
    commands: Receiver<Command>,
    on_open: Box<dyn Fn()>,
    on_quit: Box<dyn Fn()>,
}

thread_local! {
    static STATUS_ITEM: RefCell<Option<StatusItem>> = const { RefCell::new(None) };
}

/// The template glyph in [`GLYPH_RGB`], as straight RGBA.
fn glyph() -> Option<image::RgbaImage> {
    let template =
        image::load_from_memory_with_format(TEMPLATE_PNG, image::ImageFormat::Png).ok()?;
    let mut glyph = template.into_rgba8();
    for pixel in glyph.pixels_mut() {
        let alpha = pixel[3];
        *pixel = image::Rgba([GLYPH_RGB[0], GLYPH_RGB[1], GLYPH_RGB[2], alpha]);
    }
    Some(glyph)
}

/// The glyph as an SNI pixmap: ARGB32 in network byte order.
fn pixmap(glyph: &image::RgbaImage) -> ksni::Icon {
    let mut data = glyph.as_raw().clone();
    for pixel in data.as_chunks_mut::<4>().0 {
        pixel.rotate_right(1);
    }
    ksni::Icon {
        width: glyph.width() as i32,
        height: glyph.height() as i32,
        data,
    }
}

/// Writes the tray PNG where the host can read it and returns that
/// directory, or "" when it cannot be written. The host resolves the icon
/// itself, so the directory must be one it sees at the same path; the
/// Flatpak manifest shares `xdg-run/immersion` for this and the
/// single-instance socket.
fn icon_theme_dir(glyph: &image::RgbaImage) -> String {
    let Some(runtime) = dirs::runtime_dir() else {
        return String::new();
    };
    let dir = runtime.join("immersion").join("tray-icons");
    let file = dir.join(format!("{TRAY_ICON_NAME}.png"));
    let written = std::fs::create_dir_all(&dir).is_ok() && glyph.save(&file).is_ok();
    if written {
        dir.to_string_lossy().into_owned()
    } else {
        String::new()
    }
}

/// Registers the tray item. A login launch can start before the panel that
/// hosts the tray, so the item stays on the bus and registers once a host's
/// watcher appears.
pub fn install_status_item(on_open: Box<dyn Fn()>, on_quit: Box<dyn Fn()>) {
    let Some(glyph) = glyph() else { return };
    let (sender, commands) = channel();
    let online = Arc::new(AtomicBool::new(true));
    let sandboxed = std::path::Path::new("/.flatpak-info").exists();
    let tray = ImmersionTray {
        commands: sender,
        online: online.clone(),
        icon: pixmap(&glyph),
        icon_theme_path: icon_theme_dir(&glyph),
    };
    let spawned = tray
        .disable_dbus_name(sandboxed)
        .assume_sni_available(true)
        .spawn();
    match spawned {
        Ok(handle) => STATUS_ITEM.with(|slot| {
            *slot.borrow_mut() = Some(StatusItem {
                _handle: handle,
                online,
                commands,
                on_open,
                on_quit,
            });
        }),
        Err(error) => eprintln!("Immersion tray unavailable: {error}"),
    }
}

/// Whether a tray host shows the item, so a hidden window can be reopened.
pub fn status_item_available() -> bool {
    STATUS_ITEM.with(|slot| {
        slot.borrow()
            .as_ref()
            .is_some_and(|item| item.online.load(Ordering::Relaxed))
    })
}

pub fn poll_status_item() {
    STATUS_ITEM.with(|slot| {
        if let Some(item) = slot.borrow().as_ref() {
            while let Ok(command) = item.commands.try_recv() {
                match command {
                    Command::Open => (item.on_open)(),
                    Command::Quit => (item.on_quit)(),
                }
            }
        }
    });
}

/// A desktop notification with the app's icon and no actions. GPUI's own
/// notifications always carry a default action, which Immersion never
/// handles and which notify-osd shows as a dialog instead of a bubble.
pub fn show_notification(title: &str, body: &str) {
    let mut notification = notify_rust::Notification::new();
    notification
        .appname("Immersion")
        .summary(title)
        .body(body)
        .icon(crate::LINUX_APP_ID)
        .hint(notify_rust::Hint::DesktopEntry(crate::LINUX_APP_ID.into()));
    // Showing connects to the session bus, so it runs off the UI thread.
    let _ = std::thread::Builder::new()
        .name("notification".into())
        .spawn(move || {
            let _ = notification.show();
        });
}
