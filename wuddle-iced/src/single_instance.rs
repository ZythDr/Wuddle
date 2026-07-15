//! Single-instance coordination for the native Wuddle frontend.
//!
//! The primary process owns a marker in Wuddle's data directory and listens
//! only on a loopback TCP port for a `focus` notification. No user data or
//! commands are transported over this channel.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

const LOCK_FILE_NAME: &str = "wuddle-single-instance";
const CONNECT_TIMEOUT: Duration = Duration::from_millis(250);
const STARTUP_RETRIES: usize = 10;
const RETRY_DELAY: Duration = Duration::from_millis(50);

static PRIMARY_INSTANCE_ACTIVE: AtomicBool = AtomicBool::new(false);
static FOCUS_REQUESTED: AtomicBool = AtomicBool::new(false);

pub enum AcquireResult {
    Primary(SingleInstanceGuard),
    ExistingInstanceActivated,
}

/// Holds the primary-instance marker and activation listener for this process.
pub struct SingleInstanceGuard {
    lock_path: PathBuf,
    lock_file: Option<File>,
    shutdown: Arc<AtomicBool>,
    listener_thread: Option<JoinHandle<()>>,
}

/// Attempt to become Wuddle's primary process. If another Wuddle is already
/// listening, it is asked to focus its existing window instead.
pub fn acquire() -> Result<AcquireResult, String> {
    let lock_path = crate::settings::app_dir()?.join(LOCK_FILE_NAME);

    match create_primary(&lock_path) {
        Ok(guard) => Ok(AcquireResult::Primary(guard)),
        Err(CreatePrimaryError::AlreadyExists) => {
            for _ in 0..STARTUP_RETRIES {
                if activate_existing(&lock_path) {
                    return Ok(AcquireResult::ExistingInstanceActivated);
                }
                thread::sleep(RETRY_DELAY);
            }

            // A crashed process can leave the marker behind. After giving a
            // concurrently-starting Wuddle time to publish its port, recover
            // the stale marker and attempt to become the primary process.
            if let Err(error) = fs::remove_file(&lock_path) {
                if error.kind() != std::io::ErrorKind::NotFound {
                    return Err(format!(
                        "Could not clear stale single-instance marker {}: {error}",
                        lock_path.display()
                    ));
                }
            }
            create_primary(&lock_path)
                .map(AcquireResult::Primary)
                .map_err(|error| match error {
                    CreatePrimaryError::AlreadyExists => {
                        "Another Wuddle instance is starting; please try again.".to_string()
                    }
                    CreatePrimaryError::Other(error) => error,
                })
        }
        Err(CreatePrimaryError::Other(error)) => Err(error),
    }
}

/// Returns whether this process owns the single-instance listener.
pub fn is_primary_instance() -> bool {
    PRIMARY_INSTANCE_ACTIVE.load(Ordering::Relaxed)
}

/// Consumes a focus request received from a later Wuddle invocation.
pub fn take_focus_request() -> bool {
    FOCUS_REQUESTED.swap(false, Ordering::AcqRel)
}

#[derive(Debug)]
enum CreatePrimaryError {
    AlreadyExists,
    Other(String),
}

