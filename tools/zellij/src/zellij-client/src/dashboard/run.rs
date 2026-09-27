//! The dashboard's terminal loop: owns the terminal while the dashboard is shown, feeds
//! keys and mouse events to the model, runs the session actions, and hands the terminal
//! back with an outcome for the caller (attach, create, quit, shut down).

use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use zellij_utils::{
    channels::{self, ChannelWithContext, RecvTimeoutError, SenderWithContext},
    data::{BareKey, KeyModifier, KeyWithModifier},
    input::{
        cast_termwiz_key,
        mouse::{MouseEvent, MouseEventType},
    },
    vendored::termwiz::input::InputEvent,
};

use super::memory::TerminalMemory;
use super::model::{Command, Dashboard, Key, Mouse};
use super::sessions;
use crate::{
    input_handler::from_termwiz, os_input_output::ClientOsApi, stdin_ansi_parser::StdinAnsiParser,
    stdin_handler::stdin_loop, InputInstruction,
};

/// How the dashboard ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DashboardOutcome {
    /// Attach to a running session, or resurrect a saved one.
    Open { name: String, full_history: bool },
    /// Create a new session with this (validated, unused) name.
    New { name: String },
    /// Close the dashboard; sessions keep running.
    Quit,
    /// Every session has been terminated; close the dashboard.
    Shutdown,
}

pub struct DashboardSetup {
    pub explicitly_disable_kitty_keyboard_protocol: bool,
    /// A message to show in the header, and whether it is an error.
    pub notice: Option<(String, bool)>,
}

const ENTER_DASHBOARD: &str = "\u{1b}[?1049h\u{1b}[?25l\u{1b}[?1000h\u{1b}[?1006h\u{1b}[H\u{1b}[2J";
const LEAVE_DASHBOARD: &str = "\u{1b}[?1006l\u{1b}[?1000l\u{1b}[0m\u{1b}[?25h\u{1b}[?1049l";
const REFRESH_INTERVAL: Duration = Duration::from_secs(2);
const SESSION_NAME_WHILE_IN_DASHBOARD: &str = "zellij dashboard";

fn write_all(os_input: &dyn ClientOsApi, text: &str) {
    let mut stdout = os_input.get_stdout_writer();
    let _ = stdout.write_all(text.as_bytes());
    let _ = stdout.flush();
}

/// Scrolls whatever the terminal shows into its scrollback, leaving the screen blank, by
/// feeding it as many line feeds as it has rows. Used once at startup so the shell's screen
/// is not lost when a session is drawn over it.
pub fn push_screen_into_history(os_input: &dyn ClientOsApi) {
    let rows = os_input.get_terminal_size().rows;
    let feeds: String = std::iter::repeat('\n').take(rows).collect();
    write_all(os_input, &format!("{}\u{1b}[H", feeds));
}

/// Restores the terminal when the process is about to exit: cooked mode, no mouse, no
/// special keyboard protocol, cursor shown.
pub fn restore_terminal(os_input: &dyn ClientOsApi) {
    write_all(
        os_input,
        "\u{1b}[<1u\u{1b}[?2031l\u{1b}[?1004l\u{1b}[?2004l\u{1b}[?1006l\u{1b}[?1000l\u{1b}[0m\u{1b}[?25h",
    );
    let _ = os_input.unset_raw_mode();
    os_input.restore_console_mode();
}

pub fn key_from(key: KeyWithModifier) -> Key {
    let ctrl = key.key_modifiers.contains(&KeyModifier::Ctrl);
    match key.bare_key {
        BareKey::Char(c) if ctrl => Key::Ctrl(c.to_ascii_lowercase()),
        BareKey::Char(c) => Key::Char(c),
        BareKey::Up => Key::Up,
        BareKey::Down => Key::Down,
        BareKey::Left => Key::Left,
        BareKey::Right => Key::Right,
        BareKey::PageUp => Key::PageUp,
        BareKey::PageDown => Key::PageDown,
        BareKey::Home => Key::Home,
        BareKey::End => Key::End,
        BareKey::Enter => Key::Enter,
        BareKey::Esc => Key::Esc,
        BareKey::Backspace => Key::Backspace,
        BareKey::Delete => Key::Delete,
        BareKey::Tab => Key::Tab,
        _ => Key::Other,
    }
}

pub fn mouse_from(event: &MouseEvent) -> Option<Mouse> {
    if event.wheel_up {
        Some(Mouse::WheelUp)
    } else if event.wheel_down {
        Some(Mouse::WheelDown)
    } else if event.event_type == MouseEventType::Press && event.left {
        Some(Mouse::Press {
            x: event.position.column.0,
            y: event.position.line.0.max(0) as usize,
        })
    } else {
        None
    }
}

struct ResizeWatcher {
    resized: Arc<AtomicBool>,
    #[cfg(unix)]
    handle: Option<signal_hook::iterator::Handle>,
}

impl ResizeWatcher {
    fn start() -> Self {
        let resized = Arc::new(AtomicBool::new(false));
        #[cfg(unix)]
        let handle = {
            match signal_hook::iterator::Signals::new([signal_hook::consts::signal::SIGWINCH]) {
                Ok(mut signals) => {
                    let handle = signals.handle();
                    let resized = resized.clone();
                    let _ = std::thread::Builder::new()
                        .name("dashboard_sigwinch".to_string())
                        .spawn(move || {
                            for _ in signals.forever() {
                                resized.store(true, Ordering::SeqCst);
                            }
                        });
                    Some(handle)
                },
                Err(_) => None,
            }
        };
        ResizeWatcher {
            resized,
            #[cfg(unix)]
            handle,
        }
    }
    fn take(&self) -> bool {
        self.resized.swap(false, Ordering::SeqCst)
    }
    fn stop(&self) {
        #[cfg(unix)]
        if let Some(handle) = &self.handle {
            handle.close();
        }
    }
}

