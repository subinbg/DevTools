//! Reading the state of every session for the dashboard, and the session-level actions it
//! offers (terminate, delete, shut down). Nothing here exits the process: errors come
//! back as text for the dashboard to show.

use std::collections::HashSet;
use std::path::Path;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use zellij_utils::{
    consts::{
        ipc_connect, session_info_cache_file_name, session_info_folder_for_session, ZELLIJ_SOCK_DIR,
    },
    data::SessionInfo,
    input::{
        actions::Action,
        command::RunCommand,
        layout::{FloatingPaneLayout, Layout, Run, TiledPaneLayout},
    },
    ipc::{ClientToServerMsg, IpcReceiverWithContext, IpcSenderWithContext, ServerToClientMsg},
    sessions::{
        generate_unique_session_name, get_resurrectable_session_names, get_resurrectable_sessions,
        get_sessions, resurrection_layout, session_exists, validate_session_name,
    },
};

use super::model::{PaneRow, SessionRow, TabRow};

/// Every session on this machine: running ones first (by name), then the saved ones that
/// can be resurrected (most recently saved first).
pub fn load_rows() -> Vec<SessionRow> {
    let mut running_names: HashSet<String> = HashSet::new();
    let mut rows: Vec<SessionRow> = get_sessions()
        .unwrap_or_default()
        .into_iter()
        .map(|(name, age)| {
            running_names.insert(name.clone());
            let info = read_metadata(&name);
            let layout = read_layout(&name);
            SessionRow {
                clients: info.as_ref().map(|i| i.connected_clients).unwrap_or(0),
                tabs: tabs_from(info.as_ref(), layout.as_ref()),
                name,
                running: true,
                age,
            }
        })
        .collect();
    rows.sort_by(|a, b| a.name.cmp(&b.name));

    let mut saved: Vec<SessionRow> = get_resurrectable_sessions()
        .into_iter()
        .filter(|(name, _)| !running_names.contains(name))
        .map(|(name, age)| {
            let info = read_metadata(&name);
            let layout = read_layout(&name);
            SessionRow {
                clients: 0,
                tabs: tabs_from(info.as_ref(), layout.as_ref()),
                name,
                running: false,
                age,
            }
        })
        .collect();
    saved.sort_by(|a, b| a.age.cmp(&b.age).then_with(|| a.name.cmp(&b.name)));
    rows.extend(saved);
    rows
}

fn read_metadata(name: &str) -> Option<SessionInfo> {
    let raw = std::fs::read_to_string(session_info_cache_file_name(name)).ok()?;
    SessionInfo::from_string(&raw, "").ok()
}

fn read_layout(name: &str) -> Option<Layout> {
    resurrection_layout(name).ok().flatten()
}

