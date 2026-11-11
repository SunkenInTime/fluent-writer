//! Every command the menus offer: its title, where it sits, its shortcut, when it is enabled
//! or checked, and what it does. The menu bar, the key handler and the toolbar all go through here.

use rune::{Chord, KeyEvent, Tx};
use writer_core::{FocusUnit, LinePrefix, plain_text_exporter, text_editing, text_stats};

use crate::app::{App, focus_name};
use crate::theme::{Appearance, DEFAULT_TEXT_SIZE, TEXT_SIZES};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Command {
    // App
    About,
    Quit,
    // File
    NewDraft,
    OpenDraftPicker,
    OpenOtherFile,
    OpenFolder,
    OpenOverSsh,
    Save,
    SaveAs,
    Rename,
    Reveal,
    CopyForTwitter,
    Close,
    // Edit
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    Delete,
    SelectAll,
    Find,
    FindAndReplace,
    FindNext,
    FindPrevious,
    UseSelectionForFind,
    JumpToSelection,
    ToggleSpellCheck,
    SmartQuotes,
    SmartDashes,
    TextReplacement,
    // Format
    Bold,
    Italic,
    Strikethrough,
    Link,
    Image,
    Heading1,
    Heading2,
    Heading3,
    Body,
    BulletList,
    NumberedList,
    Quote,
    Indent,
    Outdent,
    // View
    ToggleSidebar,
    RecentFiles,
    ToggleFocus,
    FocusOff,
    FocusSentence,
    FocusParagraph,
    FocusTypewriter,
    BiggerText,
    SmallerText,
    DefaultTextSize,
    AppearanceSystem,
    AppearanceLight,
    AppearanceDark,
    ToggleCounts,
    FullScreen,
    // Assist
    AskCodex,
    AskClaude,
    ShowAssist,
    AddComment,
    ShowComments,
    ShowVersions,
    OlderVersion,
    NewerVersion,
    UseThisVersion,
    ConnectOutsideAgent,
    // Window
    Minimize,
    Zoom,
}

/// One row of a menu.
#[derive(Clone, Copy, Debug)]
pub enum Entry {
    Item(Command, &'static str, Keys),
    Separator,
    Submenu(&'static str, &'static [Entry]),
}

/// A command's shortcut: the Mac chord and the chord on Windows and Linux. Empty means none.
#[derive(Clone, Copy, Debug)]
pub struct Keys(pub &'static str, pub &'static str);

const NONE: Keys = Keys("", "");

impl Keys {
    pub fn spec(&self) -> &'static str {
        if cfg!(target_os = "macos") { self.0 } else { self.1 }
    }

    pub fn chord(&self) -> Option<Chord> {
        let spec = self.spec();
        (!spec.is_empty()).then(|| Chord::parse(spec).expect("a shortcut in the menu table parses"))
    }

    /// The shortcut as menus print it: "⇧⌘T" or "Ctrl+Shift+T".
    pub fn label(&self) -> String {
        self.chord().map(|c| c.label()).unwrap_or_default()
    }
}

use Command as C;
use Entry::{Item, Separator, Submenu};

// Off the Mac, ⌘ is Ctrl, ⌥⌘ is Ctrl+Alt, and the Mac's ⌃⌘ chords are Alt+Shift.
pub const FILE: &[Entry] = &[
    Item(C::NewDraft, "New", Keys("mod+n", "mod+n")),
    Item(C::OpenDraftPicker, "Open…", Keys("mod+o", "mod+o")),
    Item(C::OpenOtherFile, "Open Other File…", Keys("mod+shift+o", "mod+shift+o")),
    Item(C::OpenFolder, "Open Folder…", Keys("mod+alt+o", "mod+alt+o")),
    Item(C::OpenOverSsh, "Open over SSH…", Keys("mod+ctrl+o", "alt+shift+o")),
    Separator,
    Item(C::Save, "Save", Keys("mod+s", "mod+s")),
    Item(C::SaveAs, "Save As…", Keys("mod+shift+s", "mod+shift+s")),
    Item(C::Rename, "Rename…", Keys("mod+shift+r", "mod+shift+r")),
    Item(C::Reveal, "Show in Finder", Keys("mod+alt+r", "mod+alt+r")),
    Separator,
    Item(C::CopyForTwitter, "Copy for Twitter", Keys("mod+shift+c", "mod+shift+c")),
    Separator,
    Item(C::Close, "Close", Keys("mod+w", "mod+w")),
];

const FIND: &[Entry] = &[
    Item(C::Find, "Find…", Keys("mod+f", "mod+f")),
    Item(C::FindAndReplace, "Find and Replace…", Keys("mod+alt+f", "mod+h")),
    Item(C::FindNext, "Find Next", Keys("mod+g", "mod+g")),
    Item(C::FindPrevious, "Find Previous", Keys("mod+shift+g", "mod+shift+g")),
    Item(C::UseSelectionForFind, "Use Selection for Find", Keys("mod+e", "mod+e")),
    // The Swift menu gave this ⌥⌘J too, which Show Assist also has; Show Assist keeps it.
    Item(C::JumpToSelection, "Jump to Selection", NONE),
];

