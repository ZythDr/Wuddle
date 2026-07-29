//! Single-instance coordination for the native Wuddle frontend.
//!
//! The primary process owns an OS-backed file lock and publishes a
//! PID/port/random-nonce marker in Wuddle's data directory. Activation uses a
//! nonce-authenticated request/acknowledgement over loopback. No user data or
//! general commands are transported over this channel.

use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const MARKER_FILE_NAME: &str = "wuddle-single-instance";
const OWNERSHIP_FILE_NAME: &str = "wuddle-single-instance.lock";
const PROTOCOL_VERSION: u8 = 1;
#[cfg(any(target_os = "linux", target_os = "windows"))]
pub(crate) const RESTART_PARENT_PID_ENV: &str = "WUDDLE_RESTART_PARENT_PID";
const CONNECT_TIMEOUT: Duration = Duration::from_millis(250);
const STARTUP_RETRIES: usize = 10;
const RETRY_DELAY: Duration = Duration::from_millis(50);
#[cfg(any(target_os = "linux", target_os = "windows"))]
const RESTART_HANDOFF_TIMEOUT: Duration = Duration::from_secs(5);
#[cfg(target_os = "linux")]
const RESTART_HANDOFF_POLL_INTERVAL: Duration = Duration::from_millis(25);

static PRIMARY_INSTANCE_ACTIVE: AtomicBool = AtomicBool::new(false);
static FOCUS_REQUESTED: AtomicBool = AtomicBool::new(false);

pub enum AcquireResult {
    Primary(SingleInstanceGuard),
    ExistingInstanceActivated,
}

/// During a self-update restart, wait for the process that launched this
/// replacement to exit before taking part in single-instance coordination.
///
/// Without this handoff, the replacement can see the old process, ask it to
/// focus, and exit just before the old process shuts down—leaving no Wuddle
/// process running after an otherwise successful update.
#[cfg(any(target_os = "linux", target_os = "windows"))]
pub(crate) fn wait_for_restart_parent() {
    let Some(raw_pid) = std::env::var_os(RESTART_PARENT_PID_ENV) else {
        return;
    };
    std::env::remove_var(RESTART_PARENT_PID_ENV);

    let Some(parent_pid) = raw_pid
        .to_str()
        .and_then(|value| value.parse::<u32>().ok())
        .filter(|pid| *pid != std::process::id())
    else {
        return;
    };

    wait_for_parent_exit(parent_pid);
}

#[cfg(target_os = "linux")]
fn wait_for_parent_exit(parent_pid: u32) {
    let parent_process = PathBuf::from(format!("/proc/{parent_pid}"));
    let deadline = Instant::now() + RESTART_HANDOFF_TIMEOUT;
    while parent_process.exists() && Instant::now() < deadline {
        thread::sleep(RESTART_HANDOFF_POLL_INTERVAL);
    }
}