fn create_primary(lock_path: &Path) -> Result<SingleInstanceGuard, CreatePrimaryError> {
    let mut lock_file = match OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(lock_path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            return Err(CreatePrimaryError::AlreadyExists)
        }
        Err(error) => {
            return Err(CreatePrimaryError::Other(format!(
                "Could not create single-instance marker {}: {error}",
                lock_path.display()
            )))
        }
    };

    let listener = match TcpListener::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0)) {
        Ok(listener) => listener,
        Err(error) => {
            drop(lock_file);
            let _ = fs::remove_file(lock_path);
            return Err(CreatePrimaryError::Other(format!(
                "Could not start the single-instance listener: {error}"
            )));
        }
    };
    let port = match listener.local_addr() {
        Ok(address) => address.port(),
        Err(error) => {
            drop(lock_file);
            let _ = fs::remove_file(lock_path);
            return Err(CreatePrimaryError::Other(format!(
                "Could not read the single-instance listener address: {error}"
            )));
        }
    };

    if let Err(error) = writeln!(lock_file, "{port}").and_then(|()| lock_file.sync_all()) {
        drop(lock_file);
        let _ = fs::remove_file(lock_path);
        return Err(CreatePrimaryError::Other(format!(
            "Could not publish the single-instance listener address: {error}"
        )));
    }
    if let Err(error) = listener.set_nonblocking(true) {
        drop(lock_file);
        let _ = fs::remove_file(lock_path);
        return Err(CreatePrimaryError::Other(format!(
            "Could not configure the single-instance listener: {error}"
        )));
    }

    FOCUS_REQUESTED.store(false, Ordering::Release);
    PRIMARY_INSTANCE_ACTIVE.store(true, Ordering::Release);
    let shutdown = Arc::new(AtomicBool::new(false));
    let listener_shutdown = Arc::clone(&shutdown);
    let listener_thread = match thread::Builder::new()
        .name("wuddle-single-instance".to_string())
        .spawn(move || run_listener(listener, listener_shutdown))
    {
        Ok(listener_thread) => listener_thread,
        Err(error) => {
            PRIMARY_INSTANCE_ACTIVE.store(false, Ordering::Release);
            drop(lock_file);
            let _ = fs::remove_file(lock_path);
            return Err(CreatePrimaryError::Other(format!(
                "Could not start the single-instance listener thread: {error}"
            )));
        }
    };

    Ok(SingleInstanceGuard {
        lock_path: lock_path.to_path_buf(),
        lock_file: Some(lock_file),
        shutdown,
        listener_thread: Some(listener_thread),
    })
}

fn activate_existing(lock_path: &Path) -> bool {
    let Some(port) = fs::read_to_string(lock_path)
        .ok()
        .and_then(|contents| contents.trim().parse::<u16>().ok())
    else {
        return false;
    };
    let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
    let Ok(mut stream) = TcpStream::connect_timeout(&address, CONNECT_TIMEOUT) else {
        return false;
    };
    let _ = stream.set_write_timeout(Some(CONNECT_TIMEOUT));
    stream.write_all(b"focus\n").is_ok()
}

fn run_listener(listener: TcpListener, shutdown: Arc<AtomicBool>) {
    let mut buffer = [0u8; 16];
    while !shutdown.load(Ordering::Acquire) {
        match listener.accept() {
            Ok((mut stream, _)) => {
                let _ = stream.set_read_timeout(Some(CONNECT_TIMEOUT));
                if stream.read(&mut buffer).is_ok() {
                    FOCUS_REQUESTED.store(true, Ordering::Release);
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(30));
            }
            Err(_) => break,
        }
    }
}

impl Drop for SingleInstanceGuard {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Release);
        if let Some(listener_thread) = self.listener_thread.take() {
            let _ = listener_thread.join();
        }
        // Closing this first matters on Windows, where an open handle can keep
        // the marker from being removed.
        self.lock_file.take();
        let _ = fs::remove_file(&self.lock_path);
        PRIMARY_INSTANCE_ACTIVE.store(false, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn activation_reaches_primary_and_cleanup_removes_marker() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("wuddle-single-instance-{unique}"));
        fs::create_dir_all(&dir).unwrap();
        let lock_path = dir.join(LOCK_FILE_NAME);

        let guard = create_primary(&lock_path).unwrap();
        assert!(activate_existing(&lock_path));
        let activated = (0..20).any(|_| {
            if take_focus_request() {
                true
            } else {
                thread::sleep(Duration::from_millis(30));
                false
            }
        });
        assert!(activated);

        drop(guard);
        assert!(!lock_path.exists());
        fs::remove_dir_all(dir).unwrap();
    }
}
