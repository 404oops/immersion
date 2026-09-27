//! Single-instance guard.
//!
//! On Unix this uses a Unix domain socket at `$TMPDIR/musit-immersion-v1`.
//! On other platforms (Windows) it uses an exclusive OS file lock plus a TCP
//! loopback listener whose port is written next to the lock file. The
//! secondary instance sends "raise" and exits; the primary forwards raise
//! requests through a channel.

use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Duration;

const SERVER_NAME: &str = "musit-immersion-v1";
const RAISE_COMMAND: &[u8] = b"raise";
const PEER_READ_TIMEOUT: Duration = Duration::from_millis(500);

/// Unix-socket endpoint path. Compiled on all platforms so every build
/// type-checks it; only Unix targets call it.
#[cfg_attr(not(unix), allow(dead_code))]
fn socket_path() -> PathBuf {
    #[cfg(target_os = "linux")]
    if let Some(runtime_dir) = std::env::var_os("XDG_RUNTIME_DIR") {
        let app_dir = PathBuf::from(runtime_dir).join("immersion");
        if std::fs::create_dir_all(&app_dir).is_ok() {
            return app_dir.join(SERVER_NAME);
        }
    }
    std::env::temp_dir().join(SERVER_NAME)
}

fn lock_path() -> PathBuf {
    std::env::temp_dir().join(format!("{SERVER_NAME}.lock"))
}

fn port_path() -> PathBuf {
    std::env::temp_dir().join(format!("{SERVER_NAME}.port"))
}

pub enum InstanceGuard {
    /// This process owns the socket; raise requests arrive on the receiver.
    Primary(Receiver<()>),
    /// Another instance is running and has been asked to raise its window.
    Secondary,
}

/// Runs as primary without a working raise channel (couldn't own the
/// endpoint): the app still launches rather than refusing to start.
fn degraded_primary() -> InstanceGuard {
    let (_tx, rx) = channel();
    InstanceGuard::Primary(rx)
}

fn is_raise(buffer: &[u8]) -> bool {
    buffer
        .windows(RAISE_COMMAND.len())
        .any(|window| window == RAISE_COMMAND)
}

pub fn acquire() -> InstanceGuard {
    #[cfg(unix)]
    {
        acquire_unix()
    }
    #[cfg(not(unix))]
    {
        acquire_tcp()
    }
}

#[cfg(unix)]
fn acquire_unix() -> InstanceGuard {
    use std::os::unix::net::{UnixListener, UnixStream};

    fn notify_primary(path: &std::path::Path) -> bool {
        if let Ok(mut stream) = UnixStream::connect(path) {
            let _ = stream.write_all(RAISE_COMMAND);
            let _ = stream.flush();
            return true;
        }
        false
    }

    fn primary(listener: UnixListener) -> InstanceGuard {
        let (tx, rx): (Sender<()>, Receiver<()>) = channel();
        std::thread::Builder::new()
            .name("single-instance".to_string())
            .spawn(move || {
                for stream in listener.incoming() {
                    let Ok(mut stream) = stream else { continue };
                    // A peer that connects and never writes must not stall
                    // raise handling for later launches.
                    let _ = stream.set_read_timeout(Some(PEER_READ_TIMEOUT));
                    let mut buffer = [0u8; 64];
                    if let Ok(read) = stream.read(&mut buffer)
                        && is_raise(&buffer[..read])
                    {
                        let _ = tx.send(());
                    }
                }
            })
            .ok();
        InstanceGuard::Primary(rx)
    }

    let path = socket_path();
    match UnixListener::bind(&path) {
        Ok(listener) => primary(listener),
        Err(_) => {
            // Address in use or stale socket file: probe for a live peer.
            if notify_primary(&path) {
                return InstanceGuard::Secondary;
            }
            // Stale socket: remove and retry.
            let _ = std::fs::remove_file(&path);
            match UnixListener::bind(&path) {
                Ok(listener) => primary(listener),
                Err(_) => {
                    // Lost the rebind race: the winner is the primary now.
                    if notify_primary(&path) {
                        return InstanceGuard::Secondary;
                    }
                    degraded_primary()
                }
            }
        }
    }
}

/// TCP-loopback variant for platforms without Unix sockets. Compiled on all
/// platforms so every build type-checks it; only non-Unix targets call it.
#[cfg_attr(unix, allow(dead_code))]
fn acquire_tcp() -> InstanceGuard {
    use std::net::{TcpListener, TcpStream};

    let Ok(lock_file) = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(lock_path())
    else {
        return degraded_primary();
    };

    if lock_file.try_lock().is_ok() {
        // Primary: the OS releases the lock when the process dies, so a
        // crash can't leave the app permanently locked out.
        let Ok(listener) = TcpListener::bind(("127.0.0.1", 0)) else {
            return degraded_primary();
        };
        let Ok(local_addr) = listener.local_addr() else {
            return degraded_primary();
        };
        if std::fs::write(port_path(), local_addr.port().to_string()).is_err() {
            return degraded_primary();
        }
        // Hold the lock for the process lifetime.
        std::mem::forget(lock_file);

        let (tx, rx): (Sender<()>, Receiver<()>) = channel();
        std::thread::Builder::new()
            .name("single-instance".to_string())
            .spawn(move || {
                for stream in listener.incoming() {
                    let Ok(mut stream) = stream else { continue };
                    let _ = stream.set_read_timeout(Some(PEER_READ_TIMEOUT));
                    let mut buffer = [0u8; 64];
                    if let Ok(read) = stream.read(&mut buffer)
                        && is_raise(&buffer[..read])
                    {
                        let _ = tx.send(());
                    }
                }
            })
            .ok();
        InstanceGuard::Primary(rx)
    } else {
        // Secondary: ask the primary to raise itself.
        drop(lock_file);
        if let Some(port) = std::fs::read_to_string(port_path())
            .ok()
            .and_then(|text| text.trim().parse::<u16>().ok())
            && let Ok(mut stream) = TcpStream::connect(("127.0.0.1", port))
        {
            let _ = stream.write_all(RAISE_COMMAND);
            let _ = stream.flush();
        }
        InstanceGuard::Secondary
    }
}

/// Remove the endpoint files on clean shutdown (best-effort).
pub fn cleanup() {
    #[cfg(unix)]
    {
        let _ = std::fs::remove_file(socket_path());
    }
    #[cfg(not(unix))]
    {
        let _ = std::fs::remove_file(port_path());
    }
}
