//! The session bar (DevTools fork, `session_bar` option): one line at the bottom of a
//! session showing its name and the key that returns to the dashboard. It is drawn by the
//! tiled panes renderer under the lone terminal pane, which gives up its last row for it.

use crate::output::CharacterChunk;
use crate::panes::terminal_character::{AnsiCode, CharacterStyles, RESET_STYLES};
use crate::panes::TerminalCharacter;

const RETURN_KEY: &str = "Ctrl q";
const RETURN_LABEL: &str = "dashboard";

fn push(cells: &mut Vec<TerminalCharacter>, text: &str, styles: CharacterStyles) {
    for character in text.chars() {
        cells.push(TerminalCharacter::new_styled(character, styles.into()));
    }
}

/// The bar's row as one chunk `width` cells wide at column `x`, row `y`.
pub fn session_bar_chunk(session_name: &str, x: usize, y: usize, width: usize) -> CharacterChunk {
    let muted = RESET_STYLES.foreground(Some(AnsiCode::ColorIndex(8)));
    let dot = RESET_STYLES.foreground(Some(AnsiCode::ColorIndex(10)));
    let name = RESET_STYLES
        .foreground(Some(AnsiCode::ColorIndex(7)))
        .bold(Some(AnsiCode::On));
    let key = RESET_STYLES
        .foreground(Some(AnsiCode::ColorIndex(11)))
        .bold(Some(AnsiCode::On));

    let mut cells: Vec<TerminalCharacter> = Vec::with_capacity(width);
    push(&mut cells, " ", muted);
    push(&mut cells, "●", dot);
    push(&mut cells, " ", muted);
    push(&mut cells, session_name, name);

    let hint_width = RETURN_KEY.chars().count() + 2 + RETURN_LABEL.chars().count() + 1;
    if cells.len() + 2 + hint_width <= width {
        while cells.len() < width - hint_width {
            push(&mut cells, " ", muted);
        }
        push(&mut cells, RETURN_KEY, key);
        push(&mut cells, "  ", muted);
        push(&mut cells, RETURN_LABEL, muted);
        push(&mut cells, " ", muted);
    }
    cells.truncate(width);
    while cells.len() < width {
        push(&mut cells, " ", muted);
    }
    CharacterChunk::new(cells, x, y)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text_of(chunk: &CharacterChunk) -> String {
        chunk
            .terminal_characters
            .iter()
            .map(|c| c.character)
            .collect()
    }

    #[test]
    fn bar_shows_the_name_and_the_return_key() {
        let chunk = session_bar_chunk("alpha", 0, 29, 60);
        let text = text_of(&chunk);
        assert_eq!(text.chars().count(), 60);
        assert!(text.starts_with(" ● alpha"));
        assert!(text.ends_with("Ctrl q  dashboard "));
        assert_eq!(chunk.y, 29);
        assert_eq!(chunk.x, 0);
    }

    #[test]
    fn narrow_bars_drop_the_hint_and_never_exceed_the_width() {
        let text = text_of(&session_bar_chunk("a-very-long-session-name", 0, 0, 20));
        assert_eq!(text.chars().count(), 20);
        assert!(!text.contains("Ctrl q"));
        assert!(text.starts_with(" ● a-very-long-sessi"));
        let text = text_of(&session_bar_chunk("s", 3, 1, 8));
        assert_eq!(text.chars().count(), 8);
    }
}
