//! Linux StatusNotifierItem. Its D-Bus worker sends commands to the GPUI
//! thread; GPUI callbacks must never run on the worker.

use std::cell::RefCell;
use std::sync::mpsc::{Receiver, Sender, channel};

use ksni::blocking::TrayMethods;
use ksni::menu::StandardItem;

enum Command {
    Open,
    Quit,
}

struct ImmersionTray {
    commands: Sender<Command>,
    icon: ksni::Icon,
}

impl ksni::Tray for ImmersionTray {
    fn id(&self) -> String {
        "immersion".into()
    }

    fn title(&self) -> String {
        "Immersion".into()
    }

    fn icon_name(&self) -> String {
        "io.github._404oops.immersion".into()
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        vec![self.icon.clone()]
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
    commands: Receiver<Command>,
    on_open: Box<dyn Fn()>,
    on_quit: Box<dyn Fn()>,
}

thread_local! {
    static STATUS_ITEM: RefCell<Option<StatusItem>> = const { RefCell::new(None) };
}

fn icon() -> Option<ksni::Icon> {
    use image::GenericImageView;
    let image = image::load_from_memory_with_format(
        include_bytes!("../../assets/icons/tray-linux.png"),
        image::ImageFormat::Png,
    )
    .ok()?;
    let (width, height) = image.dimensions();
    let mut data = image.into_rgba8().into_vec();
    for pixel in data.chunks_exact_mut(4) {
        pixel.rotate_right(1);
    }
    Some(ksni::Icon {
        width: width as i32,
        height: height as i32,
        data,
    })
}

pub fn install_status_item(on_open: Box<dyn Fn()>, on_quit: Box<dyn Fn()>) {
    let Some(icon) = icon() else { return };
    let (sender, commands) = channel();
    let tray = ImmersionTray {
        commands: sender,
        icon,
    };
    let sandboxed = std::path::Path::new("/.flatpak-info").exists();
    match tray.disable_dbus_name(sandboxed).spawn() {
        Ok(handle) => STATUS_ITEM.with(|slot| {
            *slot.borrow_mut() = Some(StatusItem {
                _handle: handle,
                commands,
                on_open,
                on_quit,
            });
        }),
        Err(error) => eprintln!("Immersion tray unavailable: {error}"),
    }
}

pub fn status_item_available() -> bool {
    STATUS_ITEM.with(|slot| slot.borrow().is_some())
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
