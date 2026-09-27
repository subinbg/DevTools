//! Remembers, per host terminal, how much of each session's pane history has already been
//! printed into that terminal's scrollback, so that opening a session again in the same
//! terminal window continues the scrollback instead of reprinting it.
//!
//! The terminal is identified by the environment variables terminal emulators set per
//! window or tab (iTerm2, Terminal.app, WezTerm, kitty, tmux, X11). The state lives in a
//! small JSON file in zellij's cache directory. A terminal without such an identity gets no
//! persistence: every new `zellij` process then reprints the history once.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use zellij_utils::consts::ZELLIJ_CACHE_DIR;

#[derive(Default, Serialize, Deserialize)]
struct Stored {
    seen: BTreeMap<String, u64>,
}

pub struct TerminalMemory {
    path: Option<PathBuf>,
    seen: BTreeMap<String, u64>,
}

impl TerminalMemory {
    pub fn load() -> Self {
        let path = terminal_identity().map(|id| {
            ZELLIJ_CACHE_DIR
                .join("host_scrollback")
                .join(format!("{}.json", id))
        });
        Self::load_from(path)
    }

    fn load_from(path: Option<PathBuf>) -> Self {
        let seen = path
            .as_ref()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|raw| serde_json::from_str::<Stored>(&raw).ok())
            .map(|stored| stored.seen)
            .unwrap_or_default();
        TerminalMemory { path, seen }
    }

    /// In-memory only (no terminal identity).
    pub fn ephemeral() -> Self {
        TerminalMemory {
            path: None,
            seen: BTreeMap::new(),
        }
    }

    /// The number of history rows of this session's pane that this terminal already holds.
    pub fn rows_seen(&self, session: &str) -> Option<u64> {
        self.seen.get(session).copied()
    }

    pub fn remember(&mut self, session: &str, rows: u64) {
        self.seen.insert(session.to_string(), rows);
        self.save();
    }

    pub fn forget(&mut self, session: &str) {
        if self.seen.remove(session).is_some() {
            self.save();
        }
    }

    fn save(&self) {
        let Some(path) = &self.path else {
            return;
        };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let stored = Stored {
            seen: self.seen.clone(),
        };
        if let Ok(json) = serde_json::to_string(&stored) {
            let _ = std::fs::write(path, json);
        }
    }
}

/// A stable identity for the terminal window or tab this process runs in, if the terminal
/// emulator exposes one.
pub fn terminal_identity() -> Option<String> {
    terminal_identity_from(|name| std::env::var(name).ok())
}

fn terminal_identity_from<F: Fn(&str) -> Option<String>>(lookup: F) -> Option<String> {
    let value = |name: &str| lookup(name).filter(|v| !v.trim().is_empty());
    if let Some(id) = value("ITERM_SESSION_ID") {
        return Some(format!("iterm-{}", sanitize(&id)));
    }
    if let Some(id) = value("TERM_SESSION_ID") {
        return Some(format!("term-{}", sanitize(&id)));
    }
    if let Some(id) = value("WEZTERM_PANE") {
        return Some(format!("wezterm-{}", sanitize(&id)));
    }
    if let Some(id) = value("KITTY_WINDOW_ID") {
        let pid = value("KITTY_PID").unwrap_or_default();
        return Some(format!("kitty-{}-{}", sanitize(&pid), sanitize(&id)));
    }
    if let Some(id) = value("TMUX_PANE") {
        return Some(format!("tmux-{}", sanitize(&id)));
    }
    if let Some(id) = value("WINDOWID") {
        return Some(format!("x11-{}", sanitize(&id)));
    }
    None
}

fn sanitize(value: &str) -> String {
    value
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_prefers_the_terminal_tab_id() {
        let id = terminal_identity_from(|name| match name {
            "ITERM_SESSION_ID" => Some("w0t3p0:4B5F-12".to_string()),
            "TERM_SESSION_ID" => Some("other".to_string()),
            _ => None,
        });
        assert_eq!(id.as_deref(), Some("iterm-w0t3p0_4B5F-12"));
        let id = terminal_identity_from(|name| match name {
            "KITTY_WINDOW_ID" => Some("3".to_string()),
            "KITTY_PID" => Some("100".to_string()),
            _ => None,
        });
        assert_eq!(id.as_deref(), Some("kitty-100-3"));
        assert_eq!(terminal_identity_from(|_| None), None);
        assert_eq!(terminal_identity_from(|_| Some("  ".to_string())), None);
    }

    #[test]
    fn memory_round_trips_through_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.json");
        let mut memory = TerminalMemory::load_from(Some(path.clone()));
        assert_eq!(memory.rows_seen("a"), None);
        memory.remember("a", 120);
        memory.remember("b", 7);
        let reloaded = TerminalMemory::load_from(Some(path.clone()));
        assert_eq!(reloaded.rows_seen("a"), Some(120));
        assert_eq!(reloaded.rows_seen("b"), Some(7));
        memory.forget("a");
        let reloaded = TerminalMemory::load_from(Some(path));
        assert_eq!(reloaded.rows_seen("a"), None);
        let mut ephemeral = TerminalMemory::ephemeral();
        ephemeral.remember("x", 1);
        assert_eq!(ephemeral.rows_seen("x"), Some(1));
    }
}