/// Combines the live pane information (titles, focus) with what the resurrection layout
/// knows (commands and working directories), matching panes by their order in each tab.
fn tabs_from(info: Option<&SessionInfo>, layout: Option<&Layout>) -> Vec<TabRow> {
    let layout_tabs = layout.map(layout_tabs).unwrap_or_default();
    match info {
        Some(info) if !info.tabs.is_empty() => {
            let mut tabs: Vec<&zellij_utils::data::TabInfo> = info.tabs.iter().collect();
            tabs.sort_by_key(|t| t.position);
            tabs.into_iter()
                .map(|tab| {
                    let layout_panes = layout_tabs
                        .get(tab.position)
                        .map(|(_, panes)| panes.as_slice())
                        .unwrap_or(&[]);
                    let panes: Vec<PaneRow> = info
                        .panes
                        .panes
                        .get(&tab.position)
                        .map(|panes| {
                            panes
                                .iter()
                                .filter(|p| !p.is_plugin && !p.is_suppressed)
                                .enumerate()
                                .map(|(idx, pane)| {
                                    let from_layout = layout_panes.get(idx);
                                    PaneRow {
                                        title: pane.title.clone(),
                                        command: pane.terminal_command.clone().or_else(|| {
                                            from_layout.and_then(|(cmd, _)| cmd.clone())
                                        }),
                                        cwd: from_layout.and_then(|(_, cwd)| cwd.clone()),
                                        focused: pane.is_focused,
                                        exited: pane.exited,
                                    }
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    TabRow {
                        name: tab.name.clone(),
                        active: tab.active,
                        panes,
                    }
                })
                .collect()
        },
        _ => layout_tabs
            .into_iter()
            .enumerate()
            .map(|(idx, (name, panes))| TabRow {
                name,
                active: idx == 0,
                panes: panes
                    .into_iter()
                    .enumerate()
                    .map(|(pane_idx, (command, cwd))| PaneRow {
                        title: command.clone().unwrap_or_else(|| "shell".to_string()),
                        command,
                        cwd,
                        focused: pane_idx == 0,
                        exited: false,
                    })
                    .collect(),
            })
            .collect(),
    }
}

type LayoutPane = (Option<String>, Option<String>); // (command line, cwd)

fn layout_tabs(layout: &Layout) -> Vec<(String, Vec<LayoutPane>)> {
    if layout.tabs.is_empty() {
        layout
            .template
            .as_ref()
            .map(|(tiled, floating)| vec![(String::new(), layout_leaves(tiled, floating))])
            .unwrap_or_default()
    } else {
        layout
            .tabs
            .iter()
            .map(|(name, tiled, floating)| {
                (
                    name.clone().unwrap_or_default(),
                    layout_leaves(tiled, floating),
                )
            })
            .collect()
    }
}

fn layout_leaves(tiled: &TiledPaneLayout, floating: &[FloatingPaneLayout]) -> Vec<LayoutPane> {
    let mut out = vec![];
    walk_tiled(tiled, &mut out);
    for pane in floating {
        if !matches!(pane.run, Some(Run::Plugin(_))) {
            out.push(run_info(&pane.run));
        }
    }
    out
}

fn walk_tiled(node: &TiledPaneLayout, out: &mut Vec<LayoutPane>) {
    if node.children.is_empty() {
        if !matches!(node.run, Some(Run::Plugin(_))) {
            out.push(run_info(&node.run));
        }
    } else {
        for child in &node.children {
            walk_tiled(child, out);
        }
    }
}

fn run_info(run: &Option<Run>) -> LayoutPane {
    match run {
        Some(Run::Command(command)) => (
            Some(command_line(command)),
            command.cwd.as_deref().map(tilde),
        ),
        Some(Run::Cwd(cwd)) => (None, Some(tilde(cwd))),
        Some(Run::EditFile(path, _, cwd)) => (
            Some(format!("edit {}", path.display())),
            cwd.as_deref().map(tilde),
        ),
        _ => (None, None),
    }
}

fn command_line(command: &RunCommand) -> String {
    let mut line = command
        .command
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_else(|| command.command.display().to_string());
    for arg in &command.args {
        line.push(' ');
        line.push_str(arg);
    }
    line
}

/// Shortens a path with the home directory to `~/...`.
pub fn tilde(path: &Path) -> String {
    let display = path.display().to_string();
    if let Some(home) = std::env::var_os("HOME") {
        let home = home.to_string_lossy();
        if !home.is_empty() {
            if display == home.as_ref() {
                return "~".to_string();
            }
            if let Some(rest) = display.strip_prefix(&format!("{}/", home)) {
                return format!("~/{}", rest);
            }
        }
    }
    display
}

pub fn suggest_name() -> String {
    generate_unique_session_name().unwrap_or_else(|| {
        let n = get_sessions().map(|s| s.len()).unwrap_or(0)
            + get_resurrectable_session_names().len()
            + 1;
        format!("session-{}", n)
    })
}

pub fn running_names() -> Vec<String> {
    let mut names: Vec<String> = get_sessions()
        .unwrap_or_default()
        .into_iter()
        .map(|(name, _)| name)
        .collect();
    names.sort();
    names
}

pub fn is_running(name: &str) -> bool {
    session_exists(name).unwrap_or(false)
}

pub fn is_saved(name: &str) -> bool {
    get_resurrectable_session_names().iter().any(|n| n == name)
}

pub fn check_new_name(name: &str) -> Result<(), String> {
    validate_session_name(name)?;
    if is_running(name) {
        return Err(format!("'{}' is already running", name));
    }
    if is_saved(name) {
        return Err(format!("'{}' is a saved session; open it instead", name));
    }
    Ok(())
}

fn connect(name: &str) -> Result<IpcSenderWithContext<ClientToServerMsg>, String> {
    let path = ZELLIJ_SOCK_DIR.join(name);
    let stream = ipc_connect(&path).map_err(|e| format!("cannot reach '{}': {}", name, e))?;
    Ok(IpcSenderWithContext::new(stream))
}

/// Asks a running session to write its layout, screens and scrollback to disk right now,
/// and waits until it has.
pub fn save_session(name: &str) -> Result<(), String> {
    let mut sender = connect(name)?;
    let mut receiver: IpcReceiverWithContext<ServerToClientMsg> = sender.get_receiver();
    sender
        .send_client_msg(ClientToServerMsg::Action {
            action: Action::SaveSession,
            terminal_id: None,
            client_id: None,
            is_cli_client: true,
        })
        .map_err(|e| format!("cannot ask '{}' to save: {}", name, e))?;
    let (tx, rx) = mpsc::channel();
    std::thread::Builder::new()
        .name("dashboard_save_wait".to_string())
        .spawn(move || loop {
            match receiver.recv_server_msg() {
                Some((ServerToClientMsg::UnblockInputThread, _))
                | Some((ServerToClientMsg::Exit { .. }, _)) => {
                    let _ = tx.send(Ok(()));
                    break;
                },
                Some((ServerToClientMsg::LogError { lines }, _)) => {
                    let _ = tx.send(Err(lines.join(" ")));
                    break;
                },
                Some(_) => continue,
                None => {
                    let _ = tx.send(Err("the session closed the connection".to_string()));
                    break;
                },
            }
        })
        .map_err(|e| e.to_string())?;
    let result = rx
        .recv_timeout(Duration::from_secs(15))
        .unwrap_or_else(|_| Err(format!("'{}' did not confirm the save in time", name)));
    let _ = sender.send_client_msg(ClientToServerMsg::ClientExited);
    result
}

fn wait_until_gone(name: &str, timeout: Duration) -> Result<(), String> {
    let started = Instant::now();
    loop {
        if !is_running(name) {
            return Ok(());
        }
        if started.elapsed() > timeout {
            return Err(format!("'{}' is still running", name));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Stops a running session without saving it first.
pub fn kill(name: &str) -> Result<(), String> {
    let mut sender = connect(name)?;
    sender
        .send_client_msg(ClientToServerMsg::KillSession)
        .map_err(|e| format!("cannot stop '{}': {}", name, e))?;
    wait_until_gone(name, Duration::from_secs(5))
}

/// Terminates a session: its screens are saved first, then its programs are stopped. The
/// session stays listed and can be opened again with everything it showed.
pub fn terminate(name: &str) -> Result<(), String> {
    if !is_running(name) {
        return Err(format!("'{}' is not running", name));
    }
    save_session(name).map_err(|e| format!("not stopped, its screen could not be saved: {}", e))?;
    kill(name)
}

/// Deletes a session: stops it if it runs, and removes its saved screens and layout.
pub fn delete(name: &str) -> Result<(), String> {
    if is_running(name) {
        kill(name)?;
    }
    match std::fs::remove_dir_all(session_info_folder_for_session(name)) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("could not delete '{}': {}", name, e)),
    }
}

/// Terminates every running session, returning what happened to each.
pub fn terminate_all() -> Vec<(String, Result<(), String>)> {
    running_names()
        .into_iter()
        .map(|name| {
            let result = terminate(&name);
            (name, result)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn layout_leaves_report_commands_and_cwds() {
        let kdl = r#"layout {
            tab name="work" {
                pane split_direction="vertical" {
                    pane cwd="/tmp/a"
                    pane command="htop" cwd="/tmp/b"
                }
            }
            tab {
                pane
            }
        }"#;
        let layout = Layout::from_kdl(kdl, None, None, None).unwrap();
        let tabs = layout_tabs(&layout);
        assert_eq!(tabs.len(), 2);
        assert_eq!(tabs[0].0, "work");
        assert_eq!(
            tabs[0].1,
            vec![
                (None, Some("/tmp/a".to_string())),
                (Some("htop".to_string()), Some("/tmp/b".to_string()))
            ]
        );
        assert_eq!(tabs[1].1, vec![(None, None)]);
        let rows = tabs_from(None, Some(&layout));
        assert_eq!(rows[0].panes[1].command.as_deref(), Some("htop"));
        assert_eq!(rows[0].panes[0].title, "shell");
    }

    #[test]
    fn tilde_shortens_the_home_directory() {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/home/x".to_string());
        assert_eq!(tilde(&PathBuf::from(format!("{}/work", home))), "~/work");
        assert_eq!(tilde(&PathBuf::from(&home)), "~");
        assert_eq!(tilde(&PathBuf::from("/opt/x")), "/opt/x");
    }

    #[test]
    fn new_names_are_validated() {
        assert!(check_new_name("").is_err());
        assert!(check_new_name("a/b").is_err());
    }
}
