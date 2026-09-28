//! The dashboard's state, input handling and rendering. Everything here is pure: the
//! terminal loop in `run.rs` feeds it keys and mouse events and writes the frames it
//! renders, which keeps this part unit-testable.

use std::time::{Duration, Instant};

use super::canvas::{Canvas, Color, Rect, Style};

/// Below this many columns the details panel is not drawn.
pub const MIN_COLS_FOR_DETAILS: usize = 100;
/// How long a notice stays in the header.
const NOTICE_DURATION: Duration = Duration::from_secs(6);
/// The word that confirms a shutdown of every running session.
pub const SHUTDOWN_WORD: &str = "yes";

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct PaneRow {
    pub title: String,
    pub command: Option<String>,
    pub cwd: Option<String>,
    pub focused: bool,
    pub exited: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct TabRow {
    pub name: String,
    pub active: bool,
    pub panes: Vec<PaneRow>,
}

/// One session as listed in the dashboard.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct SessionRow {
    pub name: String,
    pub running: bool,
    pub clients: usize,
    /// Time since the session was created (running) or last saved (stopped).
    pub age: Duration,
    pub tabs: Vec<TabRow>,
}

impl SessionRow {
    pub fn pane_count(&self) -> usize {
        self.tabs.iter().map(|t| t.panes.len()).sum()
    }
    /// The pane the user would see first: the focused pane of the active tab.
    pub fn focused_pane(&self) -> Option<&PaneRow> {
        let tab = self
            .tabs
            .iter()
            .find(|t| t.active)
            .or_else(|| self.tabs.first())?;
        tab.panes
            .iter()
            .find(|p| p.focused)
            .or_else(|| tab.panes.first())
    }
    /// A one-line description: what runs in the focused pane and where.
    pub fn summary(&self) -> String {
        match self.focused_pane() {
            Some(pane) => {
                let what = pane
                    .command
                    .clone()
                    .filter(|c| !c.is_empty())
                    .unwrap_or_else(|| pane.title.clone());
                match &pane.cwd {
                    Some(cwd) if !cwd.is_empty() => format!("{}  {}", what, cwd),
                    _ => what,
                }
            },
            None => String::new(),
        }
    }
}

/// Keys the dashboard understands (already normalized from the terminal's key events).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Key {
    Up,
    Down,
    Left,
    Right,
    PageUp,
    PageDown,
    Home,
    End,
    Enter,
    Esc,
    Backspace,
    Delete,
    Tab,
    BackTab,
    Char(char),
    Ctrl(char),
    Other,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mouse {
    /// The left button was pressed at this cell (0-based column and row).
    Press {
        x: usize,
        y: usize,
    },
    WheelUp,
    WheelDown,
}

/// What the terminal loop should do after handling an event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    None,
    Refresh,
    /// Attach to (or resurrect) the session.
    Open {
        name: String,
    },
    New {
        name: String,
    },
    Terminate(String),
    Delete(String),
    Shutdown,
    Quit,
}

/// A single-line text input with a cursor.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct TextField {
    chars: Vec<char>,
    cursor: usize,
    max_len: usize,
}

impl TextField {
    pub fn new(text: &str, max_len: usize) -> Self {
        let chars: Vec<char> = text.chars().collect();
        let cursor = chars.len();
        TextField {
            chars,
            cursor,
            max_len,
        }
    }
    pub fn text(&self) -> String {
        self.chars.iter().collect()
    }
    pub fn cursor(&self) -> usize {
        self.cursor
    }
    pub fn is_empty(&self) -> bool {
        self.chars.is_empty()
    }
    /// Handles editing keys; returns true when the key was one of them.
    pub fn handle_key(&mut self, key: &Key, accept: impl Fn(char) -> bool) -> bool {
        match key {
            Key::Left => self.cursor = self.cursor.saturating_sub(1),
            Key::Right => self.cursor = (self.cursor + 1).min(self.chars.len()),
            Key::Home | Key::Ctrl('a') => self.cursor = 0,
            Key::End | Key::Ctrl('e') => self.cursor = self.chars.len(),
            Key::Backspace => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                    self.chars.remove(self.cursor);
                }
            },
            Key::Delete => {
                if self.cursor < self.chars.len() {
                    self.chars.remove(self.cursor);
                }
            },
            Key::Ctrl('u') => {
                self.chars.clear();
                self.cursor = 0;
            },
            Key::Ctrl('w') => {
                while self.cursor > 0 && self.chars[self.cursor - 1] == ' ' {
                    self.cursor -= 1;
                    self.chars.remove(self.cursor);
                }
                while self.cursor > 0 && self.chars[self.cursor - 1] != ' ' {
                    self.cursor -= 1;
                    self.chars.remove(self.cursor);
                }
            },
            Key::Char(c) if accept(*c) => {
                if self.chars.len() < self.max_len {
                    self.chars.insert(self.cursor, *c);
                    self.cursor += 1;
                }
            },
            _ => return false,
        }
        true
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Confirmable {
    Terminate,
    Delete,
}

/// The focused element of a dialog. Dialogs with a text field start on the field; the
/// buttons follow, in the order they are drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Focus {
    Field,
    /// Index of the focused button (0 = the primary action).
    Button(usize),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mode {
    List,
    Help,
    NewSession {
        field: TextField,
        focus: Focus,
    },
    Confirm {
        action: Confirmable,
        name: String,
        focus: Focus,
    },
    Shutdown {
        field: TextField,
        focus: Focus,
    },
}

/// Something a mouse click or a key can trigger.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Open,
    New,
    Terminate,
    Delete,
    Refresh,
    Help,
    ShutdownPrompt,
    Quit,
    /// The primary button of the current dialog (Yes, Create, Shut down).
    Primary,
    /// The secondary button (No, Cancel, Close).
    Cancel,
    FocusField,
}