#[cfg(target_os = "windows")]
fn wait_for_parent_exit(parent_pid: u32) {
    use windows_sys::Win32::Foundation::{CloseHandle, WAIT_TIMEOUT};
    use windows_sys::Win32::System::Threading::{
        OpenProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE,
    };

    let handle = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, parent_pid) };
    if handle == 0 {
        return;
    }
    let timeout_ms = RESTART_HANDOFF_TIMEOUT.as_millis() as u32;
    let wait_result = unsafe { WaitForSingleObject(handle, timeout_ms) };
    unsafe {
        CloseHandle(handle);
    }
    if wait_result == WAIT_TIMEOUT {
        eprintln!(
            "Wuddle restart handoff timed out while waiting for parent process {parent_pid}."
        );
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct InstanceMarker {
    version: u8,
    pid: u32,
    port: u16,
    nonce: String,
}

/// Holds the primary-instance marker and activation listener for this process.
pub struct SingleInstanceGuard {
    marker_path: PathBuf,
    ownership_file: Option<File>,
    shutdown: Arc<AtomicBool>,
    listener_thread: Option<JoinHandle<()>>,
}

/// Attempt to become Wuddle's primary process. If another Wuddle is already
/// listening, it is asked to focus its existing window instead.
pub fn acquire() -> Result<AcquireResult, String> {
    let app_dir = crate::settings::app_dir()?;
    let marker_path = app_dir.join(MARKER_FILE_NAME);
    let ownership_path = app_dir.join(OWNERSHIP_FILE_NAME);
    let ownership_file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&ownership_path)
        .map_err(|error| {
            format!(
                "Could not open Wuddle's single-instance ownership file {}: {error}",
                ownership_path.display()
            )
        })?;

    match ownership_file.try_lock_exclusive() {
        Ok(()) => create_primary(&marker_path, ownership_file).map(AcquireResult::Primary),
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
            for _ in 0..STARTUP_RETRIES {
                if activate_existing(&marker_path) {
                    return Ok(AcquireResult::ExistingInstanceActivated);
                }
                thread::sleep(RETRY_DELAY);
            }
            Err(
                "Another Wuddle process owns the application lock but did not acknowledge activation. Wuddle will not start a competing process."
                    .to_string(),
            )
        }
        Err(error) => Err(format!(
            "Could not acquire Wuddle's single-instance ownership lock: {error}"
        )),
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

fn create_primary(marker_path: &Path, ownership_file: File) -> Result<SingleInstanceGuard, String> {
    let listener = match TcpListener::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0)) {
        Ok(listener) => listener,
        Err(error) => {
            return Err(format!(
                "Could not start the single-instance listener: {error}"
            ));
        }
    };
    let port = match listener.local_addr() {
        Ok(address) => address.port(),
        Err(error) => {
            return Err(format!(
                "Could not read the single-instance listener address: {error}"
            ));
        }
    };
    let marker = InstanceMarker {
        version: PROTOCOL_VERSION,
        pid: std::process::id(),
        port,
        nonce: uuid::Uuid::new_v4().simple().to_string(),
    };
    let marker_bytes = serde_json::to_vec(&marker)
        .map_err(|error| format!("Could not encode single-instance marker: {error}"))?;
    if let Err(error) = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(marker_path)
        .and_then(|mut file| {
            file.write_all(&marker_bytes)?;
            file.sync_all()
        })
    {
        return Err(format!(
            "Could not publish the single-instance marker {}: {error}",
            marker_path.display()
        ));
    }
    if let Err(error) = listener.set_nonblocking(true) {
        let _ = fs::remove_file(marker_path);
        return Err(format!(
            "Could not configure the single-instance listener: {error}"
        ));
    }

    FOCUS_REQUESTED.store(false, Ordering::Release);
    PRIMARY_INSTANCE_ACTIVE.store(true, Ordering::Release);
    let shutdown = Arc::new(AtomicBool::new(false));
    let listener_shutdown = Arc::clone(&shutdown);
    let listener_nonce = marker.nonce;
    let listener_thread = match thread::Builder::new()
        .name("wuddle-single-instance".to_string())
        .spawn(move || run_listener(listener, listener_shutdown, listener_nonce))
    {
        Ok(listener_thread) => listener_thread,
        Err(error) => {
            PRIMARY_INSTANCE_ACTIVE.store(false, Ordering::Release);
            let _ = fs::remove_file(marker_path);
            return Err(format!(
                "Could not start the single-instance listener thread: {error}"
            ));
        }
    };

    Ok(SingleInstanceGuard {
        marker_path: marker_path.to_path_buf(),
        ownership_file: Some(ownership_file),
        shutdown,
        listener_thread: Some(listener_thread),
    })
}

fn activate_existing(marker_path: &Path) -> bool {
    let Some(marker) = fs::read(marker_path)
        .ok()
        .and_then(|contents| serde_json::from_slice::<InstanceMarker>(&contents).ok())
        .filter(|marker| {
            marker.version == PROTOCOL_VERSION && uuid::Uuid::parse_str(&marker.nonce).is_ok()
        })
    else {
        return false;
    };
    let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), marker.port);
    let Ok(mut stream) = TcpStream::connect_timeout(&address, CONNECT_TIMEOUT) else {
        return false;
    };
    let _ = stream.set_write_timeout(Some(CONNECT_TIMEOUT));
    let _ = stream.set_read_timeout(Some(CONNECT_TIMEOUT));
    let request = format!("focus {}\n", marker.nonce);
    if stream.write_all(request.as_bytes()).is_err() {
        return false;
    }
    let mut reply = [0u8; 96];
    let Ok(read) = stream.read(&mut reply) else {
        return false;
    };
    let expected = format!("ok {}", marker.nonce);
    std::str::from_utf8(&reply[..read]).is_ok_and(|reply| reply.trim() == expected)
}