const SPELLING: &[Entry] = &[Item(C::ToggleSpellCheck, "Check Spelling While Typing", NONE)];

const SUBSTITUTIONS: &[Entry] = &[
    Item(C::SmartQuotes, "Smart Quotes", NONE),
    Item(C::SmartDashes, "Smart Dashes", NONE),
    Item(C::TextReplacement, "Text Replacement", NONE),
];

pub const EDIT: &[Entry] = &[
    Item(C::Undo, "Undo", Keys("mod+z", "mod+z")),
    Item(C::Redo, "Redo", Keys("mod+shift+z", "mod+shift+z")),
    Separator,
    Item(C::Cut, "Cut", Keys("mod+x", "mod+x")),
    Item(C::Copy, "Copy", Keys("mod+c", "mod+c")),
    Item(C::Paste, "Paste", Keys("mod+v", "mod+v")),
    Item(C::Delete, "Delete", NONE),
    Item(C::SelectAll, "Select All", Keys("mod+a", "mod+a")),
    Separator,
    Submenu("Find", FIND),
    Submenu("Spelling and Grammar", SPELLING),
    Submenu("Substitutions", SUBSTITUTIONS),
];

pub const FORMAT: &[Entry] = &[
    Item(C::Bold, "Bold", Keys("mod+b", "mod+b")),
    Item(C::Italic, "Italic", Keys("mod+i", "mod+i")),
    Item(C::Strikethrough, "Strikethrough", Keys("mod+shift+u", "mod+shift+u")),
    Item(C::Link, "Link", Keys("mod+k", "mod+k")),
    Item(C::Image, "Image…", Keys("mod+shift+i", "mod+shift+i")),
    Separator,
    Item(C::Heading1, "Heading 1", Keys("mod+1", "mod+1")),
    Item(C::Heading2, "Heading 2", Keys("mod+2", "mod+2")),
    Item(C::Heading3, "Heading 3", Keys("mod+3", "mod+3")),
    Item(C::Body, "Body", Keys("mod+alt+0", "mod+alt+0")),
    Separator,
    Item(C::BulletList, "Bulleted List", Keys("mod+l", "mod+l")),
    Item(C::NumberedList, "Numbered List", Keys("mod+alt+l", "mod+alt+l")),
    Item(C::Quote, "Blockquote", Keys("mod+'", "mod+'")),
    Separator,
    Item(C::Indent, "Indent", Keys("mod+]", "mod+]")),
    Item(C::Outdent, "Outdent", Keys("mod+[", "mod+[")),
];

const FOCUS: &[Entry] = &[
    Item(C::FocusOff, "Off", Keys("mod+ctrl+0", "alt+shift+0")),
    Item(C::FocusSentence, "Sentence", Keys("mod+ctrl+1", "alt+shift+1")),
    Item(C::FocusParagraph, "Paragraph", Keys("mod+ctrl+2", "alt+shift+2")),
    Item(C::FocusTypewriter, "Typewriter", Keys("mod+ctrl+3", "alt+shift+3")),
];

const APPEARANCE: &[Entry] = &[
    Item(C::AppearanceSystem, "Match System", NONE),
    Item(C::AppearanceLight, "Light", NONE),
    Item(C::AppearanceDark, "Dark", NONE),
];

pub const VIEW: &[Entry] = &[
    Item(C::ToggleSidebar, "Show Sidebar", Keys("mod+ctrl+s", "alt+shift+s")),
    Item(C::RecentFiles, "Recent Files", Keys("mod+shift+e", "mod+shift+e")),
    Separator,
    Item(C::ToggleFocus, "Toggle Focus", Keys("mod+d", "mod+d")),
    Submenu("Focus", FOCUS),
    Separator,
    Item(C::BiggerText, "Bigger Text", Keys("mod+plus", "mod+plus")),
    Item(C::SmallerText, "Smaller Text", Keys("mod+minus", "mod+minus")),
    Item(C::DefaultTextSize, "Default Text Size", Keys("mod+0", "mod+0")),
    Separator,
    Submenu("Appearance", APPEARANCE),
    Item(C::ToggleCounts, "Show Counts", Keys("mod+alt+c", "mod+alt+c")),
    Separator,
    Item(C::FullScreen, "Enter Full Screen", Keys("mod+ctrl+f", "f11")),
];

