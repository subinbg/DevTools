//! Host-terminal scrollback (DevTools fork, `host_scrollback` option).
//!
//! A lone terminal pane that fills the screen is drawn straight into the host terminal's
//! primary screen. Whenever output scrolls rows out of the top of its viewport, the host
//! terminal is made to scroll by the same number of rows with real line feeds, so those rows
//! land in the host terminal's own scrollback and its scrollbar keeps working. A client that
//! attaches gets the pane's history printed the same way first, minus the rows its terminal
//! reported to hold already.

use std::collections::{HashMap, HashSet};

use zellij_utils::ipc::HostScrollState;

use crate::output::Output;
use crate::tab::Pane;
use crate::ClientId;

/// What the host-terminal scrollback needs to know about a terminal pane at render time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HostScrollSnapshot {
    /// Rows scrolled out of the viewport by output since they were last taken.
    pub pending: usize,
    /// Absolute index of the viewport's first row (rows that ever scrolled out of it).
    pub rows_scrolled_total: u64,
    /// Rows of history that can still be rendered.
    pub history_available: usize,
    pub alternate_screen: bool,
    pub mouse_tracking: u8,
    pub mouse_sgr: bool,
    /// The user scrolled the viewport up (zellij's scroll mode).
    pub is_scrolled: bool,
}

/// Escape sequences that scroll `rows` (oldest first) into the host terminal's scrollback:
/// each row is written at the top of the screen and then pushed out with line feeds from the
/// bottom row, at most a screen's worth at a time. `missing_rows` are rows that scrolled but
/// can no longer be rendered; the screen's own top rows are pushed for them.
pub fn host_scroll_sequence(rows: &[String], missing_rows: usize, viewport_rows: usize) -> String {
    let mut out = String::from("\u{1b}[?25l");
    if missing_rows > 0 {
        out.push_str("\u{1b}[9999;1H");
        out.push_str(&"\n".repeat(missing_rows));
    }
    for batch in rows.chunks(viewport_rows.max(1)) {
        for (i, row) in batch.iter().enumerate() {
            out.push_str(&format!("\u{1b}[{};1H{}\u{1b}[0m\u{1b}[K", i + 1, row));
        }
        out.push_str("\u{1b}[9999;1H");
        out.push_str(&"\n".repeat(batch.len()));
    }
    out
}

/// Runs the host scrollback for one terminal pane during a render pass.
///
/// `eligible` says whether the pane is the only pane and fills the whole display; if not,
/// the pending rows are simply dropped (the pane is redrawn in place as usual). Clients not
/// yet primed get their history dump; primed ones get the rows that scrolled since the last
/// render. Every client gets the pane's state reported with the render.
pub fn apply(
    pane: &mut Box<dyn Pane>,
    pane_id: u32,
    output: &mut Output,
    connected_clients: &[ClientId],
    client_seen: &HashMap<ClientId, Option<u64>>,
    primed: &mut HashSet<ClientId>,
    last_state: &mut HashMap<ClientId, HostScrollState>,
    eligible: bool,
    viewport_rows: usize,
) {
    let Some(snapshot) = pane.host_scroll_snapshot() else {
        return;
    };
    if !eligible {
        let _ = pane.take_host_scroll_pending();
        return;
    }
    // while the program in the pane uses the alternate screen (mirrored to the host
    // terminal) or the viewport is scrolled up, nothing can be pushed into the host
    // terminal's scrollback; the rows wait for the next render on the primary screen
    let primary_and_live = !snapshot.alternate_screen && !snapshot.is_scrolled;
    // only clients that draw the session into their host terminal take part
    let participating: Vec<ClientId> = connected_clients
        .iter()
        .copied()
        .filter(|client_id| client_seen.contains_key(client_id))
        .collect();
    let (dump_clients, scroll_clients): (Vec<ClientId>, Vec<ClientId>) = participating
        .iter()
        .copied()
        .partition(|client_id| !primed.contains(client_id));

    if primary_and_live {
        let pending = pane.take_host_scroll_pending();
        if pending > 0 && !scroll_clients.is_empty() {
            let rows = pane.host_scroll_history(pending.min(snapshot.history_available));
            let missing = pending.saturating_sub(rows.len());
            let sequence = host_scroll_sequence(&rows, missing, viewport_rows);
            output.add_pre_vte_instruction_to_multiple_clients(
                scroll_clients.iter().copied(),
                &sequence,
            );
        }
        for client_id in dump_clients {
            let seen = client_seen.get(&client_id).copied().flatten();
            let count = match seen {
                Some(seen) => snapshot.rows_scrolled_total.saturating_sub(seen) as usize,
                None => snapshot.history_available,
            }
            .min(snapshot.history_available);
            if count > 0 {
                let rows = pane.host_scroll_history(count);
                let sequence = host_scroll_sequence(&rows, 0, viewport_rows);
                output.add_pre_vte_instruction_to_client(client_id, &sequence);
            }
            primed.insert(client_id);
        }
    }

    // report the state to every client whose view of it changed; a changed state makes the
    // render worth sending even when nothing visible changed (a program asking for the mouse)
    let state = HostScrollState {
        pane_id,
        rows_scrolled: snapshot.rows_scrolled_total,
        viewport_rows: viewport_rows as u32,
        alternate_screen: snapshot.alternate_screen,
        mouse_tracking: snapshot.mouse_tracking,
        mouse_sgr: snapshot.mouse_sgr,
    };
    for client_id in &participating {
        if last_state.get(client_id) != Some(&state) {
            last_state.insert(*client_id, state);
            output.set_host_scroll_state(*client_id, state);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_single_scrolled_row_is_written_at_the_top_and_pushed_out() {
        let seq = host_scroll_sequence(&["row one".to_string()], 0, 24);
        assert_eq!(
            seq,
            "\u{1b}[?25l\u{1b}[1;1Hrow one\u{1b}[0m\u{1b}[K\u{1b}[9999;1H\n"
        );
    }

    #[test]
    fn rows_are_pushed_a_screen_at_a_time() {
        let rows: Vec<String> = (0..5).map(|i| format!("r{}", i)).collect();
        let seq = host_scroll_sequence(&rows, 0, 2);
        // batches of two rows, each followed by two line feeds; then the last row alone
        assert_eq!(seq.matches("\u{1b}[9999;1H").count(), 3);
        assert_eq!(seq.matches('\n').count(), 5);
        assert!(seq.contains(
            "\u{1b}[1;1Hr0\u{1b}[0m\u{1b}[K\u{1b}[2;1Hr1\u{1b}[0m\u{1b}[K\u{1b}[9999;1H\n\n"
        ));
        assert!(seq.ends_with("\u{1b}[1;1Hr4\u{1b}[0m\u{1b}[K\u{1b}[9999;1H\n"));
    }

    #[test]
    fn missing_rows_push_the_screen_as_it_is() {
        let seq = host_scroll_sequence(&[], 3, 24);
        assert_eq!(seq, "\u{1b}[?25l\u{1b}[9999;1H\n\n\n");
        let seq = host_scroll_sequence(&["x".to_string()], 2, 24);
        assert!(seq.starts_with("\u{1b}[?25l\u{1b}[9999;1H\n\n\u{1b}[1;1Hx"));
    }
}
