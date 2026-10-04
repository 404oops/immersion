//! Per-user NSIS upgrades. A copied helper owns the single-instance guard
//! during installation, so neither the installer nor helper locks the app exe.

use super::manifest::{self, Manifest, Result, bounded};
use crate::platform::single_instance::{self, InstanceGuard};
use std::fs::{self, File};
use std::io::Read;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Condvar, Mutex};
use std::time::Duration;

use windows::Win32::Foundation::{CloseHandle, ERROR_INVALID_PARAMETER, WAIT_OBJECT_0};
use windows::Win32::System::Threading::{OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject};
use winreg::{RegKey, enums::HKEY_CURRENT_USER};

static STARTED: AtomicBool = AtomicBool::new(false);
static WAKE: (Mutex<bool>, Condvar) = (Mutex::new(false), Condvar::new());

const KEY: Option<&str> = option_env!("IMMERSION_UPDATE_PUBLIC_KEY");
const CURRENT: &str = env!("CARGO_PKG_VERSION");
const CREATE_NO_WINDOW: u32 = 0x08000000;

fn target() -> &'static str {
    if cfg!(target_arch = "aarch64") {
        "windows-arm64"
    } else {
        "windows-x64"
    }
}

fn root() -> Result<PathBuf> {
    Ok(dirs::data_local_dir()
        .ok_or("no local app data directory")?
        .join("Immersion/updates"))
}

fn installed_exe() -> Result<PathBuf> {
    let key = RegKey::predef(HKEY_CURRENT_USER).open_subkey("Software\\Immersion")?;
    let dir: String = key.get_value("InstallDir")?;
    let exe = Path::new(&dir).join("Immersion.exe");
    if !exe.is_absolute() {
        return Err("invalid registered installation".into());
    }
    Ok(exe)
}

fn is_installed() -> bool {
    installed_exe()
        .ok()
        .and_then(|p| fs::canonicalize(p).ok())
        .zip(
            std::env::current_exe()
                .ok()
                .and_then(|p| fs::canonicalize(p).ok()),
        )
        .is_some_and(|(installed, running)| installed == running)
}

fn staged(dir: &Path) -> Result<Manifest> {
    manifest::staged(dir, KEY.ok_or("no update key")?, target(), CURRENT)
}

fn download() -> Result<()> {
    let root = root()?;
    fs::create_dir_all(&root)?;
    if root.join("pending").exists() {
        return Ok(());
    }
    let client = reqwest::blocking::Client::builder()
        .https_only(true)
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(300))
        .user_agent("Immersion-Updater")
        .build()?;
    let feed = format!(
        "https://github.com/404oops/immersion/releases/latest/download/update-{}.json",
        target()
    );
    let bytes = bounded(
        client.get(&feed).send()?.error_for_status()?,
        manifest::MAX_MANIFEST,
    )?;
    let sig = bounded(
        client
            .get(format!("{feed}.sig"))
            .send()?
            .error_for_status()?,
        1024,
    )?;
    let m = manifest::verify(
        &bytes,
        std::str::from_utf8(&sig)?,
        KEY.ok_or("no update key")?,
    )?;
    // A current release isn't a failed download.
    if m.version == CURRENT {
        return Ok(());
    }
    m.validate(target(), CURRENT)?;
    let temp = tempfile::Builder::new()
        .prefix("download-")
        .tempdir_in(&root)?;
    let mut file = File::create(temp.path().join("installer.exe"))?;
    let size = std::io::copy(
        &mut client
            .get(&m.url)
            .send()?
            .error_for_status()?
            .take(m.size + 1),
        &mut file,
    )?;
    file.sync_all()?;
    if size != m.size {
        return Err("installer download size mismatch".into());
    }
    drop(file);
    m.verify_installer(File::open(temp.path().join("installer.exe"))?)?;
    fs::write(temp.path().join("manifest.json"), bytes)?;
    fs::write(temp.path().join("manifest.sig"), sig)?;
    // Disabling during a download discards it rather than staging an update.
    if super::disabled() {
        return Ok(());
    }
    // An incomplete download never becomes an installable pending update.
    fs::rename(temp.path(), root.join("pending"))?;
    Ok(())
}

pub fn start() {
    if super::disabled() || KEY.is_none_or(|key| key.trim().is_empty()) || !is_installed() {
        return;
    }
    if STARTED.swap(true, Ordering::AcqRel) {
        return;
    }
    // Old copied helpers are no longer locked after their process exits.
    if let Ok(root) = root()
        && let Ok(entries) = fs::read_dir(root)
    {
        for entry in entries.flatten() {
            if entry.file_name().to_string_lossy().starts_with("helper-") {
                let _ = fs::remove_dir_all(entry.path());
            }
        }
    }
    if std::thread::Builder::new()
        .name("update-download".into())
        .spawn(|| {
            loop {
                if !super::disabled() {
                    if let Err(error) = download() {
                        eprintln!("Immersion update download: {error}");
                    }
                }
                let wake = WAKE.0.lock().unwrap();
                let (mut wake, _) = WAKE
                    .1
                    .wait_timeout_while(wake, Duration::from_secs(6 * 60 * 60), |wake| !*wake)
                    .unwrap();
                *wake = false;
            }
        })
        .is_err()
    {
        STARTED.store(false, Ordering::Release);
    }
}

