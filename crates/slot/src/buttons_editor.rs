//! Waiting for a key press, for the Buttons pages of the menu.
//!
//! The menu itself is drawn by `slot-ui`; this is only the little bit of state
//! that says "the player pressed A on the A row, listen for the next key". The
//! physical key is caught in [`crate::input::keymap`], so nothing here touches
//! the input device.

use slot_input::Btn;

use crate::input::keymap;

/// How long a row waits for a key before giving up.
pub const CAPTURE_MS: f64 = 6000.0;

/// What happened to the row that was waiting.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Captured {
    /// A key was pressed for this button.
    Key(Btn, u16),
    /// Nothing was pressed in time.
    TimedOut,
}

#[derive(Default)]
pub struct Capture {
    /// The button being rebound, and when the wait ends (clock milliseconds).
    waiting: Option<(Btn, f64)>,
    rev: u32,
}

impl Capture {
    /// Changes whenever the picture would change.
    pub fn rev(&self) -> u32 {
        self.rev
    }

    pub fn waiting(&self) -> Option<Btn> {
        self.waiting.map(|(btn, _)| btn)
    }

    /// Start waiting for a key for `btn`.
    pub fn start(&mut self, btn: Btn, now: f64) {
        keymap::capture_arm();
        self.waiting = Some((btn, now + CAPTURE_MS));
        self.rev = self.rev.wrapping_add(1);
    }

    /// Force a redraw of the key names (after a reset from elsewhere).
    pub fn bump(&mut self) {
        self.rev = self.rev.wrapping_add(1);
    }

    /// Stop waiting, if it was.
    pub fn cancel(&mut self) {
        if self.waiting.take().is_some() {
            keymap::capture_cancel();
            self.rev = self.rev.wrapping_add(1);
        }
    }

    /// Poll for the key or the timeout. Call every frame.
    pub fn poll(&mut self, now: f64) -> Option<Captured> {
        let (btn, deadline) = self.waiting?;
        if let Some(code) = keymap::capture_take() {
            self.waiting = None;
            self.rev = self.rev.wrapping_add(1);
            return Some(Captured::Key(btn, code));
        }
        if now >= deadline {
            keymap::capture_cancel();
            self.waiting = None;
            self.rev = self.rev.wrapping_add(1);
            return Some(Captured::TimedOut);
        }
        None
    }
}
