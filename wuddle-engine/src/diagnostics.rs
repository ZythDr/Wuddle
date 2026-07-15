//! Privacy-safe diagnostic events emitted by the engine.
//!
//! The engine never owns a log file. Frontends may install a sink and decide
//! how verbose events are retained. Event payloads must describe operations,
//! counts, identifiers, and decisions only; never pass credentials, command
//! arguments, request headers, or raw user configuration here.

use std::sync::{Arc, OnceLock, RwLock};
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticLevel {
    Debug,
    Trace,
}

pub type DiagnosticSink = Arc<dyn Fn(DiagnosticLevel, &'static str, &str) + Send + Sync + 'static>;

fn sink_slot() -> &'static RwLock<Option<DiagnosticSink>> {
    static SINK: OnceLock<RwLock<Option<DiagnosticSink>>> = OnceLock::new();
    SINK.get_or_init(|| RwLock::new(None))
}

pub fn set_sink(sink: Option<DiagnosticSink>) {
    if let Ok(mut slot) = sink_slot().write() {
        *slot = sink;
    }
}

pub fn emit(level: DiagnosticLevel, target: &'static str, message: impl AsRef<str>) {
    let sink = sink_slot().read().ok().and_then(|slot| slot.clone());
    if let Some(sink) = sink {
        sink(level, target, message.as_ref());
    }
}

/// Records safe start/finish timing for an engine operation.
pub struct OperationGuard {
    name: &'static str,
    started: Instant,
}

impl OperationGuard {
    pub fn new(name: &'static str) -> Self {
        emit(DiagnosticLevel::Debug, "engine", format!("{name}: started"));
        Self {
            name,
            started: Instant::now(),
        }
    }
}

impl Drop for OperationGuard {
    fn drop(&mut self) {
        emit(
            DiagnosticLevel::Debug,
            "engine",
            format!(
                "{}: finished in {} ms",
                self.name,
                self.started.elapsed().as_millis()
            ),
        );
    }
}