pub const ASSIST: &[Entry] = &[
    Item(C::AskCodex, "Ask Codex…", Keys("mod+j", "mod+j")),
    Item(C::AskClaude, "Ask Claude…", Keys("mod+shift+j", "mod+shift+j")),
    Item(C::ShowAssist, "Show Assist", Keys("mod+alt+j", "mod+alt+j")),
    Separator,
    Item(C::AddComment, "Add Comment…", Keys("mod+alt+m", "mod+alt+m")),
    Item(C::ShowComments, "Show Comments", Keys("mod+shift+m", "mod+shift+m")),
    Separator,
    Item(C::ShowVersions, "Show Versions", Keys("mod+alt+v", "mod+alt+v")),
    Item(C::OlderVersion, "Older Version", Keys("mod+alt+left", "mod+alt+left")),
    Item(C::NewerVersion, "Newer Version", Keys("mod+alt+right", "mod+alt+right")),
    Item(C::UseThisVersion, "Use This Version", Keys("mod+alt+enter", "mod+alt+enter")),
    Separator,
    Item(C::ConnectOutsideAgent, "Connect an Outside Agent…", NONE),
];

pub const WINDOW: &[Entry] = &[Item(C::Minimize, "Minimize", Keys("mod+m", "")), Item(C::Zoom, "Zoom", NONE)];

/// The menu bar, left to right. The Mac's app menu (About, Hide, Quit) is the system's.
pub const MENUS: &[(&str, &[Entry])] =
    &[("File", FILE), ("Edit", EDIT), ("Format", FORMAT), ("View", VIEW), ("Assist", ASSIST), ("Window", WINDOW)];

/// Shortcuts that sit outside the menus: Bigger Text also answers the unshifted key.
const EXTRA: &[(Command, Keys)] = &[(C::BiggerText, Keys("mod+equal", "mod+equal")), (C::Quit, Keys("mod+q", "mod+q"))];

fn each(entries: &'static [Entry], f: &mut dyn FnMut(Command, Keys)) {
    for e in entries {
        match e {
            Item(c, _, k) => f(*c, *k),
            Submenu(_, inner) => each(inner, f),
            Separator => {}
        }
    }
}

/// The command a key press asks for, if any.
pub fn command_for(ev: &KeyEvent, tx: &Tx) -> Option<Command> {
    let mut found = None;
    let mut check = |c: Command, k: Keys| {
        if found.is_none() && k.chord().is_some_and(|chord| chord.matches_tx(ev, tx)) {
            found = Some(c);
        }
    };
    for (_, entries) in MENUS {
        each(entries, &mut check);
    }
    for (c, k) in EXTRA {
        check(*c, *k);
    }
    found
}

impl App {
    /// The title a menu shows for `command` right now, where it changes with state.
    pub fn command_title(&self, command: Command, fallback: &'static str) -> String {
        match command {
            C::ToggleFocus if self.focus.dims_surroundings() => "Turn Focus Off".into(),
            C::ToggleFocus => format!("Turn {} Focus On", focus_name(self.prefs.last_dimming_focus())),
            C::CopyForTwitter if self.has_selection() => "Copy Selection for Twitter".into(),
            C::Reveal => crate::platform::REVEAL_TITLE.into(),
            _ => fallback.into(),
        }
    }

    pub fn has_selection(&self) -> bool {
        let (a, b) = self.edit.range();
        a != b
    }

    pub fn command_enabled(&self, command: Command) -> bool {
        let size = self.metrics.font_size;
        match command {
            C::BiggerText => size < TEXT_SIZES[TEXT_SIZES.len() - 1],
            C::SmallerText => size > TEXT_SIZES[0],
            C::Reveal => self.draft.remote().is_none() && (self.draft.path().is_some() || !self.draft.body.is_empty()),
            C::CopyForTwitter | C::ShowAssist | C::AskCodex | C::AskClaude | C::AddComment => !self.edit.text.is_empty(),
            _ => true,
        }
    }

    pub fn command_checked(&self, command: Command) -> bool {
        match command {
            C::FocusOff => self.focus == FocusUnit::Off,
            C::FocusSentence => self.focus == FocusUnit::Sentence,
            C::FocusParagraph => self.focus == FocusUnit::Paragraph,
            C::FocusTypewriter => self.focus == FocusUnit::Typewriter,
            C::AppearanceSystem => self.prefs.appearance() == Appearance::System,
            C::AppearanceLight => self.prefs.appearance() == Appearance::Light,
            C::AppearanceDark => self.prefs.appearance() == Appearance::Dark,
            C::ToggleCounts => self.prefs.show_counts(),
            C::ToggleSpellCheck => self.prefs.spell_check(),
            _ => false,
        }
    }

