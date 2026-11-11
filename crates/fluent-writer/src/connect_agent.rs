//! Assist › Connect an Outside Agent… (Swift `ConnectAgent.swift`): shows how to give Claude
//! Code or Codex access to drafts, comments and versions through the `fluent-writer-mcp` server
//! that ships beside the app, and can register it with either CLI.
//!
//! The Swift app showed `NSAlert` sheets. The commands sheet has four buttons and an accessory
//! view, more than `ui::Alert` holds, so it is drawn here in `ui::sheet` with `ui::button`s; the
//! two plain alerts (no connector, a CLI that failed) go through `App::show_alert`. Registering
//! runs the CLI on a thread and reports back through `App::connect_poll`.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};

use rune::{El, Tx, Waker, col, el, row, text};
use writer_agents::ExecutableLocator;

use crate::app::App;
use crate::theme::{Palette, rgba, ui_font};
use crate::ui::{self, Alert};

/// The accessory view's width and height. `ConnectAgent.swift:46-47`
const ACCESSORY_W: f32 = 460.0;
const ACCESSORY_H: f32 = 150.0;
/// The sheet's width: the accessory and the sheet's 20 pt padding each side.
const SHEET_W: f32 = ACCESSORY_W + 40.0;

/// The commands sheet, while it is up. `ConnectAgent.swift:40-66`
#[derive(Clone, Debug, PartialEq)]
pub enum Sheet {
    Commands { claude: String, codex: String, server: PathBuf, support: PathBuf },
}

/// What a registration thread reports.
enum Outcome {
    Flash(String),
    Failed { title: String, detail: String },
}

pub struct Connect {
    pub sheet: Option<Sheet>,
    outcomes: Sender<Outcome>,
    inbox: Receiver<Outcome>,
    waker: Option<Waker>,
    /// Registration threads still running, so tests can wait for them.
    pub running: usize,
}

impl Default for Connect {
    fn default() -> Connect {
        let (outcomes, inbox) = mpsc::channel();
        Connect { sheet: None, outcomes, inbox, waker: None, running: 0 }
    }
}

impl Connect {
    pub fn set_waker(&mut self, waker: Waker) {
        self.waker = Some(waker);
    }
}

/// The MCP server: `fluent-writer-mcp` next to the running executable, if it is there and can
/// run. On macOS that is inside the app bundle, as in the Swift app. `ConnectAgent.swift:8-12`
pub fn server_path() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let path = exe.parent()?.join(format!("fluent-writer-mcp{}", std::env::consts::EXE_SUFFIX));
    is_executable(&path).then_some(path)
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.metadata().is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

/// `fluent-writer`, or `fluent-writer-dev` for the dev build. `ConnectAgent.swift:14`
pub fn server_name(dev: bool) -> &'static str {
    if dev { "fluent-writer-dev" } else { "fluent-writer" }
}