fn run_listener(listener: TcpListener, shutdown: Arc<AtomicBool>, nonce: String) {
    let mut buffer = [0u8; 96];
    let expected = format!("focus {nonce}");
    let acknowledgement = format!("ok {nonce}\n");
    while !shutdown.load(Ordering::Acquire) {
        match listener.accept() {
            Ok((mut stream, _)) => {
                let _ = stream.set_read_timeout(Some(CONNECT_TIMEOUT));
                let _ = stream.set_write_timeout(Some(CONNECT_TIMEOUT));
                let valid = stream
                    .read(&mut buffer)
                    .ok()
                    .and_then(|read| std::str::from_utf8(&buffer[..read]).ok())
                    .is_some_and(|request| request.trim() == expected);
                if valid && stream.write_all(acknowledgement.as_bytes()).is_ok() {
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
        let _ = fs::remove_file(&self.marker_path);
        if let Some(ownership_file) = self.ownership_file.take() {
            let _ = FileExt::unlock(&ownership_file);
        }
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
        let marker_path = dir.join(MARKER_FILE_NAME);
        let ownership_path = dir.join(OWNERSHIP_FILE_NAME);
        let ownership_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&ownership_path)
            .unwrap();
        ownership_file.try_lock_exclusive().unwrap();

        let guard = create_primary(&marker_path, ownership_file).unwrap();
        assert!(activate_existing(&marker_path));
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
        assert!(!marker_path.exists());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn unrelated_loopback_listener_cannot_acknowledge_activation() {
        let dir = std::env::temp_dir().join(format!(
            "wuddle-single-instance-unrelated-{}",
            uuid::Uuid::new_v4().simple()
        ));
        fs::create_dir_all(&dir).unwrap();
        let marker_path = dir.join(MARKER_FILE_NAME);
        let listener =
            TcpListener::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0)).unwrap();
        let marker = InstanceMarker {
            version: PROTOCOL_VERSION,
            pid: u32::MAX,
            port: listener.local_addr().unwrap().port(),
            nonce: uuid::Uuid::new_v4().simple().to_string(),
        };
        fs::write(&marker_path, serde_json::to_vec(&marker).unwrap()).unwrap();
        let responder = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0u8; 96];
            let _ = stream.read(&mut request);
            let _ = stream.write_all(b"not-wuddle\n");
        });

        assert!(!activate_existing(&marker_path));
        responder.join().unwrap();
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn ownership_lock_prevents_a_competing_primary() {
        let dir = std::env::temp_dir().join(format!(
            "wuddle-single-instance-lock-{}",
            uuid::Uuid::new_v4().simple()
        ));
        fs::create_dir_all(&dir).unwrap();
        let ownership_path = dir.join(OWNERSHIP_FILE_NAME);
        let first = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&ownership_path)
            .unwrap();
        let second = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&ownership_path)
            .unwrap();

        first.try_lock_exclusive().unwrap();
        assert!(second.try_lock_exclusive().is_err());

        FileExt::unlock(&first).unwrap();
        drop(first);
        drop(second);
        fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(any(target_os = "linux", target_os = "windows"))]
    #[test]
    fn restart_handoff_returns_immediately_for_an_absent_parent() {
        std::env::set_var(RESTART_PARENT_PID_ENV, u32::MAX.to_string());
        let started = Instant::now();

        wait_for_restart_parent();

        assert!(started.elapsed() < Duration::from_secs(1));
        assert!(std::env::var_os(RESTART_PARENT_PID_ENV).is_none());
    }
}
