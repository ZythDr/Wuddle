use crate::Message;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    Info,
    Success,
    Warn,
    Error,
}

pub const TOAST_ANIMATION_TICKS: u8 = 11;
pub const TOAST_FRAME_MILLIS: u64 = 16;
pub const TOAST_DEFAULT_TICKS: usize = 5_000 / TOAST_FRAME_MILLIS as usize;
pub const TOAST_EXTENDED_TICKS: usize = 8_000 / TOAST_FRAME_MILLIS as usize;

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
                1.0 - smoothstep((tick as f32 / TOAST_ANIMATION_TICKS as f32).clamp(0.0, 1.0))
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
    /// Full readable lifetime, used to reset the timer when hovered.
    pub initial_ttl: usize,
    /// Hovering pauses the timer after resetting it to its full lifetime.
    pub hovered: bool,
    /// Optional message to fire when the toast body is clicked.
    pub on_click: Option<Message>,
    pub animation: ToastAnimation,
}

impl Toast {
    pub fn set_hovered(&mut self, hovered: bool) {
        self.hovered = hovered;
        if hovered && !matches!(self.animation, ToastAnimation::Exiting(_)) {
            self.ttl = self.initial_ttl;
        }
    }

    /// Advances the readable lifetime and returns true when it has expired.
    pub fn tick_lifetime(&mut self) -> bool {
        if matches!(self.animation, ToastAnimation::Visible) && !self.hovered {
            self.ttl = self.ttl.saturating_sub(1);
            return self.ttl == 0;
        }
        false
    }

    pub fn lifetime_remaining(&self) -> f32 {
        if self.initial_ttl == 0 {
            0.0
        } else {
            self.ttl as f32 / self.initial_ttl as f32
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Toast, ToastAnimation, ToastKind, TOAST_ANIMATION_TICKS};

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

    #[test]
    fn hovering_resets_and_pauses_the_readable_lifetime() {
        let mut toast = Toast {
            id: 1,
            message: String::from("Long notification"),
            kind: ToastKind::Warn,
            ttl: 20,
            initial_ttl: 100,
            hovered: false,
            on_click: None,
            animation: ToastAnimation::Visible,
        };

        toast.set_hovered(true);
        assert_eq!(toast.ttl, 100);
        assert!(!toast.tick_lifetime());
        assert_eq!(toast.ttl, 100);

        toast.set_hovered(false);
        assert!(!toast.tick_lifetime());
        assert_eq!(toast.ttl, 99);
    }
}