    /// Runs a command, unless it is disabled.
    pub fn run(&mut self, command: Command, tx: &mut Tx) {
        if !self.command_enabled(command) {
            return;
        }
        let sel = {
            let (a, b) = self.edit.range();
            a.min(b)..a.max(b)
        };
        let text = &self.edit.text;
        match command {
            C::Quit | C::Close => tx.close_window(),
            C::NewDraft => self.new_draft(tx),
            C::Save => {
                if !self.draft.is_empty_untitled() && self.flush() {
                    self.flash_message("Saved", tx);
                }
            }
            C::CopyForTwitter => {
                let source = if sel.is_empty() { text.as_str() } else { &text[sel.clone()] };
                let plain = plain_text_exporter::export(source);
                let count = text_stats::counts_for_plain_text(&plain).characters;
                tx.copy(plain);
                let what = if sel.is_empty() { "draft" } else { "selection" };
                self.flash_message(format!("Copied {what} as plain text · {} characters", crate::platform::grouped(count)), tx);
            }
            C::Reveal => {
                if self.draft.path().is_none() {
                    self.flush();
                }
                if let Some(path) = self.draft.path() {
                    crate::platform::reveal(path);
                }
            }
            C::Undo => {
                self.edit.undo();
                self.text_did_change(tx);
            }
            C::Redo => {
                self.edit.redo();
                self.text_did_change(tx);
            }
            C::Copy => {
                if !sel.is_empty() {
                    tx.copy(self.edit.selected().to_string());
                }
            }
            C::Cut => {
                if !sel.is_empty() {
                    tx.copy(self.edit.selected().to_string());
                    self.edit.insert("");
                    self.text_did_change(tx);
                }
            }
            C::Delete => {
                if !sel.is_empty() {
                    self.edit.insert("");
                    self.text_did_change(tx);
                }
            }
            C::Paste => tx.request_paste(),
            C::SelectAll => {
                let len = self.edit.text.len();
                self.edit.set_selection(0, len);
            }
            C::Bold => self.apply(&text_editing::toggle("**", text, sel), tx),
            C::Italic => self.apply(&text_editing::toggle("*", text, sel), tx),
            C::Strikethrough => self.apply(&text_editing::toggle("~~", text, sel), tx),
            C::Link => self.apply(&text_editing::link(text, sel), tx),
            C::Heading1 => self.apply(&text_editing::set_line_prefix(LinePrefix::Heading(1), text, sel), tx),
            C::Heading2 => self.apply(&text_editing::set_line_prefix(LinePrefix::Heading(2), text, sel), tx),
            C::Heading3 => self.apply(&text_editing::set_line_prefix(LinePrefix::Heading(3), text, sel), tx),
            C::Body => self.apply(&text_editing::set_line_prefix(LinePrefix::Body, text, sel), tx),
            C::BulletList => self.apply(&text_editing::set_line_prefix(LinePrefix::Bullet, text, sel), tx),
            C::NumberedList => self.apply(&text_editing::set_line_prefix(LinePrefix::Numbered, text, sel), tx),
            C::Quote => self.apply(&text_editing::set_line_prefix(LinePrefix::Quote, text, sel), tx),
            C::Indent | C::Outdent => {
                if let Some(e) = text_editing::indent_edit(text, sel, command == C::Outdent) {
                    self.apply(&e, tx);
                }
            }
            C::ToggleFocus => {
                let next = if self.focus.dims_surroundings() { FocusUnit::Off } else { self.prefs.last_dimming_focus() };
                self.set_focus(next, tx);
            }
            C::FocusOff => self.set_focus(FocusUnit::Off, tx),
            C::FocusSentence => self.set_focus(FocusUnit::Sentence, tx),
            C::FocusParagraph => self.set_focus(FocusUnit::Paragraph, tx),
            C::FocusTypewriter => self.set_focus(FocusUnit::Typewriter, tx),
            C::BiggerText => {
                if let Some(next) = TEXT_SIZES.iter().find(|s| **s > self.metrics.font_size) {
                    self.set_text_size(*next, tx);
                }
            }
            C::SmallerText => {
                if let Some(next) = TEXT_SIZES.iter().rev().find(|s| **s < self.metrics.font_size) {
                    self.set_text_size(*next, tx);
                }
            }
            C::DefaultTextSize => self.set_text_size(DEFAULT_TEXT_SIZE, tx),
            C::AppearanceSystem => self.prefs.set_appearance(Appearance::System),
            C::AppearanceLight => self.prefs.set_appearance(Appearance::Light),
            C::AppearanceDark => self.prefs.set_appearance(Appearance::Dark),
            C::ToggleCounts => {
                let on = !self.prefs.show_counts();
                self.prefs.set_show_counts(on);
            }
            C::ToggleSpellCheck => {
                let on = !self.prefs.spell_check();
                self.prefs.set_spell_check(on);
            }
            C::Zoom => tx.toggle_maximized(),
            C::Minimize => tx.minimize(),
            other => self.flash_message(format!("{other:?} isn't built yet"), tx),
        }
    }
}