/// Single-quotes an argument for a POSIX shell. `ConnectAgent.swift:16`
pub fn quoted(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

fn server_arguments(server: &Path, support: &Path) -> Vec<String> {
    vec![server.to_string_lossy().into_owned(), "--support".into(), support.to_string_lossy().into_owned()]
}

/// The arguments for `claude`. `ConnectAgent.swift:70`
fn claude_args(dev: bool, server: &Path, support: &Path) -> Vec<String> {
    let mut args: Vec<String> = ["mcp", "add", "--scope", "user", server_name(dev), "--"].map(String::from).to_vec();
    args.extend(server_arguments(server, support));
    args
}

/// The arguments for `codex`. `ConnectAgent.swift:72`
fn codex_args(dev: bool, server: &Path, support: &Path) -> Vec<String> {
    let mut args: Vec<String> = ["mcp", "add", server_name(dev), "--"].map(String::from).to_vec();
    args.extend(server_arguments(server, support));
    args
}

/// `ConnectAgent.swift:20-22`
pub fn claude_command(dev: bool, server: &Path, support: &Path) -> String {
    format!("claude mcp add --scope user {} -- {}", server_name(dev), server_arguments(server, support).iter().map(|a| quoted(a)).collect::<Vec<_>>().join(" "))
}

/// `ConnectAgent.swift:24-26`
pub fn codex_command(dev: bool, server: &Path, support: &Path) -> String {
    format!("codex mcp add {} -- {}", server_name(dev), server_arguments(server, support).iter().map(|a| quoted(a)).collect::<Vec<_>>().join(" "))
}

/// Runs `tool args` and says how it went. `ConnectAgent.swift:83-111`
fn register(tool: &str, args: &[String], name: &str) -> Outcome {
    let Some(exe) = ExecutableLocator::find(tool, &[]) else {
        return Outcome::Flash(format!("{name} isn't installed"));
    };
    let dir = exe.parent().map(Path::to_path_buf).unwrap_or_default();
    let out = std::process::Command::new(&exe)
        .args(args)
        .env_clear()
        .envs(ExecutableLocator::environment(&[dir]))
        .stdin(std::process::Stdio::null())
        .output();
    match out {
        Err(e) => Outcome::Flash(format!("Couldn't run {tool}: {e}")),
        Ok(o) if o.status.success() => Outcome::Flash(format!("Added to {name}. Start a new {name} session to use it.")),
        Ok(o) => {
            // The Swift app read stdout and stderr through one pipe; here they are joined.
            let mut all = String::from_utf8_lossy(&o.stdout).into_owned();
            all.push_str(&String::from_utf8_lossy(&o.stderr));
            let text = all.trim();
            let detail = if text.is_empty() {
                format!("It exited with status {}.", o.status.code().unwrap_or(-1))
            } else {
                let skip = text.chars().count().saturating_sub(600);
                text.chars().skip(skip).collect()
            };
            Outcome::Failed { title: format!("{name} didn't add the connector."), detail }
        }
    }
}

impl App {
    fn is_dev_build(&self) -> bool {
        self.notes.app_name == writer_core::AppVariant::Dev.app_name()
    }

    /// Assist › Connect an Outside Agent…. `ConnectAgent.swift:30-81`
    pub fn connect_agent_open(&mut self) {
        let Some(server) = server_path() else {
            self.show_alert(
                Alert::new("The agent connector is missing from this build.").informative("Build the app with script/build-app.sh, which bundles fluent-writer-mcp."),
            );
            return;
        };
        let support = self.library.support_directory().to_path_buf();
        let dev = self.is_dev_build();
        self.assist.connect.sheet =
            Some(Sheet::Commands { claude: claude_command(dev, &server, &support), codex: codex_command(dev, &server, &support), server, support });
    }

    /// A sheet button: every one closes the sheet first, as the alert's did.
    fn connect_pick(&mut self, pick: Pick, tx: &mut Tx) {
        let Some(sheet) = self.assist.connect.sheet.take() else { return };
        let Sheet::Commands { claude, codex, server, support } = sheet;
        let dev = self.is_dev_build();
        match pick {
            Pick::Claude => self.connect_register("claude", claude_args(dev, &server, &support), "Claude Code"),
            Pick::Codex => self.connect_register("codex", codex_args(dev, &server, &support), "Codex"),
            Pick::Copy => {
                tx.copy(format!("# Claude Code\n{claude}\n\n# Codex\n{codex}\n"));
                self.flash_message("Copied the connect commands", tx);
            }
            Pick::Done => {}
        }
    }

    /// Runs the CLI off the UI thread; `connect_poll` reports the result.
    fn connect_register(&mut self, tool: &'static str, args: Vec<String>, name: &'static str) {
        let c = &mut self.assist.connect;
        let send = c.outcomes.clone();
        let waker = c.waker.clone();
        c.running += 1;
        std::thread::spawn(move || {
            let _ = send.send(register(tool, &args, name));
            if let Some(w) = waker {
                w();
            }
        });
    }

    /// Takes the results of finished registrations. Returns whether anything changed.
    pub fn connect_poll(&mut self, tx: &mut Tx) -> bool {
        let mut changed = false;
        while let Ok(outcome) = self.assist.connect.inbox.try_recv() {
            changed = true;
            self.assist.connect.running = self.assist.connect.running.saturating_sub(1);
            match outcome {
                Outcome::Flash(m) => self.flash_message(m, tx),
                Outcome::Failed { title, detail } => self.show_alert(Alert::new(title).informative(detail)),
            }
        }
        changed
    }

    #[cfg(test)]
    pub fn connect_register_for_test(&mut self, tool: &'static str, name: &'static str) {
        self.connect_register(tool, Vec::new(), name);
    }

}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Pick {
    Claude,
    Codex,
    Copy,
    Done,
}

/// One command with its heading, as the accessory view stacks them: the heading 11 pt medium,
/// the command IBM Plex Mono 10 pt in the secondary colour, wrapping and selectable, in a box
/// 460×46. `ConnectAgent.swift:48-59`
fn block(key: &str, title: &str, command: &str, p: &Palette) -> El {
    let secondary = if p.dark { rgba(255, 255, 255, 0.55) } else { rgba(0, 0, 0, 0.5) };
    col()
        .w(ACCESSORY_W)
        .child(text(title).font(ui_font(11.0, true)).color(p.text).h(16.0).truncate())
        .child(
            text(command)
                .key(key)
                .font(ui_font(10.0, false).lh(13.0))
                .color(secondary)
                .wrap_anywhere()
                .selectable(true)
                .selection_color(p.selection)
                .mt(4.0)
                .h(46.0),
        )
}

/// The connect sheet, or nothing. It draws as the app's other alerts do (`ui::sheet`), with the
/// commands where the Swift alert had its accessory view and four buttons, the first (Add to
/// Claude Code) rightmost and the default. Escape does nothing: AppKit gives it only to a
/// button titled Cancel. `ConnectAgent.swift:43-66`
pub fn view(s: &App, p: &Palette) -> El {
    let Some(Sheet::Commands { claude, codex, .. }) = &s.assist.connect.sheet else {
        return el().key("connect-sheet").absolute(0.0, 0.0).size(0.0, 0.0);
    };
    let title = "Connect an outside agent";
    let content = col()
        .key("connect")
        .role("dialog")
        .label(title)
        .gap(4.0)
        .child(
            col().gap(8.0).child(text(title).key("connect-title").font(ui_font(13.0, true)).color(p.text).wrap()).child(
                text("Claude Code or Codex can then see the draft open here, your selection, comments, versions and Assist history. They can leave comments and propose versions, which you preview and choose here, the same as Assist's. Run one of these in Terminal, or click a button to do it now.")
                    .key("connect-message")
                    .font(ui_font(12.0, false))
                    .color(p.text.alpha(0.75))
                    .wrap(),
            ),
        )
        .child(
            col()
                .key("connect-accessory")
                .mt(8.0)
                .size(ACCESSORY_W, ACCESSORY_H)
                .child(block("connect-claude-command", "Claude Code", claude, p))
                .child(block("connect-codex-command", "Codex", codex, p).mt(8.0)),
        )
        .child(
            row()
                .key("connect-buttons")
                .gap(8.0)
                .mt(8.0)
                .justify_end()
                .child(ui::button("connect-done", "Done", false, p, |s: &mut App, tx| s.connect_pick(Pick::Done, tx)))
                .child(el().grow(1.0))
                .child(ui::button("connect-copy", "Copy Commands", false, p, |s: &mut App, tx| s.connect_pick(Pick::Copy, tx)))
                .child(ui::button("connect-codex", "Add to Codex", false, p, |s: &mut App, tx| s.connect_pick(Pick::Codex, tx)))
                .child(ui::button("connect-claude", "Add to Claude Code", true, p, |s: &mut App, tx| s.connect_pick(Pick::Claude, tx))),
        );
    ui::sheet("connect-sheet", p, s.window, SHEET_W, content, |_: &mut App, _| {})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_quote_every_argument() {
        let server = Path::new("/Apps/Fluent Writer.app/Contents/MacOS/fluent-writer-mcp");
        let support = Path::new("/Users/me/Library/Application Support/Fluent Writer");
        assert_eq!(
            claude_command(false, server, support),
            "claude mcp add --scope user fluent-writer -- '/Apps/Fluent Writer.app/Contents/MacOS/fluent-writer-mcp' '--support' '/Users/me/Library/Application Support/Fluent Writer'"
        );
        assert_eq!(
            codex_command(true, server, support),
            "codex mcp add fluent-writer-dev -- '/Apps/Fluent Writer.app/Contents/MacOS/fluent-writer-mcp' '--support' '/Users/me/Library/Application Support/Fluent Writer'"
        );
        assert_eq!(quoted("it's"), "'it'\\''s'");
        assert_eq!(claude_args(false, server, support)[..6], ["mcp", "add", "--scope", "user", "fluent-writer", "--"]);
    }
}
