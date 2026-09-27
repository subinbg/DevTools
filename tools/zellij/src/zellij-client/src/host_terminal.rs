//! Mirrors what the pane drawn straight into the host terminal asks for (DevTools fork,
//! `host_scrollback` option): when the program in the pane switches to the alternate
//! screen, so does the host terminal, and when it asks for mouse reporting, the host
//! terminal reports the mouse for as long as the program wants it. Without this, a session
//! drawn in the host terminal's primary screen would either scroll `less` and `vim` output
//! into the terminal's scrollback or keep the mouse away from programs that want it.

use std::io::Write;

use zellij_utils::ipc::HostScrollState;

const ENTER_ALTERNATE_SCREEN: &str = "\u{1b}[?1049h";
const EXIT_ALTERNATE_SCREEN: &str = "\u{1b}[?1049l";
const DISABLE_ALL_MOUSE: &str = "\u{1b}[?1006l\u{1b}[?1003l\u{1b}[?1002l\u{1b}[?1000l";

pub(crate) struct HostTerminalMirror {
    mirror_alternate_screen: bool,
    mirror_mouse: bool,
    alternate_screen: bool,
    mouse: (u8, bool),
}

impl HostTerminalMirror {
    /// `mirror_alternate_screen` is true when the session is drawn in the primary screen;
    /// `mirror_mouse` when zellij itself does not own the mouse (`mouse_mode false`).
    pub(crate) fn new(mirror_alternate_screen: bool, mirror_mouse: bool) -> Self {
        HostTerminalMirror {
            mirror_alternate_screen,
            mirror_mouse,
            alternate_screen: false,
            mouse: (0, false),
        }
    }

    /// The escape sequences that bring the host terminal in line with `state`, to be
    /// written before the render they belong to.
    pub(crate) fn transition(&mut self, state: &HostScrollState) -> String {
        let mut out = String::new();
        if self.mirror_alternate_screen && state.alternate_screen != self.alternate_screen {
            out.push_str(if state.alternate_screen {
                ENTER_ALTERNATE_SCREEN
            } else {
                EXIT_ALTERNATE_SCREEN
            });
            self.alternate_screen = state.alternate_screen;
        }
        if self.mirror_mouse {
            let wanted = (state.mouse_tracking.min(3), state.mouse_sgr);
            if wanted != self.mouse {
                out.push_str(&mouse_sequence(wanted));
                self.mouse = wanted;
            }
        }
        out
    }

    pub(crate) fn apply(&mut self, state: &HostScrollState, out: &mut dyn Write) {
        let sequence = self.transition(state);
        if !sequence.is_empty() {
            let _ = out.write_all(sequence.as_bytes());
        }
    }

    /// Undoes whatever is still mirrored, for when the client leaves the session.
    pub(crate) fn restore_sequence(&mut self) -> String {
        let mut out = String::new();
        if self.mouse.0 != 0 {
            out.push_str(DISABLE_ALL_MOUSE);
            self.mouse = (0, false);
        }
        if self.alternate_screen {
            out.push_str(EXIT_ALTERNATE_SCREEN);
            self.alternate_screen = false;
        }
        out
    }

    pub(crate) fn restore(&mut self, out: &mut dyn Write) {
        let sequence = self.restore_sequence();
        if !sequence.is_empty() {
            let _ = out.write_all(sequence.as_bytes());
            let _ = out.flush();
        }
    }
}

fn mouse_sequence((tracking, sgr): (u8, bool)) -> String {
    let mut out = String::from(DISABLE_ALL_MOUSE);
    if tracking >= 1 {
        out.push_str("\u{1b}[?1000h");
    }
    if tracking >= 2 {
        out.push_str("\u{1b}[?1002h");
    }
    if tracking >= 3 {
        out.push_str("\u{1b}[?1003h");
    }
    if tracking >= 1 && sgr {
        out.push_str("\u{1b}[?1006h");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(alternate_screen: bool, mouse_tracking: u8, mouse_sgr: bool) -> HostScrollState {
        HostScrollState {
            pane_id: 1,
            rows_scrolled: 0,
            viewport_rows: 24,
            alternate_screen,
            mouse_tracking,
            mouse_sgr,
        }
    }

    #[test]
    fn alternate_screen_is_mirrored_once_per_change() {
        let mut mirror = HostTerminalMirror::new(true, true);
        assert_eq!(mirror.transition(&state(false, 0, false)), "");
        assert_eq!(
            mirror.transition(&state(true, 0, false)),
            ENTER_ALTERNATE_SCREEN
        );
        assert_eq!(mirror.transition(&state(true, 0, false)), "");
        assert_eq!(
            mirror.transition(&state(false, 0, false)),
            EXIT_ALTERNATE_SCREEN
        );
    }

    #[test]
    fn mouse_requests_are_mirrored_and_restored() {
        let mut mirror = HostTerminalMirror::new(true, true);
        let seq = mirror.transition(&state(false, 2, true));
        assert!(seq.contains("\u{1b}[?1000h\u{1b}[?1002h\u{1b}[?1006h"));
        assert!(!seq.contains("?1003h"));
        assert_eq!(mirror.transition(&state(false, 2, true)), "");
        let seq = mirror.transition(&state(false, 0, false));
        assert_eq!(seq, DISABLE_ALL_MOUSE);
        mirror.transition(&state(true, 3, false));
        let restore = mirror.restore_sequence();
        assert!(restore.starts_with(DISABLE_ALL_MOUSE));
        assert!(restore.ends_with(EXIT_ALTERNATE_SCREEN));
        assert_eq!(mirror.restore_sequence(), "");
    }

    #[test]
    fn nothing_is_mirrored_when_disabled() {
        let mut mirror = HostTerminalMirror::new(false, false);
        assert_eq!(mirror.transition(&state(true, 3, true)), "");
        assert_eq!(mirror.restore_sequence(), "");
    }
}
