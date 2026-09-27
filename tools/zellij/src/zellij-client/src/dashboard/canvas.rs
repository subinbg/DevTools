//! A small cell canvas the dashboard draws into, serialized to ANSI once per frame. Using
//! a cell grid keeps overlays (the modals) simple and guarantees every row is exactly as
//! wide as the terminal.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Color {
    Default,
    /// One of the terminal's 16 ANSI colors (0-7 normal, 8-15 bright), so the dashboard
    /// follows the terminal's color scheme like lazygit does.
    Ansi(u8),
}

impl Default for Color {
    fn default() -> Self {
        Color::Default
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Style {
    pub fg: Color,
    pub bg: Color,
    pub bold: bool,
    pub dim: bool,
    pub italic: bool,
    pub reverse: bool,
}

impl Style {
    pub fn fg(mut self, color: Color) -> Self {
        self.fg = color;
        self
    }
    pub fn bg(mut self, color: Color) -> Self {
        self.bg = color;
        self
    }
    pub fn bold(mut self) -> Self {
        self.bold = true;
        self
    }
    pub fn dim(mut self) -> Self {
        self.dim = true;
        self
    }
    pub fn italic(mut self) -> Self {
        self.italic = true;
        self
    }
    pub fn reverse(mut self) -> Self {
        self.reverse = true;
        self
    }
    fn sgr(&self) -> String {
        let mut codes: Vec<String> = vec!["0".to_string()];
        if self.bold {
            codes.push("1".to_string());
        }
        if self.dim {
            codes.push("2".to_string());
        }
        if self.italic {
            codes.push("3".to_string());
        }
        if self.reverse {
            codes.push("7".to_string());
        }
        match self.fg {
            Color::Default => {},
            Color::Ansi(n) if n < 8 => codes.push((30 + n).to_string()),
            Color::Ansi(n) => codes.push((90 + (n - 8).min(7)).to_string()),
        }
        match self.bg {
            Color::Default => {},
            Color::Ansi(n) if n < 8 => codes.push((40 + n).to_string()),
            Color::Ansi(n) => codes.push((100 + (n - 8).min(7)).to_string()),
        }
        format!("\u{1b}[{}m", codes.join(";"))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Rect {
    pub x: usize,
    pub y: usize,
    pub w: usize,
    pub h: usize,
}

impl Rect {
    pub fn new(x: usize, y: usize, w: usize, h: usize) -> Self {
        Rect { x, y, w, h }
    }
    pub fn contains(&self, x: usize, y: usize) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }
    /// The area inside a one-cell frame.
    pub fn inner(&self) -> Rect {
        Rect {
            x: self.x + 1,
            y: self.y + 1,
            w: self.w.saturating_sub(2),
            h: self.h.saturating_sub(2),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Cell {
    ch: char,
    style: Style,
}

pub struct Canvas {
    cols: usize,
    lines: usize,
    cells: Vec<Cell>,
}

impl Canvas {
    pub fn new(cols: usize, lines: usize) -> Self {
        Canvas {
            cols,
            lines,
            cells: vec![
                Cell {
                    ch: ' ',
                    style: Style::default()
                };
                cols * lines
            ],
        }
    }
    pub fn cols(&self) -> usize {
        self.cols
    }
    pub fn lines(&self) -> usize {
        self.lines
    }
    fn set(&mut self, x: usize, y: usize, ch: char, style: Style) {
        if x < self.cols && y < self.lines {
            self.cells[y * self.cols + x] = Cell { ch, style };
        }
    }
    /// Writes text starting at a cell; anything past the right edge is dropped.
    pub fn put(&mut self, x: usize, y: usize, text: &str, style: Style) {
        for (i, ch) in text.chars().enumerate() {
            if x + i >= self.cols {
                break;
            }
            self.set(x + i, y, ch, style);
        }
    }
    pub fn fill_row(&mut self, y: usize, ch: char, style: Style) {
        self.fill_row_range(y, 0, self.cols, ch, style);
    }
    pub fn fill_row_range(&mut self, y: usize, x: usize, w: usize, ch: char, style: Style) {
        for i in 0..w {
            self.set(x + i, y, ch, style);
        }
    }
    pub fn clear_rect(&mut self, rect: Rect, style: Style) {
        for y in rect.y..rect.y + rect.h {
            self.fill_row_range(y, rect.x, rect.w, ' ', style);
        }
    }
    /// Draws a rounded frame with a title in its top border.
    pub fn frame(&mut self, rect: Rect, title: &str, style: Style, title_style: Style) {
        if rect.w < 2 || rect.h < 2 {
            return;
        }
        let right = rect.x + rect.w - 1;
        let bottom = rect.y + rect.h - 1;
        self.set(rect.x, rect.y, '╭', style);
        self.set(right, rect.y, '╮', style);
        self.set(rect.x, bottom, '╰', style);
        self.set(right, bottom, '╯', style);
        for x in rect.x + 1..right {
            self.set(x, rect.y, '─', style);
            self.set(x, bottom, '─', style);
        }
        for y in rect.y + 1..bottom {
            self.set(rect.x, y, '│', style);
            self.set(right, y, '│', style);
        }
        if !title.is_empty() && rect.w > 4 {
            let max = rect.w - 4;
            let title: String = title.chars().take(max).collect();
            self.put(rect.x + 2, rect.y, &title, title_style);
        }
    }
    /// Serializes the canvas: home the cursor, then every row with minimal style changes.
    pub fn to_ansi(&self) -> String {
        let mut out = String::with_capacity(self.cols * self.lines * 2);
        out.push_str("\u{1b}[H");
        for y in 0..self.lines {
            out.push_str(&format!("\u{1b}[{};1H", y + 1));
            let mut current: Option<Style> = None;
            for x in 0..self.cols {
                let cell = self.cells[y * self.cols + x];
                if current != Some(cell.style) {
                    out.push_str(&cell.style.sgr());
                    current = Some(cell.style);
                }
                out.push(cell.ch);
            }
            out.push_str("\u{1b}[0m");
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_and_text_stay_inside_the_canvas() {
        let mut c = Canvas::new(10, 3);
        c.frame(
            Rect::new(0, 0, 10, 3),
            " T ",
            Style::default(),
            Style::default().bold(),
        );
        c.put(8, 1, "abcdef", Style::default());
        let ansi = c.to_ansi();
        let plain: String = {
            let mut out = String::new();
            let mut chars = ansi.chars().peekable();
            while let Some(ch) = chars.next() {
                if ch == '\u{1b}' {
                    while let Some(n) = chars.next() {
                        if n.is_ascii_alphabetic() {
                            break;
                        }
                    }
                    continue;
                }
                out.push(ch);
            }
            out
        };
        assert!(plain.contains("╭─ T ─"), "{}", plain);
        assert!(plain.contains("╰────────╯"));
        assert!(
            plain.contains("│       ab╰"),
            "text past the edge is dropped: {}",
            plain
        );
        assert!(!plain.contains("abc"));
    }

    #[test]
    fn styles_serialize_to_sgr() {
        assert_eq!(Style::default().sgr(), "\u{1b}[0m");
        assert_eq!(
            Style::default().fg(Color::Ansi(2)).bold().sgr(),
            "\u{1b}[0;1;32m"
        );
        assert_eq!(
            Style::default()
                .fg(Color::Ansi(12))
                .bg(Color::Ansi(4))
                .sgr(),
            "\u{1b}[0;94;44m"
        );
    }

    #[test]
    fn rect_contains_and_inner() {
        let r = Rect::new(2, 3, 5, 4);
        assert!(r.contains(2, 3));
        assert!(r.contains(6, 6));
        assert!(!r.contains(7, 6));
        assert!(!r.contains(2, 7));
        assert_eq!(r.inner(), Rect::new(3, 4, 3, 2));
    }
}
