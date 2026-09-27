//! The session dashboard (DevTools fork): a full-screen list of sessions that `zellij`
//! opens when started without arguments and returns to whenever a session is left with
//! Ctrl q. Sessions can be opened, created, terminated (stopped with their screens kept)
//! and deleted from here, with the keyboard or the mouse. Shutting every session down is
//! behind a typed confirmation.

pub mod canvas;
pub mod memory;
pub mod model;
pub mod run;
pub mod sessions;

pub use memory::TerminalMemory;
pub use run::{
    push_screen_into_history, restore_terminal, run_dashboard, DashboardOutcome, DashboardSetup,
};
