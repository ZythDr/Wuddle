use crate::Message;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    Info,
    Success,
    Warn,
    Error,
}

pub const TOAST_ANIMATION_TICKS: u8 = 11;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastAnimation {
    Entering(u8),
    Visible,
    Exiting(u8),
}

impl ToastAnimation {
    pub fn visibility(self) -> f32 {
        let smoothstep = |value: f32| value * value * (3.0 - 2.0 * value);
        match self {
            Self::Entering(tick) => {
                smoothstep((tick as f32 / TOAST_ANIMATION_TICKS as f32).clamp(0.0, 1.0))
            }
            Self::Visible => 1.0,
            Self::Exiting(tick) => {
                1.0
                    - smoothstep(
                        (tick as f32 / TOAST_ANIMATION_TICKS as f32).clamp(0.0, 1.0),
                    )
            }
        }
    }

    pub fn is_animating(self) -> bool {
        !matches!(self, Self::Visible)
    }
}

#[derive(Debug, Clone)]
pub struct Toast {
    pub id: usize,
    pub message: String,
    pub kind: ToastKind,
    /// Remaining ticks before auto-dismiss (one tick = 80ms spinner period).
    pub ttl: usize,
    /// Optional message to fire when the toast body is clicked.
    pub on_click: Option<Message>,
    pub animation: ToastAnimation,
}

#[cfg(test)]
mod tests {
    use super::{ToastAnimation, TOAST_ANIMATION_TICKS};

    #[test]
    fn toast_visibility_covers_enter_visible_and_exit_endpoints() {
        assert_eq!(ToastAnimation::Entering(0).visibility(), 0.0);
        assert_eq!(
            ToastAnimation::Entering(TOAST_ANIMATION_TICKS).visibility(),
            1.0
        );
        assert_eq!(ToastAnimation::Visible.visibility(), 1.0);
        assert_eq!(ToastAnimation::Exiting(0).visibility(), 1.0);
        assert_eq!(
            ToastAnimation::Exiting(TOAST_ANIMATION_TICKS).visibility(),
            0.0
        );
    }
}
