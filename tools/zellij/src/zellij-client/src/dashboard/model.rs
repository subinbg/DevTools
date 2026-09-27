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
                    Some(cwd) if !cwd.is_empty() => format!("{} · {}", what, cwd),
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
    /// Attach to (or resurrect) the session; `full_history` reprints the pane's whole
    /// history into the host terminal even if it was printed before.
    Open {
        name: String,
        full_history: bool,
    },
    New {
        name: String,
    },
    Terminate(String),
    Delete(String),
    Shutdown,
    Quit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Confirmable {
    Terminate,
    Delete,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mode {
    List,
    Help,
    NewSession { name: String },
    Confirm { action: Confirmable, name: String },
    Shutdown { typed: String },
}

#[derive(Clone, Debug)]
struct Notice {
    text: String,
    is_error: bool,
    shown_at: Instant,
}

/// Something the mouse can hit: a session row or a button that acts like a key press.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Hit {
    Row(usize),
    Key(Key),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct HitRegion {
    rect: Rect,
    hit: Hit,
}

pub struct Dashboard {
    pub rows: Vec<SessionRow>,
    pub selected: usize,
    scroll: usize,
    pub mode: Mode,
    notice: Option<Notice>,
    cols: usize,
    lines: usize,
    suggested_name: String,
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
            hits: vec![],
            list_rows_visible: 1,
        }
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

    fn clamp_selection(&mut self) {
        if self.rows.is_empty() {
            self.selected = 0;
        } else if self.selected >= self.rows.len() {
            self.selected = self.rows.len() - 1;
        }
        let visible = self.list_rows_visible.max(1);
        if self.selected < self.scroll {
            self.scroll = self.selected;
        } else if self.selected >= self.scroll + visible {
            self.scroll = self.selected + 1 - visible;
        }
        if self.scroll > 0 && self.scroll + visible > self.rows.len() {
            self.scroll = self.rows.len().saturating_sub(visible);
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

    pub fn handle_key(&mut self, key: Key) -> Command {
        match self.mode.clone() {
            Mode::List => self.handle_list_key(key),
            Mode::Help => {
                self.mode = Mode::List;
                Command::None
            },
            Mode::NewSession { mut name } => match key {
                Key::Esc | Key::Ctrl('c') | Key::Ctrl('q') => {
                    self.mode = Mode::List;
                    Command::None
                },
                Key::Enter => {
                    let name = name.trim().to_string();
                    if name.is_empty() {
                        self.notify("A session needs a name", true);
                        Command::None
                    } else {
                        Command::New { name }
                    }
                },
                Key::Backspace => {
                    name.pop();
                    self.mode = Mode::NewSession { name };
                    Command::None
                },
                Key::Ctrl('u') => {
                    self.mode = Mode::NewSession {
                        name: String::new(),
                    };
                    Command::None
                },
                Key::Char(c) if is_session_name_char(c) => {
                    if name.chars().count() < 48 {
                        name.push(c);
                    }
                    self.mode = Mode::NewSession { name };
                    Command::None
                },
                _ => Command::None,
            },
            Mode::Confirm { action, name } => match key {
                Key::Char('y') | Key::Char('Y') | Key::Enter => {
                    self.mode = Mode::List;
                    match action {
                        Confirmable::Terminate => Command::Terminate(name),
                        Confirmable::Delete => Command::Delete(name),
                    }
                },
                Key::Char('n') | Key::Char('N') | Key::Esc | Key::Ctrl('c') | Key::Char('q') => {
                    self.mode = Mode::List;
                    Command::None
                },
                _ => Command::None,
            },
            Mode::Shutdown { mut typed } => match key {
                Key::Esc | Key::Ctrl('c') | Key::Ctrl('q') => {
                    self.mode = Mode::List;
                    Command::None
                },
                Key::Enter => {
                    if typed.trim().eq_ignore_ascii_case(SHUTDOWN_WORD) {
                        self.mode = Mode::List;
                        Command::Shutdown
                    } else {
                        self.notify(format!("Type '{}' to shut down", SHUTDOWN_WORD), true);
                        self.mode = Mode::Shutdown {
                            typed: String::new(),
                        };
                        Command::None
                    }
                },
                Key::Backspace => {
                    typed.pop();
                    self.mode = Mode::Shutdown { typed };
                    Command::None
                },
                Key::Char(c) if c.is_ascii_alphabetic() => {
                    if typed.len() < 8 {
                        typed.push(c);
                    }
                    self.mode = Mode::Shutdown { typed };
                    Command::None
                },
                _ => Command::None,
            },
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
            Key::Enter | Key::Char('o') | Key::Char(' ') | Key::Right => {
                return self.open_selected(false);
            },
            Key::Char('O') => return self.open_selected(true),
            Key::Char('n') | Key::Char('c') => {
                self.mode = Mode::NewSession {
                    name: self.suggested_name.clone(),
                };
            },
            Key::Char('t') | Key::Char('x') => match self.selected_row() {
                Some(row) if row.running => {
                    self.mode = Mode::Confirm {
                        action: Confirmable::Terminate,
                        name: row.name.clone(),
                    };
                },
                Some(row) => {
                    let msg = format!("'{}' is not running", row.name);
                    self.notify(msg, true);
                },
                None => {},
            },
            Key::Char('d') | Key::Delete => {
                if let Some(row) = self.selected_row() {
                    self.mode = Mode::Confirm {
                        action: Confirmable::Delete,
                        name: row.name.clone(),
                    };
                }
            },
            Key::Char('r') | Key::Char('R') | Key::Ctrl('r') => return Command::Refresh,
            Key::Char('?') | Key::Char('h') => self.mode = Mode::Help,
            Key::Char('Q') => {
                self.mode = Mode::Shutdown {
                    typed: String::new(),
                };
            },
            Key::Char('q') | Key::Esc | Key::Ctrl('q') | Key::Ctrl('c') | Key::Ctrl('d') => {
                return Command::Quit;
            },
            _ => {},
        }
        Command::None
    }

    fn open_selected(&mut self, full_history: bool) -> Command {
        match self.selected_row() {
            Some(row) => Command::Open {
                name: row.name.clone(),
                full_history,
            },
            None => {
                self.mode = Mode::NewSession {
                    name: self.suggested_name.clone(),
                };
                Command::None
            },
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
                            self.open_selected(false)
                        } else {
                            self.selected = idx;
                            self.clamp_selection();
                            Command::None
                        }
                    },
                    Some(Hit::Key(key)) => self.handle_key(key),
                    None => {
                        // clicking outside a modal closes it
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
            Mode::Help => self.render_help(&mut canvas, list_rect),
            Mode::NewSession { name } => self.render_new_session(&mut canvas, list_rect, &name),
            Mode::Confirm { action, name } => {
                self.render_confirm(&mut canvas, list_rect, action, &name)
            },
            Mode::Shutdown { typed } => self.render_shutdown(&mut canvas, list_rect, &typed),
        }
        canvas.to_ansi()
    }

    fn render_header(&mut self, canvas: &mut Canvas) {
        let cols = canvas.cols();
        canvas.fill_row(0, ' ', Style::default().bg(Color::Ansi(0)));
        let title = " ⬢ zellij ";
        canvas.put(
            0,
            0,
            title,
            Style::default()
                .fg(Color::Ansi(14))
                .bold()
                .bg(Color::Ansi(0)),
        );
        let mut x = title.chars().count() + 1;
        let notice = self
            .notice
            .clone()
            .filter(|n| n.shown_at.elapsed() < NOTICE_DURATION);
        match notice {
            Some(notice) => {
                let style = if notice.is_error {
                    Style::default()
                        .fg(Color::Ansi(9))
                        .bold()
                        .bg(Color::Ansi(0))
                } else {
                    Style::default().fg(Color::Ansi(10)).bg(Color::Ansi(0))
                };
                canvas.put(x, 0, &notice.text, style);
            },
            None => {
                let running = self.rows.iter().filter(|r| r.running).count();
                let saved = self.rows.len() - running;
                let text = format!("{} running · {} saved", running, saved);
                canvas.put(
                    x,
                    0,
                    &text,
                    Style::default().fg(Color::Ansi(7)).bg(Color::Ansi(0)),
                );
                x += text.chars().count();
                let _ = x;
            },
        }
        let power = "⏻ shut down ";
        let power_w = power.chars().count();
        if cols > power_w + 30 {
            let px = cols - power_w - 1;
            canvas.put(
                px,
                0,
                power,
                Style::default().fg(Color::Ansi(8)).bg(Color::Ansi(0)),
            );
            self.hits.push(HitRegion {
                rect: Rect::new(px, 0, power_w, 1),
                hit: Hit::Key(Key::Char('Q')),
            });
        }
    }

    fn render_footer(&mut self, canvas: &mut Canvas) {
        let y = canvas.lines() - 1;
        canvas.fill_row(y, ' ', Style::default());
        let hints: Vec<(&str, &str, Key)> = match &self.mode {
            Mode::List => vec![
                ("↑↓", "select", Key::Other),
                ("⏎", "open", Key::Enter),
                ("n", "new", Key::Char('n')),
                ("t", "terminate", Key::Char('t')),
                ("d", "delete", Key::Char('d')),
                ("r", "refresh", Key::Char('r')),
                ("?", "help", Key::Char('?')),
                ("q", "quit", Key::Char('q')),
            ],
            Mode::Help => vec![("any key", "back", Key::Esc)],
            Mode::NewSession { .. } => {
                vec![("⏎", "create", Key::Enter), ("esc", "cancel", Key::Esc)]
            },
            Mode::Confirm { .. } => vec![("y", "yes", Key::Char('y')), ("n", "no", Key::Char('n'))],
            Mode::Shutdown { .. } => {
                vec![("⏎", "confirm", Key::Enter), ("esc", "cancel", Key::Esc)]
            },
        };
        let mut x = 1;
        let cols = canvas.cols();
        for (key, label, action) in hints {
            let width = key.chars().count() + 1 + label.chars().count();
            if x + width + 1 > cols {
                break;
            }
            canvas.put(x, y, key, Style::default().fg(Color::Ansi(11)).bold());
            canvas.put(
                x + key.chars().count() + 1,
                y,
                label,
                Style::default().fg(Color::Ansi(8)),
            );
            if action != Key::Other {
                self.hits.push(HitRegion {
                    rect: Rect::new(x, y, width, 1),
                    hit: Hit::Key(action),
                });
            }
            x += width + 3;
        }
        if let Mode::List = self.mode {
            let tip = "Ctrl q leaves a session";
            let tw = tip.chars().count();
            if x + tw + 1 <= cols {
                canvas.put(
                    cols - tw - 1,
                    y,
                    tip,
                    Style::default().fg(Color::Ansi(8)).italic(),
                );
            }
        }
    }

    fn render_list(&mut self, canvas: &mut Canvas, rect: Rect) {
        let frame_style = Style::default().fg(Color::Ansi(10));
        let title = format!(" Sessions ({}) ", self.rows.len());
        canvas.frame(
            rect,
            &title,
            frame_style,
            Style::default().fg(Color::Ansi(10)).bold(),
        );
        let inner = rect.inner();
        if inner.h == 0 || inner.w < 10 {
            return;
        }
        self.list_rows_visible = inner.h;
        self.clamp_selection();

        if self.rows.is_empty() {
            let lines = [
                "No sessions yet.",
                "",
                "Press n to create one. A session is a shell that keeps",
                "running after you leave it with Ctrl q; its screen and",
                "scrollback are saved so it can be opened again later.",
            ];
            for (i, line) in lines.iter().enumerate() {
                if i + 1 < inner.h {
                    canvas.put(
                        inner.x + 2,
                        inner.y + 1 + i,
                        line,
                        Style::default().fg(Color::Ansi(8)),
                    );
                }
            }
            return;
        }

        let name_w = self
            .rows
            .iter()
            .map(|r| r.name.chars().count())
            .max()
            .unwrap_or(8)
            .clamp(8, 28);
        for (line_idx, row_idx) in (self.scroll..self.rows.len()).enumerate() {
            if line_idx >= inner.h {
                break;
            }
            let y = inner.y + line_idx;
            let row = &self.rows[row_idx];
            let selected = row_idx == self.selected;
            let base = if selected {
                Style::default().bg(Color::Ansi(4)).bold()
            } else {
                Style::default()
            };
            canvas.fill_row_range(y, inner.x, inner.w, ' ', base);

            let (glyph, glyph_style) = if row.running {
                ("●", base.fg(Color::Ansi(10)))
            } else {
                ("○", base.fg(Color::Ansi(8)))
            };
            canvas.put(inner.x + 1, y, glyph, glyph_style);
            let name = fit(&row.name, name_w);
            let name_style = if row.running {
                base.fg(Color::Ansi(15))
            } else {
                base.fg(Color::Ansi(7))
            };
            canvas.put(inner.x + 3, y, &name, name_style);

            let age = age_short(row.age);
            let right = if row.running {
                let clients = match row.clients {
                    0 => "detached".to_string(),
                    1 => "1 client".to_string(),
                    n => format!("{} clients", n),
                };
                format!("{}  {:>4}", clients, age)
            } else {
                format!("saved  {:>4}", age)
            };
            let right_w = right.chars().count();
            let summary_x = inner.x + 3 + name_w + 2;
            if inner.x + inner.w > right_w + 1 {
                let right_x = inner.x + inner.w - right_w - 1;
                if right_x > summary_x + 4 {
                    let summary = fit(&row.summary(), right_x - summary_x - 2);
                    let summary_style = if selected {
                        base.fg(Color::Ansi(15))
                    } else if row.running {
                        base.fg(Color::Ansi(14))
                    } else {
                        base.fg(Color::Ansi(8))
                    };
                    canvas.put(summary_x, y, &summary, summary_style);
                }
                canvas.put(right_x, y, &right, base.fg(Color::Ansi(8)));
            }
            self.hits.push(HitRegion {
                rect: Rect::new(inner.x, y, inner.w, 1),
                hit: Hit::Row(row_idx),
            });
        }
        if self.rows.len() > inner.h {
            let more = format!(
                " {}-{} of {} ",
                self.scroll + 1,
                (self.scroll + inner.h).min(self.rows.len()),
                self.rows.len()
            );
            let mw = more.chars().count();
            if rect.w > mw + 4 {
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
        let frame_style = Style::default().fg(Color::Ansi(8));
        canvas.frame(
            rect,
            " Details ",
            frame_style,
            Style::default().fg(Color::Ansi(7)).bold(),
        );
        let inner = rect.inner();
        if inner.w < 12 || inner.h < 3 {
            return;
        }
        let row = match self.selected_row() {
            Some(row) => row.clone(),
            None => return,
        };
        let mut y = inner.y;
        let x = inner.x + 1;
        let w = inner.w.saturating_sub(2);
        let put = |canvas: &mut Canvas, y: &mut usize, text: &str, style: Style| {
            if *y < inner.y + inner.h {
                canvas.put(x, *y, &fit(text, w), style);
                *y += 1;
            }
        };
        put(
            canvas,
            &mut y,
            &row.name,
            Style::default().fg(Color::Ansi(15)).bold(),
        );
        if row.running {
            let clients = match row.clients {
                0 => "nobody attached".to_string(),
                1 => "1 client attached".to_string(),
                n => format!("{} clients attached", n),
            };
            put(
                canvas,
                &mut y,
                &format!(
                    "● running · {} · started {} ago",
                    clients,
                    age_long(row.age)
                ),
                Style::default().fg(Color::Ansi(10)),
            );
        } else {
            put(
                canvas,
                &mut y,
                &format!("○ stopped · screen saved {} ago", age_long(row.age)),
                Style::default().fg(Color::Ansi(8)),
            );
        }
        y += 1;
        if row.tabs.is_empty() {
            put(
                canvas,
                &mut y,
                "No pane information yet.",
                Style::default().fg(Color::Ansi(8)),
            );
        }
        for (tab_idx, tab) in row.tabs.iter().enumerate() {
            let marker = if tab.active { "▸" } else { " " };
            let tab_line = if row.tabs.len() > 1 || !tab.name.is_empty() {
                format!("{} Tab {} · {}", marker, tab_idx + 1, tab.name)
            } else {
                format!("{} Tab {}", marker, tab_idx + 1)
            };
            let tab_style = if tab.active {
                Style::default().fg(Color::Ansi(7)).bold()
            } else {
                Style::default().fg(Color::Ansi(8))
            };
            put(canvas, &mut y, &tab_line, tab_style);
            for pane in &tab.panes {
                let what = pane
                    .command
                    .clone()
                    .filter(|c| !c.is_empty())
                    .unwrap_or_else(|| pane.title.clone());
                let mut line = format!("    {}", what);
                if let Some(cwd) = &pane.cwd {
                    if !cwd.is_empty() {
                        line.push_str("   ");
                        line.push_str(cwd);
                    }
                }
                if pane.exited {
                    line.push_str("   (exited)");
                }
                let style = if pane.focused {
                    Style::default().fg(Color::Ansi(14))
                } else {
                    Style::default().fg(Color::Ansi(7))
                };
                put(canvas, &mut y, &line, style);
            }
        }
        y += 1;
        let hint = if row.running {
            "⏎ open · t terminate (keeps the screen) · d delete"
        } else {
            "⏎ open (restores the screen) · d delete"
        };
        put(
            canvas,
            &mut y,
            hint,
            Style::default().fg(Color::Ansi(8)).italic(),
        );
        if row.running {
            put(
                canvas,
                &mut y,
                "O open and reprint the whole history",
                Style::default().fg(Color::Ansi(8)).italic(),
            );
        }
    }

    fn modal_rect(&self, over: Rect, width: usize, height: usize) -> Rect {
        let w = width.min(over.w.saturating_sub(2)).max(10);
        let h = height.min(over.h.saturating_sub(2)).max(3);
        let x = over.x + (over.w.saturating_sub(w)) / 2;
        let y = over.y + (over.h.saturating_sub(h)) / 2;
        Rect::new(x, y, w, h)
    }

    fn render_modal_frame(&mut self, canvas: &mut Canvas, rect: Rect, title: &str, color: Color) {
        canvas.clear_rect(rect, Style::default());
        canvas.frame(
            rect,
            title,
            Style::default().fg(color),
            Style::default().fg(color).bold(),
        );
    }

    fn render_buttons(
        &mut self,
        canvas: &mut Canvas,
        y: usize,
        rect: Rect,
        buttons: &[(&str, Key, Color)],
    ) {
        let total: usize = buttons
            .iter()
            .map(|(label, _, _)| label.chars().count() + 4)
            .sum::<usize>()
            + (buttons.len().saturating_sub(1)) * 2;
        let mut x = rect.x + rect.w.saturating_sub(total) / 2;
        for (label, key, color) in buttons {
            let text = format!("[ {} ]", label);
            let width = text.chars().count();
            canvas.put(x, y, &text, Style::default().fg(*color).bold());
            self.hits.push(HitRegion {
                rect: Rect::new(x, y, width, 1),
                hit: Hit::Key(key.clone()),
            });
            x += width + 2;
        }
    }

    fn render_confirm(&mut self, canvas: &mut Canvas, over: Rect, action: Confirmable, name: &str) {
        let (title, lines, color): (&str, Vec<String>, Color) = match action {
            Confirmable::Terminate => (
                " Terminate session ",
                vec![
                    format!("Terminate '{}'?", name),
                    String::new(),
                    "Its programs are stopped. The screen and the whole".to_string(),
                    "scrollback stay saved, so the session can be opened".to_string(),
                    "again later with everything it showed.".to_string(),
                ],
                Color::Ansi(11),
            ),
            Confirmable::Delete => (
                " Delete session ",
                vec![
                    format!("Delete '{}'?", name),
                    String::new(),
                    "This stops the session if it is running and throws".to_string(),
                    "away its saved screen. It cannot be opened again.".to_string(),
                ],
                Color::Ansi(9),
            ),
        };
        let rect = self.modal_rect(over, 58, lines.len() + 5);
        self.render_modal_frame(canvas, rect, title, color);
        let inner = rect.inner();
        for (i, line) in lines.iter().enumerate() {
            if i + 1 < inner.h {
                let style = if i == 0 {
                    Style::default().bold()
                } else {
                    Style::default().fg(Color::Ansi(7))
                };
                canvas.put(
                    inner.x + 2,
                    inner.y + i,
                    &fit(line, inner.w.saturating_sub(4)),
                    style,
                );
            }
        }
        let y = inner.y + inner.h - 1;
        self.render_buttons(
            canvas,
            y,
            inner,
            &[
                ("y  Yes", Key::Char('y'), color),
                ("n  No", Key::Char('n'), Color::Ansi(7)),
            ],
        );
    }

    fn render_new_session(&mut self, canvas: &mut Canvas, over: Rect, name: &str) {
        let rect = self.modal_rect(over, 58, 8);
        self.render_modal_frame(canvas, rect, " New session ", Color::Ansi(14));
        let inner = rect.inner();
        canvas.put(
            inner.x + 2,
            inner.y,
            "Name",
            Style::default().fg(Color::Ansi(7)),
        );
        let field_w = inner.w.saturating_sub(4);
        let field = format!(
            "{:<width$}",
            fit(name, field_w.saturating_sub(1)),
            width = field_w
        );
        canvas.put(inner.x + 2, inner.y + 1, &field, Style::default().reverse());
        let cursor_x = inner.x + 2 + name.chars().count().min(field_w.saturating_sub(1));
        canvas.put(
            cursor_x,
            inner.y + 1,
            "▏",
            Style::default().reverse().fg(Color::Ansi(14)),
        );
        canvas.put(
            inner.x + 2,
            inner.y + 3,
            &fit(
                "Letters, digits, '-', '_' and '.'. The session opens",
                inner.w.saturating_sub(4),
            ),
            Style::default().fg(Color::Ansi(8)),
        );
        canvas.put(
            inner.x + 2,
            inner.y + 4,
            &fit(
                "in the directory this dashboard was started from.",
                inner.w.saturating_sub(4),
            ),
            Style::default().fg(Color::Ansi(8)),
        );
        let y = inner.y + inner.h - 1;
        self.render_buttons(
            canvas,
            y,
            inner,
            &[
                ("⏎  Create", Key::Enter, Color::Ansi(10)),
                ("esc  Cancel", Key::Esc, Color::Ansi(7)),
            ],
        );
    }

    fn render_shutdown(&mut self, canvas: &mut Canvas, over: Rect, typed: &str) {
        let running = self.rows.iter().filter(|r| r.running).count();
        let rect = self.modal_rect(over, 60, 10);
        self.render_modal_frame(canvas, rect, " Shut down zellij ", Color::Ansi(9));
        let inner = rect.inner();
        let lines = [
            format!(
                "This terminates all {} running session(s) and closes",
                running
            ),
            "the dashboard. Every screen and scrollback is saved first,".to_string(),
            "so the sessions can be opened again later.".to_string(),
        ];
        for (i, line) in lines.iter().enumerate() {
            canvas.put(
                inner.x + 2,
                inner.y + i,
                &fit(line, inner.w.saturating_sub(4)),
                Style::default().fg(Color::Ansi(7)),
            );
        }
        let prompt = format!("Type  {}  and press Enter:", SHUTDOWN_WORD);
        canvas.put(inner.x + 2, inner.y + 4, &prompt, Style::default().bold());
        let field = format!("{:<8}", typed);
        canvas.put(inner.x + 2, inner.y + 5, &field, Style::default().reverse());
        let y = inner.y + inner.h - 1;
        self.render_buttons(
            canvas,
            y,
            inner,
            &[
                ("⏎  Shut down", Key::Enter, Color::Ansi(9)),
                ("esc  Cancel", Key::Esc, Color::Ansi(7)),
            ],
        );
    }

    fn render_help(&mut self, canvas: &mut Canvas, over: Rect) {
        let entries: Vec<(&str, &str)> = vec![
            ("↑ ↓  j k", "move the selection"),
            ("⏎  o  space", "open the session (or resurrect a saved one)"),
            ("O", "open and reprint its whole history into this terminal"),
            (
                "n",
                "new session (named after the prompt, in this directory)",
            ),
            (
                "t  x",
                "terminate: stop its programs, keep the saved screen",
            ),
            ("d  del", "delete: stop it and throw the saved screen away"),
            ("r", "refresh the list (it also refreshes every 2 seconds)"),
            ("Q", "shut down: terminate every session, then quit"),
            ("q  esc", "quit the dashboard; sessions keep running"),
            ("", ""),
            ("Ctrl q", "inside a session: leave it and come back here"),
            (
                "mouse",
                "click a session to select it, click again to open it",
            ),
        ];
        let rect = self.modal_rect(over, 70, entries.len() + 3);
        self.render_modal_frame(canvas, rect, " Keys ", Color::Ansi(14));
        let inner = rect.inner();
        for (i, (keys, text)) in entries.iter().enumerate() {
            if i >= inner.h {
                break;
            }
            canvas.put(
                inner.x + 2,
                inner.y + i,
                keys,
                Style::default().fg(Color::Ansi(11)).bold(),
            );
            canvas.put(
                inner.x + 15,
                inner.y + i,
                &fit(text, inner.w.saturating_sub(17)),
                Style::default().fg(Color::Ansi(7)),
            );
        }
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
                full_history: false
            }
        );
        assert_eq!(
            d.handle_key(Key::Char('O')),
            Command::Open {
                name: "beta".to_string(),
                full_history: true
            }
        );
    }

    #[test]
    fn enter_on_an_empty_list_prompts_for_a_new_session() {
        let mut d = Dashboard::new(80, 24);
        d.set_suggested_name("first".to_string());
        assert_eq!(d.handle_key(Key::Enter), Command::None);
        assert_eq!(
            d.mode,
            Mode::NewSession {
                name: "first".to_string()
            }
        );
    }

    #[test]
    fn new_session_prompt_edits_and_validates() {
        let mut d = dashboard();
        d.handle_key(Key::Char('n'));
        assert_eq!(
            d.mode,
            Mode::NewSession {
                name: "fresh-name".to_string()
            }
        );
        d.handle_key(Key::Ctrl('u'));
        for c in "my session/1".chars() {
            d.handle_key(Key::Char(c));
        }
        assert_eq!(
            d.mode,
            Mode::NewSession {
                name: "mysession1".to_string()
            },
            "spaces and slashes are dropped"
        );
        d.handle_key(Key::Backspace);
        assert_eq!(
            d.handle_key(Key::Enter),
            Command::New {
                name: "mysession".to_string()
            }
        );
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
        assert_eq!(
            d.mode,
            Mode::Confirm {
                action: Confirmable::Terminate,
                name: "alpha".to_string()
            }
        );
        assert_eq!(d.handle_key(Key::Char('n')), Command::None);
        assert_eq!(d.mode, Mode::List);
        d.handle_key(Key::Char('t'));
        assert_eq!(
            d.handle_key(Key::Char('y')),
            Command::Terminate("alpha".to_string())
        );
        d.handle_key(Key::Char('d'));
        assert_eq!(
            d.handle_key(Key::Enter),
            Command::Delete("alpha".to_string())
        );
        // a stopped session cannot be terminated
        d.handle_key(Key::End);
        assert_eq!(d.handle_key(Key::Char('t')), Command::None);
        assert_eq!(d.mode, Mode::List);
        assert!(d.notice.is_some());
    }

    #[test]
    fn shutdown_requires_the_word() {
        let mut d = dashboard();
        d.handle_key(Key::Char('Q'));
        assert_eq!(
            d.mode,
            Mode::Shutdown {
                typed: String::new()
            }
        );
        for c in "no".chars() {
            d.handle_key(Key::Char(c));
        }
        assert_eq!(d.handle_key(Key::Enter), Command::None);
        assert_eq!(
            d.mode,
            Mode::Shutdown {
                typed: String::new()
            }
        );
        for c in "yes".chars() {
            d.handle_key(Key::Char(c));
        }
        assert_eq!(d.handle_key(Key::Enter), Command::Shutdown);
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
    fn render_lists_sessions_and_records_hit_regions() {
        let mut d = dashboard();
        let frame = d.render();
        let plain = strip_ansi(&frame);
        assert!(plain.contains("alpha"));
        assert!(plain.contains("gamma"));
        assert!(plain.contains("Sessions (3)"));
        assert!(plain.contains("Details"));
        assert!(plain.contains("shut down"));
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
                full_history: false
            }
        );
        // the power button opens the shutdown prompt
        let power = d
            .hits
            .iter()
            .find(|h| h.hit == Hit::Key(Key::Char('Q')))
            .unwrap()
            .rect;
        d.handle_mouse(Mouse::Press {
            x: power.x + 1,
            y: power.y,
        });
        assert!(matches!(d.mode, Mode::Shutdown { .. }));
        // and the wheel moves the selection only in list mode
        d.handle_mouse(Mouse::WheelDown);
        assert_eq!(d.selected, 1);
        d.handle_key(Key::Esc);
        d.handle_mouse(Mouse::WheelDown);
        assert_eq!(d.selected, 2);
    }

    #[test]
    fn narrow_terminals_drop_the_details_panel() {
        let mut d = dashboard();
        d.resize(80, 20);
        let plain = strip_ansi(&d.render());
        assert!(plain.contains("alpha"));
        assert!(!plain.contains("Details"));
    }

    #[test]
    fn list_scrolls_to_keep_the_selection_visible() {
        let mut d = Dashboard::new(80, 10);
        d.set_rows((0..20).map(|i| row(&format!("s{:02}", i), true)).collect());
        d.render();
        let visible = d.list_rows_visible;
        assert!(visible < 20);
        d.handle_key(Key::End);
        d.render();
        assert_eq!(d.scroll, 20 - visible);
        let plain = strip_ansi(&d.render());
        assert!(plain.contains("s19"));
        assert!(!plain.contains("s00"));
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
        assert_eq!(r.summary(), "zsh · ~/work");
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
