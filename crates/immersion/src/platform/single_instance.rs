//! Single-instance guard (SingleInstanceGuard.cpp port).
//!
//! Uses a Unix domain socket at the same path QLocalServer would use
//! ($TMPDIR/musit-immersion-v1), so the Rust and Qt builds also exclude
//! each other. The secondary instance sends "raise" and exits; the primary
//! forwards raise requests through a channel.

use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};

const SERVER_NAME: &str = "musit-immersion-v1";
const RAISE_COMMAND: &[u8] = b"raise";

fn socket_path() -> PathBuf {
    std::env::temp_dir().join(SERVER_NAME)
}

pub enum InstanceGuard {
    /// This process owns the socket; raise requests arrive on the receiver.
    Primary(Receiver<()>),
    /// Another instance is running and has been asked to raise its window.
    Secondary,
}

pub fn acquire() -> InstanceGuard {
    #[cfg(unix)]
    {
        use std::os::unix::net::{UnixListener, UnixStream};

        let path = socket_path();
        match UnixListener::bind(&path) {
            Ok(listener) => return primary(listener),
            Err(_) => {
                // Address in use or stale socket file: probe for a live peer.
                if let Ok(mut stream) = UnixStream::connect(&path) {
                    let _ = stream.write_all(RAISE_COMMAND);
                    let _ = stream.flush();
                    return InstanceGuard::Secondary;
                }
                // Stale socket: remove and retry.
                let _ = std::fs::remove_file(&path);
                match UnixListener::bind(&path) {
                    Ok(listener) => return primary(listener),
                    Err(_) => {
                        // Can't own the socket; still run (matches Qt fallback).
                        let (_tx, rx) = channel();
                        return InstanceGuard::Primary(rx);
                    }
                }
            }
        }

        #[cfg(unix)]
        fn primary(listener: std::os::unix::net::UnixListener) -> InstanceGuard {
            let (tx, rx): (Sender<()>, Receiver<()>) = channel();
            std::thread::Builder::new()
                .name("single-instance".to_string())
                .spawn(move || {
                    for stream in listener.incoming() {
                        let Ok(mut stream) = stream else { continue };
                        let mut buffer = [0u8; 64];
                        if let Ok(read) = stream.read(&mut buffer) {
                            if buffer[..read]
                                .windows(RAISE_COMMAND.len())
                                .any(|w| w == RAISE_COMMAND)
                            {
                                let _ = tx.send(());
                            }
                        }
                    }
                })
                .ok();
            InstanceGuard::Primary(rx)
        }
    }

    #[cfg(not(unix))]
    {
        let (_tx, rx) = channel();
        InstanceGuard::Primary(rx)
    }
}

/// Remove the socket file on clean shutdown (best-effort).
pub fn cleanup() {
    #[cfg(unix)]
    {
        let _ = std::fs::remove_file(socket_path());
    }
}
