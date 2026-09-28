//! Ties the dashboard and the sessions together for the `zellij` binary (DevTools fork):
//! show the dashboard, open the chosen session in this terminal, and come back to the
//! dashboard when the session is left with Ctrl q.

use zellij_client::{
    dashboard::{self, DashboardOutcome, DashboardSetup},
    os_input_output::ClientOsApi,
    start_client_ext, ClientInfo, TerminalHandling,
};
use zellij_utils::{
    cli::CliArgs,
    consts::session_layout_cache_file_name,
    data::LayoutInfo,
    input::{config::Config, options::Options},
    ipc::ExitReason,
    sessions::{resurrection_layout, session_exists},
};

pub(crate) fn dashboard_enabled(config_options: &Options, os_input: &dyn ClientOsApi) -> bool {
    config_options.dashboard.unwrap_or(true)
        && os_input.stdin_is_terminal()
        && os_input.stdout_is_terminal()
}

pub(crate) fn host_scrollback_enabled(config_options: &Options) -> bool {
    config_options.host_scrollback.unwrap_or(true)
}

pub(crate) struct DashboardFlow {
    os_input: Box<dyn ClientOsApi>,
    opts: CliArgs,
    config: Config,
    config_options: Options,
    layout_info: Option<LayoutInfo>,
    /// The session drawn last in this terminal and how many of its history rows the terminal
    /// holds. Entering any other context starts from a cleared terminal.
    last_shown: Option<(String, u64)>,
    notice: Option<(String, bool)>,
    sessions_run: usize,
    primary_screen: bool,
}

impl DashboardFlow {
    pub(crate) fn new(
        os_input: Box<dyn ClientOsApi>,
        opts: CliArgs,
        config: Config,
        config_options: Options,
        layout_info: Option<LayoutInfo>,
    ) -> Self {
        let primary_screen = host_scrollback_enabled(&config_options);
        DashboardFlow {
            os_input,
            opts,
            config,
            config_options,
            layout_info,
            last_shown: None,
            notice: None,
            sessions_run: 0,
            primary_screen,
        }
    }

    /// Runs until the user quits the dashboard. `first` is a session to open before the
    /// dashboard is shown for the first time (`zellij attach x`, `zellij -s x`).
    pub(crate) fn run(mut self, first: Option<ClientInfo>) {
        if let Some(info) = first {
            self.run_session(info);
        }
        loop {
            let setup = DashboardSetup {
                explicitly_disable_kitty_keyboard_protocol: self
                    .config_options
                    .support_kitty_keyboard_protocol
                    .map(|e| !e)
                    .unwrap_or(false),
                notice: self.notice.take(),
            };
            match dashboard::run_dashboard(self.os_input.clone(), setup) {
                DashboardOutcome::Open { name } => match self.resolve(&name) {
                    Some(info) => self.run_session(info),
                    None => self.notice = Some((format!("{} no longer exists", name), true)),
                },
                DashboardOutcome::New { name } => {
                    let info = ClientInfo::New(name, self.layout_info.clone(), None, None);
                    self.run_session(info);
                },
                DashboardOutcome::Quit | DashboardOutcome::Shutdown => break,
            }
        }
        if self.primary_screen && self.sessions_run > 0 {
            // the sessions drew into this terminal; hand it back clean
            dashboard::clear_terminal(&*self.os_input);
        }
        dashboard::restore_terminal(&*self.os_input);
    }

    /// Makes the terminal show only the session about to be drawn. Re-entering the session
    /// shown last continues its scrollback (the server prints only the rows the terminal
    /// has not seen); any other session gets a cleared screen and scrollback and its whole
    /// history. Returns the number of history rows the terminal already holds.
    fn prepare_terminal_for(&mut self, name: &str) -> Option<u64> {
        if !self.primary_screen {
            return None;
        }
        match &self.last_shown {
            Some((shown, rows)) if shown == name => Some(*rows),
            _ => {
                dashboard::clear_terminal(&*self.os_input);
                Some(0)
            },
        }
    }

    fn resolve(&self, name: &str) -> Option<ClientInfo> {
        if session_exists(name).unwrap_or(false) {
            return Some(ClientInfo::Attach(
                name.to_string(),
                self.config_options.clone(),
            ));
        }
        match resurrection_layout(name) {
            Ok(Some(_)) => Some(ClientInfo::Resurrect(
                name.to_string(),
                session_layout_cache_file_name(name),
                false,
                None,
            )),
            _ => None,
        }
    }

    fn run_session(&mut self, info: ClientInfo) {
        let mut info = info;
        let mut name = info.get_session_name().to_string();
        loop {
            let rows_seen = self.prepare_terminal_for(&name);
            let exit = start_client_ext(
                self.os_input.clone(),
                self.opts.clone(),
                self.config.clone(),
                self.config_options.clone(),
                info,
                None,
                None,
                self.sessions_run > 0,
                false,
                TerminalHandling {
                    primary_screen: self.primary_screen,
                    teardown_on_exit: false,
                },
                rows_seen,
            );
            self.sessions_run += 1;
            self.last_shown = exit
                .host_scroll
                .as_ref()
                .map(|state| (name.clone(), state.rows_scrolled));
            let still_running = session_exists(&name).unwrap_or(false);
            self.notice = Some(match &exit.reason {
                Some(ExitReason::Error(error)) => (error.clone(), true),
                _ if !still_running => (format!("{} has ended", name), false),
                _ => (format!("{} keeps running in the background", name), false),
            });
            // a session switch requested from inside the session (CLI `switch-session`)
            match exit.reconnect.and_then(|connect| connect.name) {
                Some(next) => match self.resolve(&next) {
                    Some(next_info) => {
                        name = next;
                        info = next_info;
                    },
                    None => {
                        self.notice = Some((format!("{} does not exist", next), true));
                        break;
                    },
                },
                None => break,
            }
        }
    }
}