/// Shows the dashboard until the user picks something. The terminal must be a tty. On
/// return the terminal is in raw mode, in its primary screen, with the mouse released.
pub fn run_dashboard(
    mut os_input: Box<dyn ClientOsApi>,
    setup: DashboardSetup,
    memory: &mut TerminalMemory,
) -> DashboardOutcome {
    // A session that was just left may still have a thread blocked on stdin; changing the
    // session name makes it hand its next read over to us (see ClientOsApi::read_from_stdin).
    os_input.update_session_name(SESSION_NAME_WHILE_IN_DASHBOARD.to_string());
    os_input.set_raw_mode();
    write_all(&*os_input, ENTER_DASHBOARD);

    let (send_input, receive_input): ChannelWithContext<InputInstruction> = channels::bounded(50);
    let send_input = SenderWithContext::new(send_input);
    let stdin_ansi_parser = Arc::new(Mutex::new(StdinAnsiParser::new()));
    let _stdin_thread = std::thread::Builder::new()
        .name("dashboard_stdin".to_string())
        .spawn({
            let os_input = os_input.clone();
            let disable_kitty = setup.explicitly_disable_kitty_keyboard_protocol;
            move || {
                stdin_loop(
                    os_input,
                    send_input,
                    stdin_ansi_parser,
                    disable_kitty,
                    false,
                    None,
                )
            }
        });
    let resize_watcher = ResizeWatcher::start();

    let size = os_input.get_terminal_size();
    let mut dashboard = Dashboard::new(size.cols, size.rows);
    if let Some((text, is_error)) = setup.notice {
        dashboard.notify(text, is_error);
    }
    let mut old_mouse_event = MouseEvent::new();
    let mut refresh_due = true;
    let mut last_refresh = Instant::now();

    let outcome = loop {
        if refresh_due || last_refresh.elapsed() >= REFRESH_INTERVAL {
            dashboard.set_rows(sessions::load_rows());
            dashboard.set_suggested_name(sessions::suggest_name());
            last_refresh = Instant::now();
            refresh_due = false;
        }
        if resize_watcher.take() {
            let size = os_input.get_terminal_size();
            dashboard.resize(size.cols, size.rows);
        }
        write_all(&*os_input, &dashboard.render());

        let command = match receive_input.recv_timeout(Duration::from_millis(500)) {
            Ok((InputInstruction::KeyEvent(InputEvent::Key(key_event), raw_bytes), _)) => {
                dashboard.handle_key(key_from(cast_termwiz_key(key_event, &raw_bytes, None)))
            },
            Ok((InputInstruction::KeyWithModifierEvent(key, _, _), _)) => {
                dashboard.handle_key(key_from(key))
            },
            Ok((InputInstruction::KeyEvent(InputEvent::Mouse(mouse_event), _), _)) => {
                let event = from_termwiz(&mut old_mouse_event, mouse_event);
                match mouse_from(&event) {
                    Some(mouse) => dashboard.handle_mouse(mouse),
                    None => Command::None,
                }
            },
            Ok((InputInstruction::MouseEvent(event), _)) => match mouse_from(&event) {
                Some(mouse) => dashboard.handle_mouse(mouse),
                None => Command::None,
            },
            Ok((InputInstruction::KeyEvent(InputEvent::Resized { cols, rows }, _), _)) => {
                dashboard.resize(cols, rows);
                Command::None
            },
            Ok((InputInstruction::Exit, _)) => Command::Quit,
            Ok(_) => Command::None,
            Err(RecvTimeoutError::Timeout) => Command::None,
            Err(RecvTimeoutError::Disconnected) => Command::Quit,
        };

        match command {
            Command::None => {},
            Command::Refresh => refresh_due = true,
            Command::Open { name, full_history } => {
                break DashboardOutcome::Open { name, full_history };
            },
            Command::New { name } => match sessions::check_new_name(&name) {
                Ok(()) => break DashboardOutcome::New { name },
                Err(error) => dashboard.notify(error, true),
            },
            Command::Terminate(name) => {
                dashboard.notify(format!("Saving and stopping '{}'…", name), false);
                write_all(&*os_input, &dashboard.render());
                match sessions::terminate(&name) {
                    Ok(()) => dashboard
                        .notify(format!("Terminated '{}'; its screen is saved", name), false),
                    Err(error) => dashboard.notify(format!("'{}': {}", name, error), true),
                }
                refresh_due = true;
            },
            Command::Delete(name) => {
                dashboard.notify(format!("Deleting '{}'…", name), false);
                write_all(&*os_input, &dashboard.render());
                match sessions::delete(&name) {
                    Ok(()) => {
                        memory.forget(&name);
                        dashboard.notify(format!("Deleted '{}'", name), false);
                    },
                    Err(error) => dashboard.notify(error, true),
                }
                refresh_due = true;
            },
            Command::Shutdown => {
                dashboard.notify("Saving and stopping every session…", false);
                write_all(&*os_input, &dashboard.render());
                let results = sessions::terminate_all();
                let failed: Vec<String> = results
                    .iter()
                    .filter_map(|(name, result)| {
                        result.as_ref().err().map(|e| format!("{}: {}", name, e))
                    })
                    .collect();
                if failed.is_empty() {
                    break DashboardOutcome::Shutdown;
                }
                dashboard.notify(format!("Not shut down: {}", failed.join("; ")), true);
                refresh_due = true;
            },
            Command::Quit => break DashboardOutcome::Quit,
        }
    };

    resize_watcher.stop();
    write_all(&*os_input, LEAVE_DASHBOARD);
    outcome
}