#[derive(Clone, Debug)]
struct Notice {
    text: String,
    is_error: bool,
    shown_at: Instant,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Hit {
    Row(usize),
    Action(Action),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct HitRegion {
    rect: Rect,
    hit: Hit,
}

/// A line of the session list: a section label or a session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ListLine {
    Header(&'static str),
    Session(usize),
}

// ---- palette (the terminal's 16 colors, so the dashboard follows its theme) ----
const ACCENT: Color = Color::Ansi(4); // selection and focus background
const ACCENT_TEXT: Color = Color::Ansi(15);
const TEXT: Color = Color::Ansi(7);
const BRIGHT: Color = Color::Ansi(15);
const MUTED: Color = Color::Ansi(8);
const RUNNING: Color = Color::Ansi(10);
const KEY: Color = Color::Ansi(11);
const INFO: Color = Color::Ansi(14);
const WARN: Color = Color::Ansi(11);
const DANGER: Color = Color::Ansi(9);
const OK: Color = Color::Ansi(10);

pub struct Dashboard {
    pub rows: Vec<SessionRow>,
    pub selected: usize,
    scroll: usize,
    pub mode: Mode,
    notice: Option<Notice>,
    cols: usize,
    lines: usize,
    suggested_name: String,
    working_dir: String,
    hits: Vec<HitRegion>,
    list_rows_visible: usize,
}

impl Dashboard {
    pub fn new(cols: usize, lines: usize) -> Self {
        Dashboard {
            rows: vec![],
            selected: 0,
            scroll: 0,
            mode: Mode::List,
            notice: None,
            cols,
            lines,
            suggested_name: String::new(),
            working_dir: String::new(),
            hits: vec![],
            list_rows_visible: 1,
        }
    }

    /// The folder new sessions start in, shown in the new-session dialog.
    pub fn set_working_dir(&mut self, dir: String) {
        self.working_dir = dir;
    }

    pub fn resize(&mut self, cols: usize, lines: usize) {
        self.cols = cols;
        self.lines = lines;
    }

    pub fn set_suggested_name(&mut self, name: String) {
        self.suggested_name = name;
    }

    pub fn notify(&mut self, text: impl Into<String>, is_error: bool) {
        self.notice = Some(Notice {
            text: text.into(),
            is_error,
            shown_at: Instant::now(),
        });
    }

    pub fn selected_row(&self) -> Option<&SessionRow> {
        self.rows.get(self.selected)
    }

    /// Replaces the session list, keeping the selection on the same session when it is
    /// still listed.
    pub fn set_rows(&mut self, rows: Vec<SessionRow>) {
        let selected_name = self.rows.get(self.selected).map(|r| r.name.clone());
        self.rows = rows;
        if let Some(name) = selected_name {
            if let Some(idx) = self.rows.iter().position(|r| r.name == name) {
                self.selected = idx;
            }
        }
        self.clamp_selection();
    }

    /// The list as drawn: section labels and sessions.
    fn list_lines(&self) -> Vec<ListLine> {
        let mut lines = vec![];
        let running: Vec<usize> = (0..self.rows.len())
            .filter(|i| self.rows[*i].running)
            .collect();
        let saved: Vec<usize> = (0..self.rows.len())
            .filter(|i| !self.rows[*i].running)
            .collect();
        if !running.is_empty() {
            lines.push(ListLine::Header("Running"));
            lines.extend(running.into_iter().map(ListLine::Session));
        }
        if !saved.is_empty() {
            if !lines.is_empty() {
                lines.push(ListLine::Header(""));
            }
            lines.push(ListLine::Header("Saved"));
            lines.extend(saved.into_iter().map(ListLine::Session));
        }
        lines
    }

    fn line_of_session(lines: &[ListLine], idx: usize) -> usize {
        lines
            .iter()
            .position(|l| *l == ListLine::Session(idx))
            .unwrap_or(0)
    }

    fn clamp_selection(&mut self) {
        if self.rows.is_empty() {
            self.selected = 0;
            self.scroll = 0;
            return;
        }
        if self.selected >= self.rows.len() {
            self.selected = self.rows.len() - 1;
        }
        let lines = self.list_lines();
        let visible = self.list_rows_visible.max(1);
        let line = Self::line_of_session(&lines, self.selected);
        // keep the section label above the first session visible too
        let top = if line > 0 && matches!(lines[line - 1], ListLine::Header(_)) {
            line - 1
        } else {
            line
        };
        if top < self.scroll {
            self.scroll = top;
        } else if line >= self.scroll + visible {
            self.scroll = line + 1 - visible;
        }
        if self.scroll > 0 && self.scroll + visible > lines.len() {
            self.scroll = lines.len().saturating_sub(visible);
        }
    }

    fn move_selection(&mut self, delta: isize) {
        if self.rows.is_empty() {
            return;
        }
        let last = self.rows.len() as isize - 1;
        let target = (self.selected as isize + delta).clamp(0, last);
        self.selected = target as usize;
        self.clamp_selection();
    }

    // ------------------------------------------------------------------ input

    pub fn handle_key(&mut self, key: Key) -> Command {
        match self.mode.clone() {
            Mode::List => self.handle_list_key(key),
            Mode::Help => {
                self.mode = Mode::List;
                Command::None
            },
            Mode::NewSession { mut field, focus } => {
                match key {
                    Key::Esc | Key::Ctrl('c') | Key::Ctrl('q') => {
                        self.mode = Mode::List;
                        return Command::None;
                    },
                    Key::Enter => {
                        return match focus {
                            Focus::Field | Focus::Button(0) => self.create_session(&field),
                            Focus::Button(_) => {
                                self.mode = Mode::List;
                                Command::None
                            },
                        };
                    },
                    Key::Tab | Key::Down => {
                        let focus = match focus {
                            Focus::Field => Focus::Button(0),
                            Focus::Button(0) => Focus::Button(1),
                            Focus::Button(_) => Focus::Field,
                        };
                        self.mode = Mode::NewSession { field, focus };
                        return Command::None;
                    },
                    Key::BackTab | Key::Up => {
                        let focus = match focus {
                            Focus::Field => Focus::Button(1),
                            Focus::Button(0) => Focus::Field,
                            Focus::Button(_) => Focus::Button(0),
                        };
                        self.mode = Mode::NewSession { field, focus };
                        return Command::None;
                    },
                    _ => {},
                }
                match focus {
                    Focus::Field => {
                        field.handle_key(&key, is_session_name_char);
                        self.mode = Mode::NewSession { field, focus };
                    },
                    Focus::Button(b) => {
                        let focus = match key {
                            Key::Left | Key::Right => Focus::Button(1 - b.min(1)),
                            _ => focus,
                        };
                        self.mode = Mode::NewSession { field, focus };
                    },
                }
                Command::None
            },
            Mode::Confirm {
                action,
                name,
                focus,
            } => {
                let yes = match action {
                    Confirmable::Terminate => Command::Terminate(name.clone()),
                    Confirmable::Delete => Command::Delete(name.clone()),
                };
                let button = match focus {
                    Focus::Button(b) => b,
                    Focus::Field => 0,
                };
                match key {
                    Key::Char('y') | Key::Char('Y') => {
                        self.mode = Mode::List;
                        yes
                    },
                    Key::Char('n')
                    | Key::Char('N')
                    | Key::Esc
                    | Key::Ctrl('c')
                    | Key::Char('q') => {
                        self.mode = Mode::List;
                        Command::None
                    },
                    Key::Enter => {
                        self.mode = Mode::List;
                        if button == 0 {
                            yes
                        } else {
                            Command::None
                        }
                    },
                    Key::Left | Key::Right | Key::Tab | Key::BackTab | Key::Up | Key::Down => {
                        self.mode = Mode::Confirm {
                            action,
                            name,
                            focus: Focus::Button(1 - button.min(1)),
                        };
                        Command::None
                    },
                    _ => Command::None,
                }
            },
            Mode::Shutdown { mut field, focus } => {
                match key {
                    Key::Esc | Key::Ctrl('c') | Key::Ctrl('q') => {
                        self.mode = Mode::List;
                        return Command::None;
                    },
                    Key::Enter => {
                        return match focus {
                            Focus::Field | Focus::Button(0) => self.confirm_shutdown(&field),
                            Focus::Button(_) => {
                                self.mode = Mode::List;
                                Command::None
                            },
                        };
                    },
                    Key::Tab | Key::Down => {
                        let focus = match focus {
                            Focus::Field => Focus::Button(0),
                            Focus::Button(0) => Focus::Button(1),
                            Focus::Button(_) => Focus::Field,
                        };
                        self.mode = Mode::Shutdown { field, focus };
                        return Command::None;
                    },
                    Key::BackTab | Key::Up => {
                        let focus = match focus {
                            Focus::Field => Focus::Button(1),
                            Focus::Button(0) => Focus::Field,
                            Focus::Button(_) => Focus::Button(0),
                        };
                        self.mode = Mode::Shutdown { field, focus };
                        return Command::None;
                    },
                    _ => {},
                }
                match focus {
                    Focus::Field => {
                        field.handle_key(&key, |c| c.is_ascii_alphabetic());
                        self.mode = Mode::Shutdown { field, focus };
                    },
                    Focus::Button(b) => {
                        let focus = match key {
                            Key::Left | Key::Right => Focus::Button(1 - b.min(1)),
                            _ => focus,
                        };
                        self.mode = Mode::Shutdown { field, focus };
                    },
                }
                Command::None
            },
        }
    }

    fn create_session(&mut self, field: &TextField) -> Command {
        let name = field.text().trim().to_string();
        if name.is_empty() {
            self.notify("Enter a name", true);
            Command::None
        } else {
            Command::New { name }
        }
    }

    fn confirm_shutdown(&mut self, field: &TextField) -> Command {
        if field.text().trim().eq_ignore_ascii_case(SHUTDOWN_WORD) {
            self.mode = Mode::List;
            Command::Shutdown
        } else {
            self.notify(format!("Type {} to confirm", SHUTDOWN_WORD), true);
            self.mode = Mode::Shutdown {
                field: TextField::new("", 8),
                focus: Focus::Field,
            };
            Command::None
        }
    }

    fn handle_list_key(&mut self, key: Key) -> Command {
        match key {
            Key::Up | Key::Char('k') => self.move_selection(-1),
            Key::Down | Key::Char('j') => self.move_selection(1),
            Key::PageUp => self.move_selection(-(self.list_rows_visible as isize)),
            Key::PageDown => self.move_selection(self.list_rows_visible as isize),
            Key::Home | Key::Char('g') => self.move_selection(isize::MIN / 2),
            Key::End | Key::Char('G') => self.move_selection(isize::MAX / 2),
            Key::Enter | Key::Char('o') | Key::Char(' ') => return self.act(Action::Open),
            Key::Char('n') | Key::Char('c') => return self.act(Action::New),
            Key::Char('t') | Key::Char('x') => return self.act(Action::Terminate),
            Key::Char('d') | Key::Delete => return self.act(Action::Delete),
            Key::Char('r') | Key::Char('R') | Key::Ctrl('r') => return self.act(Action::Refresh),
            Key::Char('?') | Key::Char('h') => return self.act(Action::Help),
            Key::Char('Q') => return self.act(Action::ShutdownPrompt),
            Key::Char('q') | Key::Esc | Key::Ctrl('q') | Key::Ctrl('c') | Key::Ctrl('d') => {
                return self.act(Action::Quit);
            },
            _ => {},
        }
        Command::None
    }

    /// Runs an action from a key or a click.
    fn act(&mut self, action: Action) -> Command {
        match action {
            Action::Open => self.open_selected(),
            Action::New => {
                self.mode = Mode::NewSession {
                    field: TextField::new(&self.suggested_name, 48),
                    focus: Focus::Field,
                };
                Command::None
            },
            Action::Terminate => {
                match self.selected_row() {
                    Some(row) if row.running => {
                        self.mode = Mode::Confirm {
                            action: Confirmable::Terminate,
                            name: row.name.clone(),
                            focus: Focus::Button(0),
                        };
                    },
                    Some(row) => {
                        let msg = format!("{} is not running", row.name);
                        self.notify(msg, true);
                    },
                    None => {},
                }
                Command::None
            },
            Action::Delete => {
                if let Some(row) = self.selected_row() {
                    self.mode = Mode::Confirm {
                        action: Confirmable::Delete,
                        name: row.name.clone(),
                        focus: Focus::Button(1),
                    };
                }
                Command::None
            },
            Action::Refresh => Command::Refresh,
            Action::Help => {
                self.mode = Mode::Help;
                Command::None
            },
            Action::ShutdownPrompt => {
                self.mode = Mode::Shutdown {
                    field: TextField::new("", 8),
                    focus: Focus::Field,
                };
                Command::None
            },
            Action::Quit => Command::Quit,
            Action::Primary => match self.mode.clone() {
                Mode::NewSession { field, .. } => self.create_session(&field),
                Mode::Shutdown { field, .. } => self.confirm_shutdown(&field),
                Mode::Confirm { action, name, .. } => {
                    self.mode = Mode::List;
                    match action {
                        Confirmable::Terminate => Command::Terminate(name),
                        Confirmable::Delete => Command::Delete(name),
                    }
                },
                Mode::Help => {
                    self.mode = Mode::List;
                    Command::None
                },
                Mode::List => Command::None,
            },
            Action::Cancel => {
                self.mode = Mode::List;
                Command::None
            },
            Action::FocusField => {
                match self.mode.clone() {
                    Mode::NewSession { field, .. } => {
                        self.mode = Mode::NewSession {
                            field,
                            focus: Focus::Field,
                        }
                    },
                    Mode::Shutdown { field, .. } => {
                        self.mode = Mode::Shutdown {
                            field,
                            focus: Focus::Field,
                        }
                    },
                    _ => {},
                }
                Command::None
            },
        }
    }

    fn open_selected(&mut self) -> Command {
        match self.selected_row() {
            Some(row) => Command::Open {
                name: row.name.clone(),
            },
            None => self.act(Action::New),
        }
    }

    pub fn handle_mouse(&mut self, mouse: Mouse) -> Command {
        match mouse {
            Mouse::WheelUp => {
                if self.mode == Mode::List {
                    self.move_selection(-1);
                }
                Command::None
            },
            Mouse::WheelDown => {
                if self.mode == Mode::List {
                    self.move_selection(1);
                }
                Command::None
            },
            Mouse::Press { x, y } => {
                let hit = self
                    .hits
                    .iter()
                    .find(|region| region.rect.contains(x, y))
                    .map(|region| region.hit.clone());
                match hit {
                    Some(Hit::Row(idx)) => {
                        if self.mode != Mode::List {
                            return Command::None;
                        }
                        if idx == self.selected {
                            self.open_selected()
                        } else {
                            self.selected = idx;
                            self.clamp_selection();
                            Command::None
                        }
                    },
                    Some(Hit::Action(action)) => self.act(action),
                    None => {
                        // clicking outside a dialog closes it
                        if self.mode != Mode::List {
                            self.mode = Mode::List;
                        }
                        Command::None
                    },
                }
            },
        }
    }

    // ---------------------------------------------------------------- rendering

    /// Renders the whole screen. Also records where the session rows and buttons are, for
    /// the mouse.
    pub fn render(&mut self) -> String {
        let cols = self.cols.max(20);
        let lines = self.lines.max(6);
        let mut canvas = Canvas::new(cols, lines);
        self.hits.clear();

        self.render_header(&mut canvas);
        let body = Rect::new(0, 1, cols, lines.saturating_sub(2));
        let (list_rect, details_rect) = if cols >= MIN_COLS_FOR_DETAILS {
            let list_w = (cols * 58 / 100).max(40);
            (
                Rect::new(0, body.y, list_w, body.h),
                Some(Rect::new(list_w, body.y, cols - list_w, body.h)),
            )
        } else {
            (body, None)
        };
        self.render_list(&mut canvas, list_rect);
        if let Some(rect) = details_rect {
            self.render_details(&mut canvas, rect);
        }
        self.render_footer(&mut canvas);
        match self.mode.clone() {
            Mode::List => {},
            Mode::Help => self.render_help(&mut canvas, body),
            Mode::NewSession { field, focus } => {
                self.render_new_session(&mut canvas, body, &field, focus)
            },
            Mode::Confirm {
                action,
                name,
                focus,
            } => self.render_confirm(&mut canvas, body, action, &name, focus),
            Mode::Shutdown { field, focus } => {
                self.render_shutdown(&mut canvas, body, &field, focus)
            },
        }
        canvas.to_ansi()
    }

    fn render_header(&mut self, canvas: &mut Canvas) {
        let cols = canvas.cols();
        canvas.fill_row(0, ' ', Style::default());
        canvas.put(2, 0, "zellij", Style::default().fg(BRIGHT).bold());
        let notice = self
            .notice
            .clone()
            .filter(|n| n.shown_at.elapsed() < NOTICE_DURATION);
        let (text, style) = match notice {
            Some(notice) if notice.is_error => (notice.text, Style::default().fg(DANGER).bold()),
            Some(notice) => (notice.text, Style::default().fg(OK)),
            None => {
                let running = self.rows.iter().filter(|r| r.running).count();
                let saved = self.rows.len() - running;
                let text = match (running, saved) {
                    (0, 0) => String::new(),
                    (r, 0) => format!("{} running", r),
                    (0, s) => format!("{} saved", s),
                    (r, s) => format!("{} running · {} saved", r, s),
                };
                (text, Style::default().fg(MUTED))
            },
        };
        let width = text.chars().count();
        if width > 0 && cols > width + 2 {
            canvas.put(cols - width - 2, 0, &text, style);
        }
    }

    fn render_footer(&mut self, canvas: &mut Canvas) {
        let y = canvas.lines() - 1;
        canvas.fill_row(y, ' ', Style::default());
        let hints: Vec<(&str, &str, Option<Action>)> = match &self.mode {
            Mode::List => vec![
                ("↑↓", "move", None),
                ("⏎", "open", Some(Action::Open)),
                ("n", "new", Some(Action::New)),
                ("t", "terminate", Some(Action::Terminate)),
                ("d", "delete", Some(Action::Delete)),
                ("?", "help", Some(Action::Help)),
                ("q", "quit", Some(Action::Quit)),
                ("Q", "shut down all", Some(Action::ShutdownPrompt)),
            ],
            Mode::Help => vec![("esc", "close", Some(Action::Cancel))],
            Mode::NewSession { .. } => vec![
                ("tab", "next", None),
                ("⏎", "create", Some(Action::Primary)),
                ("esc", "cancel", Some(Action::Cancel)),
            ],
            Mode::Confirm { .. } => vec![
                ("←→", "choose", None),
                ("⏎", "confirm", None),
                ("esc", "cancel", Some(Action::Cancel)),
            ],
            Mode::Shutdown { .. } => vec![
                ("tab", "next", None),
                ("⏎", "confirm", Some(Action::Primary)),
                ("esc", "cancel", Some(Action::Cancel)),
            ],
        };
        let mut x = 2;
        let cols = canvas.cols();
        for (key, label, action) in hints {
            let width = key.chars().count() + 1 + label.chars().count();
            if x + width + 1 > cols {
                break;
            }
            canvas.put(x, y, key, Style::default().fg(KEY).bold());
            canvas.put(
                x + key.chars().count() + 1,
                y,
                label,
                Style::default().fg(MUTED),
            );
            if let Some(action) = action {
                self.hits.push(HitRegion {
                    rect: Rect::new(x, y, width, 1),
                    hit: Hit::Action(action),
                });
            }
            x += width + 3;
        }
    }

    fn render_list(&mut self, canvas: &mut Canvas, rect: Rect) {
        let frame_style = Style::default().fg(Color::Ansi(2));
        canvas.frame(
            rect,
            " Sessions ",
            frame_style,
            Style::default().fg(BRIGHT).bold(),
        );
        let inner = rect.inner();
        if inner.h == 0 || inner.w < 10 {
            return;
        }
        self.list_rows_visible = inner.h;
        self.clamp_selection();

        if self.rows.is_empty() {
            let text = "No sessions yet.";
            let x = inner.x + inner.w.saturating_sub(text.chars().count()) / 2;
            let y = inner.y + (inner.h / 3).min(inner.h.saturating_sub(1));
            canvas.put(x, y, text, Style::default().fg(MUTED));
            return;
        }

        let lines = self.list_lines();
        let name_w = self
            .rows
            .iter()
            .map(|r| r.name.chars().count())
            .max()
            .unwrap_or(8)
            .clamp(8, 28);
        for (line_idx, list_line) in lines.iter().skip(self.scroll).enumerate() {
            if line_idx >= inner.h {
                break;
            }
            let y = inner.y + line_idx;
            match list_line {
                ListLine::Header(label) => {
                    canvas.put(
                        inner.x + 2,
                        y,
                        &label.to_uppercase(),
                        Style::default().fg(MUTED).bold(),
                    );
                },
                ListLine::Session(row_idx) => {
                    let row = &self.rows[*row_idx];
                    let selected = *row_idx == self.selected;
                    let base = if selected {
                        Style::default().bg(ACCENT)
                    } else {
                        Style::default()
                    };
                    canvas.fill_row_range(y, inner.x, inner.w, ' ', base);

                    let (glyph, glyph_style) = if row.running {
                        ("●", base.fg(RUNNING))
                    } else {
                        ("○", base.fg(MUTED))
                    };
                    canvas.put(inner.x + 2, y, glyph, glyph_style);
                    let name_style = if selected {
                        base.fg(ACCENT_TEXT).bold()
                    } else if row.running {
                        base.fg(BRIGHT)
                    } else {
                        base.fg(TEXT)
                    };
                    canvas.put(inner.x + 4, y, &fit(&row.name, name_w), name_style);

                    let age = age_short(row.age);
                    let right = if row.running {
                        match row.clients {
                            0 => format!("{:>5}", age),
                            1 => format!("1 terminal  {:>5}", age),
                            n => format!("{} terminals  {:>5}", n, age),
                        }
                    } else {
                        format!("{:>5}", age)
                    };
                    let right_w = right.chars().count();
                    let summary_x = inner.x + 4 + name_w + 2;
                    if inner.x + inner.w > right_w + 2 {
                        let right_x = inner.x + inner.w - right_w - 2;
                        if right_x > summary_x + 4 {
                            let summary = fit(&row.summary(), right_x - summary_x - 2);
                            let summary_style = if selected {
                                base.fg(ACCENT_TEXT)
                            } else {
                                base.fg(MUTED)
                            };
                            canvas.put(summary_x, y, &summary, summary_style);
                        }
                        let right_style = if selected {
                            base.fg(ACCENT_TEXT)
                        } else {
                            base.fg(MUTED)
                        };
                        canvas.put(right_x, y, &right, right_style);
                    }
                    self.hits.push(HitRegion {
                        rect: Rect::new(inner.x, y, inner.w, 1),
                        hit: Hit::Row(*row_idx),
                    });
                },
            }
        }
        if lines.len() > inner.h {
            let more = format!(
                " {} more ",
                lines.len() - inner.h - self.scroll.min(lines.len() - inner.h)
            );
            let mw = more.chars().count();
            if rect.w > mw + 4 && lines.len() > inner.h + self.scroll {
                canvas.put(
                    rect.x + rect.w - mw - 2,
                    rect.y + rect.h - 1,
                    &more,
                    frame_style,
                );
            }
        }
    }

    fn render_details(&mut self, canvas: &mut Canvas, rect: Rect) {
        let frame_style = Style::default().fg(MUTED);
        let row = self.selected_row().cloned();
        let title = match &row {
            Some(row) => format!(" {} ", row.name),
            None => String::new(),
        };
        canvas.frame(
            rect,
            &title,
            frame_style,
            Style::default().fg(BRIGHT).bold(),
        );
        let inner = rect.inner();
        if inner.w < 12 || inner.h < 3 {
            return;
        }
        let Some(row) = row else {
            return;
        };
        let mut y = inner.y;
        let x = inner.x + 2;
        let w = inner.w.saturating_sub(4);
        let put = |canvas: &mut Canvas, y: &mut usize, text: &str, style: Style| {
            if *y < inner.y + inner.h {
                canvas.put(x, *y, &fit(text, w), style);
                *y += 1;
            }
        };
        if row.running {
            let attached = match row.clients {
                0 => "not attached".to_string(),
                1 => "1 terminal attached".to_string(),
                n => format!("{} terminals attached", n),
            };
            put(
                canvas,
                &mut y,
                &format!("● Running · {}", attached),
                Style::default().fg(RUNNING),
            );
            put(
                canvas,
                &mut y,
                &format!("Started {} ago", age_long(row.age)),
                Style::default().fg(MUTED),
            );
        } else {
            put(
                canvas,
                &mut y,
                "○ Stopped · screen saved",
                Style::default().fg(TEXT),
            );
            put(
                canvas,
                &mut y,
                &format!("Saved {} ago", age_long(row.age)),
                Style::default().fg(MUTED),
            );
        }
        y += 1;
        for (tab_idx, tab) in row.tabs.iter().enumerate() {
            let label = if tab.name.is_empty() {
                format!("Tab {}", tab_idx + 1)
            } else {
                format!("Tab {} · {}", tab_idx + 1, tab.name)
            };
            let style = if tab.active {
                Style::default().fg(BRIGHT).bold()
            } else {
                Style::default().fg(MUTED).bold()
            };
            put(canvas, &mut y, &label, style);
            for pane in &tab.panes {
                let what = pane
                    .command
                    .clone()
                    .filter(|c| !c.is_empty())
                    .unwrap_or_else(|| pane.title.clone());
                let mut line = format!("   {}", what);
                if let Some(cwd) = &pane.cwd {
                    if !cwd.is_empty() {
                        line.push_str("   ");
                        line.push_str(cwd);
                    }
                }
                if pane.exited {
                    line.push_str("   (finished)");
                }
                let style = if pane.focused {
                    Style::default().fg(INFO)
                } else {
                    Style::default().fg(TEXT)
                };
                put(canvas, &mut y, &line, style);
            }
            y += 1;
        }
    }

    fn dialog_rect(&self, over: Rect, width: usize, height: usize) -> Rect {
        let w = width.min(over.w.saturating_sub(2)).max(10);
        let h = height.min(over.h.saturating_sub(2)).max(3);
        let x = over.x + (over.w.saturating_sub(w)) / 2;
        let y = over.y + (over.h.saturating_sub(h)) / 2;
        Rect::new(x, y, w, h)
    }

    fn render_dialog_frame(&mut self, canvas: &mut Canvas, rect: Rect, title: &str, color: Color) {
        canvas.clear_rect(rect, Style::default());
        canvas.frame(
            rect,
            &format!(" {} ", title),
            Style::default().fg(color),
            Style::default().fg(color).bold(),
        );
    }

    /// Draws centered buttons; the focused one is highlighted.
    fn render_buttons(
        &mut self,
        canvas: &mut Canvas,
        y: usize,
        rect: Rect,
        buttons: &[(&str, Action, Color)],
        focused: Option<usize>,
    ) {
        let total: usize = buttons
            .iter()
            .map(|(label, _, _)| label.chars().count() + 4)
            .sum::<usize>()
            + buttons.len().saturating_sub(1) * 3;
        let mut x = rect.x + rect.w.saturating_sub(total) / 2;
        for (i, (label, action, color)) in buttons.iter().enumerate() {
            let text = format!("  {}  ", label);
            let width = text.chars().count();
            let style = if focused == Some(i) {
                Style::default().bg(*color).fg(Color::Ansi(0)).bold()
            } else {
                Style::default().fg(*color)
            };
            canvas.put(x, y, &text, style);
            self.hits.push(HitRegion {
                rect: Rect::new(x, y, width, 1),
                hit: Hit::Action(*action),
            });
            x += width + 3;
        }
    }

    /// Draws a text field with its cursor; highlighted when focused.
    fn render_field(
        &mut self,
        canvas: &mut Canvas,
        x: usize,
        y: usize,
        width: usize,
        field: &TextField,
        focused: bool,
    ) {
        let style = if focused {
            Style::default().bg(MUTED).fg(BRIGHT)
        } else {
            Style::default().bg(Color::Ansi(0)).fg(TEXT)
        };
        let text = field.text();
        canvas.put(x, y, &fit(&format!(" {}", text), width), style);
        if focused {
            let cursor_x = x + 1 + field.cursor().min(width.saturating_sub(2));
            let under = text.chars().nth(field.cursor()).unwrap_or(' ');
            canvas.put(
                cursor_x,
                y,
                &under.to_string(),
                Style::default().bg(BRIGHT).fg(Color::Ansi(0)),
            );
        }
        self.hits.push(HitRegion {
            rect: Rect::new(x, y, width, 1),
            hit: Hit::Action(Action::FocusField),
        });
    }

    /// Draws wrapped paragraphs at the top of a dialog's inner area; returns the next row.
    fn render_paragraphs(
        &mut self,
        canvas: &mut Canvas,
        inner: Rect,
        paragraphs: &[(String, Style)],
    ) -> usize {
        let width = inner.w.saturating_sub(4);
        let mut y = inner.y;
        for (text, style) in paragraphs {
            if text.is_empty() {
                y += 1;
                continue;
            }
            for line in wrap(text, width) {
                if y + 1 >= inner.y + inner.h {
                    return y;
                }
                canvas.put(inner.x + 2, y, &line, *style);
                y += 1;
            }
        }
        y
    }

    /// Height a dialog needs for these paragraphs plus `extra` rows and its buttons.
    fn dialog_height(paragraphs: &[(String, Style)], width: usize, extra: usize) -> usize {
        let text_rows: usize = paragraphs
            .iter()
            .map(|(text, _)| {
                if text.is_empty() {
                    1
                } else {
                    wrap(text, width.saturating_sub(4)).len()
                }
            })
            .sum();
        text_rows + extra + 2 /* blank + buttons */ + 2 /* frame */
    }

    fn dialog_width(&self, over: Rect) -> usize {
        64.min(over.w.saturating_sub(4)).max(24)
    }

    fn render_confirm(
        &mut self,
        canvas: &mut Canvas,
        over: Rect,
        action: Confirmable,
        name: &str,
        focus: Focus,
    ) {
        let (title, color, yes_label, body) = match action {
            Confirmable::Terminate => (
                "Terminate session",
                WARN,
                "Terminate",
                "Its programs will be stopped. The screen and scrollback are saved, so the \
                 session can be opened again later.",
            ),
            Confirmable::Delete => (
                "Delete session",
                DANGER,
                "Delete",
                "The session will be stopped and its saved screen and scrollback removed. \
                 This cannot be undone.",
            ),
        };
        let question = match action {
            Confirmable::Terminate => format!("Terminate {}?", name),
            Confirmable::Delete => format!("Delete {}?", name),
        };
        let paragraphs = vec![
            (question, Style::default().fg(BRIGHT).bold()),
            (String::new(), Style::default()),
            (body.to_string(), Style::default().fg(TEXT)),
        ];
        let width = self.dialog_width(over);
        let rect = self.dialog_rect(over, width, Self::dialog_height(&paragraphs, width, 0));
        self.render_dialog_frame(canvas, rect, title, color);
        let inner = rect.inner();
        self.render_paragraphs(canvas, inner, &paragraphs);
        let focused = match focus {
            Focus::Button(b) => Some(b),
            Focus::Field => Some(0),
        };
        self.render_buttons(
            canvas,
            inner.y + inner.h - 1,
            inner,
            &[
                (yes_label, Action::Primary, color),
                ("Cancel", Action::Cancel, TEXT),
            ],
            focused,
        );
    }

    fn render_new_session(
        &mut self,
        canvas: &mut Canvas,
        over: Rect,
        field: &TextField,
        focus: Focus,
    ) {
        let folder = if self.working_dir.is_empty() {
            String::new()
        } else {
            format!("Folder: {}", self.working_dir)
        };
        let paragraphs = vec![
            ("Name".to_string(), Style::default().fg(TEXT)),
            (String::new(), Style::default()), // the field
            (String::new(), Style::default()),
            (folder, Style::default().fg(MUTED)),
        ];
        let width = self.dialog_width(over);
        let rect = self.dialog_rect(over, width, Self::dialog_height(&paragraphs, width, 0));
        self.render_dialog_frame(canvas, rect, "New session", INFO);
        let inner = rect.inner();
        self.render_paragraphs(canvas, inner, &paragraphs);
        let field_w = inner.w.saturating_sub(4);
        self.render_field(
            canvas,
            inner.x + 2,
            inner.y + 1,
            field_w,
            field,
            focus == Focus::Field,
        );
        let focused = match focus {
            Focus::Button(b) => Some(b),
            Focus::Field => None,
        };
        self.render_buttons(
            canvas,
            inner.y + inner.h - 1,
            inner,
            &[
                ("Create", Action::Primary, OK),
                ("Cancel", Action::Cancel, TEXT),
            ],
            focused,
        );
    }

    fn render_shutdown(
        &mut self,
        canvas: &mut Canvas,
        over: Rect,
        field: &TextField,
        focus: Focus,
    ) {
        let running = self.rows.iter().filter(|r| r.running).count();
        let body = match running {
            0 => "No session is running. The dashboard will close.".to_string(),
            1 => "The running session will be saved and stopped, and the dashboard will close."
                .to_string(),
            n => format!(
                "All {} running sessions will be saved and stopped, and the dashboard will close.",
                n
            ),
        };
        let paragraphs = vec![
            (
                "Shut down zellij?".to_string(),
                Style::default().fg(BRIGHT).bold(),
            ),
            (String::new(), Style::default()),
            (body, Style::default().fg(TEXT)),
            (String::new(), Style::default()),
            (
                format!("Type {} to confirm", SHUTDOWN_WORD),
                Style::default().fg(TEXT),
            ),
        ];
        let width = self.dialog_width(over);
        let rect = self.dialog_rect(over, width, Self::dialog_height(&paragraphs, width, 1));
        self.render_dialog_frame(canvas, rect, "Shut down", DANGER);
        let inner = rect.inner();
        let y = self.render_paragraphs(canvas, inner, &paragraphs);
        self.render_field(canvas, inner.x + 2, y, 12, field, focus == Focus::Field);
        let focused = match focus {
            Focus::Button(b) => Some(b),
            Focus::Field => None,
        };
        self.render_buttons(
            canvas,
            inner.y + inner.h - 1,
            inner,
            &[
                ("Shut down", Action::Primary, DANGER),
                ("Cancel", Action::Cancel, TEXT),
            ],
            focused,
        );
    }

    fn render_help(&mut self, canvas: &mut Canvas, over: Rect) {
        let entries: Vec<(&str, &str)> = vec![
            ("↑ ↓  j k", "move"),
            ("⏎  o  space", "open the selected session"),
            ("n", "new session"),
            ("t  x", "terminate (the screen is saved)"),
            ("d", "delete (the saved screen is removed)"),
            ("r", "refresh"),
            ("q  esc", "quit (sessions keep running)"),
            ("Q", "shut down all sessions and quit"),
            ("", ""),
            ("Ctrl q", "return to the dashboard from a session"),
            ("mouse", "select with a click, open with a second click"),
        ];
        let width = 72.min(over.w.saturating_sub(4)).max(24);
        let rect = self.dialog_rect(over, width, entries.len() + 4);
        self.render_dialog_frame(canvas, rect, "Help", INFO);
        let inner = rect.inner();
        for (i, (keys, text)) in entries.iter().enumerate() {
            if i + 2 >= inner.h {
                break;
            }
            canvas.put(
                inner.x + 2,
                inner.y + i,
                keys,
                Style::default().fg(KEY).bold(),
            );
            canvas.put(
                inner.x + 15,
                inner.y + i,
                &fit(text, inner.w.saturating_sub(17)),
                Style::default().fg(TEXT),
            );
        }
        self.render_buttons(
            canvas,
            inner.y + inner.h - 1,
            inner,
            &[("Close", Action::Cancel, INFO)],
            Some(0),
        );
    }
}

pub fn is_session_name_char(c: char) -> bool {
    c.is_alphanumeric() || c == '-' || c == '_' || c == '.'
}

/// Truncates with an ellipsis or pads with spaces to exactly `width` characters.
pub fn fit(text: &str, width: usize) -> String {
    let count = text.chars().count();
    if count <= width {
        let mut s = text.to_string();
        s.extend(std::iter::repeat(' ').take(width - count));
        s
    } else if width == 0 {
        String::new()
    } else {
        let mut s: String = text.chars().take(width - 1).collect();
        s.push('…');
        s
    }
}

/// Greedy word wrap to lines of at most `width` characters (words longer than the width
/// are split).
pub fn wrap(text: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut lines = vec![];
    let mut line = String::new();
    let mut line_len = 0;
    for word in text.split_whitespace() {
        let mut word: Vec<char> = word.chars().collect();
        while !word.is_empty() {
            let word_len = word.len();
            if line_len == 0 {
                let take = word_len.min(width);
                line.extend(word.drain(..take));
                line_len = take;
            } else if line_len + 1 + word_len <= width {
                line.push(' ');
                line.extend(word.drain(..));
                line_len += 1 + word_len;
            } else {
                lines.push(std::mem::take(&mut line));
                line_len = 0;
            }
        }
    }
    if !line.is_empty() || lines.is_empty() {
        lines.push(line);
    }
    lines
}

pub fn age_short(age: Duration) -> String {
    let secs = age.as_secs();
    if secs < 60 {
        "now".to_string()
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else if secs < 86_400 {
        format!("{}h", secs / 3600)
    } else if secs < 7 * 86_400 {
        format!("{}d", secs / 86_400)
    } else if secs < 60 * 86_400 {
        format!("{}w", secs / (7 * 86_400))
    } else {
        format!("{}mo", secs / (30 * 86_400))
    }
}

pub fn age_long(age: Duration) -> String {
    let secs = age.as_secs();
    if secs < 60 {
        "less than a minute".to_string()
    } else if secs < 3600 {
        let m = secs / 60;
        format!("{} minute{}", m, if m == 1 { "" } else { "s" })
    } else if secs < 86_400 {
        let h = secs / 3600;
        let m = (secs % 3600) / 60;
        if m == 0 {
            format!("{} hour{}", h, if h == 1 { "" } else { "s" })
        } else {
            format!("{}h {}m", h, m)
        }
    } else {
        let d = secs / 86_400;
        let h = (secs % 86_400) / 3600;
        if h == 0 {
            format!("{} day{}", d, if d == 1 { "" } else { "s" })
        } else {
            format!("{}d {}h", d, h)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(name: &str, running: bool) -> SessionRow {
        SessionRow {
            name: name.to_string(),
            running,
            clients: 1,
            age: Duration::from_secs(3700),
            tabs: vec![TabRow {
                name: "main".to_string(),
                active: true,
                panes: vec![PaneRow {
                    title: "zsh".to_string(),
                    command: None,
                    cwd: Some("~/work".to_string()),
                    focused: true,
                    exited: false,
                }],
            }],
        }
    }

    fn dashboard() -> Dashboard {
        let mut d = Dashboard::new(120, 30);
        d.set_rows(vec![
            row("alpha", true),
            row("beta", true),
            row("gamma", false),
        ]);
        d.set_suggested_name("fresh-name".to_string());
        d
    }

    fn type_text(d: &mut Dashboard, text: &str) {
        for c in text.chars() {
            d.handle_key(Key::Char(c));
        }
    }

    fn field_text(d: &Dashboard) -> String {
        match &d.mode {
            Mode::NewSession { field, .. } | Mode::Shutdown { field, .. } => field.text(),
            _ => panic!("no field in {:?}", d.mode),
        }
    }

    fn focus(d: &Dashboard) -> Focus {
        match &d.mode {
            Mode::NewSession { focus, .. }
            | Mode::Shutdown { focus, .. }
            | Mode::Confirm { focus, .. } => *focus,
            _ => panic!("no focus in {:?}", d.mode),
        }
    }

    #[test]
    fn selection_moves_and_clamps() {
        let mut d = dashboard();
        assert_eq!(d.selected, 0);
        assert_eq!(d.handle_key(Key::Down), Command::None);
        assert_eq!(d.selected, 1);
        d.handle_key(Key::Char('j'));
        d.handle_key(Key::Char('j'));
        assert_eq!(d.selected, 2, "stops at the last row");
        d.handle_key(Key::Home);
        assert_eq!(d.selected, 0);
        d.handle_key(Key::Char('G'));
        assert_eq!(d.selected, 2);
        d.handle_key(Key::Up);
        assert_eq!(d.selected, 1);
    }

    #[test]
    fn left_and_right_do_nothing_in_the_list() {
        let mut d = dashboard();
        assert_eq!(d.handle_key(Key::Right), Command::None);
        assert_eq!(d.handle_key(Key::Left), Command::None);
        assert_eq!(d.mode, Mode::List);
        let mut empty = Dashboard::new(80, 24);
        assert_eq!(empty.handle_key(Key::Right), Command::None);
        assert_eq!(empty.mode, Mode::List);
    }

    #[test]
    fn set_rows_keeps_the_selected_session() {
        let mut d = dashboard();
        d.handle_key(Key::Down);
        d.handle_key(Key::Down);
        assert_eq!(d.selected_row().unwrap().name, "gamma");
        d.set_rows(vec![row("gamma", false), row("alpha", true)]);
        assert_eq!(d.selected_row().unwrap().name, "gamma");
        d.set_rows(vec![row("alpha", true)]);
        assert_eq!(d.selected, 0, "clamps when the session is gone");
        d.set_rows(vec![]);
        assert_eq!(d.selected, 0);
        assert!(d.selected_row().is_none());
    }

    #[test]
    fn enter_opens_the_selected_session() {
        let mut d = dashboard();
        d.handle_key(Key::Down);
        assert_eq!(
            d.handle_key(Key::Enter),
            Command::Open {
                name: "beta".to_string(),
            }
        );
    }

    #[test]
    fn enter_on_an_empty_list_prompts_for_a_new_session() {
        let mut d = Dashboard::new(80, 24);
        d.set_suggested_name("first".to_string());
        assert_eq!(d.handle_key(Key::Enter), Command::None);
        assert!(matches!(d.mode, Mode::NewSession { .. }));
        assert_eq!(field_text(&d), "first");
    }

    #[test]
    fn text_field_edits_with_a_cursor() {
        let mut f = TextField::new("abc", 10);
        assert_eq!(f.cursor(), 3);
        f.handle_key(&Key::Left, |_| true);
        f.handle_key(&Key::Left, |_| true);
        f.handle_key(&Key::Char('X'), |_| true);
        assert_eq!(f.text(), "aXbc");
        assert_eq!(f.cursor(), 2);
        f.handle_key(&Key::Delete, |_| true);
        assert_eq!(f.text(), "aXc");
        f.handle_key(&Key::Backspace, |_| true);
        assert_eq!(f.text(), "ac");
        f.handle_key(&Key::Home, |_| true);
        f.handle_key(&Key::Char('-'), |_| true);
        assert_eq!(f.text(), "-ac");
        f.handle_key(&Key::End, |_| true);
        f.handle_key(&Key::Char('!'), |c| c != '!');
        assert_eq!(f.text(), "-ac", "rejected characters are ignored");
        f.handle_key(&Key::Ctrl('u'), |_| true);
        assert!(f.is_empty());
        assert!(
            !f.handle_key(&Key::Tab, |_| true),
            "tab is not an editing key"
        );
    }

    #[test]
    fn new_session_prompt_edits_validates_and_navigates() {
        let mut d = dashboard();
        d.handle_key(Key::Char('n'));
        assert_eq!(field_text(&d), "fresh-name");
        assert_eq!(focus(&d), Focus::Field);
        d.handle_key(Key::Ctrl('u'));
        type_text(&mut d, "my session/1");
        assert_eq!(
            field_text(&d),
            "mysession1",
            "spaces and slashes are dropped"
        );
        d.handle_key(Key::Left);
        d.handle_key(Key::Backspace);
        assert_eq!(
            field_text(&d),
            "mysessio1",
            "the cursor moves with the arrows"
        );
        d.handle_key(Key::End);
        d.handle_key(Key::Backspace);
        assert_eq!(field_text(&d), "mysessio");
        // tab moves to the buttons; enter on Cancel closes without creating
        d.handle_key(Key::Tab);
        assert_eq!(focus(&d), Focus::Button(0));
        d.handle_key(Key::Right);
        assert_eq!(focus(&d), Focus::Button(1));
        assert_eq!(d.handle_key(Key::Enter), Command::None);
        assert_eq!(d.mode, Mode::List);
        // enter in the field or on Create creates
        d.handle_key(Key::Char('n'));
        assert_eq!(
            d.handle_key(Key::Enter),
            Command::New {
                name: "fresh-name".to_string()
            }
        );
        d.handle_key(Key::Esc); // the runner would have left; start over
        d.handle_key(Key::Char('n'));
        d.handle_key(Key::Down);
        assert_eq!(focus(&d), Focus::Button(0));
        assert_eq!(
            d.handle_key(Key::Enter),
            Command::New {
                name: "fresh-name".to_string()
            }
        );
        d.handle_key(Key::Esc);
        d.handle_key(Key::Char('n'));
        d.handle_key(Key::Ctrl('u'));
        assert_eq!(
            d.handle_key(Key::Enter),
            Command::None,
            "an empty name is refused"
        );
        assert!(matches!(d.mode, Mode::NewSession { .. }));
        d.handle_key(Key::Esc);
        assert_eq!(d.mode, Mode::List);
    }

    #[test]
    fn terminate_and_delete_need_confirmation() {
        let mut d = dashboard();
        d.handle_key(Key::Char('t'));
        assert!(matches!(
            d.mode,
            Mode::Confirm {
                action: Confirmable::Terminate,
                ..
            }
        ));
        assert_eq!(
            focus(&d),
            Focus::Button(0),
            "terminate starts on its button"
        );
        assert_eq!(d.handle_key(Key::Char('n')), Command::None);
        assert_eq!(d.mode, Mode::List);
        d.handle_key(Key::Char('t'));
        assert_eq!(
            d.handle_key(Key::Enter),
            Command::Terminate("alpha".to_string())
        );
        d.handle_key(Key::Char('d'));
        assert_eq!(focus(&d), Focus::Button(1), "delete starts on Cancel");
        assert_eq!(
            d.handle_key(Key::Enter),
            Command::None,
            "enter on Cancel does nothing"
        );
        d.handle_key(Key::Char('d'));
        d.handle_key(Key::Left);
        assert_eq!(focus(&d), Focus::Button(0));
        assert_eq!(
            d.handle_key(Key::Enter),
            Command::Delete("alpha".to_string())
        );
        d.handle_key(Key::Char('d'));
        assert_eq!(
            d.handle_key(Key::Char('y')),
            Command::Delete("alpha".to_string())
        );
        // a stopped session cannot be terminated
        d.handle_key(Key::End);
        assert_eq!(d.handle_key(Key::Char('t')), Command::None);
        assert_eq!(d.mode, Mode::List);
        assert!(d.notice.is_some());
    }

    #[test]
    fn shutdown_requires_the_word_and_edits_with_arrows() {
        let mut d = dashboard();
        d.handle_key(Key::Char('Q'));
        assert!(matches!(d.mode, Mode::Shutdown { .. }));
        type_text(&mut d, "no");
        assert_eq!(d.handle_key(Key::Enter), Command::None);
        assert_eq!(field_text(&d), "");
        type_text(&mut d, "ys");
        d.handle_key(Key::Left);
        d.handle_key(Key::Char('e'));
        assert_eq!(field_text(&d), "yes");
        assert_eq!(d.handle_key(Key::Enter), Command::Shutdown);
        d.handle_key(Key::Char('Q'));
        type_text(&mut d, "yes");
        d.handle_key(Key::Tab);
        d.handle_key(Key::Tab);
        assert_eq!(focus(&d), Focus::Button(1));
        assert_eq!(
            d.handle_key(Key::Enter),
            Command::None,
            "enter on Cancel closes"
        );
        assert_eq!(d.mode, Mode::List);
        d.handle_key(Key::Char('Q'));
        assert_eq!(d.handle_key(Key::Esc), Command::None);
        assert_eq!(d.mode, Mode::List);
    }

    #[test]
    fn quit_keys() {
        let mut d = dashboard();
        assert_eq!(d.handle_key(Key::Char('q')), Command::Quit);
        assert_eq!(d.handle_key(Key::Ctrl('q')), Command::Quit);
        assert_eq!(d.handle_key(Key::Esc), Command::Quit);
    }

    #[test]
    fn render_lists_sessions_in_sections_and_records_hit_regions() {
        let mut d = dashboard();
        let frame = d.render();
        let plain = strip_ansi(&frame);
        assert!(plain.contains("RUNNING"));
        assert!(plain.contains("SAVED"));
        assert!(plain.contains("alpha"));
        assert!(plain.contains("gamma"));
        assert!(plain.contains("2 running · 1 saved"));
        assert!(!plain.contains("Ctrl q"), "no tip line on the dashboard");
        assert!(
            plain.contains("q quit   Q shut down all"),
            "quit and shut down sit together at the end"
        );
        // clicking the second row selects it, clicking it again opens it
        let row_y = d
            .hits
            .iter()
            .find_map(|h| match h.hit {
                Hit::Row(1) => Some(h.rect.y),
                _ => None,
            })
            .unwrap();
        assert_eq!(
            d.handle_mouse(Mouse::Press { x: 5, y: row_y }),
            Command::None
        );
        assert_eq!(d.selected, 1);
        assert_eq!(
            d.handle_mouse(Mouse::Press { x: 5, y: row_y }),
            Command::Open {
                name: "beta".to_string(),
            }
        );
        // the footer hint opens the shutdown prompt
        let hint = d
            .hits
            .iter()
            .find(|h| h.hit == Hit::Action(Action::ShutdownPrompt))
            .unwrap()
            .rect;
        d.handle_mouse(Mouse::Press {
            x: hint.x + 1,
            y: hint.y,
        });
        assert!(matches!(d.mode, Mode::Shutdown { .. }));
        // its buttons are clickable: Cancel closes it
        d.render();
        let cancel = d
            .hits
            .iter()
            .find(|h| h.hit == Hit::Action(Action::Cancel))
            .unwrap()
            .rect;
        d.handle_mouse(Mouse::Press {
            x: cancel.x + 1,
            y: cancel.y,
        });
        assert_eq!(d.mode, Mode::List);
        // and the wheel moves the selection only in list mode
        d.handle_mouse(Mouse::WheelDown);
        assert_eq!(d.selected, 2);
        d.handle_key(Key::Char('?'));
        d.handle_mouse(Mouse::WheelUp);
        assert_eq!(d.selected, 2);
    }

    #[test]
    fn empty_list_shows_only_the_short_message() {
        let mut d = Dashboard::new(100, 24);
        let plain = strip_ansi(&d.render());
        assert!(plain.contains("No sessions yet."));
        assert!(!plain.contains("Press n"));
    }

    #[test]
    fn narrow_terminals_drop_the_details_panel() {
        let mut d = dashboard();
        d.resize(80, 20);
        let plain = strip_ansi(&d.render());
        assert!(plain.contains("alpha"));
        assert!(!plain.contains("Started"));
    }

    #[test]
    fn list_scrolls_to_keep_the_selection_visible() {
        let mut d = Dashboard::new(80, 10);
        d.set_rows((0..20).map(|i| row(&format!("s{:02}", i), true)).collect());
        d.render();
        let visible = d.list_rows_visible;
        assert!(visible < 20);
        d.handle_key(Key::End);
        let plain = strip_ansi(&d.render());
        assert!(plain.contains("s19"));
        assert!(!plain.contains("s00"));
        d.handle_key(Key::Home);
        let plain = strip_ansi(&d.render());
        assert!(
            plain.contains("RUNNING"),
            "scrolling back shows the section label again"
        );
        assert!(plain.contains("s00"));
    }

    #[test]
    fn dialog_text_is_wrapped_not_truncated() {
        let mut d = dashboard();
        d.handle_key(Key::Char('t'));
        let plain = strip_ansi(&d.render());
        assert!(plain.contains("Terminate alpha?"));
        assert!(!plain.contains('…'), "no ellipsis in a dialog: {}", plain);
        assert!(plain.contains("opened again later."));
        d.resize(50, 24);
        let plain = strip_ansi(&d.render());
        assert!(
            !plain.contains('…'),
            "wraps on a narrow screen too: {}",
            plain
        );
        assert!(plain.contains("Cancel"));
        assert_eq!(wrap("aa bb cc", 5), vec!["aa bb", "cc"]);
        assert_eq!(wrap("abcdefgh", 3), vec!["abc", "def", "gh"]);
        assert_eq!(wrap("", 5), vec![""]);
    }

    #[test]
    fn helpers() {
        assert_eq!(fit("abc", 5), "abc  ");
        assert_eq!(fit("abcdef", 4), "abc…");
        assert_eq!(age_short(Duration::from_secs(30)), "now");
        assert_eq!(age_short(Duration::from_secs(90)), "1m");
        assert_eq!(age_short(Duration::from_secs(7200)), "2h");
        assert_eq!(age_short(Duration::from_secs(3 * 86_400)), "3d");
        assert_eq!(age_long(Duration::from_secs(3660)), "1h 1m");
        let r = row("x", true);
        assert_eq!(r.summary(), "zsh  ~/work");
    }

    fn strip_ansi(s: &str) -> String {
        let mut out = String::new();
        let mut chars = s.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '\u{1b}' {
                if chars.peek() == Some(&'[') {
                    chars.next();
                    while let Some(&n) = chars.peek() {
                        chars.next();
                        if n.is_ascii_alphabetic() {
                            break;
                        }
                    }
                }
                continue;
            }
            out.push(c);
        }
        out
    }
}