pub fn settings_changed() {
    start();
    *WAKE.0.lock().unwrap() = true;
    WAKE.1.notify_one();
}

/// Called before the backend exists, so no watchers or pending writes are stopped.
pub fn install_pending() -> bool {
    if super::disabled()
        || KEY.is_none_or(|key| key.trim().is_empty())
        || !is_installed()
        || std::env::args_os().any(|a| a == "--immersion-skip-update")
    {
        return false;
    }
    let Ok(root) = root() else {
        return false;
    };
    let pending = root.join("pending");
    if !pending.exists() {
        return false;
    }
    if let Err(error) = staged(&pending) {
        eprintln!("Immersion rejected staged update: {error}");
        let _ = fs::remove_dir_all(pending);
        return false;
    }
    let launch = || -> Result<()> {
        let helper = tempfile::Builder::new()
            .prefix("helper-")
            .tempdir_in(&root)?;
        let exe = helper.path().join("helper.exe");
        fs::copy(std::env::current_exe()?, &exe)?;
        let mut command = Command::new(exe);
        command
            .arg("--immersion-update-helper")
            .arg(std::process::id().to_string());
        if std::env::args_os().any(|a| a == "--autostart") {
            command.arg("--autostart");
        }
        command.creation_flags(CREATE_NO_WINDOW).spawn()?;
        // A later normal launch deletes this directory; its exe is locked now.
        let _ = helper.keep();
        Ok(())
    };
    match launch() {
        Ok(()) => true,
        Err(error) => {
            eprintln!("Immersion could not start updater: {error}");
            false
        }
    }
}

fn wait_for_parent(pid: u32) -> Result<()> {
    // Keep a process handle, rather than polling a PID that could be reused.
    let handle = match unsafe { OpenProcess(PROCESS_SYNCHRONIZE, false, pid) } {
        Ok(handle) => handle,
        // The launcher may already have exited before the child starts.
        Err(error)
            if error.code() == windows::core::HRESULT::from_win32(ERROR_INVALID_PARAMETER.0) =>
        {
            return Ok(());
        }
        Err(error) => return Err(error.into()),
    };
    let result = unsafe { WaitForSingleObject(handle, 60_000) };
    let _ = unsafe { CloseHandle(handle) };
    if result != WAIT_OBJECT_0 {
        return Err("updater parent did not exit".into());
    }
    Ok(())
}

pub fn run_helper_if_requested() -> bool {
    let args: Vec<String> = std::env::args().collect();
    let mode = args.get(1).map(String::as_str);
    if !matches!(
        mode,
        Some("--immersion-update-helper" | "--immersion-relaunch")
    ) {
        return false;
    }
    let Some(pid) = args.get(2).and_then(|p| p.parse::<u32>().ok()) else {
        return true;
    };
    if let Err(error) = wait_for_parent(pid) {
        eprintln!("Immersion updater: {error}");
        return true;
    }
    if mode == Some("--immersion-relaunch") {
        return false;
    }
    let InstanceGuard::Primary(_rx) = single_instance::acquire() else {
        return true;
    };
    // A preference changed during handoff must still let the original app open.
    let result = if super::disabled() { Ok(()) } else { apply() };
    if let Err(error) = result {
        eprintln!("Immersion update installation: {error}");
    }
    // Never loop on a failed installer; a future background check can retry.
    if let Ok(root) = root() {
        let _ = fs::remove_dir_all(root.join("pending"));
    }
    if let Ok(exe) = installed_exe() {
        let mut command = Command::new(exe);
        command
            .arg("--immersion-relaunch")
            .arg(std::process::id().to_string())
            .arg("--immersion-skip-update");
        if args.iter().any(|a| a == "--autostart") {
            command.arg("--autostart");
        }
        if let Err(error) = command.spawn() {
            eprintln!("Immersion updater could not relaunch the app: {error}");
        }
    }
    single_instance::cleanup();
    true
}

fn apply() -> Result<()> {
    let pending = root()?.join("pending");
    staged(&pending)?;
    let exe = installed_exe()?;
    let dir = exe.parent().ok_or("no installation directory")?;
    // NSIS requires /D to be the final, unquoted parameter, even with spaces.
    let status = Command::new(pending.join("installer.exe"))
        .arg("/S")
        .raw_arg(format!("/D={}", dir.display()))
        .creation_flags(CREATE_NO_WINDOW)
        .status()?;
    if !exe.is_file() && dir.join("Immersion.previous.exe").is_file() {
        fs::rename(dir.join("Immersion.previous.exe"), &exe)?;
    }
    if !status.success() {
        return Err(format!("installer exited with {status}").into());
    }
    Ok(())
}
