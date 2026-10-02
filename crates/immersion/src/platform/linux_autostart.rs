//! Linux launch at login: an XDG autostart entry. It passes
//! [`AUTOSTART_ARG`] so a login launch can start in the tray while a launch
//! from the app menu still opens the window.

use std::path::PathBuf;

use super::AUTOSTART_ARG;

/// Whether this process was started by the autostart entry.
pub fn launched_at_login() -> bool {
    std::env::args().skip(1).any(|arg| arg == AUTOSTART_ARG)
}

/// A Flatpak's config directory is private to the sandbox, so an entry
/// written there would never be read; it needs the Background portal.
fn sandboxed() -> bool {
    std::path::Path::new("/.flatpak-info").exists()
}

fn entry_path() -> Option<PathBuf> {
    let file = format!("{}.desktop", crate::LINUX_APP_ID);
    Some(dirs::config_dir()?.join("autostart").join(file))
}

/// The path to launch: the AppImage itself rather than its mount point,
/// which changes every run.
fn executable() -> Option<String> {
    if let Some(appimage) = std::env::var_os("APPIMAGE") {
        return Some(appimage.to_string_lossy().into_owned());
    }
    let exe = std::env::current_exe().ok()?;
    Some(exe.to_string_lossy().into_owned())
}

/// Quotes one Exec argument. The desktop entry spec applies its string
/// escapes before its quoting rule, so escaped characters take two
/// backslashes, a literal backslash four, and `%` doubles as a field code.
fn quote_exec_arg(arg: &str) -> String {
    let mut quoted = String::from("\"");
    for ch in arg.chars() {
        match ch {
            '\\' => quoted.push_str("\\\\\\\\"),
            '"' | '`' | '$' => {
                quoted.push_str("\\\\");
                quoted.push(ch);
            }
            '%' => quoted.push_str("%%"),
            _ => quoted.push(ch),
        }
    }
    quoted.push('"');
    quoted
}

fn entry(executable: &str) -> String {
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=Immersion\n\
         Comment=Automatic version history for creative projects\n\
         Exec={} {AUTOSTART_ARG}\n\
         Icon={}\n\
         Terminal=false\n\
         X-GNOME-Autostart-enabled=true\n",
        quote_exec_arg(executable),
        crate::LINUX_APP_ID,
    )
}

pub fn is_launch_at_startup_supported() -> bool {
    !sandboxed() && entry_path().is_some()
}

/// Writes or removes the entry. It is rewritten on every launch while
/// enabled, so it follows the binary when a package moves it.
pub fn set_launch_at_startup(enabled: bool) {
    if sandboxed() {
        return;
    }
    let Some(path) = entry_path() else {
        return;
    };
    if !enabled {
        let _ = std::fs::remove_file(path);
        return;
    }
    let Some(executable) = executable() else {
        return;
    };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(path, entry(&executable));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exec_argument_is_quoted_and_escaped() {
        assert_eq!(
            quote_exec_arg("/usr/bin/immersion"),
            "\"/usr/bin/immersion\""
        );
        assert_eq!(quote_exec_arg("/opt/My App/imm"), "\"/opt/My App/imm\"");
        assert_eq!(quote_exec_arg("/a$b"), "\"/a\\\\$b\"");
        assert_eq!(quote_exec_arg("/a\\b"), "\"/a\\\\\\\\b\"");
        assert_eq!(quote_exec_arg("/100%"), "\"/100%%\"");
    }

    #[test]
    fn entry_launches_with_the_autostart_flag() {
        let text = entry("/usr/bin/immersion");
        assert!(text.contains("\nExec=\"/usr/bin/immersion\" --autostart\n"));
        assert!(text.starts_with("[Desktop Entry]\n"));
    }
}
