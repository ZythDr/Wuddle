use crate::Message;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastKind { Info, Warn, Error }

#[derive(Debug, Clone)]
pub struct Toast {
    pub id: usize,
    pub message: String,
    pub kind: ToastKind,
    /// Remaining ticks before auto-dismiss (one tick = 80ms spinner period).
    pub ttl: usize,
    /// Optional message to fire when the toast body is clicked.
    pub on_click: Option<Message>,
}
