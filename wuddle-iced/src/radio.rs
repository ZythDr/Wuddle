//! Internet radio — Everlook Broadcasting Co.
//!
//! Runs the HTTP stream and rodio playback in a dedicated OS thread so the
//! tokio/Iced runtime is never blocked. A oneshot channel signals the result
//! back so the caller can use Task::perform to integrate with Iced updates.

use std::io::{self, Cursor, Read, Seek, SeekFrom};
use std::sync::mpsc::{self, SyncSender};
use std::time::Duration;

pub const STREAM_URL: &str = "https://radio.turtle-music.org/stream";

/// How many bytes to pre-buffer so symphonia can seek during format detection.
/// MP3 frame at 128 kbps ≈ 417 bytes; 3–5 frames is enough for detection.
/// 4 KiB ≈ 0.25 s of audio at 128 kbps — keeps startup fast.
const HEADER_BYTES: usize = 4_096;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum RadioCmd {
    Stop,
    Volume(f32),
    /// Gradually ramp volume from current to `target` over ~1 second.
    FadeIn(f32),
    /// Gradually ramp volume from current to 0 over ~1 second.
    FadeOut,
}

/// Returned on successful start. Clone-safe; all clones control the same session.
#[derive(Debug, Clone)]
pub struct RadioHandle {
    cmd_tx: SyncSender<RadioCmd>,
}

impl RadioHandle {
    pub fn stop(&self) {
        let _ = self.cmd_tx.try_send(RadioCmd::Stop);
    }
    pub fn set_volume(&self, v: f32) {
        let _ = self.cmd_tx.try_send(RadioCmd::Volume(v));
    }
    pub fn fade_in(&self, target: f32) {
        let _ = self.cmd_tx.try_send(RadioCmd::FadeIn(target));
    }
    pub fn fade_out(&self) {
        let _ = self.cmd_tx.try_send(RadioCmd::FadeOut);
    }
}

/// Blocking entry point. Call from a plain `std::thread` or via
/// `tokio::sync::oneshot` + `std::thread::spawn` (see main.rs).
///
/// Blocks until the stream is connected and the first audio chunk is decoded,
/// then returns a `RadioHandle` and leaves a background thread running.
pub fn start(volume: f32) -> Result<RadioHandle, String> {
    let (cmd_tx, cmd_rx) = mpsc::sync_channel::<RadioCmd>(16);
    let (ready_tx, ready_rx) = mpsc::sync_channel::<Result<(), String>>(1);

    std::thread::spawn(move || {
        // Audio device — must stay alive for the lifetime of playback.
        let (_stream, stream_handle) = match rodio::OutputStream::try_default() {
            Ok(s) => s,
            Err(e) => {
                let _ = ready_tx.send(Err(format!("Audio device: {e}")));
                return;
            }
        };

        // HTTP stream.
        let response = match reqwest::blocking::get(STREAM_URL) {
            Ok(r) => r,
            Err(e) => {
                let _ = ready_tx.send(Err(format!("Connect: {e}")));
                return;
            }
        };

        // Pre-buffer header for symphonia format detection.
        let reader = match RadioStreamReader::new(response) {
            Ok(r) => r,
            Err(e) => {
                let _ = ready_tx.send(Err(format!("Buffer: {e}")));
                return;
            }
        };

        // Decode.
        let decoder = match rodio::Decoder::new(reader) {
            Ok(d) => d,
            Err(e) => {
                let _ = ready_tx.send(Err(format!("Decode: {e}")));
                return;
            }
        };

        // Sink.
        let sink = match rodio::Sink::try_new(&stream_handle) {
            Ok(s) => s,
            Err(e) => {
                let _ = ready_tx.send(Err(format!("Sink: {e}")));
                return;
            }
        };
        sink.set_volume(volume);
        sink.append(decoder);

        // Signal caller that we're ready.
        let _ = ready_tx.send(Ok(()));

        // Command loop — keeps the thread (and _stream) alive.
        loop {
            match cmd_rx.recv_timeout(Duration::from_millis(20)) {
                Ok(RadioCmd::Stop) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                Ok(RadioCmd::Volume(v)) => sink.set_volume(v),
                Ok(RadioCmd::FadeIn(target)) => {
                    let start = sink.volume();
                    for i in 1..=50u32 {
                        sink.set_volume(start + (target - start) * (i as f32 / 50.0));
                        std::thread::sleep(Duration::from_millis(20));
                        // Allow Stop to interrupt the fade immediately
                        match cmd_rx.try_recv() {
                            Ok(RadioCmd::Stop) => { sink.stop(); return; }
                            _ => {}
                        }
                    }
                    sink.set_volume(target);
                }
                Ok(RadioCmd::FadeOut) => {
                    let start = sink.volume();
                    for i in 1..=50u32 {
                        sink.set_volume(start * (1.0 - i as f32 / 50.0));
                        std::thread::sleep(Duration::from_millis(20));
                        match cmd_rx.try_recv() {
                            Ok(RadioCmd::Stop) => { sink.stop(); return; }
                            _ => {}
                        }
                    }
                    sink.set_volume(0.0);
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if sink.empty() {
                        break; // Stream ended or decoder finished
                    }
                }
            }
        }

        sink.stop();
        // _stream drops here, releasing the audio device.
    });

    ready_rx
        .recv()
        .map_err(|_| "Radio thread exited before signalling ready".to_string())?
        .map(|_| RadioHandle { cmd_tx })
}

// ---------------------------------------------------------------------------
// RadioStreamReader — Read + Seek shim for an HTTP streaming response
// ---------------------------------------------------------------------------

/// Wraps a `reqwest::blocking::Response` so it satisfies `Read + Seek`.
///
/// The first `HEADER_BYTES` are pre-read into a buffer so that symphonia's
/// format prober can seek backwards during detection. Beyond that, only
/// forward sequential reads are supported; seek requests past the buffer
/// return `ErrorKind::Unsupported`, which symphonia handles gracefully for
/// stream sources.
struct RadioStreamReader {
    header: Cursor<Vec<u8>>,
    rest: reqwest::blocking::Response,
    pos: u64,
    header_len: u64,
}

impl RadioStreamReader {
    fn new(mut response: reqwest::blocking::Response) -> io::Result<Self> {
        let mut buf = vec![0u8; HEADER_BYTES];
        let mut filled = 0;
        while filled < buf.len() {
            match response.read(&mut buf[filled..]) {
                Ok(0) => break,
                Ok(n) => filled += n,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }
        buf.truncate(filled);
        let header_len = filled as u64;
        Ok(Self {
            header: Cursor::new(buf),
            rest: response,
            pos: 0,
            header_len,
        })
    }
}

impl Read for RadioStreamReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.pos < self.header_len {
            let n = self.header.read(buf)?;
            self.pos += n as u64;
            Ok(n)
        } else {
            let n = self.rest.read(buf)?;
            self.pos += n as u64;
            Ok(n)
        }
    }
}

impl Seek for RadioStreamReader {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        let target: u64 = match from {
            SeekFrom::Start(n) => n,
            SeekFrom::Current(0) => return Ok(self.pos),
            SeekFrom::Current(n) if n > 0 => self.pos.saturating_add(n as u64),
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "radio: backward or end-relative seek",
                ))
            }
        };

        if target <= self.header_len {
            self.header.seek(SeekFrom::Start(target))?;
            self.pos = target;
            Ok(self.pos)
        } else {
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "radio: forward seek past pre-buffered header",
            ))
        }
    }
}
